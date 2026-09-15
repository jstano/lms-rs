# Complete the ADVANCED (KBI_RELATED) Work-Content Generator — TDD, Full Parity, Functional Core

## Context

The Rust `ADVANCED` generator (`planner/src/workcontent/generators/advanced/`), porting Java's
`KBI_RELATED` standard type, is a complete stub — `generate_work` returns an empty result. This
is the largest, most algorithmically dense part of the whole planning engine: ~60 Java
production classes plus ~63 Spock test files under `workcontent/kbirelated/` (roughly 2x the
size of the `BASIC`/`SIMPLE_NON_FLOWED` path already planned in
`planner/src/workcontent/BASIC_GENERATOR_PLAN.md`). This plan follows the exact same
conventions established there — read them as a prerequisite if picking this up fresh:

- **TDD**: for every piece, port the *existing* Java/Spock tests to Rust as failing tests
  first, then implement to green. Ground truth lives in a sibling checkout, not this repo:
  - Production source root (`JPR`): `/Users/jstano/workspace/unifocus/taps/src/java/com/unifocus/watson/server/labor/planner/engine/workcontent/kbirelated/`
  - Spock test root (`JTR`): `/Users/jstano/workspace/unifocus/taps/src/junit/com/unifocus/watson/server/labor/planner/engine/workcontent/kbirelated/`
  - ⚠️ **The reference copy in `lms-rs/planner/java/engine/` is NOT behaviorally identical**,
    despite what earlier revisions of this plan claimed. Confirmed divergence found while
    porting Phase B5: `DistributionMinutesToBodiesConverter`'s below-one branch reads
    `Numbers.round(value)` in the local copy but `TDouble.round(value, 4)` in production. The
    two give different answers (`0.13` → `0` vs `0.13`), and only production's matches the
    Spock table. **Read algorithms from `JPR` and tables from `JTR`.** The local copy is a
    convenient index of what exists, not an authority on what it does.
- **Functional core, I/O pushed to the edges**: read → input → calculate → output → save.
  Everything in this plan is the "calculate" stage — pure, no I/O.
- **`Result`/`Option` everywhere, no panics** in the calculation core, using the
  `GenerationError` enum in `planner/src/workcontent/generators/error.rs` (added by Phase A0.2,
  **not** by the BASIC plan), extended with new variants as needed.

## ⚠️ Reconciliation with the shipped code (read this first)

This plan was originally written against a projected version of the BASIC port that was never
built that way. The naming and conventions below are what actually shipped; **Groups C–J still
refer to the old names in places, so translate as you go.**

| This plan originally said | What actually exists |
|---|---|
| `common/rounding.rs`, `round_to_decimals` | `common/numbers.rs`, `round_to(value, decimals)` |
| `round_raw_hours` = 4 decimals | now true — it was shipped at 2 and **fixed** in Phase A0.1 |
| `Kbi` / `KbiId` | `BusinessDriver` / `BusinessDriverId` |
| `FlowPlan` / `FlowPattern` | `DistributionSchedule` / `DistributionPattern` |
| `ShiftRelatedStandard` | `ShiftStandard` (extended in Phase A2) |
| `AssignmentMinMaxCoverage` | `JobMinMaxCoverage` — the code is still `Job`-based, not `Assignment`-based |
| `AssignmentShift` / `AssignmentShiftDetail` | `JobShift` / `JobShiftDefinition` |
| `basic_environment_resolution::assignment_shift_detail` | `JobShift::shift_detail_for_date(date)` |
| `PlannerSettings::max_shift` | `PlannerSettings::max_shift_length` (public field, not a getter) |
| free functions (`pub fn generate(params)`) | trait + `Impl` struct DI, matching shipped BASIC |
| a new `DateTimeRangeWithPeriodLength` | `date_range_rs` already has the index maths as `DateTimeRange::with_period_len`; `common/date_time_range_with_period_length.rs` is only a thin owned facade over it |

Reuse from the shipped BASIC code, which is complete and green: `basic_calculator::calculate`
(Group G1), `basic_work_content_creator` (Group H5), `JobShift::shift_detail_for_date` and
`JobShiftDefinition::to_date_time_range` (Group I1), `PlannerSettings::dates` (Group I2).

## The 8-step algorithm (REQUIREMENTS.md, confirmed against source)

```
1. Collect shift context for (date, AssignmentShift); skip if invalid/no times.
2. Non-staff minutes/period — aggregate ShiftRelatedStandardsGenerator + SpreadStandardsGenerator
   + RecurringStandardsGenerator output (raw, no breaks).
3. Add paid breaks to non-staff minutes (per-period array, not a single scalar).
4. Convert non-staff minutes → bodies/period; apply min/max staffing.
5. Staff minutes/period, using the bodies from step 4 as a shift-count multiplier.
6. Combine non-staff (no-break) + staff minutes; apply breaks again.
7. Convert combined minutes → final bodies/period; apply min/max (authoritative).
8. Produce work blocks (WorkContent) and planned shifts from final bodies.
```
Steps 2–8 are each a distinct Rust module below; step 1 is the date/shift-detail resolution
already built for BASIC (`basic_environment_resolution`), reused here.

## Scope

**In scope**: the full live call chain from `KBIRelatedGenerator` down through every class it
touches for `ShiftRelatedStandard` (non-`Staff` and `Staff` work types), `SpreadStandard`
(fixed + dynamic), and `RecurringTaskStandard` — i.e., steps 1–8 above with real distribution
math (flow curves, spreaders, min/max staffing, break redistribution).

**Out of scope** (consistent with BASIC's scoping rationale — infrastructure nothing in this
project has yet, or genuinely separate concerns):
- `TaskStandard`/`TaskStandardDetail`/`TaskStandardRange`/`TaskStandardFrequency` and
  `WorkType::Task` — `WorkTotalMinutesCalculator`'s TASK branch is real production code, but
  modeling the full task-standard family is its own separate effort (frequency/expectancy
  formulas, its own environment-scoped lookup). Port the branch as an explicit
  `GenerationError::TaskStandardsNotSupported` for now — flagged clearly, not silently wrong.
- `RetentionCapacityUtilization`/`RevenueCenter` (feeds `FlowPlanRetentionDistributor` and
  `FlowPlanCapacityDistributor`) — real entities in production, not explored in this pass.
  Modeled here as an injectable provider trait (see Phase D2) so the distribution math is
  fully testable against known inputs without modeling the revenue-center config tree; wiring
  a real provider is a follow-up.
- DB-backed integration tests (`AssignmentShiftDetailServiceIntegrationTest`-style,
  `@Transactional` Spring/Hibernate tests) — not portable as Rust unit tests. The three
  **fully-live, non-DB-fixture** integration scenarios (`NonFlowedWithClosingWorkIntegrationTest`,
  `NonFlowedWithOpeningWorkIntegrationTest`, `SecondNonFlowedWithOpeningWorkIntegrationTest`)
  are re-expressed as Rust acceptance tests in Phase J, since their expected outputs are
  concrete and don't require a live DB to assert.
- `ExistingPlannedShiftMatcher`'s dedup-against-already-scheduled-shifts logic is ported
  (Phase H2) but has no real "existing schedule" data source yet (no persistence) — it's
  wired and tested against in-memory fixtures only, same "read" deferral as BASIC.
- Dynamic-standard-set mode (`plannerModel.getStandardSetForDate`) — same as BASIC, out of
  scope; `processSingleShiftDate`'s only other difference from `processShiftDates` (calling
  the per-date method once vs. looping `EffectiveDateFilter`) is kept, but always resolves the
  plan-wide standard set.

## Known decisions / deviations (flagged, following BASIC's precedent of confirming from source rather than guessing)

1. **`DistributionItem::add_to_period_value` rounds via `round_percent`** (`TDouble.roundPercent`,
   confirmed = `round_to(value, 4)`). **`subtract_from_period_value` does NOT round**
   (straight subtraction) — a real, confirmed asymmetry in Java; replicate faithfully rather
   than "fixing" it. ✅ Done in Phase A1.
   **Correction:** the original claim that `ShareWithDistributor` depends on
   `subtract_from_period_value` is **wrong**. `ShareWithDistributor.java:57` calls
   `item.addToPeriodValue(-1 * Numbers.roundRawHours(minutesPerPeriod))` — it subtracts through
   the *adding* path, so it **does** round. The only confirmed caller of the unrounded path is
   `DistributionItemTracker.deductMinimumValueFromListItems` (Phase H1). Fix Phase D4's
   description accordingly when Group D is picked up.
