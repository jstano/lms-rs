# Parity audit: BASIC and ADVANCED against the Java engine

## What this document is

An evidenced answer to "are the BASIC and ADVANCED algorithms complete and correct?"

The short answer: **BASIC is complete and reproduces the Java engine's own test suite. ADVANCED
is part-built and still generates nothing** — its foundations, distribution utilities and
spreaders are ported and parity-checked, but nothing above them exists yet. Neither statement is a proof of
equivalence; see [Limits](#limits) at the end for exactly what is and isn't established.

Every claim below cites a file and line. Ground truth is the sibling checkout, not this repo's
`planner/java/` copy — see [A note on the reference copy](#a-note-on-the-reference-copy).

- `JPR` = `/Users/jstano/workspace/unifocus/taps/src/java/com/unifocus/watson/server/labor/planner/engine/`
- `JTR` = `/Users/jstano/workspace/unifocus/taps/src/junit/com/unifocus/watson/server/labor/planner/engine/`

---

## 1. BASIC (`SIMPLE_NON_FLOWED`)

### 1a. Class map — complete

Every Java class on the BASIC path has a Rust counterpart. Nothing is missing.

| Java class | Rust module | Notes |
|---|---|---|
| `simplenonflowed/SimpleNonFlowedGenerator` | `generators/basic/basic.rs` | Java also saves via `PlannerSaver`; no persistence layer exists here (by design, `DATA_MODEL.md`). |
| `simplenonflowed/SimpleNonFlowedStandardsProcessor` | `basic_standards_processor.rs` | |
| `simplenonflowed/SimpleNonFlowedCalculator` | `basic_calculator.rs` | |
| `simplenonflowed/SimpleNonFlowedCalculationResult` | `BasicCalculationResult` in `basic_calculator.rs:15` | |
| `simplenonflowed/SimpleNonFlowedWorkContentCreator` | `basic_work_content_creator.rs` | |
| `simplenonflowed/PlannedShiftCreator` | `basic_planned_shift_creator.rs` | |
| `simplenonflowed/WorkContentCreatorUtility` | `WorkContent::fixed` (`domain/work_content.rs:78`) | See 1c-4 — the general form is unreachable from BASIC. |
| `calculator/WorkContentCalculator` | `work_content_calculator.rs` | |
| `calculator/TotalWorkMinutesForStandardsCalculator` | `total_work_minutes_calculator.rs` | |
| `calculator/CalculateWorkMinutesPerUnit` | `calculate_work_minutes_per_unit` in `total_work_minutes_calculator.rs:88` | Merged into the caller's module. |

### 1b. Test map — the Java suite now runs against the Rust

Before this audit, **none** of the Java test tables existed in Rust. The Rust tests were
independently written — good tests, but invented scenarios traceable to
`java/engine/workcontent/simplenonflowed/REQUIREMENTS.md`, not to the Spock suite. The
220-hour break-accumulation regime the Java calculator table exercises had no Rust coverage
at all.

They have now been ported, into `java_parity_tests` modules alongside the existing tests.
**56 ported assertions; all pass; none required a change to the production code.** (Now 58, with
the two Phase 0a additions.)

| Spock test | Cases | Status |
|---|---|---|
| `simplenonflowed/SimpleNonFlowedCalculatorTest.testCalculations` | 17 | ✅ ported → `basic_calculator.rs` |
| `simplenonflowed/SimpleNonFlowedCalculatorTest` min-shift precedence | 1 | ✅ ported → `basic_calculator.rs` |
| `calculator/CalculateWorkMinutesPerUnitTest.testCalculations` | 21 | ✅ all 21 ported → `total_work_minutes_calculator.rs` (the last needed Phase 0a) |
| `calculator/WorkContentCalculatorTest` | 5 | ✅ ported → `work_content_calculator.rs` |
| `simplenonflowed/SimpleNonFlowedWorkContentCreatorTest` | 3 (4 rows) | ✅ ported → `basic_work_content_creator.rs` |
| `simplenonflowed/PlannedShiftCreatorTest` | 2 | ✅ ported → `basic_planned_shift_creator.rs` |
| `calculator/TotalWorkMinutesForStandardsCalculatorTest` suppress-value | 4 rows | ✅ ported → `total_work_minutes_calculator.rs` |
| `calculator/TotalWorkMinutesForStandardsCalculatorTest.totalWorkMinutes` | 3 rows | ⚪ not ported — pure mock wiring (the work-minute values are stubbed returns, not computed). Its one real behaviour, "a standard whose shift detail does not resolve contributes nothing", depends on the per-standard environment-scoped detail lookup that this port collapses (1c-1). |
| `simplenonflowed/SimpleNonFlowedGeneratorTest` | 2 | ⚪ not ported — 14 mocks, tests Spring wiring and `PlannerSaver`. No persistence layer exists. |
| `simplenonflowed/SimpleNonFlowedStandardsProcessorTest` | 2 | ⚪ not ported — 22 mocks, tests `EffectiveDateFilter` interaction. Rust uses `PlannerSettings::dates` (`planner_settings.rs:57`) instead. |
| `calculator/CalculateWorkMinutesPerUnitTest.testCalculateFrequencyMinutes` | 1 | ⚪ not ported — `TaskStandard` family, never modelled (out of scope in both generator plans). |
| `simplenonflowed/WorkContentCreatorUtilityTest` | 1 | ⚪ not ported — exercises the general `createWorkContentRecord`, unreachable from BASIC (1c-4). |
| `simplenonflowed/SimpleNonFlowedCalculationResultTest` | 1 | ⚪ not ported — bean getters, already covered. |

**The ported tests have teeth.** Verified by mutation: changing `numbers::truncate` to
`numbers::round` on the full-shift division (`basic_calculator.rs:103`) — a one-token change —
failed 5 of the 17 calculator rows. Reverted.

### 1c. Divergence catalogue

Four differences from Java. Two were real narrowings and are now closed; a third is largely
closed; two turned out not to be divergences at all.

**1c-1 — Environment tier collapsed. ✅ CLOSED (Phase 0b).**
*Was:* Java's `ShiftRelatedStandardValue` level, which holds one standard value per operating
environment, had been folded into the range, keying variation on day-of-week instead.

*Now:* `ShiftStandardRange` holds a `Vec<ShiftStandardValue>` keyed by environment, and the
two-tier lookup from `utlities/ShiftRelatedStandardValueFilter` — the driver-and-date environment
first, falling back to the date's — is ported onto the `Providers` bundle
(`generators/advanced/providers.rs`).

One deliberate extension: a value may carry no environment at all, which Java has no concept of
because Java always has environments. The non-flowed path here has none, so that is how its
standards are shaped — and it is what let all 56 BASIC parity tests pass through the change
unaltered.

*Still deferred:* nothing resolves environments from real data. `EnvironmentResolver` has only
in-memory implementations, which is the same "read" deferral the non-flowed path makes.

**1c-2 — `Units::UnitsPerPerson` not modelled. ✅ CLOSED (Phase 0a).**
*Was:* `java/engine/entities/Units.java:12` has 8 variants; `domain/units.rs` had 7, so one row
of the Java table could not even be expressed.

*Now:* the variant exists and `calculate_work_minutes_per_unit` returns
`Err(GenerationError::InvalidUnits)` for it, matching Java's `IllegalArgumentException`. The
ported `CalculateWorkMinutesPerUnitTest` table is complete at all 21 rows — the
`UNITS_PER_PERSON` row passes because the zero-standard-value guard precedes the unit switch in
both engines, and a separate test pins the error branch that guard hides.

**1c-3 — `Result`/`GenerationError` in BASIC. ✅ MOSTLY CLOSED (Phase 0a).**
*Was:* BASIC was entirely infallible, where `BASIC_GENERATOR_PLAN.md` had specified `Result`
throughout.

*Now:* threading `UnitsPerPerson` made the whole chain fallible — `calculate_work_minutes_per_unit`
→ `total_work_minutes` → `BasicStandardsProcessor` → the shared `WorkGenerator` trait →
`generate_work_content`. Done in Phase 0 rather than deferred because the flowed generator needs a
fallible trait regardless (`TaskStandardsNotSupported`, `FillGapsNotImplemented`).

*Remaining:* the two cases Java throws on are still unreachable here by construction — a missing
driver returns `0` (`planner_model.rs:73`), and an unresolvable environment yields no value rather
than an error. Neither is a defect; both are consequences of how this port shapes its data.

**1c-4 — `WorkContentCreatorUtility`'s general form. (Not a divergence.)**
Java's `createWorkContentRecord` takes `earliestStart`, `latestStart` and `latestEnd`
independently; Rust's `WorkContent::fixed` collapses them. Checked
`JPR/simplenonflowed/SimpleNonFlowedWorkContentCreator.java`: **the BASIC path only ever calls
`createFixedWorkContentRecord`**, which itself collapses all three to the range's endpoints
before delegating. Rust's `fixed` is a faithful port of that. The general form is used only by
paths outside this scope. `WorkContent::new` (`work_content.rs:37`) retains the full field set,
so nothing is lost.

**1c-5 — Zero-standard-value guard. (Not a divergence.)** Rust guards
`standard_value == 0.0` before the unit switch; so does Java, at
`CalculateWorkMinutesPerUnit.java:18-20`. Matching.

### 1d. Verdict on BASIC

Complete against the Java path, and now agreeing with the Java engine's own expectations on
every case that engine tests. Two deliberate narrowings remain open (1c-1, 1c-2); both matter
more for finishing ADVANCED than for BASIC's own correctness.

---

## 2. ADVANCED (`KBI_RELATED`)

### 2a. Completeness — complete

Every algorithm on the `kbirelated` path has a Rust counterpart or a recorded scope exclusion,
`generate_work` runs the whole eight-step pipeline over every effective date and shift, and the
Java engine's own whole-plan scenarios reproduce exactly. Groups 0 through J are done.

Two inputs the engine consults are still not sourced from anywhere, because this port does not
model them. Both are seams on `AdvancedWorkGenerator` rather than gaps in the algorithm:
- the **seasonal calendar**, through `with_providers`. Standards are priced per environment, so
  a generator without a resolver costs every standard at zero and plans nothing — the same as
  Java against an empty calendar, and asserted by a test so the default is stated rather than
  silent.
- the **existing schedule**, through `with_existing_planned_shifts`. Left empty, every shift the
  generator plans is written, which is Java's behaviour against empty lists.

Group J (acceptance) is what remains.

| | Count |
|---|---|
| Java production classes under `workcontent/kbirelated/` | 60 |
| Spock test files under `JTR/workcontent/kbirelated/` | 59 |
| Rust modules under `generators/advanced/` | 32 |
| Rust tests in the crate | 874 |

### 2b. Group ledger

Per `ADVANCED_GENERATOR_PLAN.md`'s Groups A–J:

| Group | Scope | Status |
|---|---|---|
| 0 — unblock | `UnitsPerPerson`, environment tier, six provider traits | ✅ done |
| A — foundations | rounding, errors, `DistributionItem` + array creator, domain entities, `GeneratorParameters`, `WorkContentTrackerBean` | ✅ done |
| B — distribution utilities | pattern conversion, min/max coverage + adjustment, spread-value conversion, minutes→bodies, aggregation | ✅ done |
| C — spreaders | Beginning / Ending / Middle / Even / Varying / Break | ✅ done — 131 tests |
| D — distributors | `Distributor` trait + factory, flow-plan / retention / capacity, opening / closing, share-with | ✅ done — 73 tests |
| E — non-staff producers | shift-related, spread, recurring standards generators; `WorkTotalMinutesCalculator` | ✅ done — 36 tests |
| F — staff computation | `StaffGenerator`, `StaffStandardsProcessor` | ✅ done — 14 tests |
| G — breaks + final bodies | `BreakCalculator`, the 8-step orchestrator | ✅ done — 25 tests (G2's assembly closed out after H) |
| H — output creation | `DistributionItemTracker`, matcher, planned-shift and work-content record creators, results container | ✅ done — 84 tests |
| I — orchestration | per-date / per-plan processors, generator wiring | ✅ done — 13 tests |
| J — acceptance | four non-flowed scenarios + the rebuilt flowed matrix | ✅ done — 14 tests |

### 2c. Correctness of what does exist

Groups A and B were built test-first against the Spock tables, so unlike BASIC they were
parity-checked as they were written. The ported values include the confirmed rounding drift
(`149.0008`, `608.04`, the `5.8335` top-up), the three 24-element min/max adjuster arrays, the
50-row minutes→bodies threshold table, the `1470.0` cross-granularity spread totals, and the
full 12-row least-common-denominator table.

Two findings from that work, both recorded in `ADVANCED_GENERATOR_PLAN.md`:
- `numbers::round_raw_hours` shipped at 2 decimals; Java's is 4. Fixed. It had one caller,
  `total_work_minutes_calculator.rs:110`, so **BASIC had been carrying a live precision bug**.
- The plan's Decision #2 (a "latent carry-forward bug" in `MaximumValuesAdjuster`) was
  **withdrawn as unfounded** — `remainder` is provably always zero on the truncate path.

### 2d. Verdict on ADVANCED

**Complete against the Java path, and agreeing with the Java engine's own expectations on every
whole-plan scenario that engine describes.** That is the same claim §1d makes for BASIC, and it
is made on the same evidence: every portable Spock table passes, and the end-to-end scenarios
produce the exact times the Java fixtures assert.

What that rests on, and what it does not:

- **Unit parity.** Groups 0 through I were written test-first against the Spock tables and match
  them, including every confirmed asymmetry: the middle spreader's unrounded add; the even and
  varying spreaders' wider clamped window; the break spreader's silent `Varying` no-op; the
  share-with distributor subtracting through the *adding* path so its value is rounded; the two
  deliberately different minimum-finding rules in `DistributionItemTracker`; and the
  planned-shift creator's end-of-array guard that the work-content creator deliberately lacks.
- **Whole-plan parity.** All four non-flowed scenarios reproduce their exact shift times, on the
  first run and with no implementation change. The flowed matrix reproduces all eight of
  `FlowedIntegrationTest`'s `verifyTest*` expectations — see §2e, which is where the caveats are.
- **What is still not established:** equivalence over untested inputs. The ported tables inherit
  Java's own coverage gaps, and no differential harness against the live engine was built. Two
  inputs are also still unsourced — the seasonal calendar and the existing schedule — so nothing
  here says the generator behaves correctly against *real* data, only that it agrees with Java
  given the same inputs.

Group H's tables passed against the implementation unchanged, which is the strongest unit-level
evidence in the port — `PlannedShiftRecordCreatorTest` asserts the start time, end time and
duration of every shift that falls out of a full peel, so an error anywhere in the tracker, the
endpoint rules, the minimum-shift stretch or the deduction would have shown.

One reading in the earlier Group H notes was wrong and is corrected in
`ADVANCED_GENERATOR_PLAN.md`: the six-row endpoint table belongs to `findPlannedShiftEndpoint`,
not to the long-shift function.

### 2e. The flowed matrix — what its passing does and does not prove

`FlowedIntegrationTest` carries `@Ignore` on the whole feature method, so **no row of it runs
upstream**, including the one row that is not commented out. Its expectations are therefore a
record of what the engine was believed to do, not proof of what it does. All eight distinct cases
reproduce in Rust, which is real evidence — but evidence that the Rust engine agrees with a
written-down belief, not with a green Java run.

Three findings came out of rebuilding it, all of which the Rust tests now assert:

- **Its `distributionOption` column is inert.** The non-staff standard is FLOWED, so it follows
  the curve and the non-flowed shape is never consulted. Varying it across BEGINNING / MIDDLE /
  END gives the identical plan.
- **Its `staffValue` column is inert too, because of a defect in the fixture:** the staff
  standard is built, configured, given a flow plan — and never added to the assignment. It is
  dead in all 32 rows. Attaching it changes the plan, so the omission is not harmless.
- **The plan-level distribution shape must be left unset**, as the Java fixture leaves it. The
  break spreader reads the *plan's* shape rather than the standard's, and its `Varying` branch —
  the unset default — silently places nothing. Setting it to the standard's shape instead puts an
  hour of break at the front of the shift, stacks a second body over 07:00–08:00, and adds a
  spurious 07:00–11:00 shift to every row. This was found by making exactly that mistake: seven
  of the eight rows failed with that one extra shift until the setting was left alone.

Those two inert columns are why the 32-row matrix collapses to eight cases rather than being
revived row by row. The collapse is tested, not assumed.

Two Java assertions in `ShareWithDistributorTest` could not be reproduced as written, and the
reason is a defect in that test rather than in either engine: its schedule list is a Spock
`@Shared` field that is never cleared between feature methods, so the later cases silently
inherit full-shift schedules from earlier ones. Run in isolation the half-schedule case deducts
`-240` rather than `-480`, and two adjacent half-schedules leave the period on their boundary
untouched — it is claimed by the earlier window, which matches it inclusively, then rejected by
the exclusive check, so the later window is never consulted. Both Rust tests assert the isolated
behaviour and carry the explanation.

---

## A note on the reference copy

`planner/java/engine/` is **not** a faithful copy of production, despite earlier claims in both
generator plans. Confirmed divergence: `DistributionMinutesToBodiesConverter`'s below-one branch
reads `Numbers.round(value)` in the local copy but `TDouble.round(value, 4)` in production. Those
give different answers (`0.13` → `0` vs `0.13`), and only production's matches the Spock table.

Use the local copy as an index of what exists. Read algorithms from `JPR` and tables from `JTR`.

---

## Limits

**What is established.** The Rust BASIC path agrees with the Java engine's own test expectations
on every case that engine tests, verified by 56 ported assertions whose sensitivity was confirmed
by mutation. Every Java class on the BASIC path has a counterpart. Every difference between the
two models is catalogued with evidence. ADVANCED's true extent is stated exactly.

**What is not.** This is not a proof of equivalence. The ported tests inherit the Java suite's own
coverage gaps — where Java doesn't test a case, neither engine is pinned there, and BASIC's
untested regions remain untested in both. No differential harness was built, so the two engines
have never been run over the same inputs and compared; that would require the `taps` gradle
monorepo building plus a serialization bridge, and is the natural next step if stronger assurance
is wanted.

**Open decisions.** 1c-1 (environment tier) and 1c-2 (`UnitsPerPerson`) are deliberate narrowings,
not defects. Both block ADVANCED Group E2 and need a call before that group starts.