2. ~~**`MaximumValuesAdjuster.applyMax`'s remainder handling on the truncate branch**~~ —
   **WITHDRAWN (Phase B3, confirmed from `JPR`).** The claimed latent carry-forward bug does not
   exist. `remainder` is only ever *assigned a non-zero value* inside the `if (delayOverflow)`
   guard; the other two branches both set it to `0`, and it starts at `0`. So on the truncate
   path it is provably always zero and there is nothing to reset. **No deviation was applied** —
   the Rust port writes `remainder = if delay_overflow { wanted - ceiling } else { 0 }`, which is
   a faithful transcription, not a fix. The Java truncate fixture's expected array
   (`[2,2,2,2,3,3,2,...]`, where the period after a clamp falls straight back to its own demand)
   confirms the behavior directly.
3. **`PlannedShiftRecordCreator` drops its final peeled layer's shifts when that layer's
   `ending_index` reaches the end of the array** (the `isAtEndOfPeriods` check happens
   *before* `records.addAll(...)` for that iteration); **`WorkContentRecordCreator` has no
   equivalent guard**. Both behaviors are independently confirmed by passing tests in their
   respective Java suites — **preserve both exactly as-is**, do not "fix" one to match the
   other.
4. **`ExistingPlannedShiftMatcher.plannedShiftsMatch`** compares `assignment` by reference
   identity (`!=`) but `job`/dates/`shiftType` by value — replicate faithfully (Rust: compare
   `assignment_id` by value, since Rust has no reference-identity equivalent worth
   introducing only for this one field — this is a deliberate, noted behavior change from
   "identity" to "value" comparison, which should be equivalent in practice since two
   `Assignment`s with the same id are the same assignment).
5. **`LowestCommonDenominatorConverter`'s known rounding drift** (15min→10min flow-to-planner
   conversion sums to `608.04` instead of `608`, and the Java test's total-check assertion is
   commented out, acknowledging it). **Decision: replicate bit-for-bit** (match the confirmed
   rounding algorithm exactly, drift included) rather than "correct" it — consistent with this
   whole port's philosophy of matching verified Java behavior over assumed-cleaner math.
6. **Max-value-of-`0` is a sentinel for "no cap defined"** in both `MaximumValuesAdjuster` (in-
   and post-shift) — not "cap at zero." Same convention appears in `AssignmentMinMaxCoverageFilter`'s
   fallback (`(0,0)` pairs for days with no configured coverage). Must not be conflated with a
   genuine zero-staffing cap.
7. **Duplicate suppress-value formula** (`suppressed_kbi_value = max(kbi_value - suppress_value, 0)`,
   `shift_length = min(max_shift_length, shift_definition.shift_length_hours())`) appears
   verbatim in both `StaffGenerator` and `WorkTotalMinutesCalculator` in Java (confirmed by
   identical test tables in both `StaffGeneratorTest` and `WorkTotalMinutesCalculatorTest`).
   **Consolidate into one shared Rust function** (`basic_total_work_minutes`-adjacent, reused
   by both — see Phase E4), backed by the shared table as its test.
8. Error variants added to `GenerationError` (`generators/error.rs`) for this plan:
   `TaskStandardsNotSupported`, `FillGapsNotImplemented`, `NoDynamicSpreadService`,
   `LowestCommonDenominatorUndefined` (mirrors Java's `IllegalStateException` in
   `calculateLeastCommonDenominator` — should be unreachable for the supported period-length
   set 10/15/30/60, but kept total rather than `unreachable!()`).

---

## Phase 0, Groups A–J — COMPLETE ✅ (the port is done)

Foundations and distribution utilities are ported, green, and match their Java ground truth.
The crate went from 215 to 425 passing tests with no warnings. What exists now, by phase:

**A0 — rounding + errors**
- `common/numbers.rs`: `round_raw_hours` **corrected to 4 decimals** (it shipped at 2, which was
  a latent precision bug in BASIC's sole caller, `total_work_minutes_calculator.rs`);
  added `round_percent` (4 dp) and made `round_to(value, decimals)` public.
- `generators/error.rs`: `GenerationError` with `TaskStandardsNotSupported`,
  `FillGapsNotImplemented`, `NoDynamicSpreadService`, `LowestCommonDenominatorUndefined { a, b }`.

**A1 — the period array**
- `common/date_time_range_with_period_length.rs`: owned facade over `date_range_rs`'s
  `DateTimeRange::with_period_len`, exposing `start_index`/`end_index`/`index_range`/
  `number_of_periods` as `Option` (a non-positive period length has no indexes).
  `index_range()` is inclusive and stops one before the end index, matching Java's
  `getIndexRange()`.
- `generators/advanced/distribution_item.rs`: `DistributionItem` with the rounding asymmetry.
- `generators/advanced/distribution_item_list_creator.rs`: `DistributionItemListCreator` trait
  with `create_array(range)`, `create_array_for(params)`, `clone_and_reset_array`. Always two
  days, always anchored at midnight of the start date.

**A2 — domain entities**
New under `domain/`: `environment.rs`, `distribution_method.rs`, `distribution_pattern.rs`
(with `period_length_in_minutes()` deriving 60/30/15/10/5 from the period count),
`distribution_schedule.rs` (`pattern_for_date`), `spread_standard.rs`,
`recurring_task_standard.rs`, `job_min_max_coverage.rs` (`MinMaxStaffing`, with
`has_maximum()` encoding the zero-is-uncapped sentinel).
Extended: `ShiftStandard` gained the six flowed fields as **defaulted chainable setters**
(`distributed_by`, `with_distribution_schedule`, `ignoring_retention`, `within_work_window`, …)
rather than a 14-argument constructor; `Job` gained `with_flowed_standards(..)` plus three
lookups; `PlannerModel` gained `business_driver_value_for_week`.
- `common/dates.rs`: `days_between`, `week_containing(date, week_ending_day)`.

**A3 — generation context**
- `generators/advanced/generator_parameters.rs`: `GeneratorParameters<'a>` holding
  `shift` and `shift_detail` separately (the Rust `JobShiftDefinition` has no back-reference to
  its shift, unlike Java's `AssignmentShiftDetail`). Also a `fixtures::Context` helper —
  **later groups should build test parameters through it**, since `GeneratorParameters` borrows
  six things that otherwise need six separate bindings alive in every test.
- `generators/advanced/work_content_tracker.rs`: `WorkContentTrackerBean`.

**B1–B6 — distribution utilities**
- `pattern_conversion.rs` — `PatternToDistributionConverter`, dispatching to split / sum /
  common-denominator regrouping. All three collapse into one `regroup_through(items, values,
  pattern_periods, sub_periods)` helper, since larger→smaller is just `(1, n)`, smaller→larger
  is `(n, 1)`, and the ten/fifteen pairing is `(2, 3)` or `(3, 2)`. Also
  `calculate_least_common_denominator`. The known drift is reproduced exactly: a group is
  topped up when its rounded shares fall short but never trimmed when they overshoot, giving
  the confirmed `149.0008` and `5.8335` values.
- `min_max_coverage.rs` — `min_max_coverage` + `populate_min_max_distribution`, with an
  injectable `EnvironmentResolver` trait standing in for the unmodelled seasonal calendar.
- `min_max_adjuster.rs` — `MinMaxAdjuster` trait plus `apply_minimum_values` /
  `apply_maximum_values`. All three Java fixture arrays pass byte-exact.
- `spread_values_conversion.rs` — `SpreadValuesConverter`; totals `1470.0` at every granularity.
- `minutes_to_bodies.rs` — `MinutesToBodiesConverter`; the full Java rounding table passes,
  including every `threshold == 0.0` strict-comparison row.
- `distribution_aggregator.rs` — `DistributionAggregator`.

**Deviation from the original B6 note:** it claimed B6's tests could not run until Group C
landed. That was an artifact of the Java fixtures using spreaders to *construct* their inputs,
not a real dependency. The Rust tests build input arrays directly and B6 is fully green now.

**Phase 0 (unblock) — done.** `Units::UnitsPerPerson` added and the whole work-minutes chain made
fallible up to the `WorkGenerator` trait; the environment tier restored under `ShiftStandardRange`
with Java's two-tier lookup; six provider traits in `generators/advanced/providers.rs` with a
`Providers` bundle and `NoProviders` defaults. See `PARITY_AUDIT.md` 1c-1/1c-2/1c-3.

**Group C (spreaders) — done, 131 tests.** `generators/advanced/spreaders/`: a `WorkSpreader`
trait with the no-op `populate_against_existing_work` default, shared `spread_bounds` /
`exclusive_spread_bounds` helpers, and six spreaders. Every confirmed asymmetry is reproduced and
commented at the site: the middle spreader does **not** round on the way in; the even and varying
spreaders keep the range's own end index and add one back after the max-shift clamp, giving a
window a period wider than the others; the break spreader's `Varying` fallback silently places
nothing. The break spreader's full Spock suite passes, including the six-row rounding table and
the gap-in-the-tail case where an empty period counts as fully open rather than as a barrier.

**Group D (distributors) — done, 73 tests.** `generators/advanced/distributors/`: the
`Distributor` trait, `get_distributor` dispatch (with `FillGaps` reported as an error, as Java
throws), and six distributors — non-flowed, flowed, retention, capacity, opening, closing,
share-with. `FlowPlanSpreadDistributor` is not a module: it is a one-line delegation to
`spread_values_conversion`, already ported in Group B, so it is folded in.

Notes for later groups:
- `PlannerModel` gained `max_duration_minutes_for_dynamic_work` and
  `legacy_flow_pattern_rounding`, both plain plan-level settings Java holds the same way.
- `DistributionPatternProvider` returns the **already-converted** curve, matching Java's
  `FlowPatternDataProvider`, rather than a pattern id — the schedule lookup, day-of-week pick and
  granularity conversion all sit behind that one call.
- The capacity carryover's "recompute-then-subtract" step is transcribed literally and commented:
  it recomputes the carryover to the value it already held, so it provably does nothing. Kept so
  the two sources read alike.

**Group E (non-staff producers) — done, 36 tests.** Five modules:
`work_total_minutes.rs` (E4), `shift_related_standards.rs` (E1), `spread_standards.rs` (E2),
`recurring_standards.rs` (E3), `non_staff_standards.rs` (E5, including `NonStaffResults`).

The confirmed asymmetries are reproduced and commented at the site:
- **E1's two halves gate differently.** Non-flowed standards are totalled by shape and only
  count when their driver is trading; every other standard is costed whether or not it is.
- **E1 totals before distributing.** Five standards all set to BEGINNING produce one block of
  work, not five overlapping ones — the grouping key is the standard's *own* shape, with
  "unset" a group of its own, so two unset standards stay together.
- **`FillGaps` standards are dropped in silence**, not reported: the guard sits before the
  distributor is ever requested, so `FillGapsNotImplemented` is unreachable from this path.
- **E3 gates only variable tasks.** Fixed recurring work happens whether or not the driver
  trades, because it was never volume-driven.
- **E5 pushes the recurring array unconditionally**, so a shift with no tasks due still comes
  back with an empty array in the results — an `add`, not an `addAll`.

Two shared helpers were extracted rather than duplicated:
`ShiftStandard::suppressed_driver_value` (used by E4, the non-flowed path, and F1 next) and
`work_total_minutes::shift_length_for_standards` (E4 and F1).

`PlannerModel` gained `week_ending_day`, which decides what "this week" means to a weekly
standard. `DynamicSpreadProvider` now returns `Option<Vec<i32>>` keyed on the shift range,
matching Java's `DynamicSpreadService`.

**Group F (staff computation) — done, 14 tests.** One module, `staff_standards.rs`, holding
both Java classes.

**Deviation from Java's structure, deliberately.** Java's `StaffStandardsProcessor` builds the
provisional planned shifts itself, by calling `PlannedShiftRecordCreator` — which is Group H and
does not exist yet. The Rust `process` takes the shifts as a parameter instead. That removes the
forward dependency entirely and is the better seam anyway: this stage only needs to *count* the
shifts, not to know how they were arrived at. **Group G2 must call H4 and pass the result in.**

Confirmed behaviours, commented at the site:
- **The shift count is the largest plan type's, not the sum.** A projected plan writes each
  shift under both FORECAST and ORIGINAL, so summing would double the staffing work.
- **No openness gate**, unlike the non-flowed half of E1 — staffing work is owed for whoever is
  on shift whether or not the driver is trading.
- Contributions are truncated as they are accumulated, matching Java's `int +=` narrowing.

Reuses `work_total_minutes::calculate_minutes` and `ShiftStandard::suppressed_driver_value` from
Group E rather than repeating the formula, which is what the plan's Decision #7 asked for.
`Job` gained `staff_shift_standards_for_standard_set_and_shift`, the mirror of the non-staff one.

**G1 (`break_calculator.rs`) — done, 7 tests.** Reuses `basic_calculator::calculate` — the
confirmed shared calculation — and hands the result to the break spreader. The
`total_existing_work <= 0` short-circuit the Java suite never covered (it mocks the calculator,
so the path is invisible to it) now has a test.

**G2 (`standards_processor_for_shift.rs`) — done, 18 tests (assembly closed out after Group H).**
All three sequences are built and tested:
- `process_non_staff_standards` — steps 1 to 3
- `combine_with_breaks` + `calculate_final_bodies` — steps 5 to 7
- `aggregate_staff_minutes` — step 4's aggregation

**The step that matters.** Step 5 combines the **pre-break** non-staffing minutes with the
staffing minutes; breaks are then recomputed from scratch over the combined day. Feeding it the
with-break total instead would count every break twice. `NonStaffResults` therefore carries the
pre-break minutes deliberately, and there is a test that simulates the mistake and asserts the
minutes inflate. That assertion is on **minutes, not bodies** — at the fixture's rounding
thresholds a rounded head count absorbs the difference entirely and shows nothing wrong, which
is exactly how this bug would escape a body-count test.

**Assembly, closed out after Group H.** `process_staff_standards` builds the provisional shifts
for step 4 from the non-staffing bodies, and `process_standards_for_shift` runs all eight steps
and returns an `AdvancedResults`. Two things are worth keeping in view:
- The step-4 shifts are **provisional and discarded**. They exist only to be counted; the shifts
  actually written come from the final head count at step 7, after the staffing work has been
  folded back in. A test asserts none of the provisional shifts is one of the written ones.
- **Only the forecast copy is matched** against the schedule, because that check lives inside
  Java's `PROJECTED` branch. Replanning a day therefore leaves the forecast alone and rewrites
  the plan's own copies every run. The first version of that test assumed matching applied to
  every plan type and failed; the assertion now states the real rule.

**Still open from Group A/B scope:** `min_max_coverage` and the `MinMaxAdjuster` take an
`EnvironmentResolver`, and nothing resolves environments for real yet. Group I made this a seam
rather than a gap — `AdvancedWorkGenerator::with_providers` takes the resolver — but a real
source still has to be supplied by whatever reads the plan's data, and until it is, the
generator plans nothing.

**Group H (output creation) — done, 84 tests.** Six modules, and the densest Java fixtures in the
port. Every Java table passed against the implementation unchanged.

- **H1 `distribution_item_tracker.rs` (50 tests).** All eight functions, with the two
  minimum-finding rules kept deliberately apart: the work-content rule resets to the block's
  first period and lets an uncovered period pull the depth to zero; the planned-shift rule seeds
  from the depth already held and steps over uncovered periods. No Java table contrasts them, so
  a test here does. `adjust_for_min_shift` can take a block outside the shift window and is not
  clamped — the three `min_shift = 16.0` rows, where the minimum is capped at a six-hour shift,
  pin that behaviour.
  - **Correction to the earlier reading of `DistributionItemTrackerTest`:** the six-row endpoint
    table belongs to `findPlannedShiftEndpoint`, not to the long-shift function, which has a
    four-row table of its own. The long-shift rows back off trailing empty periods, so an
    eight-hour maximum over nine covered periods ends at index 25, not at the maximum.
  - `deduct_minimum_value_from_list_items` is the only confirmed user of the unrounded
    `subtract_from_period_value`.
- **H2 `existing_planned_shift_matcher.rs` (13 tests).** Java removes matched shifts from lists
  hanging off `PlannerModel`; the model is shared immutably here, so the pools live in the
  matcher. The consuming behaviour is the same and is now genuinely asserted — Java's own
  "pool is empty afterwards" assertions run against mocks that hand back a fresh list each call
  and cannot fail. Assignments are compared by id rather than by object identity.
- **H3 `planned_shift_creator.rs` (8 tests).** `shift_date` (when the work starts) and
  `date_shift_generated_from` (the template it came from) stay separate, pinned by the
  midnight-crossing case. Shift categories and the labor-level `isAssignment()` distinction are
  not modelled; the assignment id is set unconditionally, as shipped BASIC does.
- **H4/H5 `planned_shift_record_creator.rs` + `work_content_record_creator.rs` (18 tests) —
  implemented together, and the asymmetry between them is preserved.** H4 breaks *before* adding
  a layer whose `ending_index` reaches the end of the array; H5 has no such guard and records the
  work regardless. Both are confirmed by passing Java tests, and a Rust test asserts the contrast
  directly so a later tidying pass has to argue with a red test rather than with a comment.
  All three of `PlannedShiftRecordCreatorTest`'s shift-shape fixtures — five shifts without long
  shifts, four with, six on the 20:00 shift — matched exactly on the first run.
- **H6 `kbi_related_results.rs` (3 tests).** Named `AdvancedResults`, minus the logger field.

`PlannerModel` gained `with_planner_mode`, matching its existing `with_*` builders, so tests can
plan the same fixture in projected and standard modes.

**Group I (wiring) — done, 13 tests. `generate_work` now generates.** Java's two wiring classes
collapse into one `standards_processor.rs`, mirroring how shipped BASIC keeps both loops in
`basic_standards_processor.rs`: `process` walks the effective dates, `process_for_date` is the
single-date entry point, and both go through `process_standards_for_date`, which skips a shift
whose detail does not resolve for the date or has no times.

- **The matcher is threaded through the whole run**, not rebuilt per shift or per date. An
  existing shift accounts for exactly one newly planned shift wherever in the run it turns up,
  and there is a test that schedules only the first date's shifts and checks the second date is
  untouched.
- **`AdvancedWorkGenerator` now calls the processor** and maps `AdvancedResults` into
  `WorkResults`. It holds `Providers<'static>` plus the existing-schedule pools, with
  `with_providers` and `with_existing_planned_shifts` as the seams — Java reads both off
  `PlannerModel`, which this port does not model.
- **The default generator plans nothing, and that is a decision.** With no `EnvironmentResolver`,
  `work_total_minutes` resolves no environment and every standard costs zero. That matches Java
  against an empty seasonal calendar, and is asserted by a test so it reads as intent rather than
  as the old stub's silence.
- **Single-date divergence:** Java resolves the standard set *for that date* in
  `processSingleShiftDate` and the plan's own set in the full run. This port holds one standard
  set per plan, so both paths read the same one. Noted at the site.

New test fixture `generators/advanced/fixtures.rs`: `JobFixture` builds a whole job — several
shifts across several dates, with standards keyed to the shift ids — because the wiring stages
need a shape `generator_parameters::fixtures::Context` cannot express (it describes one shift on
one date). Every mutator rebuilds the job and model together, since standards are keyed on shift
ids that are minted with the shifts.

Both Java wiring tests mock the stage below them and count calls. The Rust ports run the real
stage and compare against the same job configured with only the valid shift — a stronger
assertion, since it pins the output and not just the call count.

**Group J (acceptance) — done, 14 tests, in `generators/advanced/acceptance.rs`.** Whole-plan
scenarios: standards in, shift times out. Every other test in this port checks one stage against
one table; these are the only ones that catch a stage being right alone and wrong in sequence.

**The four non-flowed scenarios all passed on the first run, exact times, no implementation
change.** They are worth naming because each exercises a different part of the peel:
- `NonFlowedWithClosingWorkIntegrationTest` → `08:00–16:00` and `13:00–17:00`. Nine hours of
  coverage over an eight-hour ceiling, with the leftover hour stretched *backwards* to the
  four-hour minimum.
- `NonFlowedWithOpeningWorkIntegrationTest` → `07:00–15:00` and `12:00–16:00`. The mirror image.
- `SecondNonFlowedWithOpeningWorkIntegrationTest`, both cases: `limit_shift_to_max_shift` true
  gives `00:00–08:00` and `00:00–06:00` (the spread is capped, so the work stacks two deep over
  the first six hours); false gives `00:00–08:00` and `08:00–14:00` (the work lies flat across
  fourteen hours and the shifts follow one another). The pair is the clearest demonstration in
  the suite of what that setting actually does.

**The flowed matrix — read `PARITY_AUDIT.md` §2e before trusting it.** `FlowedIntegrationTest`
carries `@Ignore` on the whole feature method, so no row of it runs upstream, including the one
that is not commented out. All eight distinct cases reproduce in Rust; that is agreement with a
written-down belief, not with a green Java run.

Three findings, each now asserted rather than assumed:
- **`distributionOption` is inert** — the standard is FLOWED, so the non-flowed shape is never
  read. BEGINNING / MIDDLE / END give the identical plan.
- **`staffValue` is inert because the fixture never attaches its staff standard.** It is built,
  configured, given a flow plan, and then dropped. Dead in all 32 rows. Attaching it in Rust
  changes the plan, so the omission is a real defect, not a harmless one.
- **The plan-level distribution shape must be left unset.** The break spreader reads the *plan's*
  shape, not the standard's, and its `Varying` branch — the unset default — silently places
  nothing. Setting it to the standard's shape instead adds a spurious `07:00–11:00` shift to every
  row. Found by making that mistake: seven of eight rows failed with exactly that one extra shift.

Those two inert columns are why the 32-row matrix collapses to eight cases rather than 32 mostly
duplicate rows. **Contrary to the plan's expectation, none of the 31 revived rows needed to be
marked `#[ignore]`** — they were not disabled because they disagreed with the engine.

## Group C — Spreaders (intraday curve shapes)

New directory `generators/advanced/spreaders/`. Shared helper: `adjust_ending_index_for_limit_max_shift`
(clamp `ending_index` to `starting_index + max_shift_in_periods - 1` when `limit_shift_to_max_shift`).

### C1. `BeginningWorkSpreader` / `EndingWorkSpreader`

**Model:** Opus
Fill one period at a time from the front (Beginning) or back (Ending), wrapping around
(re-adding to already-filled periods) if total work exceeds the range span. Each add rounded
via `round_raw_hours`.

**Test (TDD, port verbatim):** full `where:` tables for both — 10/15/30-min periods × 24h/48h
spans, confirmed exact values including the wraparound front-load pattern (e.g. `1000min → [90,70,60,60]`).

### C2. `MiddleWorkSpreader`

**Model:** Opus (confirmed asymmetry vs. C1/C4 — no `round_raw_hours` wrapper on the add; a
model porting by pattern-match from the other spreaders could "fix" this incorrectly)
Ping-pongs outward from the center pair (`middle_index = (ending-starting)/2`), alternating
front/back, **without** the `round_raw_hours` wrapper on the add (raw add, only
`DistributionItem`'s own `round_percent` rounding applies — confirmed asymmetry vs. C1).

**Test (TDD, port verbatim):** full table (10/15/30-min × 24h/48h), including the exact
center/next-to-center split (`minutesInPeriod0/1`, `minutesInMiddle`, `minutesInPeriod2/3`).

### C3. `EvenWorkSpreader`

**Model:** Opus (the confirmed `+1` HACK on the clamped ending index is easy to "correct" away)
Pure division — `minutes_per_period = round_raw_hours(total / shift_period_count)`, same value
in every period, no iteration/wraparound. Note the confirmed `+1` on the clamped ending index
when `limit_shift_to_max_shift` applies (documented HACK in Java source — replicate as-is).

**Test (TDD, port verbatim):** the per-period-value table (420/480/540/1000 min over an
8-period 10-min-period window → 8.75/10/11.25/20.8333 per period).

### C4. `VaryingWorkSpreader`

**Model:** Opus
Ping-pongs across the **whole** range (not centered), forward-then-backward-then-forward,
bouncing at each boundary; each add rounded via `round_raw_hours`.

**Test (TDD, port verbatim):** the 5-case table (single-period 12min; exact-fit 480min;
overflow-bounce 510min/990min; both `limit_shift_to_max_shift` cases at 8 and 16 periods).

### C5. `BreakSpreader`

**Model:** Opus (the most algorithmically dense single class in the whole port — two-phase
open-shift-run detection plus a fallback dispatch with a deliberate no-op `Varying` case)
The most complex spreader — two-phase: (1) `fill_work_into_end_of_shift` tries to slot break
minutes into "open" (under-capacity) trailing periods of existing work, walking backward from
`ending_index` to find the contiguous open-shift-count run, then forward to allocate; (2) any
leftover falls through to `spread_remaining_work`, dispatching to Beginning/Middle/End/Even
per `PlannerSettings::default_non_flowed_distribution_method` (no `Varying` fallback — Java's
switch has no case for it, silently does nothing; replicate as a no-op branch, not an error,
since it's reachable production behavior not a misconfiguration).

**Test (TDD, port verbatim):** `BreakSpreaderTest`'s full suite — no-existing-work (falls to
each of Beginning/End/Middle/Even fallback); no-open-room-at-shift-end (also falls through);
partial-open-tail cases; the `buildComplexArray` multi-shift scenario with a zero-existing-work
gap in the middle (confirmed to still count as "open," not treated as a break in the backward
scan); and the rounding-threshold open/closed-period boundary table.

---

## Group D — Distributors

### D1. `Distributor` trait + `DistributorFactory` + `NonFlowedWorkDistributor`

**Model:** Opus
```rust
pub trait Distributor {
    fn distribute(&self, work_minutes: f64, standard: Option<&ShiftRelatedStandard>, params: &GeneratorParameters) -> Vec<DistributionItem>;
}
pub fn get_distributor(standard: &ShiftRelatedStandard) -> Result<Box<dyn Distributor>, GenerationError>
    // DistributionMethod::FillGaps -> Err(FillGapsNotImplemented); others per the confirmed dispatch table
pub fn get_non_flowed_distributor(method: NonFlowedDistributionMethod) -> Box<dyn Distributor>
```
`NonFlowedWorkDistributor` is a thin wrapper: build a fresh array, delegate to the injected
`WorkSpreader` (Group C).

**Test (TDD, port verbatim):** `DistributorFactoryTest`'s full dispatch table (all 6
`DistributionMethod` variants + all 5 `NonFlowedDistributionMethod` variants, `FillGaps` → `Err`);
`NonFlowedWorkDistributorTest`'s interaction/wiring test.

### D2. `FlowPlanDistributor` + `FlowPlanRetentionDistributor` + `FlowPlanCapacityDistributor`

**Model:** Opus (the capacity carryover algorithm's "recompute-then-subtract" step is genuinely
subtle — highest-risk single phase in Group D for a near-but-wrong port)
Introduce a `RetentionCapacityProvider` trait (out-of-scope entity, see Scope section) so this
math is fully testable:
```rust
pub trait RetentionCapacityProvider {
    fn utilization_for(&self, date_time: LocalDateTime) -> Option<RetentionCapacityUtilization>;
}
pub struct RetentionCapacityUtilization { pub retention_hours: f64, pub capacity: f64, pub utilization: f64 }
```
- `FlowPlanRetentionDistributor::apply_retention`: additive smear — for each period with
  `value > 0`, look up retention window via the provider, compute
  `additional_periods = max(ceil(retention_hours*3600/(period_length*60)) - 1, 0)`, and add
  (not replace) the period's `round_percent`ed value onto every period in
  `[i, i+additional_periods]` clamped to array bounds.
- `FlowPlanCapacityDistributor::apply_capacity`: per-period `net_capacity = capacity*utilization`,
  `minutes_per_guest = work_in_minutes/kbi_value`, `capacity_per_period = round_raw_hours(net_capacity*minutes_per_guest)`;
  clamp each period to capacity, carrying overflow forward and back-filling later
  under-capacity periods up to their own headroom (see the two-branch algorithm in the
  research report — port exactly, including the "recompute-then-subtract" carryover step).
- `FlowPlanDistributor::distribute`: look up flow pattern via a `FlowPatternDataProvider` trait
  (`None` → return the all-zero array immediately); apply retention (unless
  `standard.ignore_retention`); apply the flow-pattern percentage per period
  (`work_minutes * round_percent(pattern_value/100.0)`, rounded via `round_up`-if-legacy or
  `round_raw_hours` per `PlannerModel::legacy_flow_pattern_rounding` — port both branches,
  default to the non-legacy `round_raw_hours` path unless a fixture requires the legacy one);
  finally apply capacity.

**Test (TDD, port verbatim):** `FlowPlanDistributorTest`'s 2 cases (null flow pattern →
identity passthrough; full distribute-with-retention-then-capacity wiring case);
`FlowPlanRetentionDistributorTest`'s two tests — the full 15-min/30-min retention-hours-to-
additional-periods table (0.0 through 2.0 in 0.25 steps) and the retention-window-cutoff test
(RCU becoming unavailable partway through, confirmed via the `retentionEnd=21:00` vs `22:00`
rows); `FlowPlanCapacityDistributorTest`'s parameterized case (the two `usedMinutes`/`shiftEndDateTime`
rows, confirming both the settling-ceiling behavior and the "minutes lost when shift cut short"
edge case).

### D3. `OpeningWorkDistributor` / `ClosingWorkDistributor`

**Model:** Opus
Mirror-image window-adjustment distributors: compute an effective earliest/latest boundary
(defaulting one side to the shift boundary, computing the other via `fix_start_time`/`fix_end_time`
walking by `work_minutes` rounded up to the nearest period, capped at
`max_duration_minutes_for_dynamic_work`), return an all-zero array if the adjusted window would
extend past the shift boundary the wrong way, else spread via a hard-wired `EndingWorkSpreader`
(Opening) or `BeginningWorkSpreader` (Closing) over the adjusted window.

**Test (TDD, port verbatim):** both test files' invalid-boundary case, simple fixed-window
case, and the 5-row null-default-permutation table each (earliest/latest both null, one null,
both set).

### D4. `ShareWithDistributor`

**Model:** Opus
Walks periods (wrapping around the shift range while `work_minutes > 0`), finds the first
matching share-with schedule per period (via an injected lookup — no `ShareWithSchedule`
entity modeled yet, same provider-trait treatment as D2), and **subtracts**
(`subtract_from_period_value`, unrounded per Decision #1) up to `min(remaining, period_length)`
per matching period.

**Test (TDD, port verbatim):** `ShareWithDistributorTest`'s full suite — zero minutes, no
schedules, single full-shift schedule (with wraparound doubling when `work_minutes >
period_length` fits multiple passes), two overlapping full-shift schedules (redundant, same
result), half-day schedules (single and combined-to-full-day). Skip the one `@Ignore`d
overlapping-half-schedules case (undecided upstream per the Java comment) — note it as an
explicitly unresolved edge case in the Rust test file too, not silently omitted.

---

## Group E — Non-staff standard producers (feed Step 2)

### E1. `ShiftRelatedStandardsGenerator`

**Model:** Sonnet 5
```rust
pub fn generate(params: &GeneratorParameters) -> Result<Vec<Vec<DistributionItem>>, GenerationError>
```
Two halves: `handle_non_flowed_standards` groups non-staff `NonFlowed` standards by
`NonFlowedDistributionMethod`, sums `WorkTotalMinutesCalculator`-computed minutes per group
**gated by `kbi_is_open`** (needs a `ForecastStructure`-equivalent gate — model as a trait
method/injectable check, out-of-scope entity per Scope section, default-open for testing),
then one `distribute` call per group. `handle_other_standards` computes per-standard minutes
for every non-`NonFlowed` standard **without** the `kbi_is_open` gate (confirmed asymmetry),
skips `FillGaps` standards entirely (no error, just excluded), else distributes individually.
`get_kbi_value` dispatches `WorkType::Weekly` → `kbi_value_for_week`, else `kbi_value`.

**Test (TDD, port verbatim):** the empty-standards case (live); the `@Ignore`d-but-still-worth-
porting "non-flowed standards total like work before distribution" 6-standard grouping case
(2 BEGINNING summed, 2 END summed, 2 individual FLOWED/WEEKLY) — port it as a **live** Rust
test even though Java left it disabled, since the algorithm is otherwise unverified by any
running test.

### E2. `SpreadStandardsGenerator` + `FixedSpreadStandardsProcessor` + `DynamicSpreadStandardsProcessor`

**Model:** Sonnet 5
```rust
pub fn generate(params: &GeneratorParameters) -> Result<Vec<Vec<DistributionItem>>, GenerationError>
```
Filters `job.spread_standards` to ones on the current `AssignmentShift` (id match) gated by
`kbi_is_open`, dispatches `SpreadStandardType::Fixed` → `fixed_spread::process` (looks up
`SpreadStandardValue` by kbi-value/volume-range/environment, mutates a log-detail side channel
— skip the logging mutation in Rust, no audit trail per Scope — then
`FlowPlanSpreadDistributor::spread`, i.e. `spread_values_conversion::convert_spread_values`),
`Dynamic` → `dynamic_spread::process` (looks up a `DynamicSpreadService` via provider,
`None` → `Ok(None)`; else builds a full-day array and adds `calculate_work` per dynamic value
at the shift-start-offset index, where `calculate_work` looks up the matching
`DynamicSpreadStandard` by volume range/environment — `None` → `0.0` — and branches on
`Units::UnitsPerPerson`/`MinutesPerUnit`/other→`0.0`).

**Test (TDD, port verbatim):** `SpreadStandardsGeneratorTest`'s shift-ID-gate + null-result-
filter case (7 standards, 2 survive); `FixedSpreadStandardsProcessorTest`'s 2 cases (normal +
null-value); `DynamicSpreadStandardsProcessorTest`'s 2 cases (empty dynamic values; the full
8-period `[40,161,350,428,0,0,0,0]` → `[30.0,80.5,0.0,128.4]` case, including the "value falls
in an unconfigured gap range → 0" edge case at index 6).

### E3. `RecurringStandardsGenerator` + `RecurringTaskFilter`

**Model:** Sonnet 5
```rust
pub fn filter_for_shift_and_date_and_standard_set(job: &Job, shift: &JobShift, date: LocalDate, standard_set_id: StandardSetId) -> Vec<&RecurringTaskStandard>
pub fn generate(params: &GeneratorParameters) -> Result<Vec<DistributionItem>, GenerationError>  // single flat array, not nested
```
Filter: `occurs_during_shift == shift.id && is_applicable_to_date(date) && standard_set_id matches`.
Generate: sum `Fixed` standards' `fixed_hours_with_formula` unconditionally, `Variable`
standards' `total_variable_hours_with_formula(kbi_value)` **gated by `kbi_is_open`**; if
total minutes `> 0`, distribute via `get_non_flowed_distributor(Beginning)` (hard-wired,
standard param `None`).

**Test (TDD, port verbatim):** `RecurringTaskFilterTest`'s shift-identity isolation case (4
standards across 2 standard-sets/4 shifts, filter isolates exactly 1 by shift identity —
note the Java test doesn't cover date-mismatch/standard-set-mismatch exclusion directly; add
those two cases explicitly in the Rust suite since the filter logic supports them but Java
left them unverified).

### E4. `WorkTotalMinutesCalculator` (+ shared suppress-value helper, Decision #7)

**Model:** Sonnet 5
```rust
pub fn suppressed_kbi_value(kbi_value: i32, suppress_value: i32) -> i32   // max(kbi_value - suppress_value, 0), shared by StaffGenerator (Group F) too
pub fn calculate_minutes(standard: &ShiftRelatedStandard, kbi_value: i32, params: &GeneratorParameters) -> Result<f64, GenerationError>
    // WorkType::Task -> Err(TaskStandardsNotSupported) per Scope; else the shift-related-standard branch
    // reusing basic_environment_resolution's value-lookup + basic_work_minutes_per_unit's calculate()
```
**Test (TDD, port verbatim):** `WorkTotalMinutesCalculatorTest`'s dispatch table (VARIABLE/TASK/DAILY
× present/absent detail → correct branch + 0-fallback; TASK → now `Err` per scope cut, note
this diverges from Java's `0.0` — flag explicitly since it changes the table's 3rd/4th rows'
expected outcome) and its suppress-value table (shared verbatim with `StaffGeneratorTest`,
Phase F1 — write it once, reference from both).

### E5. `NonStaffRelatedStandardsProcessor` + `NonStaffResults`

**Model:** Sonnet 5
```rust
pub struct NonStaffResults { pub non_staff_work_minutes_per_period: Vec<DistributionItem>, pub non_staff_bodies_after_breaks_applied: Vec<DistributionItem> }
pub fn process(params: &GeneratorParameters) -> Result<Vec<Vec<DistributionItem>>, GenerationError>
    // shift_related::generate (addAll) + spread::generate (addAll) + recurring::generate (single .push, not extend)
```
**Test (TDD, port verbatim):** `NonStaffRelatedStandardsProcessorTest`'s fan-out shape
assertion; `NonStaffResultsTest`'s plain getter round-trip.

---

## Group F — Staff computation (Step 5)

### F1. `StaffGenerator`

**Model:** Sonnet 5
```rust
pub fn handle_staff_work(params: &GeneratorParameters, number_of_shifts: i32) -> Result<Vec<Vec<DistributionItem>>, GenerationError>
```
Group non-staff-filtered-out (i.e. `WorkType::Staff`) standards by `NonFlowedDistributionMethod`,
sum minutes per group via the shared suppress-value formula (E4), multiply the per-shift total
by `number_of_shifts`, distribute via `get_non_flowed_distributor`.

**Test (TDD, port verbatim):** the suppress-value table (shared with E4 — write once, two
call sites in the test suite referencing the same table). Skip the `@Ignore`d stale
`testHandleStaffWork` (references an obsolete API shape, not authoritative).

### F2. `StaffStandardsProcessor`

**Model:** Sonnet 5
```rust
pub fn process(params: &GeneratorParameters, non_staff_bodies: &[DistributionItem]) -> Result<Vec<Vec<DistributionItem>>, GenerationError>
```
Builds temporary (unpersisted) planned shifts from `non_staff_bodies` via
`planned_shift_record_creator::create_do_not_attempt_to_match_existing` (Group H, built later —
note the forward dependency), groups by `PlanType`, takes the **max** group size as
`number_of_shifts` (not sum), calls `StaffGenerator::handle_staff_work` only if shifts exist.

**Test (TDD, port verbatim):** `StaffStandardsProcessorTest`'s max-of-groups case (2 FORECAST +
2 ORIGINAL → 2, not 4).

---

## Group G — Break calculation + final bodies (Steps 3, 6, 7)

### G1. `BreakCalculator` (kbirelated's own — confirmed NOT a duplicate of BASIC's `basic_break_calculator`)

**Model:** Opus
```rust
pub fn handle_breaks(params: &GeneratorParameters, existing_work_minutes_per_period: &[DistributionItem]) -> Vec<DistributionItem>
```
Sums existing work (`round`), and **only if `> 0`**: calls `basic_calculator::calculate` (reused
verbatim from the BASIC plan — this is the confirmed reuse point) to get a single
`work_hours_to_cover_breaks` total for the day, then hands `round(hours*60)` total break-minutes
to `BreakSpreader::populate_array_with_work_minutes` (Group C5) to redistribute across periods
weighted against `existing_work_minutes_per_period`. Zero existing work → zero-filled array,
no calculator/spreader call at all (confirmed short-circuit).

**Test (TDD, port verbatim):** `BreakCalculatorTest`'s wiring test (25 total work minutes → 2.63
break-hours mocked → spread — this is a plumbing test in Java, not real math, since both
`SimpleNonFlowedCalculator` and `BreakSpreader` are mocked there; the Rust version should use
the *real* `basic_calculator`/`break_spreader` since both are ported by this point, making it a
genuine integration test rather than a wiring stub) **plus** a new explicit test for the
`total_existing_work <= 0` short-circuit path, which the Java suite never covered (confirmed
gap — close it here).

### G2. `KbiRelatedStandardsProcessorForShift` — the 8-step orchestrator

**Model:** Opus (threads every prior group together in one exact sequence — the highest
blast-radius phase in the whole plan if a step is misordered or a tag/argument swapped)
```rust
pub fn process_standards_for_shift(params: &GeneratorParameters) -> Result<(Vec<WorkContent>, Vec<PlannedShift>), GenerationError>
```
Wires everything above in the exact confirmed sequence (see the call-chain diagram in the
Context section): non-staff (E5) → aggregate (B6) → breaks (G1) → bodies (B5) → min/max (B3,
tags `NS_MIN`/`NS_MAX`) → staff (F2) → aggregate → combine+breaks again → bodies → min/max
(tags `BODIES_MIN`/`BODIES_MAX`, authoritative) → work content (H5) + planned shifts (H4).
Skip all `WorkContentLog`/`WorkContentLogDetail` audit-trail creation calls (out of scope,
same as BASIC) — every `plannerModel.createNewWorkContentLog(Detail)`/`addDistributionItemArrayToCurrentWorkLog`
call in the Java source is simply omitted, not stubbed.

**Test (TDD, port verbatim):** `KbiRelatedStandardsProcessorForShiftTest`'s full wiring
assertion — since every collaborator is now a real function (not a mock), re-express this as
an end-to-end fixture test asserting the final `WorkResults` shape, using the same intermediate
values the Java test hard-codes as mock returns (reconstructible now that every stage is real:
build a fixture whose non-staff/staff standards actually produce those exact intermediate
arrays, or at minimum assert the final `work_contents`/`planned_shifts` counts match `2`/`4`
per the Java test's final assertion).

---

## Group H — Output creation (Step 8)

### H1. `DistributionItemTracker`

**Model:** Opus (richest, most edge-case-dense test suite in the whole port — two deliberately
different "minimum in range" functions, min-shift extension that can legitimately exceed the
shift window, multiple midnight-crossing/array-bounds edge cases)
Static-style module, `generators/advanced/distribution_item_tracker.rs`:
```rust
pub fn find_first_non_zero(bodies: &[DistributionItem], params: &GeneratorParameters) -> Option<WorkContentTrackerBean>   // truncates (int) cast, not rounds
pub fn adjust_for_min_shift(bean: &mut WorkContentTrackerBean, shift_end_date_time: LocalDateTime, params: &GeneratorParameters)
    // can legitimately extend the block LONGER than the nominal shift window (Decision: replicate, don't clamp — confirmed by the minShift=16.0 test rows)
pub fn find_work_content_endpoint(bean: &mut WorkContentTrackerBean, bodies: &[DistributionItem], params: &GeneratorParameters)          // unbounded by max-shift
pub fn find_planned_shift_endpoint(bean: &mut WorkContentTrackerBean, bodies: &[DistributionItem], params: &GeneratorParameters)          // capped at max-shift, stops at first zero
pub fn find_planned_long_shift_endpoint(bean: &mut WorkContentTrackerBean, bodies: &[DistributionItem], params: &GeneratorParameters)      // caps strictly at max-shift, backs off trailing zeros, clamps to array bounds
pub fn find_minimum_value_in_range(bean: &mut WorkContentTrackerBean, bodies: &[DistributionItem])                 // skips only the MAX_VALUE sentinel (work-content path)
pub fn find_planned_shift_minimum_value_in_range(bean: &mut WorkContentTrackerBean, bodies: &[DistributionItem])   // skips zero AND sentinel (planned-shift path) — a DIFFERENT function, not a shared one (confirmed divergence)
pub fn deduct_minimum_value_from_list_items(bodies: &mut [DistributionItem], bean: &WorkContentTrackerBean)
```
**Test (TDD, port verbatim):** the exhaustive `DistributionItemTrackerTest` suite — this is
the single richest test file in the whole port: `find_first_non_zero` (empty, leading-zero-skip,
all-zero→None); `find_work_content_endpoint`'s 3-row table; `find_minimum_value_in_range`'s
3-row table; the deduction test; `find_planned_shift_endpoint`'s 6-row table (including the
`minShift=maxShift=0.0` zero-length edge case and the midnight-crossing row);
`find_planned_long_shift_endpoint`'s 4-row table plus the two endpoint-outside-array-bounds
tests (including the "next-day start, 96 periods/day, starting_index=186" case); and
`adjust_for_min_shift`'s two full tables — the 4-row simple table and the ~19-row exhaustive
table including the `minShift=16.0` block-longer-than-shift-window rows (Decision-critical,
must not be "fixed").

### H2. `ExistingPlannedShiftMatcher`

**Model:** Sonnet 5 (small, self-contained, fully specified by 9 direct test cases)
```rust
pub fn find_matching_planned_shift(scheduled: &mut Vec<PlannedShift>, pending: &mut Vec<PlannedShift>, clear_schedules: bool, candidate: &PlannedShift) -> Option<PlannedShift>
    // tries `scheduled` first unconditionally (removing on match), then `pending` only if !clear_schedules
fn planned_shifts_match(a: &PlannedShift, b: &PlannedShift) -> bool
    // job_id ==, assignment_id == (value compare, Decision #4), start/end/shift_type ==
```
**Test (TDD, port verbatim):** all 9 `ExistingPlannedShiftMatcherTest` cases — no-existing,
no-match(shiftType), scheduled-match-with-removal, pending-match-with-removal, and the 6
single-field-mismatch/one-full-match unit tests on `planned_shifts_match` directly.

### H3. `PlannedShiftCreator`

**Model:** Opus (the `shift_date`-vs-`date_shift_generated_from` distinction is easy to collapse
into one field by mistake)
```rust
pub fn create_planned_shift_records(bean: &WorkContentTrackerBean, params: &GeneratorParameters, check_for_match: bool) -> Vec<PlannedShift>
```
Per `bean.min_value` iterations, branch on `PlannerMode`: `Projected` → FORECAST (match-checked
if `check_for_match`) + ORIGINAL (always added); `RePlan`(RE_PROJECT) → ORIGINAL only;
`Standard`/`Budget` → STANDARD only. Field mapping per shift: `shift_date =
bean.start_date_time.date()` (from work start — **not** `params.shift_date`!),
`date_shift_generated_from = bean.shift_start_date_time.date()` (original template date,
Decision-critical distinction, confirmed by the dedicated midnight-crossing test).

**Test (TDD, port verbatim):** `PlannedShiftCreatorTest`'s 3 cases — empty bean (`min_value=0`
→ no shifts); default-category assignment; and the midnight-crossing
`shift_date`-vs-`date_shift_generated_from` test (PROJECTED mode, template date 08-06, work
block on 08-07 → both FORECAST and ORIGINAL confirmed with the correct, *different* dates).

### H4. `PlannedShiftRecordCreator`

**Model:** Opus (the early-break-before-add divergence from H5 is exactly the kind of asymmetry
a less careful pass would "harmonize" away — must not)
```rust
pub fn create_or_match_existing(bodies: &[DistributionItem], params: &GeneratorParameters) -> Vec<PlannedShift>       // checkForMatch = true
pub fn create_do_not_attempt_to_match_existing(bodies: &[DistributionItem], params: &GeneratorParameters) -> Vec<PlannedShift>  // checkForMatch = false
```
`max_shift == 0.0` → empty (with the res_maxShiftZero progress-message path dropped, no
audit/messaging in scope). Loop: find-first-nonzero → (long-shift or normal endpoint, per
`generate_long_shifts` setting) → `adjust_for_min_shift` → planned-shift-flavored min-find →
deduct → **early-break-before-add if `ending_index >= bodies.len()-1`** (Decision #3, confirmed
divergence from H5 — preserve).

**Test (TDD, port verbatim):** all 6 `PlannedShiftRecordCreatorTest` cases — maxShift==0 empty
case; the two concrete "peel layers" worked examples (non-long-shift: 5 shifts with exact
durations/times; long-shift: 4 shifts, then the 8pm-window 6-shift case with 3 max-length
layers) — these are the strongest ground-truth regression fixtures in the whole distribution
engine, get them byte-exact; the two existing-shift-matching cases (scheduled + pending); the
do-not-match-existing case (2 results despite matchable existing shifts present).

### H5. `WorkContentRecordCreator`

**Model:** Opus (same reasoning as H4 — implement together with H4 by the same person/session
if possible, specifically to keep the confirmed divergence intentional rather than accidental)
```rust
pub fn create(bodies: &[DistributionItem], params: &GeneratorParameters) -> Vec<WorkContent>
```
Loop: find-first-nonzero → `find_work_content_endpoint` (unbounded) → `find_minimum_value_in_range`
(sentinel-only variant) → deduct → **always** `records.extend(...)` (no early-break guard,
Decision #3) → per-layer, per-`PlanType`, delegate to `basic_work_content_creator::create_work_content_record`
(reused from the BASIC plan — confirmed shared class in Java, `WorkContentCreatorUtility`).

**Test (TDD, port verbatim):** `WorkContentRecordCreatorTest`'s record-count table (all-ones →
2; two 9-element "peel" arrays → 10 each; the 48-period multi-hump array → 8) — the exact
count arithmetic is `(number of peel layers) × (number of plan types)`.

### H6. `KbiRelatedResults`

**Model:** Sonnet 5
```rust
pub struct KbiRelatedResults { pub planned_shifts: Vec<PlannedShift>, pub work_contents: Vec<WorkContent> }
impl KbiRelatedResults { pub fn clear(&mut self) }
```
(No `WorkContentLogger` field — audit trail out of scope.)

**Test:** basic add/contains/clear round-trip, mirroring `KbiRelatedResultsTest` minus the
logger-clear assertion.

---

## Group I — Top-level orchestration

### I1. `KbiRelatedStandardsProcessorForDate`

**Model:** Sonnet 5
```rust
pub fn process_standards_for_date(job: &Job, planner_model: &PlannerModel, planner_settings: &PlannerSettings, shift_date: LocalDate) -> Result<(Vec<WorkContent>, Vec<PlannedShift>), GenerationError>
```
For each `JobShift` on the job's standard set: resolve detail via
`basic_environment_resolution::assignment_shift_detail(planner_model, shift, None, date)?`;
**skip** (continue) if `None` or `!has_times()`; else build `GeneratorParameters` and call G2.

**Test (TDD, port verbatim):** `KbiRelatedStandardsProcessorForDateTest`'s 3-shift skip-rule
case (valid detail processed; null detail skipped; invalid-times detail skipped — 0 calls for
the latter two).

### I2. `KbiRelatedStandardsProcessor`

**Model:** Sonnet 5
```rust
pub fn process_shift_dates(job: &Job, planner_model: &PlannerModel, planner_settings: &PlannerSettings) -> Result<(Vec<WorkContent>, Vec<PlannedShift>), GenerationError>
pub fn process_single_shift_date(job: &Job, shift_date: LocalDate, planner_model: &PlannerModel, planner_settings: &PlannerSettings) -> Result<(Vec<WorkContent>, Vec<PlannedShift>), GenerationError>
```
Loops `planner_settings.dates(planner_model)` (BASIC's existing date-iteration helper, standing
in for `EffectiveDateFilter` per the established out-of-scope decision) calling I1 per date and
concatenating results; `process_single_shift_date` calls I1 once for the given date.

**Test (TDD, port verbatim):** `KbiRelatedStandardsProcessorTest`'s 2 cases (all-effective-dates
concatenation; single-date calls the per-date method exactly once).

### I3. `KBIRelatedGenerator` — wire into `AdvancedWorkGenerator`

**Model:** Sonnet 5
File: `generators/advanced/advanced.rs`. Replace the stub `generate_work` to call I2, map the
`(Vec<WorkContent>, Vec<PlannedShift>)` into `WorkResults::with_work_content(...)` (extended in
the BASIC plan, Phase 12), and return `Result<WorkResults, GenerationError>` per the
`WorkGenerator` trait signature BASIC's Phase 14 already changed.

**Test (TDD):** an end-to-end fixture exercising the full 8-step pipeline once, asserting
non-empty, well-formed output — the acceptance-level proof the whole chain is wired correctly
(Group J below provides the richer acceptance scenarios).

---

## Group J — Acceptance tests (from the fully-live integration scenarios)

**Model:** Opus (whole-system regression suite — best done by whichever session/model has the
most context on Groups A–I, since debugging a failure here means tracing back through the full
chain; Opus for the debugging work this group will likely surface)

Port these three **non-`@Ignore`d, non-DB-fixture-dependent** Java integration tests as Rust
integration tests over the real (in-memory) pipeline — building the fixture graph directly in
Rust rather than through Hibernate/DAOs:

- **`NonFlowedWithClosingWorkIntegrationTest`**: 8h daily NON_FLOWED/END standard (8.0h) + a
  CLOSING standard (1.0h, window 16:00–17:00), no breaks, periodLength=30, minShift=4/maxShift=8
  → expect exactly 2 planned shifts: `08:00–16:00` and `13:00–17:00` (closing-work shift
  back-extended to satisfy the 4h minimum).
- **`NonFlowedWithOpeningWorkIntegrationTest`**: mirror scenario (BEGINNING + OPENING, window
  07:00–08:00, 1.0h) → expect `07:00–15:00` and `12:00–16:00`.
- **`SecondNonFlowedWithOpeningWorkIntegrationTest`**: 24h shift window, NON_FLOWED/BEGINNING
  14.0h standard, min=4/max=8 → `limit_shift_to_max_shift=true` gives `00:00–08:00` +
  `00:00–06:00`; `limit_shift_to_max_shift=false` gives the "normal" sequential
  `00:00–08:00` + `08:00–14:00`.

Treat these as the final, whole-system regression suite — if Groups A–I are all individually
green but one of these three fails, the bug is in how they're wired together, not in any one
algorithm.

---

## Critical files

- `planner/src/workcontent/common/rounding.rs` — extend with `round_percent` (Phase A1)
- `planner/src/workcontent/generators/error.rs` — extend `GenerationError` (per Decision #8)
- `planner/src/workcontent/domain/{flow_plan,spread_standard,recurring_task_standard,assignment_min_max_coverage,distribution_method}.rs` (new), extend `shift_related_standard.rs`, `job.rs`, `planner_model.rs`, `kbi.rs` — Group A
- `planner/src/workcontent/generators/advanced/` — new home for everything else (mirrors Java's
  `workcontent/kbirelated/` package layout): `distribution_item.rs`, `generator_parameters.rs`,
  `work_content_tracker.rs`, `flow_pattern_conversion.rs`, `min_max_coverage.rs`,
  `min_max_adjuster.rs`, `spread_values_conversion.rs`, `minutes_to_bodies.rs`,
  `distribution_aggregator.rs`, `spreaders/{beginning,ending,middle,even,varying,break}.rs`,
  `distributors/{mod,flow_plan,flow_plan_retention,flow_plan_capacity,opening,closing,share_with}.rs`,
  `shift_related_standards_generator.rs`, `spread_standards_generator.rs`,
  `recurring_standards_generator.rs`, `recurring_task_filter.rs`, `work_total_minutes_calculator.rs`,
  `non_staff_related_standards_processor.rs`, `staff_generator.rs`, `staff_standards_processor.rs`,
  `break_calculator.rs`, `kbi_related_standards_processor_for_shift.rs`,
  `distribution_item_tracker.rs`, `existing_planned_shift_matcher.rs`, `planned_shift_creator.rs`,
  `planned_shift_record_creator.rs`, `work_content_record_creator.rs`, `kbi_related_results.rs`,
  `kbi_related_standards_processor_for_date.rs`, `kbi_related_standards_processor.rs`, `advanced.rs`
  (rewritten)
- Reused from the BASIC plan (no changes needed, just imported): `basic_calculator::calculate`
  (Group G1), `basic_work_content_creator::create_work_content_record` (Group H5),
  `basic_environment_resolution::assignment_shift_detail` (Group I1)
- Java ground truth — production: `JPR/workcontent/kbirelated/**/*.java`; tests:
  `JTR/workcontent/kbirelated/**/*.groovy` plus
  `JTR/workcontent/eventrelated/plannedshifts/` (none needed here) — all file-level references
  are given per-phase above.

## Verification

Same TDD discipline as the BASIC plan: write the ported test(s) for a phase, confirm red,
implement, confirm green, keep `cargo check -p planner` / `cargo test -p planner` passing after
every phase. Recommended build order follows the group letters (A→J) — each group only depends
on groups before it, except: B6's tests need Group C's spreaders to run (noted inline in B6);
F2 forward-references H4 (noted inline); G1 reuses G-external `basic_calculator` from the
BASIC plan (must be done first, or done in parallel if that plan is further along). Groups C,
D, E are each internally independent and could be parallelized across sessions once Group A/B
land, since none of them depend on each other — only on the Group A/B foundations.

**Review checkpoints and context resets**: given this plan's size (~30 phases across 10
groups), don't run it in one session. Stop for review — and clear context before resuming —
at these boundaries:
- **Checkpoint 1** (after Group B): foundations + distribution utilities in place, all pure
  functions, all green. Low risk.
- **Checkpoint 2** (after Group D): spreaders + distributors done — the densest,
  subtlety-heaviest engine internals in the plan (all Opus-flagged). Highest-value review
  point; read the diffs against the "Known decisions / deviations" list line by line before
  moving on, since this is where a wrong-but-plausible port would hide.
- **Checkpoint 3** (after Group F): non-staff + staff standard producers done — the pipeline
  can now compute minutes/period end-to-end (just not breaks/bodies/output yet).
- **Checkpoint 4** (after Group G): the 8-step orchestrator is wired — breaks, bodies,
  min/max, staff multiplier all threaded together correctly. Second highest-value review
  point, since a misordered or mis-tagged step here silently corrupts everything downstream.
- **Checkpoint 5** (after Group H): output creation (work content + planned shifts) done —
  another Opus-heavy, deviation-preserving group worth a careful look.
- **Checkpoint 6** (after Group I): the generator is wired end-to-end and functional.
- **Checkpoint 7** (after Group J): the three acceptance scenarios pass — done.
As with the BASIC plan, context resets are safe at these boundaries because each checkpoint's
only durable output is code + passing tests, and this doc is written to be sufficient
standalone context for a fresh session to resume from. Don't reset mid-group — phases within
a group typically share fixtures or have a noted forward/backward dependency on a sibling
phase.
