# Work-rules engine — parity audit

Running record of what has been ported, what was deliberately changed, and how
much of the Java test suite each piece is backed by.

## Methodology

**Ground truth is the `taps` checkout**, never `workrules/java/`. That directory
is a convenience index copied at the start of the port; `planner`'s own
`PARITY_AUDIT.md` documents a confirmed case where the equivalent local copy had
already drifted from production. Cite `taps/...` paths in this file.

`com.unifocus.tbx.core` is the exception: it is not in `taps` at all (only test
stubs of the package are). Its ground truth is
`~/workspace/unifocus/legacy-tbx-10.0/legacy-tbx-core/src/main/java/com/unifocus/tbx/core/`.
**Open question:** `taps/build.gradle` does not visibly pin a tbx version, so
this has not been confirmed as the exact version `taps` builds against.

Ported Java test cases go in a `#[cfg(test)] mod java_parity_tests` alongside
the regular `mod tests`, following `planner`. Nothing in Wave 0 has one yet —
see "Test backing" below for why.

## Status

| Wave | Piece | State |
|---|---|---|
| 0 | Support layer | **done** |
| 0 | Catalogue — `RuleType`, `RuleClass`, `Module` | **done** |
| 0 | Parameters — `RuleParams`, `RuleConfig`, priority | **done** |
| 0 | Entity core — 11 entities | **done** |
| 0 | `RuleUtils` resolution | **done** |
| 0 | Dispatch | **done** |
| 0 | DAO ports | **partial, by design** — 8 of 19 defined; see `rules/ports.rs` |
| 1 | `punchrounding` — 6 of 6 rules + runner | **done** |
| 1 | `punchvalidation` — 4 of 4 rules | **done** |
| 3 | `hoursdistribution` — `TimeCard` surface | **done** |
| 3 | `hoursdistribution` — the six shared helpers | **done** |
| 3 | `hoursdistribution` — 19 of 19 rules | **done** |
| — | `regularhoursdistribution` — 3 of 3 rules + runner | **done** |
| 2 | `regularrate` — 7 of 8 rules | **done, one deferred** |
| 2 | `doubletimerate` — 5 of 5 rules | **done** |
| 2 | `overtimerate` — 8 of 8 rules | **done** |
| 2 | `earningrate` — 10 of 10 rules | **done** |
| 4 | `schedulerestriction` — 5 of 6 rules | **done, one deferred** |
| 4 | `schedulelunch` — 3 of 3 rules | **done** |
| 4 | `shiftadjustment` — 7 of 7 rules | **done** |
| 4 | `shiftcorrection` — 3 of 3 rules + runner | **done** |
| 4 | `dailyearning` — 9 of 9 rules + runner | **done** |
| 4 | `weeklyearning` — 8 of 8 rules + runner | **done** |
| 4 | `shiftearning` — 23 of 23 rules + runner + registry (slices 1-5), `HolidayMatcher`, `ShiftTimeInPeriodCalculator` | **done** — see "Scoping — `shiftearning`" and "`shiftearning` — slice 5" |
| 4 | `schedulelabel` — 5 of 5 rules + runner | **done** |
| 4 | `schedulelabeladdendum` — 3 of 3 rules + both runners | **done** |
| 4 | `schedulechangevalidation` — 3 of 3 rules + runner | **done** |
| 4 | `shiftdifferenceot` (`ShiftDiffOt`) — 1 of 1 rule + runner | **done** |
| 1+ | the other 13 rule types | not started — itemized in "Unstarted scope inventory" below |

32 `RuleType`s in all; 19 ported, 13 not started.
(An earlier revision of this row said "16": a miscount, 32 − 11 − 4 = 17.)

## Where the work stands

**2,089 tests, 889 of them transcribed Java assertions.** Clippy clean,
`cargo fmt` clean.

Waves 0 and 1 are complete: the support layer, the catalogue, the parameter and
config machinery, resolution, dispatch, eleven entities, and both
`punchrounding` (6 rules + runner) and `punchvalidation` (4 rules).

Wave 3's `hoursdistribution` is **complete** — all 19 catalogue entries, every
Groovy case in the family transcribed. It was taken out of plan order at the
user's request; the plan puts the rate families (Wave 2) first because the
overtime rules lean on them. **Nothing blocked on that**, which is now settled
rather than provisional: no rule in the family needed a rate.

**`regularhoursdistribution` is also complete**, ported out of plan order for
the same reason as `hoursdistribution` — see its own section below. It is a
different family from `hoursdistribution` despite the name: `HoursDistribution`
(code `HD`) moves already-distributed hours between regular, overtime and
double-time buckets over a work week; `RegularHoursDistribution` (code `RH`)
puts a shift's own net hours onto its regular bucket in the first place, one
shift at a time, before `HoursDistribution`'s rules ever run.

**`regularrate`, the first rate family, is done — 7 of its 8 rules.** See
"`regularrate` — done" below.

**`doubletimerate`, the second rate family, is done — all 5 rules.** See
"`doubletimerate` — done" below.

**`overtimerate`, the third rate family, is done — all 8 rules.** See
"`overtimerate` — done" below.

**`earningrate`, the fourth and last rate family, is done — all 10 rules.**
See "`earningrate` — done" below. All four rate families are now complete.

**`schedulerestriction` is done — 5 of its 6 rules**, the first of seven
families now being ported for scheduling at the user's request (ahead of the
other, unprioritized not-started families). See "`schedulerestriction` — done,
one deferred" at the end of this file.

**`schedulelunch` is done — all 3 rules.** See "`schedulelunch` — done" at the
end of this file.

**`shiftadjustment` is done — all 7 rules.** See "`shiftadjustment` — done" at
the end of this file.

**`shiftcorrection` is done — all 3 rules and its runner.** See
"`shiftcorrection` — done" at the end of this file.

**`dailyearning` is done — all 9 rules and its runner**, in three slices. See
"`dailyearning` — slice 3" at the end of this file.

**`weeklyearning` is done — all 8 rules and its runner.** See "`weeklyearning` —
done" at the end of this file. Next up: `shiftearning`.

**`shiftearning` is done — all 23 rules, its registry and `ShiftDifferentialRunner`**,
in five slices. See "`shiftearning` — slice 5" at the end of this file.

### Unstarted scope inventory

Measured against `taps/taps/src/main/java/com/unifocus/watson/server/labor/rules/algorithm/`
and the 225-entry `RuleClass` catalogue (`rules/rule_class.rs`). "Catalogue"
is the number of `RuleClass` entries for that `RuleType`; "Java impls" is the
count of `*Impl.java` files in the Java directory, which **includes per-family
abstract bases**, so it normally exceeds the catalogue count by 1-3. Counts are
a first measurement, not a scoping pass: per the "Measure before scheduling"
lesson, read the Java before estimating any of these.

**Prioritized next, in this order, at the user's request** (`shiftcorrection` and `dailyearning` are now done):

| RuleType (code) | Catalogue | Java impls | Java directory / notes |
|---|---:|---:|---|
| `ShiftDifferential` (DF) | 23 | 24 | `shiftearning` — own `utility` subpackage; "closer to a second `hoursdistribution`" |

**Behind those, unprioritized:**

| RuleType (code) | Catalogue | Java impls | Java directory / notes |
|---|---:|---:|---|
| `BenefitAccrual` (BA) | 15 | 16 | `benefitaccrual` |
| `BenefitExpiration` (BE) | 7 | 8 | `benefitexpiration` |
| `ClosePayPeriod` (CP) | 2 | 3 | `closepayperiod` |
| `EmployeeAlert` (EA) | 7 | 8 | `employeealerts`; plus top-level `ActualsAlertRuleImpl` and `ScheduleAlertRuleImpl` |
| `EmployeeEvent` (EE) | 3 | 4 | `employeeevent` |
| `EmployeePoints` (EP) | 18 | 21 | `employeepoints` |
| `HolidayEligibility` (HE) | 12 | 13 | `holidayeligibility`; `HolidayMatcher` at the algorithm root |
| `MealPunch` (MP) | 2 | 2 | `mealpunch`; single-item type |
| `PostCalc` (PC) | 8 | 13 | `postcalc`; runner `PostCalcRunner` |
| `PostPunch` (PP) | 9 | 10 | `postpunch` |
| `ReconcileEmployee` (RE) | 2 | 3 | `reconcileemployee` |
| `TimeOffDistribution` (TD) | 3 | 4 | `timeoffdistribution`; single-item type |
| `TimeOffEarning` (TE) | 3 | 3 | `timeoffearning` |

**Naming traps.** The Java directory names `dailyearning`, `weeklyearning` and
`shiftearning` correspond to `RuleType`s called `DailyDifferential`,
`WeeklyDifferential` and `ShiftDifferential`; the runners are the
`*DifferentialRunner` classes. Do not search the catalogue for "earning".

**Shared algorithm-root helpers not yet ported** (in
`algorithm/` next to the family directories): `ShiftMatchingUtil`,
`ShiftTimeInPeriodCalculator`, `MinimumHoursWorkedRuleHelper`, `HolidayMatcher`.
`shiftearning`'s `utility` subpackage exists in Rust only as
`shift_time_window_utility.rs`, pulled forward by `schedulelunch` (see its
section); the rest of that subpackage is untouched.

### Remaining work by product module

Grouped by `RuleType::modules()` (`rules/rule_type.rs`), which says which
licensed module each rule type belongs to. This is the licensing metadata, not
a check of how the `scheduler` crate consumes rules.

**Scheduling-only types**

| Type | Rules | State |
|---|---:|---|
| `ScheduleRestriction` | 6 | 5 done, 1 deferred |
| `ScheduleLabel` | 5 | done |
| `ScheduleLabelAddendum` | 3 | done |
| `ScheduleChangeValidation` | 3 | done |
| `ScheduleLunch` | 3 | done |

Remaining: only the deferred `ScheduleRestriction` rule. All three scheduling
families and their runners are ported.

**Both modules (T&A and Scheduling)**

- `ShiftDiffOt` — 1 rule, **done** (it is `ShiftDifferenceOTRuleImpl`, the same class once thought deferred).
- `EmployeeAlert` — 7 rules, not started, plus the `ActualsAlertRuleImpl` and
  `ScheduleAlertRuleImpl` impls.
- `HoursDistribution`, `RegularRate`, `OvertimeRate`, `DoubleTimeRate` — done,
  except the deferred `AnnualSalaryOverHoursRegRateRuleImpl`.

**T&A-only types not yet ported (~130 catalogue rules)**

- Prioritized, 23 rules: `ShiftDifferential` 23
  (`ShiftCorrection` and `DailyDifferential` are done).
- Other T&A, ~47 rules: `PostCalc` 8, `PostPunch` 9, `HolidayEligibility` 12,
  `MealPunch` 2, `ReconcileEmployee` 2, `ClosePayPeriod` 2, `EmployeeEvent` 3,
  `TimeOffDistribution` 3, `TimeOffEarning` 3 (plus `EmployeeAlert` above).
- Benefits and points, ~40 rules: `BenefitAccrual` 15, `BenefitExpiration` 7,
  `EmployeePoints` 18. The metadata files these under T&A, but `Module` also
  has separate Benefits and Attendance entries, so whether they count toward
  "T&A complete" is a scoping decision, not settled here.

**Likely minimum for T&A + Scheduling** (~100 rules plus runners; benefits,
points, alerts and time-off can be deferred):

1. Finish scheduling: ~~`ScheduleLabel`~~ (done), ~~`ScheduleLabelAddendum`~~ (done),
   ~~`ScheduleChangeValidation`~~ (done), the deferred `ScheduleRestriction` rule,
   `ShiftDiffOt`.
2. The one remaining prioritized differential type (`ShiftCorrection` and
   `DailyDifferential` are done).
3. The payroll-calculation and punch-lifecycle types: `PostCalc`, `PostPunch`,
   `HolidayEligibility`, `MealPunch`, `ReconcileEmployee`, `ClosePayPeriod`.
4. The runner layer (see below), unported for nine of the fourteen finished
   families.

### Runner inventory

Java runners live in `taps/.../server/labor/calcshift/rules/`. Rust has
`rules/runner/{punch_rounding,regular_hours_distribution,shift_correction,daily_differential,weekly_differential}.rs` and the shared `clear_time_card_earnings.rs`.

| Java runner | State |
|---|---|
| `PunchRoundingRunner` (+ `PunchRoundingUtil`) | **ported** |
| `RegularHoursDistributionRunner` | **ported** |
| `HoursDistributionRunner` | not ported — rules are, the runner is not |
| `RegularRateRunner`, `OTRateRunner`, `DTRateRunner`, `EarningRateRunner` (+ `EarningRateRunnerData`, `EarningRateRunnerPair`), `PremiumRateRunner`, `RateRunner` | not ported — rate rules are |
| `ShiftAdjustmentRunner` | not ported — rules are |
| `ScheduleLunchRunner`, `AuditableScheduleLunchRunner` | not ported — rules are |
| `ShiftCorrectionRunner` | **ported** (`rules/runner/shift_correction.rs`) |
| `DailyDifferentialRunner` (+ `DifferentialRunner`'s clearing, via `ClearTimeCardEarnings`) | **ported** (`rules/runner/daily_differential.rs`) |
| `WeeklyDifferentialRuleRunner` | **ported** (`rules/runner/weekly_differential.rs`) |
| `ShiftDifferentialRunner` | **ported** (`rules/runner/shift_differential.rs`) |
| `ShiftDifferenceOTRunner` | **ported** (`rules/runner/shift_difference_ot.rs`) |
| `PostCalcRunner` | not ported — rules not started |
| `ScheduleChangeValidationRunner` | **ported** (`rules/runner/schedule_change_validation.rs`) |
| `ScheduleLabelRunner` | **ported** (`rules/runner/schedule_label.rs`) |
| `ScheduleLabelAddendumRunner`, `ScheduleLabelAddendumExtraDataRunner` | **ported** (`rules/runner/schedule_label_addendum.rs`) |
| `EarningSorter`, `RuleItemPriorityComparator`, `AbstractRunner` | support classes; only the priority comparator is ported (`rules/priority.rs`, post-punch only) |

So for nine of the fourteen ported families the rules exist and are tested
individually but nothing yet drives them in Java's order; the runner layer
(and the end-to-end calculation order across families) is a separate piece of
work that no wave currently covers.

### Deferred and open items, in one place

These are each recorded where they arose; this is the roll-up.

- `AnnualSalaryOverHoursRegRateRuleImpl` — deferred (divergence 57; needs a
  service that is not a DAO).
- `schedulerestriction`'s sixth rule — deferred (see its section).
- ~~`ShiftDifferenceOTRuleImpl`~~ — ported as `ShiftDiffOt` (see "`ShiftDiffOt` — done"). It had a catalogue entry after all.
- `MinDailyHrsRuleConfig` overlapping-tier validation — not ported (divergence 79).
- `JSONUtils` multi-select id-list helpers — deferred until a family needs them.
- DAO ports — `rules/ports.rs` currently holds 11 traits (10 DAO ports plus
  `MinWagePort`, which is not a DAO — divergence 38). The Status table's "8 of
  19" predates the rate and scheduling families; the 19 is the Java DAO count
  and has not been re-derived. Re-count before the next family.
- `tbx.core` ground truth is `legacy-tbx-10.0`; the version `taps` builds
  against is unconfirmed (see Methodology).

### `hoursdistribution` — done

The `TimeCard` surface, all six shared helpers, all four DAO ports the family
needs (plus `MinWagePort`, divergence 38), and **all 19 rules**.

| Ported | Java spec cases |
|---|---:|
| `RegHrsOnly` | no spec (a no-op) |
| `HolidayDTHrs` | 4/4 |
| `WeeklyOTSecJobHrs` | 2/2 |
| `WeeklyOTHrs` | 7/7 |
| `PayPeriodOTHrs` | 8/8 |
| `ScheduledShiftOT` | 10/10 |
| `CaliforniaExtendedOTHrs` | 12/12 |
| `RollingXWeeksOTHrs` | 3/3 |
| `DailyWeekly6thDayOT7thDayDTNonConsec` | 3/3 |
| `PerMonthOTHrs` | 5/5 |
| `CaliforniaOTHrs` | 6/6 |
| `TwentyFourHourOT` | 31/31, plus its config's 7 |
| `ContractOTHrs` | 6/6 |
| `DailyWeekly7thDTHrs` | 7/7 |
| `CaliforniaExtSpecialJobOTHrs` | 9/9 |
| `MinHrsForFullTimeOT` | 18/19 — see below |
| `DlyWklyOffConsecOTMinBreak` | 16/16, three rows are the sibling's — see below |
| `DailyWeekly6thOT7thDTHrs` | 13/13 |
| `DlyWklyConsecOTMinBreakSpanningMidnight` | 13/13 |

**Eighteen of the nineteen passed their whole Groovy spec on the first run.**
The exception is `DlyWklyOffConsecOTMinBreak`, whose last case was copied from
its sibling's spec and asserts that rule's numbers — now resolved, see below.
Every other discrepancy found while porting has been a transcription slip on
this side, caught by the test and corrected against the Java.

`CaliforniaExtendedOTHrs` and `TwentyFourHourOT` are the two that matter most
for confidence. The first exercises both shared accumulators and
`PriorDaysCalculator` end to end across twelve cases; the second is the family's
largest spec — 27 methods, 31 executions — and pins 24-hour-window arithmetic no
amount of reading the source settles.

**Nothing remains in this family.** What the porting order taught, for whoever
starts the next one:

**Measure before scheduling.** The one-line descriptions this file used to carry
were wrong or badly short five times out of eight: `CaliforniaOTHrs` turned out
to write earnings and brought `EarningTypePaySet` with it; `TwentyFourHourOT`
brought `ShiftUtil.getNetHoursInRange`; `ContractOTHrs` was described as a
`ContractHrsRule`, which it is not, and brought the contract calculators;
`CaliforniaExtSpecialJobOTHrs` shadows three shared types, not one; and
`DlyWklyOffConsecOTMinBreak` could not be settled at all until its sibling was
ported. Read the Java before estimating.

**Port siblings together.** The last two rules share a spec case, and the only
way to tell which rule it belonged to was to have both. The same is true of the
three rules that declare `dailyDataProducer` identically, and of the four that
shadow the shared accumulators.

`ShiftDifferenceOTRuleImpl` exists as a twentieth `*RuleImpl`. *(Later: it is
the single rule of `RuleType::ShiftDiffOt` and is ported under that type.)*

### Deliberate divergences — index

One hundred and seven so far, listed at the end of the section that introduced them. The
numbering is continuous and never reused, so a citation like "divergence 22"
resolves anywhere in the file.

| # | Section |
|---|---|
| 1–12 | Wave 0 — support layer, then catalogue/parameters/entities |
| 13 | The punch/shift ownership shape |
| 14–17 | Wave 0 — resolution, dispatch, ports |
| 18 | Wave 1 — punchrounding |
| 19–20 | Wave 1 — punchvalidation |
| 21–27 | The `TimeCard` surface |
| 28–32 | The shared helpers |
| 33–50 | Rules |
| 51–53 | `regularhoursdistribution` |
| 54–57 | `regularrate` |
| 58–59 | `doubletimerate` |
| 60–62 | `overtimerate` |
| 63–68 | `earningrate` |
| 69–71 | `schedulerestriction` |
| 72–75 | `schedulelunch` |
| 76–79 | `shiftadjustment` |
| 80–85 | `shiftcorrection` |
| 86–94 | `dailyearning` |
| 95–99 | `weeklyearning` |
| 100–115 | `shiftearning` |
| 116–118 | `schedulelabel` |
| 119–121 | `schedulelabeladdendum` |
| 122–124 | `schedulechangevalidation` |
| 125–126 | `shiftdifferenceot` |

The load-bearing ones for anyone continuing Wave 3: **22** (index forms are the
primitive, because rules write through what they filtered), **23** (one struct
for both time card implementations) together with **41** (which strains it),
**24** (pay-group-derived dates are card fields), **32** (a missing job status
excludes rather than throws), and **38** (`MinWagePort` is not a DAO port, and
carries its own revisit condition).

### The working method, for a clean restart

Each rule has gone: read the Java, read its Groovy spec, port the rule with
doc comments citing the Java, write behaviour tests from the source, then add a
`mod java_parity_tests` transcribing **every** case in the spec. Findings and
deliberate divergences land in this file as they are discovered. The Groovy is
weaker than it looks in several places — see the spec-quality findings under
"Rules" — so the transcriptions assert more than the originals do wherever the
original's predicate was accidentally vacuous.

## Scoping — the rate families

Measured before starting, per the lesson `hoursdistribution` left behind:
"measure before scheduling," because guessed one-liners were wrong five times
out of eight there. Every description below was read off the Java, not
inferred from the class name.

**Four catalogue entries, not thirty-four.** `RuleType.REGULAR_RATE` ("RR"),
`OVERTIME_RATE` ("OR"), `DOUBLE_TIME_RATE` ("DR") and `EARNING_RATE` ("ER")
are each **one** catalogue entry with several selectable concrete
implementations behind it — the same shape as Wave 1's `punchrounding` (one
`RuleType`, six rule classes) and `punchvalidation`, not the
`hoursdistribution` shape (one `RuleType` per rule). `RuleUtils`/`RuleType.java`
confirms it: there is no `REG_RATE_HOME_JOB`-style per-rule entry anywhere in
the enum. So the four marker interfaces/abstract classes
(`RegularRateRuleImpl`, `OvertimeRateRuleImpl`, `DoubleTimeRateRuleImpl`,
`EarningRateRuleImpl`) are real supertraits to port, not extra catalogue rows —
unlike `ShiftDifferenceOTRuleImpl` in the last wave, which really was deferred
because it had no catalogue entry at all.

**Thirty-four concrete rule classes across four families, 54 spec cases in 18
spec files:**

| Family | Concrete rules | Spec files | Spec cases |
|---|---:|---:|---:|
| `regularrate` | 8 | 9 (`CombinationJobsRegRateRuleImpl` has 3) | 39 |
| `overtimerate` | 9 (incl. marker) | 2 | 3 |
| `doubletimerate` | 6 (incl. marker) | 1 | 3 |
| `earningrate` | 12 (incl. marker, excl. `EligibleHours`) | 5 | 10 |

`overtimerate` and `doubletimerate` are the thinnest-tested families in the
crate so far — most of their rules (`CommissionBased*`, `HomeDept*`,
`HomeJob*`, `Job*Rate`, `WeightedOTRateRuleImpl`) have **no Groovy spec at
all**; only the FLSA and guaranteed-wage rules do. Their behaviour tests will
have to be written from the Java the way Wave 0's were, not transcribed.

### The shared interface

`regularrate.RateRuleImpl` is the root every family but `earningrate` extends:

```java
public interface RateRuleImpl extends RuleImpl {
   void execute(EmployeeShift shift, HoursDistribution distribution, TimeCard dataset, RuleItem ruleItem);
   void execute(EmployeeEarning earning, TimeCard dataset, RuleItem ruleItem);
}
```

Every `RegularRateRuleImpl`/`OvertimeRateRuleImpl`/`DoubleTimeRateRuleImpl`
implements **both** overloads — one rates a shift's distribution, the other
rates a standalone earning — and almost every concrete rule's earning overload
is a near-duplicate of its distribution overload with `earning.setRate(...)`
in place of `setRegularRates`/`setOvertimeRates`/`setDoubleTimeRates`. That
duplication is in the Java itself, not a porting artifact; expect the Rust
trait to keep both methods rather than collapsing them.

`earningrate.EarningRateRuleImpl` is a **separate root**, extending the bare
`RuleImpl` rather than `RateRuleImpl`: `void execute(TimeCard, EmployeeEarning,
Map<String,String>)` — one method, no `RuleItem`, raw params instead. It never
touches a shift or distribution at all. Read literally this family prices
*earnings*, not worked time, and several of its rules (`EarningFixedRateRuleImpl`,
`CalculatedAccrualRateRuleImpl`, `PriorBalancesRateRuleImpl`) have nothing to
do with wage rates in the hourly sense — they price benefit/accrual payouts.

`BaseRegularRateRuleImpl.setRegularRates` writes `distribution.baseRate` +
`rateRuleItemID`; `BaseOvertimeRateRuleImpl`/`BaseDoubleTimeRateRuleImpl.set*Rates`
write `distribution.premiumRate` + `rateRuleItemID` and log (not throw) when
the computed rate is `0`. All three fields already exist on the Rust
`HoursDistribution` (`base_rate`, `premium_rate`, `rate_rule_item_id`,
`entity/hours_distribution.rs:36-137`) — this part needs no new entity work.

### Corrected one-line descriptions

**`regularrate`** (8 concrete rules, all implement both `RateRuleImpl` overloads):

| Rule | What it actually does |
|---|---|
| `JobRegRateRuleImpl` | The job's own `EmployeeJobStatus.hourlyRate` on the effective date — the simple case every other rule complicates. Silently no-ops (writes nothing) if the job status is null on the distribution path; **throws** on the earning path (`jobStatus.getHourlyRate()` on a null). |
| `HomeJobRegRateRuleImpl` | The employee's *home* job status's rate, `0.0` if they have none that date — no job/department comparison at all. |
| `HomeDeptRegRateRuleImpl` | Home job's rate **if** the shift's job shares the home job's parent department; otherwise the job's own effective rate, optionally raised to the employee's own job-status rate if `payGreater` is set. |
| `FactorJobRegRateRuleImpl` | The job's own rate **times a configured factor** — a scaling rule, not a lookup rule. |
| `ShiftCategoryRegRateRuleImpl` | Rewrites the rate only for shifts (or earnings, unconditionally) matching a configured set of shift-category ids: fixed rate, or job rate ± addition/multiplier, then optionally floored at minimum wage. Distributions outside the category either keep the plain job-status rate or are skipped entirely, gated by `isOpenForEditingOn`/`isOpenForEditingFor`. |
| `ShiftCategoryMinWageRegRateRuleImpl` — **different rule from the above despite the name overlap** | Always starts from the job-status rate; only *floors* it at minimum wage, and only for shifts in the configured category list. No fixed-rate/multiplier path, no open-for-editing gate. |
| `AnnualSalaryOverHoursRegRateRuleImpl` | Backs out an *effective hourly rate* from a salaried employee's period salary: `(periodSalary − otherConfiguredEarningDollars) / totalWorkedHoursInPayPeriod`, recomputed fresh on every call from `PayRateCalculator.getPeriodSalary` — not a stored per-shift value. Needs a `PayRateCalculator` port/service and `Employee.getPayGroup().currentPayPeriod()`, neither ported yet. |
| `CombinationJobsRegRateRuleImpl` | The family's largest and only one with real complexity (369+334+173 spec lines across three specs). For each date, computes a "daily" candidate rate (only same-date shifts/earnings, grouped by job status, keeping groups meeting an hours threshold) and a "weekly" candidate (whole scheduling week, grouped, keeping groups meeting a **days**-worked threshold, using the employee's **last** effective job status over the week rather than the point-in-time one), each candidate limited to rates *higher* than the shift's own and optionally required to be a separate job id, then pays `max(daily, weekly)` — falling back to the shift's own status if nothing qualifies. |

**`overtimerate`** (`OvertimeRateRuleImpl` is the abstract marker; 8 concrete rules):

| Rule | What it actually does |
|---|---|
| `FLSAOTRateRuleImpl` | `flsaData.getEffectiveRegularRate(applyMinWagePerShift) * otFactor`, plus a minimum-wage shortfall make-up added on top if `shift.getRegRate() < minWage`. The **only** overtime rule that reads `shift.getRegRate()` — i.e. the only one with a real dependency on `RegularRate` having run first. Picks its FLSA week/pay-period key via `Property.getCurrentWeek()`/`getPayPeriod()`, gated on `PayPeriodType`. |
| `JobOTRateRuleImpl` | Job-status rate (floored at min wage) × factor. **Throws `RuntimeException`** if the job status resolves to null — the only overtime rule that throws rather than silently producing `0`/skipping. |
| `HomeJobOTRateRuleImpl` | Home job-status rate (`0.0` if none), floored at min wage, × factor. |
| `HomeDeptOTRateRuleImpl` | Home job's rate if same parent department as the shift's job, else the job's own effective rate; floored at min wage; × factor. Same department comparison as `HomeDeptRegRateRuleImpl` but without the `payGreater` branch. |
| `CommissionBasedOTRateRuleImpl` | Same-date, same-job earnings-of-configured-types ÷ same-date same-job worked hours, floored at a configured minimum commission rate *and* at minimum wage, × factor. Structurally identical arithmetic to its `doubletimerate` sibling below. |
| `GuaranteedWageOTRateRuleImpl` | Prices overtime off a **weekly guaranteed wage**: `homeJobRate × configuredWeeklyHours`, divided by the week's actual worked hours (shifts + regular/premium earnings) to get an effective hourly rate, then `× factor`, then rounded with `roundHours` (not `roundCurrency`, unlike every sibling). |
| `WeightedOTRateRuleImpl` | The FLSA weighted-average-rate calculation, but backs out only the **incremental** rate to add on top of the distribution's own `baseRate`: computes total OT dollars due at the true weighted rate × factor, subtracts what straight-time-at-baseRate would already cover, and divides the remainder back over the hours — so what it writes as `premiumRate` is a top-up, not the whole rate. The earning overload does the same subtraction against `earning.getRate() * earning.getHours()`. |
| `FLSAWeightedOTRateRuleImpl` | Confusingly named next to the above — this one is the *plain* FLSA-rate × factor rule (same shape as `FLSAOTRateRuleImpl`'s core formula), keyed by `Weeks(periodEndDate)` instead of `Property.getCurrentWeek()`. No min-wage make-up term. |

**`doubletimerate`** (`DoubleTimeRateRuleImpl` marker; 5 concrete rules — a proper
subset of `overtimerate`'s shapes, no `Job`/`Guaranteed`/`Weighted` analogue):

| Rule | What it actually does |
|---|---|
| `FLSADTRateRuleImpl` | FLSA rate × dtFactor, keyed by `Weeks(periodEndDate)` — the `doubletimerate` sibling of `FLSAWeightedOTRateRuleImpl`, not of `FLSAOTRateRuleImpl` (no min-wage shortfall make-up, no `Property.getCurrentWeek()`/pay-period branch). |
| `HomeDeptDTRateRuleImpl` | Same department-match-then-floor-at-min-wage shape as `HomeDeptOTRateRuleImpl`, × dtFactor. |
| `HomeJobDTRateRuleImpl` | Same shape as `HomeJobOTRateRuleImpl`, × dtFactor. |
| `JobDTRateRuleImpl` | Job-status rate floored at min wage × dtFactor — **does not throw** on a null job status (unlike `JobOTRateRuleImpl`); a null status NPEs instead on `jobStatus.getHourlyRate()`, an unguarded call the OT sibling explicitly guards against. |
| `CommissionBasedDTRateRuleImpl` | Identical arithmetic to `CommissionBasedOTRateRuleImpl`, own config keys (`DOUBLETIME_FACTOR_PROP` instead of `OVERTIME_FACTOR_PROP`). |

**`earningrate`** (`EarningRateRuleImpl` interface, `AvgWageEarningRateRule`
sub-interface with one implementor; `EligibleHours` is a plain DTO, not a
rule; 11 concrete rules):

| Rule | What it actually does |
|---|---|
| `EarningFixedRateRuleImpl` | Sets the earning's rate to one configured constant. The simplest rule in the whole wave. |
| `EarningFactorRateRuleImpl` | Job-status rate (home job's if configured, else the earning's own job) by UOM (hourly or piece), × factor **only if** the earning's type is in a configured allow-list, then floored at min wage if the earning type says so. |
| `EarningOverrideJobRateRuleImpl` | If the employee's plain job rate already meets minimum wage, **delegates entirely** to a fresh `EarningFactorRateRuleImpl` instance constructed with that rule's *default* config values (not this rule's own params) — an unusual cross-rule call, not a shared helper. Otherwise applies a configured override rate (itself possibly replaced by minimum wage) × factor. |
| `HomeJobRateRuleImpl` | Home job-status rate by UOM (hourly/piece), `0.0` if no home job that date, × factor, floored at min wage if configured. |
| `HomeDeptRateRuleImpl` | Same department-match-then-fallback shape as `HomeDeptRegRateRuleImpl`/`HomeDeptOTRateRuleImpl`, but UOM-aware (piece rate only reads the home job status, never falls back to the job's own). |
| `ContractDailyRateRuleImpl` | Backs out a **daily** rate from a contract: home job's hourly-or-piece rate × factor (floored at min wage), times `contractHours / contractDays`, or `0.0` outright if `contractDays == 0` or there's no home job status. |
| `FLSAEarningRateRuleImpl` | The `earningrate` counterpart of `FLSAOTRateRuleImpl`'s core formula (FLSA effective rate × factor, `Property.getCurrentWeek()`/`getPayPeriod()` gated on `PayPeriodType`) — implements `AvgWageEarningRateRule`, the family's only user of that sub-interface, despite the name suggesting it belongs with `FLSAWeightedOTRateRuleImpl` instead. |
| `AvgDayXWeeksRateRuleImpl` | Prices an average-day benefit payout: home job rate × factor (floored at min wage if the earning type says so), × `(net hours / distinct worked days)` averaged over a DAO-supplied window starting N weeks before the current scheduling week — `0.0` if the employee was hired after that window's end or the DAO finds no start date. Needs a new `HolidayDataDAO`-style port plus a raw-SQL "eligible hours" aggregate the Java runs directly against Hibernate (`EligibleHours`, a plain three-field DTO holding `dateCount`/`netHours`/`regHours`). |
| `CalculatedAccrualRateRuleImpl` | The family's largest and most stateful rule (230-line spec) — blends an accrual payout's rate across every unapplied hours/cost transaction bucket for the employee, oldest first, falling back to the current job rate for buckets older than a year or for hours left over once buckets are exhausted; caches per-employee bucket state across calls within one run (`@Scope("prototype")`, a `Map` field keyed by employee id) rather than recomputing per earning. Needs `AccrualTransactionDAO` and `EmployeeEarningDAO.getBankedRateForRule`, plus an `AccrualTransaction` entity — none ported. |
| `PriorBalancesRateRuleImpl` | Divides a prior accrual period's ending wage balance by its ending hours balance, memoized per employee for the run. Needs the same `AccrualTransactionDAO` (a different method: `getLatestTransactionsPriorToPayPeriod`) and `AccrualTransaction` entity as the rule above. |
| `AvgWageEarningRateRule` | A one-implementor marker sub-interface (`FLSAEarningRateRuleImpl`); no separate behaviour of its own — port as a marker trait or fold it away, the way `ContractHrsRule` was ported as a supertrait rather than a distinct type. |

### Dependency order between the families

**Refuted as stated: `RegularRate` does not gate `OvertimeRate`/`DoubleTimeRate`
as a family.** Of the 14 concrete OT/DT rules, only `FLSAOTRateRuleImpl` reads
`shift.getRegRate()` — the value a `RegularRate` rule would have written earlier
in the same shift's pipeline. Every other OT/DT rule prices independently, off
`EmployeeJobStatus.hourlyRate`, minimum wage, FLSA data, or configured
constants — none of them read the distribution's `baseRate` or the shift's
`regRate` at all. (`WeightedOTRateRuleImpl` reads `distribution.getBaseRate()`,
but that is the **distribution's own** base rate set moments earlier in the
same `execute` call chain, by whichever `RegularRate` rule ran on this
property — not a separate-phase dependency the way `hoursdistribution` depended
on prior weeks.)

So the real ordering constraint is narrower than the Status table's old phrase
suggested: **within one calculation run**, `RegularRate` rules run before
`OvertimeRate`/`DoubleTimeRate` rules for the *same* distribution (matching how
the engine dispatches rate rule types in sequence per `RuleType.java`'s
declaration order: `REGULAR_RATE`, `OVERTIME_RATE`, `DOUBLE_TIME_RATE`), but
nothing stops porting `overtimerate`/`doubletimerate` before `regularrate` is
finished — only `FLSAOTRateRuleImpl` and `WeightedOTRateRuleImpl` need
`EmployeeShift.reg_rate`/`HoursDistribution.base_rate` to be meaningfully
populated to test end-to-end, and both can be tested with a hand-set `reg_rate`
in isolation.

**`EarningRate` is genuinely independent.** Its interface doesn't take a
`RuleItem`, a shift, or a distribution — only `TimeCard` + `EmployeeEarning` +
raw params — and none of its 11 rules read `regRate`, `baseRate`, or anything
another rate family wrote. It can be ported in any order relative to the other
three. It is, however, the family needing the most **new** infrastructure
(`AccrualTransaction`, two DAO ports, a raw-SQL aggregate, `PayRateCalculator`),
so it is not obviously the cheapest to go first either.

**Recommended order, cheapest infrastructure first:** `regularrate` (all
Rust-side surface already exists except `EmployeeShift.reg_rate` and
`Property`'s week/pay-period accessors) → `doubletimerate` (smallest family,
5 concrete rules, reuses everything `regularrate` and `overtimerate` need) →
`overtimerate` (adds only `FLSAData`/`Weeks`-style FLSA week keying, already
partly present via `TimeCard::flsa_data_map`) → `earningrate` last (needs the
new accrual DAO ports and `PayRateCalculator`, and is fully decoupled from the
other three so nothing blocks starting it in parallel if desired).

### What's missing on the Rust side before any rule can be ported

Already in place and needing no new work: `HoursDistribution.base_rate` /
`premium_rate` / `rate_rule_item_id` (`entity/hours_distribution.rs:36-137`);
`TimeCard::flsa_data_map`/`flsa_data_map_for`/`current_pay_period`
(`entity/time_card.rs:170,737,811`); `MinWagePort` (`rules/ports.rs`,
divergence 38); `FlsaData::effective_regular_rate` (`entity/flsa_data.rs:144`);
`EarningType::earn_type`/`uom`/`pay_at_least_min_wage`
(`entity/earning_type.rs:81-91`); `Employee::employee_job_status`/
`home_employee_job_status`/`last_home_employee_job_status_for_period`
(`entity/employee.rs:138-183`).

Missing, roughly in the order the recommended porting order needs them:

- **`EmployeeShift.reg_rate`** — a plain `f64` field with getter/setter in
  Java, read by `FLSAOTRateRuleImpl` and (via `distribution.getBaseRate()`,
  its already-ported analogue) `WeightedOTRateRuleImpl`. Not yet on
  `entity/employee_shift.rs`.
- **`Property.getCurrentWeek()` / `getPayPeriod()`** returning a `DateRange`
  containing a given date, and **`Property.getPayPeriodType()`**. Read by
  `CombinationJobsRegRateRuleImpl`, `FLSAOTRateRuleImpl`,
  `GuaranteedWageOTRateRuleImpl`, `FLSAEarningRateRuleImpl`. `Property`
  currently exposes only `period_end_date()`/`week_end_day()`
  (`entity/property.rs:56-72`) — no week/pay-period range machinery yet, and
  no `PayPeriodType` enum reachable from it.
- **`Employee.getLastEffectiveJobStatusForPeriod`** (distinct from the already-ported
  `last_home_employee_job_status_for_period` — this one is job-specific, not
  home-job-specific) and **`Employee.getEffectiveJobStatus`** (point-in-time,
  distinct from `employee_job_status` if that method's semantics differ —
  needs a read of `Employee.java` to confirm before porting, not assumed).
  Both used only by `CombinationJobsRegRateRuleImpl`.
- **`Employee.getPayGroup().currentPayPeriod()`** — a `PayGroup` entity/port,
  used only by `AnnualSalaryOverHoursRegRateRuleImpl`. `Employee` currently
  stores `pay_group_id` but no `PayGroup` entity exists.
- **`PayRateCalculator.getPeriodSalary(Employee, LocalDate)`** — a calculator
  service, not a DAO; used only by `AnnualSalaryOverHoursRegRateRuleImpl`. Not
  ported, not scoped in detail yet.
- **`Assignment.getParentAssignment()`** (only `parent_assignment_id()` exists
  today — needs either a resolving port or the caller doing the lookup) and
  **`Assignment.getEffectiveHourlyPayRate(LocalDate)`**. Read by
  `HomeDeptRegRateRuleImpl`, `HomeDeptOTRateRuleImpl`, `HomeDeptDTRateRuleImpl`,
  `HomeDeptRateRuleImpl`.
- **`ShiftCategory`** — no entity yet. Needed for `ShiftCategoryRegRateRuleImpl`
  and `ShiftCategoryMinWageRegRateRuleImpl` (`shift.getShiftCategory()`), and
  for `EmployeeShift` to expose it.
- **`EmployeeShift.getEmployeeJobStatus()`** — a shift-cached job status
  accessor `ShiftCategoryRegRateRuleImpl` and `EarningOverrideJobRateRuleImpl`'s
  earning analogue (`EmployeeEarning.getEmployeeJobStatus()`) both read
  directly, apparently pre-resolved on the entity rather than looked up fresh
  each time the way every other rule in these families does it. Worth
  confirming against `EmployeeShift.java`/`EmployeeEarning.java` before
  assuming it's just a cache of `employee.getEmployeeJobStatus(job, date)`.
- **`EmployeeShift.getWorkedHours()`** — distinct from the already-ported
  `net_hours`; `CombinationJobsRegRateRuleImpl` groups on it specifically.
  Needs confirming whether it differs from net hours (e.g. excludes breaks
  differently) before assuming it's an alias.
- **New DAO ports** for `earningrate` only: something covering
  `HolidayDataDAO.getWeekStartOfNthPastWorkedWeek` plus the raw
  `EligibleHours` SQL aggregate (`AvgDayXWeeksRateRuleImpl`);
  `AccrualTransactionDAO.getTransactionsForEmployeeWithUnappliedHours` and
  `.getLatestTransactionsPriorToPayPeriod`; `EmployeeEarningDAO.getBankedRateForRule`.
  All three need an `AccrualTransaction` entity that doesn't exist yet.

### Findings, so far (from reading, not yet from porting)

- **Two same-named-sounding rules in `regularrate` do different things.**
  `ShiftCategoryRegRateRuleImpl` and `ShiftCategoryMinWageRegRateRuleImpl` both
  filter on a configured shift-category list, but the first *replaces* the
  rate (fixed value, or job rate with an addition/multiplier) while the second
  only ever *floors* the plain job-status rate at minimum wage. Reading one
  does not tell you what the other does, despite the near-identical name — the
  same trap `hoursdistribution` hit with `HolidayDTHrs`/holiday-calendar
  lookups going by two different keys.
- **`JobOTRateRuleImpl` throws on a null job status; its `doubletimerate` and
  `regularrate` counterparts don't guard it at all** (`JobDTRateRuleImpl`,
  `JobRegRateRuleImpl`'s earning overload) — one NPEs, the other throws a
  custom `RuntimeException` with a message naming the shift or earning id.
  Three different failure behaviours for the same missing-job-status
  condition, family-wide.
- **`FLSAWeightedOTRateRuleImpl` and `FLSADTRateRuleImpl` key their FLSA week
  off `new Weeks(property.getPeriodEndDate())`, while `FLSAOTRateRuleImpl` and
  `FLSAEarningRateRuleImpl` key off `Property.getCurrentWeek()`/`getPayPeriod()`
  gated on `PayPeriodType`.** Two different week-resolution mechanisms
  live side by side across what look like sibling rules, and the names don't
  signal which one a given rule uses.
- **`WeightedOTRateRuleImpl` computes a *top-up* rate, not the OT rate
  itself** — it subtracts what straight time at the distribution's own
  `baseRate` would already have paid before dividing the remainder back into a
  `premiumRate`. Reading `setOvertimeRates(shift, distribution, rate, ...)` in
  isolation elsewhere in the family suggests `rate` is the whole overtime rate;
  here it's an increment. Needs its own doc comment flagging this before
  porting, so a future rule reader doesn't assume the shared shape.
- **`EarningOverrideJobRateRuleImpl` delegates to a *fresh, differently
  configured* instance of another concrete rule** rather than calling a shared
  helper — `new EarningFactorRateRuleConfig().getDefaultValues()`, not this
  rule's own `params`. If `EarningFactorRateRuleImpl` is ported as a
  free-standing struct, this call site needs to construct one with default
  config explicitly, not share the params map naively.
- **`CalculatedAccrualRateRuleImpl` and `PriorBalancesRateRuleImpl` both cache
  state in a field across calls within one calculation run**, keyed by
  employee id (`@Scope("prototype")`/`@Scope("request")` Spring semantics —
  effectively "lives for one dataset's worth of rule executions"). Neither
  hoursdistribution nor the rate families so far have needed a rule struct to
  carry state across `execute` calls; this is new territory for how the Rust
  rule trait's `&mut self`/ownership needs to work for these two, and is worth
  settling before porting either.

## `regularrate` — done, one deferred

**67 tests, 39 of them transcribed Java assertions.** All eight catalogue entries
have an algorithm except `AsohwRrr` (`AnnualSalaryOverHoursRegRateRuleImpl`),
deferred with the family for the reason its own module doc gives.

| Ported | Java spec cases |
|---|---:|
| `JobRegRateRuleImpl` | 1/1 |
| `HomeJobRegRateRuleImpl` | 1/1 |
| `FactorJobRegRateRuleImpl` | 1/1 |
| `HomeDeptRegRateRuleImpl` | 2/2 |
| `ShiftCategoryMinWageRegRateRuleImpl` | 2/2 |
| `ShiftCategoryRegRateRuleImpl` | 3/3 |
| `CombinationJobsRegRateRuleImpl` | 14/14, 11/11, 4/4 (three spec files) |
| `AnnualSalaryOverHoursRegRateRuleImpl` | not ported — see below |

**Measuring the scoping section against what porting actually needed found it
half wrong**, the same lesson `hoursdistribution` left behind:

- **"Property week/pay-period accessors + `PayPeriodType`" needed no new
  surface at all.** `TimeCard::current_pay_period`/`pay_period_type` already
  stand in for `Property.getPayPeriod()`/`getPayPeriodType()` (divergence 24),
  which is the value `AnnualSalaryOverHoursRegRateRuleImpl`'s
  `employee.getPayGroup().currentPayPeriod()` reaches. And
  `Property.getCurrentWeek().getDateRangeContainingDate(date)` —
  `CombinationJobsRegRateRuleImpl`'s only property dependency — turned out to
  already be exactly how `RegularHoursByWorkWeekRule` reaches the property
  (divergence 51): `WeeklyDateRange::with_end_date(property.period_end_date(id))
  .range_containing_date(date)`. Nothing new needed adding for either.
- **`EmployeeShift.getEmployeeJobStatus()` is not cached**, despite reading
  like it might be from the outside — it is exactly
  `TimeCard::employee_job_status_for_shift`, already ported.
- **`EmployeeShift.getWorkedHours()` is not an alias of `net_hours`** — it was
  already its own field (`EmployeeShift::worked_hours`), ported with the
  `TimeCard` surface and never revisited since.
- **What genuinely needed adding**: `EmployeeShift.reg_rate` and
  `.shift_category_id`; `AssignmentPayRate` plus
  `Assignment::effective_hourly_pay_rate`; the `ShiftCategory` entity;
  `Employee::effective_job_status`/`last_effective_job_status_for_period`
  (the job-specific siblings of the already-ported home-job methods).

**`AnnualSalaryOverHoursRegRateRuleImpl` is not ported.** It backs an hourly
rate out of `PayRateCalculator.getPeriodSalary(Employee, LocalDate)` — a
calculator service, not a DAO query, and genuinely out of scope for this pass
(unlike the two items above, this dependency is real). Deferred the way
`ShiftDifferenceOTRuleImpl` was deferred from `hoursdistribution`: a catalogue
entry exists, no algorithm does yet.

**`CombinationJobsRegRateRuleImpl`** is the family's only complex rule — see
its module doc for the full algorithm and the two things reading the Java
alone would not settle: that grouping by job id is loss-less versus Java's
job-status-object identity, and that `mustBeSeparateJob` compares the
**job**, not the specific status row (a job whose rate changed mid-week is
still "the same job").

### Deliberate divergences (continued)

54. **`AssignmentPayRate` resolves through the existing `AssignmentPort`, not
    a new one.** `Assignment::effective_hourly_pay_rate` needs to walk the
    parent-assignment chain exactly as `HomeDeptRegRateRuleImpl` already
    needed `AssignmentPort` for; one port serves both.

55. **`ShiftCategory` is carried by id only.** `EmployeeShift` stores
    `shift_category_id: Option<i32>`, the same shape as `job_id`, rather than
    a reference to the new `ShiftCategory` entity — no ported rule reads
    anything off a shift category but its id.

56. **`CombinationJobsRegRateRuleImpl` groups by job id, not job-status
    identity.** Java's `Map<EmployeeJobStatus, List<Double>>` relies on
    reference/id equality Hibernate provides for free; every shift or earning
    for one job on one date (or across one week, for the weekly path)
    resolves to exactly the same status, so partitioning by `job_id` is the
    identical grouping without needing `Hash`/`Eq` on the entity.

57. **`AnnualSalaryOverHoursRegRateRuleImpl` is deferred, not ported.** Needs
    `PayRateCalculator.getPeriodSalary`, a calculator service with no Rust
    surface yet — the one genuine gap the scoping section's missing-surface
    list correctly flagged for this family (see divergence 54-56's siblings
    above for the three items it flagged that turned out unnecessary).

## `doubletimerate` — done

**29 tests, 8 of them transcribed Java assertions.** All five catalogue
entries have an algorithm: `JobDTRateRuleImpl`, `HomeJobDTRateRuleImpl`,
`HomeDeptDTRateRuleImpl`, `FLSADTRateRuleImpl`, `CommissionBasedDTRateRuleImpl`.
Ported second, per the recommended order in "Scoping — the rate families":
smallest family, and it needed nothing beyond what `regularrate` had already
proven out (`AssignmentPort`, `MinWagePort`) plus `PropertyPort` and
`TimeCard::flsa_data_map`, both already in place from Wave 0/3.

| Ported | Java spec cases |
|---|---:|
| `JobDTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `HomeJobDTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `HomeDeptDTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `FLSADTRateRuleImpl` | 2 of 3 (the third asserts `fixMap` plumbing, not arithmetic — see below) |
| `CommissionBasedDTRateRuleImpl` | no spec — behaviour tests written from the Java |

**Nothing in the scoping section's missing-surface list for `doubletimerate`
needed adding** — the section's own recommendation ("reuses everything
`regularrate` and `overtimerate` need") undersold it slightly: `overtimerate`
hasn't been ported yet either, and `doubletimerate` needed none of its
surface, only `regularrate`'s.

**`FLSADTRateRuleImplTest`'s first case, "fixMap should be called when the
rule is run", is not transcribed.** It exists in Java to assert a Spring-bean
implementation detail — that calling `execute` mutates the passed-in
`RuleItem`'s params map in place via `fixMap`. This crate's `RuleParams::fixed`
returns a new, defaulted-and-merged `RuleParams` rather than mutating the
caller's map (divergence 9's shape, applied consistently everywhere params
are read), so there is nothing of that assertion left to port — every other
test in this family already exercises `.fixed()` on every params read, which
is the same guarantee from a different angle.

### Deliberate divergences (continued)

58. **`CommissionBasedDTRateRuleConfig.EARNING_TYPES_PROP` is declared but
    never read by the algorithm.** The config defines, defaults and validates
    its own `earningTypes` parameter (distinct from the inherited
    `premiumTypes`), but `CommissionBasedDTRateRuleImpl` computes both the
    shift path's earning total *and* the earning path's gate from
    `ruleConfig.getEarningTypeIdsList(params)` — the inherited method, which
    only ever reads `premiumTypes`. `earningTypes` is dead configuration,
    reproduced as-is: `CommissionBasedDTRateRuleConfig` carries the parameter
    in `default_values`/`validate.rs`, but `commission_based_dt_rate.rs`
    sources every earning-type-id list from `PREMIUM_TYPES`. Same shape as
    `regularrate`'s `SELECTED_HOURS_DISTRIBUTION_TYPES` finding one family up.

59. **`BaseDoubleTimeRateRuleImpl.setDoubleTimeRates`'s zero-rate log is
    dropped, not reproduced.** Java logs (does not throw or skip the write)
    when the computed rate is `0`. Nothing in the rules tree uses a logging
    framework — the crate has no equivalent call anywhere else — so
    `set_double_time_rates` writes the zero rate unconditionally and silently.
    Every observable effect (the write itself) is preserved; only the log
    line, which no test anywhere in either language's suite asserts on, is
    not.

## `overtimerate` — done

**33 tests, 3 of them transcribed Java assertions.** All eight catalogue
entries have an algorithm: `JobOTRateRuleImpl`, `HomeJobOTRateRuleImpl`,
`HomeDeptOTRateRuleImpl`, `FLSAOTRateRuleImpl`, `FLSAWeightedOTRateRuleImpl`,
`GuaranteedWageOTRateRuleImpl`, `WeightedOTRateRuleImpl`,
`CommissionBasedOTRateRuleImpl`. Ported third, per the recommended order in
"Scoping — the rate families" — it adds one port beyond what `doubletimerate`
needed (`EarningTypePort`, for `GuaranteedWageOTRateRuleImpl`'s
regular-or-premium earning filter), already introduced by `regularrate`'s
`CombinationJobsRegRateRuleImpl`.

| Ported | Java spec cases |
|---|---:|
| `JobOTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `HomeJobOTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `HomeDeptOTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `FLSAOTRateRuleImpl` | 2 of 3 (the third asserts `fixMap` plumbing, not arithmetic — see the `doubletimerate` section's identical finding) |
| `FLSAWeightedOTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `GuaranteedWageOTRateRuleImpl` | 2/2 |
| `WeightedOTRateRuleImpl` | no spec — behaviour tests written from the Java |
| `CommissionBasedOTRateRuleImpl` | no spec — behaviour tests written from the Java |

**Refuted, as the scoping section itself flagged might happen: the family did
not need `overtimerate`-specific new infrastructure beyond `EarningTypePort`.**
`FLSAOTRateRuleImpl` — the one rule the scoping section called out as needing
`Property.getCurrentWeek()`/`getPayPeriod()` gated on `PayPeriodType` — turned
out to need no new surface at all: `TimeCard::pay_period_containing` already
stands in for `Property.getPayPeriod().getDateRangeContainingDate` (divergence
24, same finding `regularrate` and `doubletimerate` already made for their own
property lookups), and `TimeCard::pay_period_type()` already stands in for
`employee.getProperty().getPayPeriodType()`.

### Deliberate divergences (continued)

60. **`JobOTRateRuleImpl`'s two Java instance fields collapse to nothing.**
    Java tracks which overload is running with `this.shift`/`this.earning`,
    set at the top of each `execute` and never cleared, so a shared private
    `getRate` can throw the right message for whichever one triggered the
    missing-job-status condition. `JobOTRateRule::execute_for_shift`/
    `execute_for_earning` already know which overload they are — no shared
    state needed, the two call sites just panic with their own message.

61. **`GuaranteedWageOTRateRuleImpl`'s earning path does not round the
    addition.** Every other rule in `regularrate`/`doubletimerate`/
    `overtimerate` that adds a computed rate onto an earning's existing rate
    wraps the sum in `roundCurrency`
    ([`add_overtime_rate`](crate::rules::algorithm::overtimerate::add_overtime_rate)
    and its `doubletimerate` twin). `GuaranteedWageOTRateRuleImpl` does not:
    `earning.setRate(getOtRate(...) + earning.getRate())`, no rounding around
    the `+`. `getOtRate` already rounds its own result with `roundHours`, so
    nothing is lost in practice — but the shape is different enough that this
    rule does not call the shared helper, reproduced with its own bespoke
    `execute_for_earning`.

62. **`CommissionBasedOTRateRuleConfig.EARNING_TYPES_PROP` is declared but
    never read**, the same finding as `CommissionBasedDTRateRuleConfig`
    (divergence 58) one family over — both the commission total and the
    earning-path gate read the inherited `premiumTypes` via
    `getPremiumEarningTypeIdsList`, never the rule's own `earningTypes`.

## `earningrate` — done

**41 tests, 15 of them transcribed Java assertions.** All ten catalogue
entries have an algorithm: `EarningFixedRateRuleImpl`, `EarningFactorRateRuleImpl`,
`EarningOverrideJobRateRuleImpl`, `HomeJobRateRuleImpl`, `HomeDeptRateRuleImpl`,
`ContractDailyRateRuleImpl`, `FLSAEarningRateRuleImpl`, `AvgDayXWeeksRateRuleImpl`,
`PriorBalancesRateRuleImpl`, `CalculatedAccrualRateRuleImpl`. Ported last, per
the recommended order in "Scoping — the rate families" — the family needing
the most new infrastructure, and fully decoupled from the other three.
`AvgWageEarningRateRule`, the one-implementor marker sub-interface, folded
away rather than becoming a separate Rust trait — see `mod.rs`.

| Ported | Java spec cases |
|---|---:|
| `EarningFixedRateRuleImpl` | 1/1 |
| `EarningFactorRateRuleImpl` | no spec — behaviour tests written from the Java |
| `EarningOverrideJobRateRuleImpl` | 6/6 |
| `HomeJobRateRuleImpl` | no spec — behaviour tests written from the Java |
| `HomeDeptRateRuleImpl` | no spec — behaviour tests written from the Java |
| `ContractDailyRateRuleImpl` | no spec — behaviour tests written from the Java |
| `FLSAEarningRateRuleImpl` | 2/2 |
| `AvgDayXWeeksRateRuleImpl` | no spec — behaviour tests written from the Java |
| `PriorBalancesRateRuleImpl` | 3/3 |
| `CalculatedAccrualRateRuleImpl` | 3/3 |

**Measuring the scoping section against what porting actually needed found it
about half wrong again**, the same lesson every other rate family already
left behind:

- **A `PayRateCalculator`-equivalent port — flagged as "likely needed again
  here" by the `regularrate` section — turned out not to be needed at all.**
  None of this family's ten rules call anything like `getPeriodSalary`; no
  such port was added. Refuted, not confirmed.
- **What genuinely needed adding, confirmed by reading each rule**:
  `EmployeeJobStatus.contract_days` (a plain `f64`, alongside the
  already-ported `contract_hours`); [`AccrualTransactionPort`](crate::rules::ports::AccrualTransactionPort)
  plus the new [`AccrualTransaction`](crate::entity::accrual_transaction::AccrualTransaction)
  entity; [`HolidayDataPort`](crate::rules::ports::HolidayDataPort) plus the
  [`EligibleHours`](crate::rules::ports::EligibleHours) DTO; and
  `EmployeeEarningPort::banked_rate_for_rule`. Everything else — `AssignmentPort`,
  `MinWagePort`, `EarningTypePort`, `PropertyPort`, `Assignment::effective_hourly_pay_rate`,
  `Employee::home_employee_job_status`/`employee_job_status` — `regularrate`
  had already proved out.

**The family's interface shape forced one genuinely new decision.**
`EarningRateRuleImpl` takes raw parameters, not a `RuleItem`, so
[`EarningRateRule::execute`](crate::rules::algorithm::earningrate::EarningRateRule::execute)
takes `&RuleParams` directly rather than reading through `rule_item.params()`.
And because `CalculatedAccrualRateRuleImpl` and `PriorBalancesRateRuleImpl`
both cache state in a field across calls within one run — new territory
flagged in the scoping section — `execute` takes `&mut self` for the whole
family, not just those two. See `mod.rs` for both.

**A Groovy int-typing quirk, not a Rust divergence, explains one spec's
numbers.** `EarningOverrideJobRateRuleImplTest` declares
`static int minimumWage = 10.55`; Groovy's static typing truncates the
literal to `10` at assignment, and that truncated `10` (not `10.55`) is what
flows into every `AssignmentPayRate.minWage` the spec builds. Read `10.55` at
first glance and every expected rate in that spec looks unreconcilable; the
transcribed cases use `10.0` throughout, matching what the spec actually
exercises rather than what it appears to declare.

### Deliberate divergences (continued)

63. **`AccrualTransaction` carries four of the Java entity's nineteen
    fields** — `earningType`, `asOfDate`, `unappliedHours`,
    `endingTotalBalance` — the same narrow-slice practice every other entity
    in this crate already follows.

64. **`AvgDayXWeeksRateRuleImpl`'s hire-date guard compares against the
    averaging window's *end* date, not its start.** `calcPeriodEndDate` —
    the day before the week containing the earning date starts — is what
    `employee.getHireDate().isOnOrBefore(...)` checks; the window's start
    date (from `HolidayDataDAO.getWeekStartOfNthPastWorkedWeek`) only matters
    once that guard has already passed.

65. **`CalculatedAccrualRateRuleImpl`'s bucket-count-mismatch handling has a
    Java bug in one direction, reproduced rather than fixed.** When there are
    more hours transactions than cost transactions, Java correctly trims the
    unmatched hours transactions. When there are fewer, it runs
    `costMap.keySet().removeIf(key -> !hoursTransactions.contains(key))` —
    checking a `LocalDate` key against a `List<AccrualTransaction>` with
    `List.contains`, which can never be `true` for any element. The predicate
    is therefore always `true`, and the branch empties the whole cost map
    rather than trimming it. `build_buckets_for_employee` reproduces this
    exactly: an hours-count shortfall clears `cost_by_as_of_date` outright.

66. **`EarningOverrideJobRateRuleConfig.validateProperties` does not call its
    parent's.** It builds a fresh `ValidationResults`, so unlike every other
    config in the family it does not require `selectedEarnings` to be
    non-empty; and where the with-factor default requires `rateFactor >= 0`,
    this config's own validation requires `>= 0.01`
    (`AbstractRuleConfig.SMALLEST_VALID_DOUBLE`, a stricter bound also
    applied to `overrideRate` when `useMinWage` is false).

67. **`PriorBalancesRateRuleConfig.validateProperties` also skips its
    parent's `selectedEarnings` check**, the same shape as divergence 66 —
    but its sibling `CalculatedAccrualRateRuleConfig`, extending the same
    base class, *does* call `super.validateProperties()` before adding its
    own two accrual-id checks. Two configs in one family handle the base
    check differently.

68. **`EarningRateRule::execute` takes `&mut self`, not `&self`** — the one
    shape difference from every other rate family's trait, needed by
    `CalculatedAccrualRateRuleImpl` and `PriorBalancesRateRuleImpl`'s
    per-employee caches even though the other eight rules in the family never
    touch their own mutability. See `mod.rs`.

## Scoping — `hoursdistribution`

Measured before starting, so it does not have to be re-derived.

**19 catalogue entries, not 25.** The package's 29 files include nine that are
not rules: the `HoursDistributionRuleImpl` interface, `ContractHrsRuleImpl`
(itself an interface, which `RuleUtils.getContractHours` reaches for by
`instanceof`), `ShiftDifferenceOTRule`, and the six helpers below (one of which,
`PriorDaysCalculator`, sits in the `priordayscalculator` subpackage). That
leaves twenty `*RuleImpl` classes against 19 catalogue entries —
`ShiftDifferenceOTRuleImpl` is the odd one out, and is deferred with the rules.

**`RegHrsOnlyRuleImpl.execute` is empty.** The default fallback rule
(`REG_ONLY_HDR`) is a genuine no-op — regular hours are distributed by the
runner, and this rule means "leave them alone". Do not go looking for the body.

**The helpers, which several rules share** — all six are now ported; see "The
shared helpers" below:

| Java | What it holds |
|---|---|
| `DailyAccumulator` | a day's hours/OT/DT plus the daily limits; `computeDailyOvertime`/`computeDailyDoubleTime` |
| `WeeklyAccumulator` | a week's hours/OT, the consecutive-day counter and its modifier arithmetic |
| `DailyData` | a date with its earnings and its (shift, earnings) pairs |
| `ConsecutiveDaysCalculator` | 63 lines |
| `EarningMapper` | 71 lines |
| `priordayscalculator/PriorDaysCalculator` | 85 lines |

Both accumulators round through `TDouble.roundHours` on every `add*`, so they
depend on `common/numbers.rs` being exact.

**The `TimeCard` surface this family needs** — about twenty methods, against the
one (`schedules()`) ported through Wave 1. **All of them are now ported**; see
"The `TimeCard` surface" below. By use: `getEmployee` (36),
`getOTHoursDistributionTypeId` (21), `isOpenForEditingOn` (18),
`getShiftsWithDistributionsForPeriod` (18), `getDTHoursDistributionTypeId` (12),
`getEarnings` (12), `getRegularHoursDistributionTypeIds` (12),
`distributionIsPremium` (7), `getShifts` (6), `getCalculationMode` (6),
`getDatasetStartDate` (6), `getEarningsForPeriod` (5),
`distributionIsRegular` (5), `isOpenForEditingFor` (5), `getStatMap` (3),
`employeeShiftStream` (3), `getFlsaDataMap` (2), `getShiftsForPeriod` (2),
`getSchedulesWithDistributionsForPeriod` (2), `getCalculationStartDate` (1),
`getHoursDistributionTypeIds` (1),
`distributionsWithShiftDuringPeriodMatchingType` (1).

Several are `default` methods in the Java interface and carry real logic — the
OT and DT bucket lookups match on **name** (`HoursDistributionType.OVERTIME_NAME`),
not id, so a site that renames a bucket silently stops getting overtime.

**Four new ports, all now defined.** Three arrived — the two
`employeeShiftConsecutiveDaysDAO` methods as `EmployeeShiftConsecutiveDaysPort`,
with the calculators that call them, and `holidayDAO.findAllForProperty` as
`HolidayPort` with `HolidayDTHrs` (which also brought the `Holiday` entity),
and `employeeShiftDAO.getNetAndOTHoursForPeriod` extended `EmployeeShiftPort`
with `RollingXWeeksOTHrs`. `earningTypeDAO.findByID` and the
`employeeEarningDAO` methods were already defined. The family also needed a
fifth collaborator Java reaches through the entity graph rather than a DAO —
see divergence 38's `MinWagePort`.

**Also needed:** `LegacyDatePeriod`, which every `execute` takes as its work
week, and `FlsaData` for the two `getFlsaDataMap` call sites. Both are now
settled — see below.

## Rules — 19 of 19

| Rust | Java | Ported cases |
|---|---|---:|
| `algorithm/hoursdistribution/reg_hrs_only.rs` | `RegHrsOnlyRuleImpl` | no Java spec (a no-op) |
| `…/holiday_dt_hrs.rs` | `HolidayDTHrsRuleImpl` | **4/4** |
| `…/weekly_ot_sec_job_hrs.rs` | `WeeklyOTSecJobHrsRuleImpl` | **2/2** |
| `…/weekly_ot_hrs.rs` | `WeeklyOTHrsRuleImpl` | **7/7** |
| `…/pay_period_ot_hrs.rs` | `PayPeriodOTHrsRuleImpl` | **8/8**, but see below |
| `…/scheduled_shift_ot.rs` | `ScheduledShiftOTRuleImpl` | **10/10** |
| `…/california_extended_ot_hrs.rs` | `CaliforniaExtendedOTHrsRuleImpl` | **12/12** |
| `…/rolling_x_weeks_ot_hrs.rs` | `RollingXWeeksOTHrsRuleImpl` | **3/3** |
| `…/daily_weekly_6th_ot_7th_dt_non_consec.rs` | `DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl` | **3/3** |
| `…/per_month_ot_hrs.rs` | `PerMonthOTHrsRuleImpl` | **5/5** |
| `…/california_ot_hrs.rs` | `CaliforniaOTHrsRuleImpl` | **6/6** |
| `…/twenty_four_hour_ot.rs` | `TwentyFourHourOTRuleImpl` | **31/31** |
| `…/contract_ot_hrs.rs` | `ContractOTHrsRuleImpl` | **6/6** |
| `…/daily_weekly_7th_dt_hrs.rs` | `DailyWeekly7thDTHrsRuleImpl` | **7/7** |
| `…/california_ext_special_job_ot_hrs.rs` | `CaliforniaExtSpecialJobOTHrsRuleImpl` | **9/9** |
| `…/min_hrs_for_full_time_ot.rs` | `MinHrsForFullTimeOTRuleImpl` | **18/19** |
| `…/dly_wkly_off_consec_ot_min_break.rs` | `DlyWklyOffConsecOTMinBreakRuleImpl` | **16/16** |
| `…/daily_weekly_6th_ot_7th_dt_hrs.rs` | `DailyWeekly6thOT7thDTHrsRuleImpl` | **13/13** |
| `…/dly_wkly_consec_ot_min_break_spanning_midnight.rs` | `DlyWklyConsecOTMinBreakSpanningMidnightRuleImpl` | **13/13** |
| `entity/employee_shift.rs` — `is_adjustment_only_shift` | the same | — |
| `common/json_ids.rs` — `ids_from_json` | `JSONUtils.getIdsFromJSON` | — |
| `…/daily_data.rs` — `build_daily_data_map` | the `dailyDataProducer` two rules declare identically | — |
| `common/contract.rs` | `labor.contract` — the interface, factory and three calculators | — |
| `common/enums/schedule_mode.rs` | `ScheduleMode` | — |
| `entity/time_card.rs` — `pay_period_type`, `schedule_mode` | `Property`'s two getters | — |
| `…/mod.rs` — `ContractHrsRule` | `ContractHrsRuleImpl` | — |
| `…/config.rs` | `HoursDistributionRuleConfig` + all nineteen rules' configs | **7**, from `TwentyFourHourOTRuleConfigTest` |
| `entity/employee_shift.rs` — `net_hours_in_range` | `ShiftUtil.getNetHoursInRange` and its four helpers | — |
| `rules/types/earning_type_pay_set.rs` | `EarningTypePaySet` | — |
| `rules/types/earning_type_pay_map.rs` | `EarningTypePayMap` | — |
| `entity/time_card.rs` — `earnings_mut`, `earning_is_not_salaried_exempt` | the live `getEarnings()` list, `…IsNotSalariedExemptForEarning` | — |
| `entity/employee_earning.rs` — `calc_and_set_total_dollars`, `with_shift` | the same | — |
| `rules/ports.rs` — `EmployeeShiftPort::net_and_ot_hours_for_period` | `getNetAndOTHoursForPeriod` | — |
| `rules/ports.rs` — `MinWagePort` | `Employee.getMinWage`'s entity walk | — |
| `algorithm/utility/hours_distribution_factory.rs` | `HoursDistributionFactory` | — |
| `common/json_ids.rs` | `JSONUtils`'s id-list half | — |
| `entity/holiday.rs` | `Holiday` | — |
| `rules/ports.rs` — `HolidayPort` | `HolidayDAO.findAllForProperty` | — |

The family trait is `HoursDistributionRule::execute(&mut dyn TimeCard,
&DateRange, &RuleItem)`. Unlike `PunchRoundingRule`, where the time card is
read-only context four of six rules ignore, here the card **is** what the rule
rewrites, so it arrives `&mut` and reads go through the index primitives.

### Findings

- **The family matches distribution buckets two different ways, and one rule
  is immune to the other's failure mode.** `HolidayDTHrsRuleImpl` uses the
  constants `HoursDistributionType.REGULAR_ID` and `DT_ID` straight off the
  class; the overtime rules go through `TimeCard.getOTHoursDistributionTypeId()`,
  which matches on the literal name `"Overtime"`. So renaming a bucket breaks
  overtime and leaves holiday double time working, while a property that
  configured double time under a different id breaks the reverse way. Both
  mechanisms live in the same family.

- **A premium distribution is created with `originalHours` of zero**, not with
  the hours it carries. `HoursDistributionFactory.createPremiumDistribution`
  passes `0` for that argument while `hours` takes the premium amount. It is
  what stops an overtime hour being counted twice — the accumulators sum
  `originalHours` when measuring a week against its limits, so the regular hour
  it came from is the only one that counts. `HoursDistribution::new` sets
  `original_hours` equal to `hours`, which is right for a fresh regular row and
  wrong here, so the factory builds the struct field by field. Pinned by
  `a_premium_distribution_has_zero_original_hours`.

- **The last two rules are siblings, and one of them settles a question the
  other could not.** `DlyWklyOffConsecOTMinBreak`'s last spec case,
  `when both consec and weekly OT is checked…`, was recorded here as
  unreconciled in three rows. The same case — same eight shifts, same four
  parameters, same expectations — appears in
  `DlyWklyConsecOTMinBreakSpanningMidnightRuleImplTest`, where **it passes**.
  Two config differences explain all three rows:

  | | `DlyWklyOffConsecOTMinBreak` | the spanning-midnight sibling |
  |---|---|---|
  | `minTimeBetweenShifts` default | 7.0 | **10.0** |
  | `consecDayHrsLimit` | not declared | **40.0**, and the rule reads it |

  The fixture's first gap is 3.25 hours, so the shortfall is `min(10 - 3.25, 4)`
  = 4 there and `min(7 - 3.25, 4)` = 3.75 here; and the fifth consecutive day
  pays `8 + min(28, 35) - 35` = 1 there where this rule has no hours limit and
  pays the whole day. The case was copied into the wrong spec. Both
  transcriptions now assert what their own rule produces, and each names the
  other. **Resolved by porting the sibling**, not by reading.

- **`DlyWklyConsecOTMinBreakSpanningMidnight` is the only rule with an
  hours-based consecutive-day limit.** Reaching the consecutive-day *count*
  only opens the question; what is paid is everything past `consecDayHrsLimit`
  **hours** across the run:
  `hours + min(priorConsecutiveDaysHours, limit) - limit - overtime`.

  `priorConsecutiveDaysHours` is advanced **one day late** — Java adds
  `currentDailyData.hoursForConsecDays()` before reassigning `currentDailyData`,
  so the total never includes the day being processed. And it reads
  `getHours()`, the current value, so a day already reduced contributes less.
  The port carries the lag explicitly as a `yesterday` variable.

- **Its break rule fires only across midnight**, where the sibling's fires on
  any pair of shifts with different shift dates or a long enough gap on one:
  `priorShift.getEndDateTime().toLocalDate().equals(shift.getStartDateTime().toLocalDate().minusDays(1))`.
  Two shifts on one date never qualify. `minTimeBetweenPayFullShift` then
  chooses whether a violation costs the shortfall or the **whole shift**.

  It also asks its two halves two different ways: the prior-run seeding filters
  with `HoursDistribution.isRegularType`, the static predicate testing the
  `REGULAR_ID` constant, while the in-week half goes through
  `getRegularHoursDistributionTypeIds()`. The same split the family has seen
  before.

- **`DailyWeekly6thOT7thDTHrs` has two consecutive-day thresholds, and they are
  disjoint.** `isDuringOTConsecDaysRange()` is `counter >= otLimit && counter <
  dtLimit` and `isDuringDTConsecDaysRange()` is `counter >= dtLimit`, so the
  sixth day is "OT range", the seventh is "DT range", and a day is never both.
  On the seventh `shiftDT` equals `shiftOT`, so no overtime row is written at
  all.

- **`payDailyOT` and `payDailyDT` suppress rows without changing the
  arithmetic.** `addDistributions` reassigns its **local** `premiumHours` — to
  `doubleTime` inside the consecutive-day branch, to `0` outside it — so the
  regular row loses only what was written, while the caller's copy still feeds
  `addOvertime` and `addWeeklyOT`. And `computeDailyDoubleTime`'s last branch
  ignores `payDailyDT` entirely: with the flag clear the figure is computed,
  discarded, and **still added to the day's double-time total**. Pinned.

- **`isDuringDaysInWeekOtRange` mixes its two counters**:
  `daysInWeekCounter >= otConsecDaysLimit && consecutiveDaysCounter < dtConsecDayLimit`.
  `overrideConsecDayOt` selects it, so six non-consecutive worked days reach the
  sixth-day rule — but the seventh *consecutive* day still shuts it off.
  `daysInWeekCounter` is never reset and never wrapped.

- **`bothConsecutiveAndWeeklyOt` is dead in a third rule.**
  `DailyWeekly6thOT7thDTHrs` and `DlyWklyConsecOTMinBreakSpanningMidnight` both
  assign it to a field nothing reads, as `CaliforniaExtendedOTHrs` hands it to
  an accumulator field nothing reads. **`MinHrsForFullTimeOT` is the only rule
  in the family that does anything with it.** Four rules read the parameter;
  one acts on it.

- **`DailyWeekly6thOT7thDTHrs`'s `shiftMatches` asserts nothing for a zero.**

  ```groovy
  void shiftMatches(EmployeeShift shift, netHours, regHours, otHours, dtHours) {
     if (regHours > 0) { assert … }
     if (otHours  > 0) { assert … }
     if (dtHours  > 0) { assert … }
  }
  ```

  So `shiftMatches(shift1, 8, 0, 0, 0)` — the line every case uses for the two
  shifts outside the week, commented "should not calc" — asserts **nothing**,
  and could not detect the rule paying them. `netHours` is never read in the
  body either, so the first number is decorative in all 117 calls. Eighth
  spec-weakness in the family, and the second where a helper silently drops
  assertions. The transcriptions assert the whole triple, zeros included.

- **`DlyWklyOffConsecOTMinBreak` adds two ideas nothing else in the family
  has**: overtime for working a day you were **not scheduled**, and overtime for
  not getting a long enough **break between two shifts**. The break rule finds
  the latest shift on the whole card ending before this one starts, and pays
  `min(minTimeBetweenShifts - gap, the shift's own regular hours)` — `max`ed
  with the ordinary daily figure, so it tops up rather than replaces.

  A split shift has to be told apart from a rest violation, and the test is
  awkward: a gap on **one date** is exempt only while it stays *under*
  `dailyShiftSplitLimit`. A five-hour gap on one day is neither a split shift
  nor a rest, and is paid.

- **The day's earning hours are written into the distribution's own hours.**

  ```java
  distribution.setHours(roundHours((distribution.getHours() + dailyData.getEarningHours()) - premiumHours - dtHours));
  ```

  A seven-hour shift on a day carrying a two-hour configured earning comes out
  with an eight-hour regular row. No earning row is ever created or changed —
  this rule reads earnings and writes only distributions. And because the write
  is behind `premiumHours > 0`, the fold happens **only on days that pay a
  premium**; under the limit the same earning leaves the row alone. Both pinned.

- **`premiumHours` is decremented before the row is reduced.**
  `addDistributions` does `premiumHours -= dtHours` inside the double-time
  branch, and the `setHours` line below reads the decremented local — so the
  regular row loses the double time once, not twice. The caller's copy is
  untouched, so the accumulators still see the whole figure. Easy to get wrong;
  a hand-check of the spec's `combine all rule scenarios` is what caught it here.

- **Double time is recomputed from the day's total every time and never netted
  off.** `calculateDT` is `max(hours - dailyDTLimit, 0)` — unrounded, with no
  `addDoubleTime` on the accumulator at all. A second distribution on a day
  already past the limit writes a second, overlapping double-time row.

- **The daily data map is built a day wider than it is read.**
  `dailyDataProducer` runs `[workWeek.start - 1, workWeek.end]`; the loop asks
  only for dates in the week. The extra day is built and discarded.

- **Three rows of its last case do not reconcile, and the imports say why.**
  `when both consec and weekly OT is checked…` is `combine all rule scenarios`
  with its first shift moved from 20:00–00:00 to 17:45–21:45 — which changes the
  break gap from exactly one hour to 3.25 and the shortfall from 4 to 3.75 —
  but with the expectations left as they were. A third row, at the fifth
  consecutive day, does not follow from that either: `isDuringOTConsecDays()`
  holds there, so `computeWeeklyOT` returns zero whatever the flag says.

  The case imports three of its four parameter constants from
  `DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig` — the **sibling rule's**
  config — including a `CONSEC_DAY_HRS_LIMIT` this rule never reads. It looks
  copied from that rule's spec. The transcription asserts what this rule
  produces and labels the three rows; **it has not been confirmed against a Java
  run**, and the sibling's spec is the place to settle it.

- **Two rules define "adjustment-only shift" differently.**
  `EmployeeShift.isAdjustmentOnlyShift()` is
  `punches.isEmpty() && !workedAdjustmentsEmpty()`, which this rule uses to skip
  the break rule. `TwentyFourHourOTRuleImpl.isShiftAdjustmentOnly` is
  `!hasErrors() && !hasBothTimes()`. A shift with no punches and no adjustment
  answers differently to each.

- **A fourth spelling of "this week only".** `THIS_WEEK_ONLY` is `thisWeekOnly`
  here, against `consecDaysInWeek` elsewhere — a different key for the same
  idea, alongside the `dailyOTLimit`/`dailyOtLimit` pair already recorded.

- **`MinHrsForFullTimeOT` pays *double time* to part-timers and overtime to
  everyone else**, and decides which from the **whole week's** hours, computed
  up front. `eligibilityLimit >= totalWeeklyHours` — so a part-timer's Monday is
  paid differently depending on whether they pick up a shift on Saturday. Every
  chunk of hours, distribution or earning, runs the same three-rung ladder:
  consecutive days, then part time, then the daily and weekly limits.

- **`maxDTPaid` is a weekly budget that overtime spends too.** Every premium
  hour the rule writes — consecutive-day overtime and daily or weekly overtime
  alike — is added to `premiumHoursAllocated`, which is what
  `calculateUnallocatedDT` measures against. So overtime paid on a consecutive
  day reduces the double time available later in the week; the spec has a case
  named for it.

- **`bothConsecutiveAndWeeklyOt` does something here.**
  `CaliforniaExtendedOTHrs` reads the same key and hands it to an accumulator
  field nothing reads (already recorded). This rule uses it to pick between two
  weekly formulas: set, `min(workedHours - limit, currentChunkHours)`, which
  **ignores what has already been allocated** and so pays an hour again that
  consecutive-day overtime already paid; clear,
  `max(0, workedHours - limit - otHoursAllocated)`. The spec's last two cases
  are the two sides, and the second calls the unset behaviour "legacy".

- **Two keys differ from the rest of the family only by letter case.** This rule
  reads `dailyOTLimit` and `weeklyOTLimit` from
  `HoursDistributionConfigConstants`, where five other rules read `dailyOtLimit`
  and `weeklyOtLimit`. A property copying a parameter from one rule to another
  gets the default and no warning. Ported as `DAILY_OT_LIMIT_PROP_CAPS` and
  `WEEKLY_OT_LIMIT_PROP_CAPS` so the two cannot be confused in Rust.

- **A shift is processed once per distribution it has on the day.**
  `dailyShiftMap` flat-maps shifts to distributions, groups by **distribution
  date**, and maps back to the shift — so a shift with two distributions dated
  the same day appears in that day's list twice and its whole `computeShiftHours`
  runs twice, counting its earnings twice and walking its distributions twice.
  Reproduced; pinned by
  `a_shift_with_two_distributions_on_one_day_is_processed_twice`.

- **One branch cannot be taken.** The distribution stream filters
  `isOpenForEditingOn`, and the first arm of the branch that follows is
  `distributionIsPremium(d) && !isOpenForEditingOn(d.getDate())`. Dead. Not
  ported.

- **It does not filter salaried-exempt shifts**, where every other rule in the
  family does — `getShiftsWithDistributionsForPeriod` is used raw. Its two
  helpers *do* filter: `EarningMapper` drops exempt earnings and
  `ConsecutiveDaysCalculator` drops exempt shifts. So one rule applies the test
  to its earnings and its day counter but not to its hours.

- **Four of its nineteen cases assert nothing.** They put their checks inside a
  `(0..N).each { … }` closure, and Spock applies implicit assertions only to
  top-level expressions in a `then:` block — so `dist.size() == 1` and the
  `assertListHasCorrectHoursDistribution` call are evaluated and discarded.
  Seventh spec-weakness in the family and the first of this shape; the
  transcriptions assert them for real.

  A nineteenth case, `work rule should always call the fix map method`, is a
  mock-interaction test on `ruleConfig.fixMap`. `fixed()` is called
  unconditionally here and there is nothing to observe, so it is the one case
  not transcribed — hence 18 of 19.

- **`CaliforniaExtSpecialJobOTHrs` does not choose between its two sets of
  limits — it *averages* them, weighted by hours.** Nothing else in the family
  does this. `HourLimits` walks the whole work week before anything is paid,
  picking the special limits for a distribution whose day is a configured
  special day of week **and** whose shift's job is in the configured list, and
  accumulating `hours × limit` against `hours`. The weekly limit the accumulator
  runs on is `roundHours(totalWeeklyLimits / totalWeeklyHours)`; each day's two
  limits are the same quotient over that day's hours.

  So a week that mixes a 35-hour-week special job with ordinary work gets a
  limit strictly between the two. The spec pins it at **38.83** for 14 special
  hours in a 59.86-hour week — and note the limit is computed from the **whole**
  week, so a Sunday shift changes what Monday is paid.

- **Dividing by zero is reachable in Java and silent.** A day with no shifts
  gives `0/0` = `NaN` for both daily limits, and a week with none gives `NaN`
  for the weekly limit. Every later comparison against `NaN` is false, so
  nothing is written — which is also the outcome here, because such a day has no
  distributions to walk. The port answers `0.0`; see divergence 48.

- **It declares its own `DailyData` *and* its own pair of accumulators**, making
  it the only rule to shadow three of the shared types at once. The `DailyData`
  is `(date, shifts, specialDay)` with **no earnings**, which is what the
  warning on the shared struct has been about since it was ported. Its
  `WeeklyAccumulator` takes a fourth constructor argument seeding the counter,
  and rounds in `computeWeeklyOT` where the shared one does not; its
  `DailyAccumulator` returns zero double time unless `payDT`.

- **`specDaysOfWeek` and `specJobs` are read by the *throwing* JSON parser.**
  `JSONUtils.getIdsFromJSON` declares `throws JSONException` and this rule calls
  it without a catch, so a corrupt list aborts the calculation — where the same
  mistake in `holidayTypes`, read through `getIdsListForKey`, silently selects
  nothing. That makes **three** spellings of one operation in the tree:
  `id_list` swallows, `ids_from_json` panics, `EarningTypePaySet::from_json_string`
  panics. All three are faithful.

  The day numbers are **Sunday-based**, not ISO — compared against
  `DateUtil.translateDOWFromISO(date.getDayOfWeek())`. Mixing the two
  conventions shifts the whole rule by one day, silently; pinned by
  `the_special_days_are_sunday_based_not_iso`.

- **`employeeJobStatusIsNotSalariedExemptForEarning` is declared and never
  used** — the rule reads no earnings at all. Fourth dead member in the family,
  after `setBothConsecutiveAndWeeklyOt`, `PayPeriodOTHrs`'s unused DAO and
  `CaliforniaOTHrs`'s `payLevelMap`.

- **Its spec mocks `PriorDaysCalculator`, not the DAO under it.** Every other
  transcription in the family stubs `EmployeeShiftConsecutiveDaysDAO`, because
  that is what the Groovy mocks; here the Groovy mocks the calculator itself to
  answer 4. The calculator is ported rather than stubbed, so the transcription
  stubs the DAO at **3** and lets the real calculator add the 11-21 shift it
  finds between the dataset start date and the week. Noted on the stub.

- **`DailyWeekly7thDTHrs` shadows *both* shared accumulators**, the third rule
  in the family to do so after `CaliforniaExtSpecialJobOTHrs`'s inner
  `DailyData` and `DailyWeekly6thDayOT7thDayDTNonConsec`'s inner pair. Its
  inner classes take the shared names and differ in four ways:

  | | the shared class | this rule's inner class |
  |---|---|---|
  | `WeeklyAccumulator(…)` | `(weeklyLimit, consecDayLimit, maxConsecDays)` | `(weeklyLimit)` alone |
  | the consecutive-day test | `counter >= consecDayLimit` | `counter == 7`, **exactly** |
  | `computeWeeklyOT` | two formulas behind a flag, **unrounded** | one formula, **rounded**, zero on the seventh day |
  | daily double time | always computed | zero unless `payDailyDT`, or it is the seventh day |

  Ported private to the rule as `WeekTotals` and `DayTotals`. Its inner
  `DailyData`, by contrast, is a field-for-field copy of the shared one and uses
  it directly.

- **On the seventh consecutive day the double-time row takes the whole day and
  no overtime row is written at all.** Weekly overtime is forced to zero, daily
  overtime becomes `hours - overtime` and daily double time becomes
  `hours - doubleTime` — so `shiftDT` equals `shiftOT`, `premiumHours -
  doubleTime` is zero, and the overtime branch is never taken. All seven spec
  cases assert the same `(0, 0, 8.5)` for that day whatever else they change.

  The test is `== 7`, not `>= 7`. Unreachable past seven inside a seven-day
  week, but a longer period handed in as a work week would silently stop paying
  it.

- **An earning that already carries a premium level is booked as premium before
  it is counted.** `computeEarningHours` reads the pay level out of the pay set
  and adds the hours to the day's overtime (level 1 or 2) and double time
  (level 2) *before* `addHours`. `CaliforniaOTHrs` reads the same pay set and
  does none of this — it treats every configured earning as regular hours. One
  parameter, two meanings.

- **`payDailyDT` is not `payDT`.** A fifth distinct spelling in the family
  (`payDailyDT`), and it gates only the ordinary daily double-time limit:
  the seventh day pays double time whether it is set or not. The two California
  rules' `payDT` gates the premium split itself, which this rule does not have —
  `addDistributions` here branches on `doubleTime > 0` alone.

- **The `dailyDataProducer` is written out twice in Java, identically**, once in
  `CaliforniaOTHrsRuleImpl` and once here — same filter, same partition, same
  `ShiftStartTimeComparator` ordering. Ported once as
  `daily_data::build_daily_data_map` and called from both, so there is no second
  place for them to drift. `CaliforniaExtSpecialJobOTHrsRuleImpl` builds a
  different shape and does not use it.

- **Correction: `ContractOTHrs` is not a `ContractHrsRule`.** This file
  previously described it as "the second `ContractHrsRuleImpl`". It is not —
  `ContractOTHrsRuleImpl implements HoursDistributionRuleImpl`, and
  `PerMonthOTHrsRuleImpl` is that interface's **only** implementor, so
  `RuleUtils.getContractHours` still resolves to exactly one rule by
  `instanceof`. The claim was inferred from the name. Nothing was built on it,
  but it is the second scoping claim here to turn out wrong when the rule was
  actually read, so measure before scheduling.

- **`ContractOTHrs` pays overtime past a *contracted* number of hours**, not a
  statutory one: the property sets weekly contract hours and a
  `ContractCalculator` scales them to the pay period. Three calculators behind a
  factory keyed on the property's pay period type and schedule mode, ported to
  one enum in `common/contract.rs` the way divergence 3 collapsed the rounding
  strategies.

  **A weekly schedule pins the contract to one week.** The factory's first test
  is `scheduleMode == WEEKLY && payPeriodType != WEEKLY`, which selects the
  calculator that ignores the period and returns the weekly figure unchanged —
  so a property that schedules weekly but pays monthly contracts for one week's
  hours a period, not a month's. Reproduced.

  The two real calculators round at **different precisions through different
  rules**: `WeekBasedContractCalculator` at two places, which carries the
  epsilon nudge, and `DaysInPeriodContractCalculator` at zero, which is plain
  half-to-even. The `TDouble` finding in Wave 0 is why those are not the same
  operation, and `round_to` keeps them apart.

- **It is the only rule that writes `CalcDataSetStat`** — three of the five
  constants, one triple per pay period the week touches, keyed by period start.
  That part of the old description was right.

- **Its two seeding paths disagree on all three counts.** Same split as
  `RollingXWeeksOTHrs` — the card inside the dataset, the DAOs outside it — and
  the same class of divergence:

  | | off the card | from the DAO |
  |---|---|---|
  | net hours | `hours` of **every** distribution, premium rows included | `originalHours` where type = 1 |
  | overtime | `hours` of the overtime **and** double-time buckets | `hours` where type = 2, **plus** type = 3 |
  | earnings | always folded in | only in `TA` mode |

  The card path counting premium rows into net hours is the sharp one: an hour
  already paid as overtime is counted a second time toward the contract.

- **The DAO's third column is read after all.** `PARITY_AUDIT` recorded under
  `RollingXWeeksOTHrs` that `getNetAndOTHoursForPeriod` "also selects a third
  `dtHours` column that the rule never reads", and `NetAndOtHours` carried two
  fields on that basis. True of that rule; **false of this one**, which reads
  all three and adds `dtHours` to the same running total as `otHours`. So one
  caller counts double time as overtime already paid and the other does not, off
  one query. The port now carries `dt_hours` and says so. Pinned by
  `the_dao_path_counts_double_time_as_overtime_already_paid`.

- **Its seeding window is inverted for the first week of every period**, exactly
  as `PerMonthOTHrs`'s is: `shiftPeriod` runs `[payPeriodStart,
  workWeek.getStartDate() - 1]`, which is backwards whenever the week starts on
  or before the period. The card path's filters then match nothing. Fourth
  inverted-range case in the family. The Java spec's earnings case **depends** on
  it — its shift loop contributes nothing and the earning is the whole seed.
  Pinned by `the_seeding_window_is_inverted_for_a_week_starting_the_period`.

- **A worked holiday is paid whole at double time, and still counts toward the
  contract.** The branch moves every hour of the row to double time and zeroes
  the row, consulting no limit — but first adds the original hours to the period
  total and the paid hours to the period's overtime. So a holiday both fills the
  contract and is paid on top of it, which is not what the spec case's title
  ("do not count toward the contract hours") says; its own asserted period total
  of 17 includes the holiday's five.

  The branch is guarded on `hours > 0`, so a row already at zero falls through
  to the overtime branch instead — a second pass over a calculated holiday
  behaves differently from the first. Pinned.

- **The holiday calendar is the *shift's* property here.**
  `shift.getJob().getProperty()`, where `HolidayDTHrs` reads
  `distribution.getPropertyID()`. Two rules in one family, two answers for a
  multi-property employee — and this file already records the `HolidayDTHrs`
  side as deliberate.

- **`isEligibleForOT` resolves the home job before testing `homeDeptOnly`**, so
  an employee with no home job status on the shift's date throws even when the
  flag is off and the home job is irrelevant. See divergence 46.

- **Its distribution sort is ascending where `TwentyFourHourOT`'s is
  descending.** Both sort the shift's **live** list in place; this one ascending
  by date, which is what makes a midnight-spanning shift fill its contract from
  the earlier day first. Third in-place sort in the family. The shift list
  itself is sorted on a copy.

- **Three of the six spec cases assert almost nothing, and two of them assert
  something false.** Their `any {}` predicates are four bare comparisons on
  consecutive lines with no `&&`, so only the last is the closure's return value
  — the same shape as `RollingXWeeksOTHrs`'s, and the sixth spec-weakness of
  this kind in the family. Two of the discarded lines assert
  `originalHours == 10` on a row they also call overtime, and a factory-built
  premium row carries **zero** original hours. Joined with `&&` those cases
  would fail. The transcriptions assert the whole row, with the value the
  factory actually writes.

- **`TwentyFourHourOT` measures against a rolling 24-hour work day, not a
  calendar day.** The window opens at the employee's first clock-in on a date
  and runs twenty-four hours, so a shift early the next morning can fall inside
  yesterday's window and push it over the limit. It is the only rule in the
  family whose day is not a date, and the only one that needs
  `ShiftUtil.getNetHoursInRange` — punch-pair intervals intersected with a
  window — where every sibling reads hours off distributions.

- **A spanning shift is visited twice and paid on the second visit.**
  `getShiftsForWorkDay` walks the work day's own date and then appends the
  **next** date's shifts that start strictly inside the window. Those carry
  `currentDayShift == false`: they bank overtime into `otAccumulator` and write
  nothing. The following work day reaches the same shift with the flag true,
  adds to what was banked, and writes the total. That is why the accumulator is
  keyed by shift and survives across days, and why `weeklyHrsAccumulated` only
  ever advances on the current-day visit — otherwise the shift would count
  toward the week twice.

- **The two "already accounted for" counters are updated asymmetrically.**
  `workDayOTAccountedFor` advances inside the work-day helper, as soon as the
  figure is computed; `weeklyOTAccountedFor` advances in the caller, and by the
  **capped** `min(otAccumulator, shift.getNetHours())` rather than by the weekly
  figure. So weekly overtime is only booked as paid to the extent a shift could
  actually absorb it, while work-day overtime is booked whether or not it lands.

- **The rate gate switches off work-day overtime only.** `overRateThreshold`
  compares the FLSA regular rate for the week, or the employee's last home job
  status rate, against `rateThreshold` — whose default is 99,999, so the gate is
  off unless a property turns it on. When on, the work-day measure returns zero
  and the weekly measure is untouched: a highly paid employee still earns
  overtime past forty hours. Both spec rows of the case named for this gate have
  it **off** — one because 8.50 is not over 8.0, the other because the mocked
  `FlsaData` answers zero — so the branch the case is named for is never taken.

- **Adjustment-only shifts count for the week and not for the day.**
  `isShiftAdjustmentOnly` is `!hasErrors() && !hasBothTimes()`, so a shift with
  no punched times is excluded from the work-day measure while its distributions
  still feed the week. Four spec cases turn on it. They also sort to **midnight**
  through `ShiftStartTimeComparator`, ahead of anything with a start time, which
  a fifth case is named for.

- **`createAndDistributeOT` sorts the shift's own distribution list in place.**
  `Collections.sort(shift.getHoursDistributions(), reverseOrder(comparing(getDate)))`
  reorders the **live** list, latest date first, whether or not any overtime is
  paid. Second case of this in the family after `RollingXWeeksOTHrs`'s, and the
  reordering is load-bearing here rather than incidental: overtime is then spent
  against the latest date first, which is what makes a midnight-spanning shift
  pay out of its second day before its first. Pinned by
  `the_shifts_own_distribution_list_is_left_sorted_latest_date_first`.

- **`Optional.of(...).filter(shiftsAreNotEmpty)` can never fail.**
  `getShiftsGroupedByDate` builds one entry per date of the work week, empty
  list or not, so the map is non-empty for any non-empty range however few
  shifts the card holds. The guard is dead and is not ported. The same shape
  makes `endDateIsDifferentAndHasShifts` — which tests the map's **key set** —
  really ask only whether the next day is still inside the work week, and its
  other half, `!start.equals(end)`, is never false for two datetimes
  twenty-four hours apart.

- **`ShiftUtil.getWorkedDateTimeRangesFromShift` pairs punches positionally and
  sorts the shift's live punch list to do it.** Indices 0 and 1, 2 and 3, and so
  on in `PunchTimeComparator` order — not by punch type, exactly as `getBreaks`
  does it, which is how a break splits one shift into two worked intervals. An
  odd number of punches throws; see divergence 45.

- **`TwentyFourHourOTRuleImplTest` is the family's largest and best spec** — 27
  methods, four of them tabled, 31 executions, and the only one that pins
  arithmetic no amount of reading the source settles. All transcribed, all
  passing on the first run. Its config has a spec too,
  `TwentyFourHourOTRuleConfigTest`, the first in the family; its
  `testGetDefaultValues` and six `testValidateProperties` rows are transcribed
  beside the other configs, and its `testGetProperties` case is the l2fprod UI
  half that `config.rs` does not port.

- **`A shift without punches should not get daily OT` cannot reach its gate.**
  The shift is dated the day before the work week, so `getShiftsForPeriod` drops
  it before anything looks at the punches. Fifth case of this kind in the family,
  after the two in `ScheduledShiftOTRuleImplTest` and the two weak
  `CaliforniaOTHrs` ones. Transcribed as written, with a second assertion beside
  it that puts the punchless shift inside the week so the case exercises what it
  claims to.

- **`CaliforniaOTHrs` cannot pay weekly overtime under its own defaults.** It
  never calls `weeklyAccumulator.setOriginalHours(...)`, where the extended
  sibling calls it before every `computeWeeklyOT`. So `originalHours` stays at
  zero, and the formula `premiumHoursCountTowardsWeeklyOT` selects —
  `min(hours - weeklyLimit, originalHours)` — can only return zero. That flag's
  default is `"true"`. A property gets weekly overtime out of this rule only by
  setting it `false`, which switches to `hours - weeklyLimit - weeklyOT`. The
  Groovy agrees: its one weekly case sets the flag explicitly and would assert
  nothing without it. Pinned both ways by
  `the_weekly_limit_pays_nothing_under_the_default_formula`. Daily overtime and
  double time are unaffected.

  This is the same accumulator and the same flag as divergence-free
  `CaliforniaExtendedOTHrs`, which *does* call the setter — so the two
  California rules read the identical parameter and get opposite behaviour from
  it. Neither the config nor the spec says so.

- **Its consecutive-day limits are `final` fields, not parameters, and prior
  weeks are not seeded.** `consecDaysLimit = 7` and `maxConsecDays = 36500` are
  hardcoded, so `CaliforniaOTHrsRuleConfig` declares no key for either — where
  the extended config exposes both. And it calls
  `updateConsecutiveDays(DailyData)`, the middle overload, which resets on an
  unworked day and increments otherwise but **never wraps at the modifier**; so
  `maxConsecDays` only ever feeds a modifier nothing consults. There is no
  `PriorDaysCalculator` call either, so the counter starts at zero on every
  `execute` and the seventh-day rule fires only for seven days worked inside the
  work week. Pinned by `prior_weeks_do_not_seed_the_consecutive_day_counter`.

- **The earnings half of a rule is opt-in, and silent by default.** Only
  earnings whose type appears in the `earningTypePaySet` parameter are
  considered, and `HoursDistributionRuleWithPayMappingsConfig` defaults that to
  a pay set with **no mappings at all** — only `premiumLevels`. So a property
  that has not configured one gets the distribution half of `CaliforniaOTHrs`
  and none of the earning half, with nothing anywhere reporting it. The Groovy
  supplies a pay set in every case, so the default path is untested in Java.

- **A premium earning is paid by adding three rows, not by editing one.** The
  source earning is left exactly as it was; the rule writes a **negative earning
  at the original type** cancelling the premium hours, then the premium hours
  back at the overtime and double-time types. The same instinct as
  `HolidayDTHrs` zeroing its regular row rather than removing it — the reader
  can still see where the hours came from — and the same carve-out arithmetic:
  the double-time row takes `shiftDT` and the overtime row the remainder.

- **An earning reaches the arithmetic through exactly one of two paths.**
  `dailyDataProducer` partitions the week's earnings on `getShift() != null`:
  the ones naming a shift are processed with that shift, the rest as the day's
  standalone earnings. That partition is the whole content of the spec case
  `an earning is not processed twice`. Note the consequence for an overnight
  shift, which appears under every date it distributes into: **its earnings are
  processed once per day it spans**, since they hang off the shift rather than
  off a date.

- **`getPremiumEarningTypeID` returns `0` for an unmapped regular type**, and
  the Java caller hands that straight to `earningTypeDAO.findByID(0)`, which
  finds nothing — so the premium earning is created against **no earning type at
  all** rather than not created. Reproduced. Its other failure throws; see
  divergence 44.

- **`payLevelMap` is assigned in `initParameters` and never read.** The third
  dead field in the family, after `setBothConsecutiveAndWeeklyOt`'s and
  `PayPeriodOTHrsRuleImpl`'s unused `EmployeeShiftDAO`. Ported as
  `EarningTypePaySet::pay_level_map` because it is the type's API rather than
  the rule's, and noted as dead on both.

- **`EmployeeEarning`'s setters recompute `totalDollars`, and the port was not
  doing it.** `setHours`, `setRate` and `setDollars` each end in
  `calcAndSetTotalDollars()` —
  `roundDisplayCurrency(roundDisplayCurrency(hours * rate) + dollars)`, with the
  hourly part taken to cents *before* the flat dollars are added. The same shape
  as `HoursDistribution.setHours` recomputing `totalCosts`, which this file
  already recorded as settled; the earning entity had been ported before
  anything wrote to it, so the callback was missing. Fixed and pinned.

- **`CaliforniaOTHrsRuleImplSpec` is weaker than it looks in two of its six
  cases.** `spanning shifts hours in the previous period are not counted to the
  ot threshold` and `an earning is not processed twice` assert only
  `hoursDistributions.size()` and `earnings.size()`. The first is load-bearing
  in a way its author may not have intended: its week totals **exactly** forty
  hours against a forty-hour limit, so it passes only because
  `hours - weeklyLimit` is strict. Both transcriptions assert the hours beside
  the counts. Fourth spec-weakness of this kind in the family. The file is also
  named `…Test.groovy` while the class inside it is `…Spec`.

- **`HolidayDTHrs` zeroes the regular row rather than removing it.** The shift
  ends up holding both — a zero-hour regular distribution and a full double-time
  one — so the reader can still see where the hours came from. It also makes the
  rule idempotent in the way that matters: a second pass finds the regular row
  at zero and creates an empty double-time row rather than doubling the pay.

- **The holiday calendar is looked up per *distribution* property, not per
  shift.** `distribution.getPropertyID()` is the property owning the job the
  hours were worked at, which for a multi-property employee need not be the
  employee's own or the shift's. Pinned by
  `the_holidays_consulted_are_the_distributions_property_not_the_shifts`.

- **Java throws when a property has two holidays on one date.** The cache is
  built with `toMap(Holiday::getHolidayDate, holidayTypeIDMapper)`, whose
  collector rejects duplicate keys — a configuration the DAO can return and the
  collector cannot survive. See divergence 35.

- **`getIdList` swallows malformed JSON and returns an empty list**, so a
  corrupt multi-select parameter silently configures the rule with nothing
  selected rather than aborting. The opposite of the choice `int_at` makes for a
  malformed number (divergence 9); both are faithful to their Java. And the
  `catch` sits outside the element loop, so **one** bad element discards the
  whole list, not just itself.

- **`PerMonthOTHrs` is the first `ContractHrsRuleImpl`** — the marker interface
  `RuleUtils.getContractHours` reaches for by `instanceof`, so the engine can
  price a contract without knowing which rule defines it. Ported as a
  `ContractHrsRule` supertrait of `HoursDistributionRule`.

- **A mid-month hire is pro-rated by *calendar* days, and the daily rate is
  rounded before it is multiplied back up.** `roundHours(monthlyContract / daysInMonth)`
  then `roundHours(hoursPerDay * daysRemaining)` — so a 160-hour January gives
  5.16 hours a day and a hire on the 16th gets **82.56**, not the 82.58 an
  unrounded rate would produce. Pinned both ways by
  `a_mid_month_hire_is_pro_rated_by_calendar_days`.

- **Its month-seeding window is routinely inverted.** The card path sums shifts
  over `[firstOfMonth, workWeek.getStartDate() - 1]`, which runs backwards for
  any week starting on or before the first of the month — most first-week
  calculations — so the seeding silently contributes nothing. The Java spec's
  last case depends on it. Pinned by
  `the_seeding_window_is_inverted_for_a_week_starting_the_month`. Third
  inverted-range case in the family, after `ConsecutiveDaysCalculator`'s and
  `PriorDaysCalculator`'s.

- **It has its own JSON id parser with a different failure mode from the shared
  one.** `getEarningTypeIdsList` inlines the parse rather than calling
  `JSONUtils.getIdsListForKey`, and declares its list **outside** the `try` — so
  a malformed element leaves the partially built list intact where `JSONUtils`
  discards the whole thing. `[3,4,{},5]` yields `[3, 4]` here and `[]` there.
  Pinned by `the_rules_own_json_parser_keeps_what_it_read_before_a_bad_element`,
  which asserts both spellings side by side.

- **Only the port path folds earnings in, and only in TA mode.** The card path
  reads the card's earnings regardless of calculation mode. Two seeding paths,
  two policies, in one method pair.

- **`DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl` shadows *both* shared
  accumulators with private inner classes of the same names.** The second
  shadowing case in the family after `CaliforniaExtSpecialJobOTHrsRuleImpl`'s
  inner `DailyData`, and the more dangerous one, because the constructors take
  the same number of arguments with different meanings:

  | | the shared class | this rule's inner class |
  |---|---|---|
  | `WeeklyAccumulator(a, b, c)` | `(weeklyLimit, consecDayLimit, maxConsecDays)` | `(weeklyLimit, workedDayOTLimit, workedDayDTLimit)` |
  | day counter | resets on an unworked day, wraps at a modifier | **only ever increments** |
  | `computeWeeklyOT` | two formulas behind a flag | one formula |
  | `DailyAccumulator(…)` | `(date, dailyOTLimit, dailyDTLimit)` | `(date, dailyOTLimit)` |
  | daily double time | `computeDailyDoubleTime` | none; its `doubleTime` field is never written or read |

  Mistaking one for the other compiles in Java and silently moves which day the
  limits change on. Ported private to the rule's module under names
  (`WorkedDayAccumulator`, `DailyHours`) that cannot be confused with the
  shared ones.

- **The non-consecutive counter never resets**, which is the rule's whole
  point: six worked days across a week with gaps still reaches the sixth-day
  rule. Pinned by `the_worked_day_counter_never_resets`.

- **Double time is a bucket choice, not an amount.** One `premiumHours` figure
  is computed and `isDuringDTConsecDaysRange()` decides which bucket takes
  *all* of it — so on the seventh worked day the overtime bucket gets nothing,
  where `CaliforniaExtendedOTHrs` splits the same total between the two.

- **The daily limit is suspended rather than lowered** once either worked-day
  range is active: from the sixth worked day the first hour is already
  overtime.

- **`RollingXWeeksOTHrs` counts prior weeks two different ways depending on one
  date.** `needMoreData` compares the window's start against
  `getDatasetStartDate()`: inside the dataset the totals are summed off the
  card, outside it they come from the DAO — and the two disagree.

  | | off the card | from the DAO |
  |---|---|---|
  | net hours | `originalHours` of every **regular bucket** | `originalHours` where `HoursDistributionTypeID = 1` |
  | overtime | `hours` of every **non-regular** bucket, double time included | `hours` where `HoursDistributionTypeID = 2` |

  A property with a second non-premium bucket, or any double time in the
  window, gets different answers on either side of that boundary. Both paths
  are reproduced as written and the card path is pinned by
  `the_card_path_counts_every_non_regular_bucket_as_overtime`. The DAO also
  selects a third `dtHours` column that the rule never reads.

- **`RollingXWeeksOTHrsRuleImpl` sorts the shift's own distribution list in
  place.** `sortHoursDistributions` calls
  `Collections.sort(shift.getHoursDistributions(), …)` on the live list, not a
  copy, so the shift is left reordered by date whether or not any overtime was
  paid — visible to any later rule that reads "the first regular distribution",
  as `PayPeriodOTHrs` does. Reproduced; pinned by
  `the_shifts_own_distribution_list_is_left_sorted_by_date`.

- **It is the one unrounded write in the family.**
  `hoursDistribution.setHours(hoursDistribution.getHours() - shiftRollingOT)`,
  where every sibling wraps the same expression in `TDouble.roundHours`. The
  overtime being subtracted is itself rounded, so the result is usually clean.

- **`RollingXWeeksOTHrsRuleImplTest`'s `any {}` blocks assert almost nothing.**
  Each is four bare comparisons on consecutive lines, and only the **last** is
  the closure's return value — so the predicate reduces to "some distribution
  is of the overtime type", with the hours, date and original hours evaluated
  and discarded. The transcriptions assert the whole row instead. That is the
  third spec-weakness of this kind in the family, after the one-sided tolerance
  in `CaliforniaExtendedOTHrsRuleImplTest` and the never-reached branch in
  `PayPeriodOTHrsRuleImplTest`.

- **`CaliforniaExtendedOTHrsRuleImplTest` is by far the strongest spec in the
  family** — twelve cases asserting net, regular, overtime and double-time
  hours on every shift, about a hundred assertions. All transcribed, all
  passing on the first run, which is the first end-to-end confirmation that the
  two accumulators and `PriorDaysCalculator` are right.

  Its `hasNetRegOTDT` helper checks `(actual - expected) <= 0.01`, which is
  **one-sided**: an actual far *below* the expected value passes. The
  transcriptions assert equality instead — strictly stronger, and they still
  pass, so a rounding divergence in either direction would surface here where
  it would not in Java.

- **`CaliforniaExtendedOTHrsRuleConfig` puts `premiumHoursCountTowardsWeeklyOT`
  into its defaults twice — `"false"` then `"true"`.** The later `put` wins, so
  the effective default is `true` while a reader scanning the list top-down sees
  `false`. `testWeekly45`'s numbers only come out under the formula `true`
  selects, so this is load-bearing rather than cosmetic. Pinned by
  `the_premium_formula_is_the_default_despite_the_earlier_put`.

- **`shiftOT` is a maximum of the two overtime measures, and double time is
  carved out of it.** `max(computeWeeklyOT(), computeDailyOvertime())` — an hour
  that is both daily and weekly overtime is paid once — and then the
  double-time row takes `shiftDT` out of that same total while the overtime row
  gets `shiftOT - shiftDT`. The regular row loses `shiftOT`, not the sum.

- **`setOriginalHours` is called per distribution**, immediately before
  `computeWeeklyOT`, so the premium formula's cap is *that* distribution's
  original hours. Without it a week's whole excess would land on one row.

- **The accumulators advance even when nothing is written.** The
  open-for-editing and `premiumHours > 0` guards wrap only `addDistributions`;
  the three `add*` calls after it are unguarded. A distribution in a closed
  period therefore counts toward the week and has its overtime booked as
  already paid — which is precisely what the spec case
  `ot in prior pay period is not double counted` is asserting.

- **An overnight shift is processed once per day it spans.** The daily shift map
  is built from `getShiftsWithDistributionsForPeriod(ArbitraryDateRange.of(date))`,
  so a shift with distributions on two days appears under both — and advances
  the consecutive-day counter on both.

- **The spec case named for `BOTH_CONSECUTIVE_AND_WEEKLY_OT` does not test it.**
  The flag reaches `WeeklyAccumulator.setBothConsecutiveAndWeeklyOt`, whose
  field nothing reads; the case's numbers follow from its *other* parameter,
  `CONSEC_DAYS_IN_WEEK`. Confirmed directly by
  `both_consec_and_weekly_ot_changes_nothing`, which runs the default fixture
  both ways and gets identical output. That closes out the finding first noted
  when the accumulator was ported.

- **`ScheduledShiftOTRuleImpl` is the first rule that has to tell the two time
  card implementations apart**, and it does it with
  `timeCard instanceof ScheduleCalcDataSet`. That is a direct challenge to
  divergence 23's "one struct covers both"; see divergence 41 for what it
  becomes and why the translation is not free.

- **It pays the gap between worked and scheduled hours with no limit involved.**
  Every other overtime rule in the family measures against a configured number
  of hours; this one measures an actual shift against the schedule that starts
  nearest it and pays the difference. It is also the only rule so far that reads
  `getSchedules()`.

- **The default threshold is zero**, so out of the box only a schedule starting
  at the *same instant* as the shift matches. The window is inclusive at both
  ends (`containsDateTimeInclusiveOfEndDateTime`), and ties on distance resolve
  to whichever schedule the card lists first, because `sorted().findFirst()` is
  stable. All three are pinned.

- **Overtime is charged to the latest day first**, and unlike
  `WeeklyOTSecJobHrs` a closed distribution is skipped **before** the budget is
  touched — so the hours land on an open day instead of vanishing. Two rules,
  two `continue`s, opposite consequences.

- **Two cases of `ScheduledShiftOTRuleImplTest` never reach the gate they are
  named for.** `shifts without both times` and `shifts that is in error` build
  their shift with no `shiftDate`, so `getShiftsForPeriod` drops them at
  `period.containsDate(null)` before `hasBothTimes()` or `hasErrors()` is
  consulted. `shiftDate` is not nullable in the port, so each transcription
  gives the shift a date inside the period — which makes the case exercise what
  it claims to. The assertions are unchanged.

- **`PayPeriodOTHrsRuleImplTest` never reaches the code it tests.** Every
  fixture builds shifts with distributions and **no punches**, and the rule
  accumulates `shift.getWorkedHours()` — which comes from punches. So the
  period total is zero in all eight cases and `convertHoursDistributionToOT` is
  never called. Even `should handle zero OT limit` misses, because the test is
  `accumulatedWorkedHours > periodOTLimit` and `0.0 > 0.0` is false. Six of the
  eight cases assert only `noExceptionThrown()` and `size() >= 1`. They are
  transcribed as a no-op pin; the arithmetic is covered by tests written here
  instead.

- **`PayPeriodOTHrs` accumulates worked hours, not distribution hours.** The
  two weekly rules sum `HoursDistribution.getOriginalHours()`; this one sums
  the shift's punch-derived `getWorkedHours()`. The two are independent, and a
  shift with distributions but no punches contributes nothing however many
  hours those distributions claim. Pinned by
  `a_shift_with_no_punches_contributes_no_worked_hours`.

- **It ignores the work week it is handed**, using it only to locate the pay
  period, and then rescopes everything to that period. So it recalculates the
  whole period once per week of it — which is why it *assigns* the overtime row
  (`setHours(totalOTHours)`) instead of adding to it, and why it rewrites the
  regular rows from `originalHours` instead of decrementing `hours`. **That
  makes it the one rule in the family so far that is idempotent.** Pinned by
  `running_it_twice_changes_nothing`.

- **Its absorption loop has two quirks, both kept.** `otHoursToAdd` is reduced
  by each row's *whole* `originalHours` rather than by what that row absorbed,
  so a first row larger than the budget drives it negative and every later row
  is skipped — left where it was rather than restored. And the loop skips only
  the **overtime** bucket, so a double-time row it reaches while the budget is
  still non-negative is rewritten from its own original hours, which for a
  factory-built premium row is zero. Whether that happens depends on where the
  row sits in the list; both orders are pinned.

- **`PayPeriodOTHrsRuleImpl` injects an `EmployeeShiftDAO` and never calls it.**
  Nothing to port.

- **`WeeklyOTHrs` and `WeeklyOTSecJobHrs` are the same job approached from
  opposite ends, and differ in three ways that are not cosmetic.**

  | | `WeeklyOTHrs` | `WeeklyOTSecJobHrs` |
  |---|---|---|
  | order | shifts earliest first, distributions earliest date first | secondary job first, then latest first throughout |
  | overtime already paid | seeded from **earnings** of configured types | always starts at zero |
  | a second OT row on one date | **merged** into the existing one | always appended |

  The second row of that table is the one to watch: `WeeklyOTHrs` lets a
  property mark hourly earning types as overtime already paid, and those hours
  are counted against the week before any shift is. Nothing in the sibling does.

- **The `hoursdistribution` family's rules are "run once per week", as a
  class.** Both weekly rules reset their accumulators at the top of `execute`
  and measure `originalHours` while writing `hours`; premium rows carry zero
  original hours, so a second pass recomputes the same overtime and subtracts
  it again from already-reduced hours. `WeeklyOTHrs` hides it slightly by
  merging into the existing overtime row rather than leaving a duplicate, but
  the regular hours still fall. Both are pinned. This is the opposite of
  `punchrounding`, which resets every punch before rounding and is idempotent
  by construction.

- **`WeeklyOTHrsRuleImplSpec`'s 7(i) case does not test 7(i).** It builds its
  rule item with `CHECK_7I` alone, so `weeklyOtLimit` takes its default of 40
  while the fixture holds twenty hours — the week produces no overtime whether
  or not the exemption fires. Transcribed as written, with a second assertion
  beside it that sets a limit the week actually crosses.

- **`WeeklyOTSecJobHrsRuleImpl` is not idempotent, and running it twice drives
  regular hours negative.** It measures `getOriginalHours()` and writes
  `getHours()`:

  ```java
  double distributionOT = Math.min(distribution.getOriginalHours(), totalOT);
  distribution.setHours(roundHours(distribution.getHours() - distributionOT));
  ```

  Premium rows carry zero original hours, so a second `execute` over an already
  calculated week measures the same total, computes the same overtime, and
  subtracts it again from hours that were already reduced. Eight regular hours
  cut to two become **minus four**, with a second premium row beside the first.
  Nothing resets `hours` from `originalHours` first, the way the punch-rounding
  runner resets every punch before rounding. I had written the opposite in the
  module doc before a test caught it; pinned now by
  `a_second_pass_drives_the_regular_hours_negative`, so making the rule
  idempotent later is a visible change rather than a silent one.

- **The secondary-job ordering is the whole rule.** The week's excess is taken
  out of secondary-job shifts before home-job ones, latest shift first, and
  within a shift latest distribution date first — so an employee's home-job pay
  stays whole. `WeeklyOTHrsRuleImpl` is the same arithmetic without it. The
  order survives Java's `partitioningBy`, which preserves encounter order
  within each half, and `ShiftStartTimeComparator` calls two shifts on the same
  date with no start time **equal**, so ties fall back to the order the time
  card holds them in. Both properties needed a stable sort to reproduce.

- **`ShiftStartTimeComparator` falls back to midnight on the shift date** when
  a shift has no start time — which is every shift in the two ported specs, so
  the tie-breaking above is the normal case rather than an edge one.

- **`distributeOT` spends the budget even when it cannot pay.** The
  open-for-editing guard wraps only the write; `totalOT` is reduced outside it.
  So a distribution in a closed pay period consumes overtime that then cannot
  be paid anywhere else. Pinned by
  `a_distribution_in_a_closed_period_is_left_alone_but_still_spends_the_budget`.

- **`WeeklyOTSecJobHrsRuleImpl` null-checks the job status; the two helpers do
  not.** `nonSalariedExemptShift()` reads
  `employeeJobStatus != null && employeeJobStatus.getPayType().isNotSalariedExempt()`,
  where `ConsecutiveDaysCalculator` and `EarningMapper` dereference the same
  expression unguarded. Divergence 32 chose this rule's behaviour for all
  three, so the three now agree — and `TimeCard::shift_is_not_salaried_exempt`
  spells it once.

- **`RegHrsOnlyRuleImpl.execute` really is empty.** Regular hours are already
  distributed by the calc pipeline before any rule in this family runs; the rule
  is what a property configures when it wants none of them moved. It exists as a
  catalogue entry so a runner has something to fall back to.

### Deliberate divergences (continued)

33. **`HolidayDTHrs`'s holiday cache is per `execute`, not per request.** Java's
    `holidays` field sits on a `@Scope("request")` bean and survives across the
    work weeks of one calculation; here it is built inside each `execute` for
    the properties that week's distributions name. Same DAO answers, same
    result — divergence 15's case again — and it keeps `execute` taking `&self`
    instead of needing interior mutability for a cache nothing reads.

34. **`createSplitDistributions` and `createDistributionOfConfiguredType` are
    not ported.** They need `BreaksAndAdjustmentsCalculator` and
    `SingleDistributionTypeRuleConfig`, which no ported rule touches. They
    arrive with the day-split families.

35. **Two holidays on one date take the last one read rather than throwing.**
    Java's `toMap` collector rejects the duplicate key and the calculation dies.
    Neither behaviour is specified by anything, and an arbitrary pick is better
    than a panic inside a rule engine. Pinned by
    `two_holidays_on_one_date_do_not_panic`.

36. **`EmployeeShift.getEmployeeJobStatus()` becomes a `TimeCard` method.**
    Java's is a `@Transient` getter on the shift —
    `getEmployee().getEmployeeJobStatus(getJob(), getShiftDate())` — reachable
    because a shift back-references its employee. The entity model here is
    one-way, so the question is asked of the card, which holds the employee.
    `TimeCard::shift_is_not_salaried_exempt` wraps the filter three call sites
    were each spelling out inline.

37. **A rule that finds no `"Overtime"` bucket returns without paying.**
    `WeeklyOTSecJobHrsRuleImpl` passes `timeCard.getOTHoursDistributionTypeId()`
    straight into the factory, unboxing a possibly-null `Integer`; a property
    that renamed the bucket throws there. There is no bucket to put the hours
    in under any reading, so the rule pays nothing. Same call divergence 20 and
    32 made.

38. **Minimum wage is a port, not an entity walk.** `Employee.getMinWage(Assignment, LocalDate)`
    resolves through `Assignment.getEffectiveMinWage` — the latest
    `AssignmentPayRate` effective on or before the date, falling back up the
    parent-assignment chain — and then, when that is zero, through
    `Property.getMinWage()` to `State.getMinWage()`. Four entities, none of
    which any ported rule touches for anything else, to produce one scalar.
    `MinWagePort` takes the property and job **ids**, exactly as divergence 18
    already keyed `PropertyDataPort`: a caller has navigated the chain and
    wants the number off the end of it.

    **When to revisit:** if a second rule needs scalars off `Property` or
    `AssignmentPayRate`, port the chain rather than adding a second port. One
    caller so far.

39. **A missing FLSA row means the 7(i) exemption is not established.** Java
    reads `getFlsaDataMap().get(workWeek.getEndDate())` and dereferences it, so
    a week the calc pipeline did not fill throws. The exemption is something to
    be *shown*, and an absent row shows nothing, so the week is calculated
    normally. Same for an employee with no home job status in the week, which
    Java also dereferences unguarded.

40. **The property's current pay period is a `TimeCard` accessor.**
    `PayPeriodOTHrsRuleImpl` reaches it as
    `employee.getProperty().getPayPeriod()`, which is
    `getPayGroup().currentPayPeriod()`. `PayGroup` is not ported and the card is
    already the stand-in for what it knows (divergence 24), so
    `TimeCard::current_pay_period` holds the range and
    `pay_period_containing(date)` walks from it — `DateRange::range_containing_date`
    is `DefaultDateRange.getDateRangeContainingDate` exactly.

41. **`instanceof ScheduleCalcDataSet` becomes
    `TimeCard::is_run_from_scheduling()`.** `ScheduledShiftOTRuleImpl` returns
    immediately when the card is a `ScheduleCalcDataSet`, which divergence 23
    cannot express — there is one struct. The stand-in is the calculation mode:
    `matches!(mode, AutoSchedule | EditSchedule)`, which is exactly how
    `ConsecutiveDaysCalculator.isRunFromScheduling` asks the same question, so
    the engine already treats the mode as the discriminator elsewhere.

    **It is not provably the same test.** `ScheduleCalcDataSet.calculationMode`
    is a settable field, so a schedule data set carrying `TA` would take the
    `instanceof` branch and not this one. Nothing in the tree constructs that,
    and a real one always carries a scheduling mode — but the two tests are
    equivalent by convention rather than by construction.

    **When to revisit:** if a second rule needs the distinction, or if anything
    turns out to build a `ScheduleCalcDataSet` in `TA` mode, split the struct
    in two rather than widening this.

42. **A rule-written earning's note is the rule item's name alone.** Java writes
    `ruleItem.getRuleSet().getName() + " - " + ruleItem.getName()`. `RuleItem`
    here carries only `rule_set_id` — `RuleSet` owns its items and not the
    reverse — so the set's name is not reachable from the rule. Nothing in the
    rules tree reads a note back; it is display text on a pay register.

    Rejected: denormalizing the set name onto `RuleItem`, which changes a
    constructor every ported rule's tests already call, and widening
    `HoursDistributionRule::execute` to take the owning `RuleSet`, which changes
    the trait for all 19 rules to carry one string.

    **When to revisit:** if anything parses a note, or if a second write site
    needs the set name, put the name on `RuleItem` rather than spreading the
    divergence.

43. **`earningTypeDAO.findByID` is not called when creating an earning.** Java
    looks the `EarningType` entity up purely to hand it to
    `newEarning.setEarningType(...)`. `EmployeeEarning` here holds
    `earning_type_id: i32`, so the id that
    `EarningTypePaySet::premium_earning_type_id` already returned is written
    straight through and the round trip has no work to do. `EarningTypePort`
    stays defined for the callers that want the entity.

    This is why `CaliforniaOTHrsRule` takes one port where Java autowires two.

44. **A premium level past the configured list is `0`, not a throw.**
    `getPremiumEarningTypeID` indexes
    `payMap.getPremiumEarningTypeIDs().get(premiumLevel)` directly, so a pay map
    that configures overtime and not double time throws
    `IndexOutOfBoundsException` the first time anybody works past the
    double-time limit — a live configuration failing on some days and not
    others. Returning `0` makes it behave like the unmapped-type case Java
    already returns `0` for, which is divergence 20, 32 and 37's call again.
    Pinned by `a_level_past_the_configured_premiums_is_zero_not_a_panic`.

    Malformed pay-set JSON is the opposite call and **does** panic, matching
    `RuleParams::int_at` (divergence 9): that is a corrupt parameter rather than
    a coherent one with a gap in it. Both spellings are documented on the type.

45. **An odd number of punches drops the unpaired one rather than throwing.**
    `ShiftUtil.getWorkedDateTimeRangesFromShift` steps its loop by two and reads
    `get(i + 1)` unguarded, so a shift that is still clocked in throws
    `IndexOutOfBoundsException` out of `TwentyFourHourOT`. The trailing punch is
    dropped here, so such a shift contributes the time it has closed. Divergences
    20, 32 and 37's reading again.

    `ShiftUtil.getBreaks` has the same positional pairing and guards it by
    refusing shifts with three punches or fewer, which is why that one was
    portable as written; this entry point has no such guard.

46. **The home job is resolved only when `homeDeptOnly` is set.**
    `ContractOTHrsRuleImpl.isEligibleForOT` assigns
    `employee.getHomeEmployeeJobStatus(shiftDate).getJob()` on its own line,
    before the `!homeDeptOnly ||` short-circuit that is the only thing reading
    it — so an employee with no home job status on that date throws even when
    the flag is off. The lookup is made conditional here. That is divergence
    39's reading: an absent record establishes nothing, so with the flag off the
    shift is calculated normally, and with it on the department match cannot be
    shown and the shift is excluded.

47. **Pay period accumulators are seeded up front rather than lazily.** Java
    creates each one the first time a distribution in that period is reached,
    interleaving seeding with writing. Seeding reads only earnings — which this
    rule never writes — and distributions dated strictly **before** the work
    week, while every write is guarded on `workWeek.containsDate`. The two
    windows are disjoint, so hoisting is behaviour-identical, and it lets
    `execute` hold `&mut` for the write loop without reading the card back
    through it. `setContractHours` runs on every Java call rather than only on
    creation, and is the same value each time for a given period.

48. **A weighted limit over zero hours is `0.0`, not `NaN`.**
    `CaliforniaExtSpecialJobOTHrsRuleImpl`'s `HourLimits` divides
    `totalLimits / totalHours` unguarded, so a day or a week with no hours
    yields `NaN` in Java. Nothing is written either way — every comparison
    against `NaN` is false, and a day with no hours has no distributions to walk
    — so the two agree on the only reachable path. `0.0` is a value a reader can
    follow, and it keeps a `NaN` from escaping into an accumulator if the helper
    is ever called from somewhere new.

49. **One `dailyDataProducer`, not two.** `CaliforniaOTHrsRuleImpl` and
    `DailyWeekly7thDTHrsRuleImpl` each declare the producer as a private field,
    and the two are byte-identical — the same earning filter, the same
    partition on whether an earning names a shift, the same
    `ShiftStartTimeComparator` ordering. Ported once as
    `daily_data::build_daily_data_map` and called from both, so there is no
    second place for them to drift.

    `CaliforniaExtSpecialJobOTHrsRuleImpl` and
    `DlyWklyOffConsecOTMinBreakRuleImpl` each build a **different** shape and
    keep their own; only these two share.

50. **The schedule list is read for emptiness, not for its contents.**
    `DlyWklyOffConsecOTMinBreakRuleImpl`'s `DailyData` holds the day's scheduled
    shifts, and the only question anything asks of them is
    `scheduledShifts.isEmpty()` — `shouldPayUnscheduledOTOnDay`. So the port
    keeps a `bool` rather than a list of positions, which sidesteps the question
    of whether an index means a position in `shifts()` or in `schedules()`.

    **When to revisit:** if a rule ever needs the scheduled shifts themselves,
    give `TimeCard` a `schedule_indices_with_distributions_for_period` beside
    the shift one rather than widening this.

## The shared helpers — done

| Rust | Java | Used by |
|---|---|---|
| `algorithm/hoursdistribution/daily_accumulator.rs` | `DailyAccumulator` | 8 rules |
| `…/weekly_accumulator.rs` | `WeeklyAccumulator` | 9 rules |
| `…/daily_data.rs` | `DailyData` | `CaliforniaOTHrsRuleImpl` |
| `…/earning_mapper.rs` | `EarningMapper` | `MinHrsForFullTimeOTRuleImpl` |
| `…/consecutive_days_calculator.rs` | `ConsecutiveDaysCalculator` | `MinHrsForFullTimeOTRuleImpl` |
| `…/prior_days_calculator.rs` | `priordayscalculator.PriorDaysCalculator` | 5 rules |
| `rules/ports.rs` — `EmployeeShiftConsecutiveDaysPort` | `EmployeeShiftConsecutiveDaysDAO` | the two calculators |

**87 tests, 31 of them transcribed Java assertions.** Three of the six have
Groovy specs in `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/`,
and every case in all three is transcribed:

| Rust | Java | Ported cases |
|---|---|---:|
| `consecutive_days_calculator.rs` | `ConsecutiveDaysCalculatorTest.groovy` | 8 |
| `prior_days_calculator.rs` | `priordayscalculator/PriorDaysCalculatorTest.groovy` | 14 |
| `earning_mapper.rs` | `EarningMapperTest.groovy` | 9 |
| the two accumulators, `daily_data.rs` | no Java spec | — |

All passed on the first run of their helper. The two accumulators and
`DailyData` have no Java spec of their own; they are covered only indirectly,
through the rule specs that have not been ported yet, so their tests here were
written against the Java source by reading it.

**`DailyData` now has a caller.** It was ported ahead of its only user and sat
unconstructed until `CaliforniaOTHrs`; that rule's six transcribed cases are the
first Java-backed evidence that the struct and
`update_consecutive_days_for_day` are right. `DailyData::was_worked` reading the
shifts and not the earnings is exercised by
`an_earning_only_day_breaks_the_consecutive_day_run`.

Two adjustments to the transcriptions, neither touching what is asserted. The
Groovy drives the dataset start date through a mocked `PayGroup` —
`currentPayPeriod().getStartDate()` minus ten days — which is the derivation
divergence 24 does not port, so each case sets the resulting date on the card
and names the pay period start it came from. And `EarningMapperTest` anchors its
week on `new LocalDate()`; it is pinned to a fixed date, as `punchvalidation`'s
were and for the same reason.

One thing the `PriorDaysCalculator` spec gets away with: it passes
`earnings.collect { it.id }` as the configured **earning type** ids, and its one
fixture earning has id 1 and an earning type whose id is also 1. The rows pass
by coincidence. Transcribed as the type id, which is what the production code
reads.

### Findings

- **The two consecutive-day calculators answer the same question differently,
  and neither is a simplification of the other.** They disagree on five points:

  | | `ConsecutiveDaysCalculator` | `PriorDaysCalculator` |
  |---|---|---|
  | shifts with errors | excluded, unless run from scheduling | counted |
  | salaried-exempt job statuses | excluded | counted |
  | earnings | never counted | counted when the rule asks |
  | week starting on the dataset start date | falls through the loop to the DAO | short-circuits to the DAO value |
  | the DAO call | only when the whole window is worked | always, before anything else |

  One rule uses the first, five use the second. A rule must be ported against
  the one it actually calls.

- **The two DAO methods count back from different anchor dates.**
  `getPriorConsecutiveDayWorked` measures to `getDatasetStartDate()`;
  `getPriorConsecutiveDaysWorkedIncludingEarnings` measures to
  `getCalculationStartDate()` — ten days later. Both cache under the *dataset*
  start date, and only differing `CalcDataSetStat` constants keep the two keys
  apart. Whether the asymmetry is deliberate is not decidable from the source;
  it is reproduced and documented on the port.

- **The daily limits swap roles on a consecutive-day-limit day.** Not an on/off
  switch: overtime drops the daily OT limit entirely (every hour is overtime
  from the first), and double time starts measuring at the **overtime** limit
  rather than the double-time one. Pinned by two tests.

- **`WeeklyAccumulator` has three `updateConsecutiveDays` overloads with three
  different behaviours**, and only the `List<EmployeeShift>` one applies the
  modifier wrap. The `int` one neither resets nor wraps — it is how a rule
  seeds the counter from a prior-days calculator — and the `DailyData` one
  resets and increments but never wraps. They are named apart here
  (`update_consecutive_days`, `…_for_day`, `…_for_shifts`) because picking the
  wrong one silently changes when the limits apply.

- **The wrap's `% != 0` guard is load-bearing.** Landing exactly on the
  modifier leaves the counter *at* the modifier rather than zeroing it, so the
  limit day itself still tests as over the limit and only the day after resets
  to 1. With California's 7 and 7 the counter runs 1…13, then 1 again. Pinned
  by `the_counter_stays_over_the_limit_on_the_limit_day`.

- **`computeWeeklyOT` does not round**, alone among the accumulators'
  arithmetic — every `add*` on both classes goes through `TDouble.roundHours`
  and this does not. The caller rounds.

- **`premiumHoursCountTowardsWeeklyOT` selects a formula; it does not filter
  hours.** With it set the answer is `min(hours - weeklyLimit, originalHours)`
  — capped at the original hours and *not* net of the overtime already
  accumulated; with it clear it is the ordinary `hours - weeklyLimit -
  weeklyOT`. The two give different answers from identical state, which
  `the_premium_flag_selects_a_different_formula` pins.

- **`setBothConsecutiveAndWeeklyOt` writes a field nothing reads.** Exactly one
  rule calls the setter; the other four that read the same rule parameter keep
  their own copy and act on that. Dropped.

- **`CaliforniaExtSpecialJobOTHrsRuleImpl` declares a private inner class also
  called `DailyData`**, with a different constructor (date, shifts, a
  day-of-week flag, no earnings), shadowing the package one inside that file.
  Only `CaliforniaOTHrsRuleImpl` uses the shared `DailyData`. Noted in the
  module doc so the rule is not ported against the wrong struct.

- **`DailyData.getShifts().isEmpty()` is what breaks a consecutive-day run, not
  the earnings.** A day carrying a holiday earning and no shift resets the
  counter to zero through the `DailyData` overload — even though
  `PriorDaysCalculator` would count that same day as worked when the rule asks
  it to include earnings.

- **An `EarningMapper` earning lands in exactly one of the two maps.**
  `getEarningsForDate(d)` returns what is dated `d` *and attached to no shift*;
  an earning on a shift is only ever reachable through `getEarningsForShift`.

- **`ConsecutiveDaysCalculator` has no guard against an inverted window.**
  `PriorDaysCalculator` checks `workWeek.getStartDate().isAfter(datasetStartDate)`
  first; this one builds the range regardless. Java survives it because the
  backwards `for` loop's `!date.isBefore(startDate)` test simply never fires,
  and it falls through to the DAO. The port keeps the same loop shape so the
  same thing happens.

### Deliberate divergences (continued)

28. **The DAO's stat-map cache is not ported.** Java wraps both queries in
    `timeCard.getStatMap().computeIfAbsent(key, supplier).intValue()`. Nothing
    anywhere else reads either `CONSECUTIVE_DAYS_FROM_DATASET_START_DATE`
    constant, so the stat map is purely the cache here — divergence 15's case
    again, a memo over a pure function. The port exposes the query; an
    implementation that wants memoization can do it internally.

29. **The port returns `i32` where Java returns `int` and every caller widens
    it.** Both calculators declare `double statValue = dao...()`. Keeping the
    port's own type honest and widening at the two call sites says which half
    is integral.

30. **`WeeklyAccumulator::update_consecutive_days_for_day` takes a `bool`, not a
    `DailyData`.** Java's overload asks `dailyData.getShifts().isEmpty()` and
    nothing else. Passing the answer keeps the accumulator independent of
    `DailyData` — which matters given the shadowed inner class above.

31. **`DailyData` and `EarningMapper` hold indices, not references.** Same
    reason as divergence 22: the rules that consume them write distributions
    onto the shifts while still reading the time card.

32. **A shift or earning against a job the employee held no status for on that
    date is dropped.** `ConsecutiveDaysCalculator.isNotSalariedExemptForShift`
    and `EarningMapper.shouldAddEarning` both dereference
    `getEmployeeJobStatus(...)` unguarded and throw. Nothing says such a record
    was worked non-exempt; excluding it is the reading divergence 20 took.

## The `TimeCard` surface — done

| Rust | Java | Notes |
|---|---|---|
| `entity/time_card.rs` — `TimeCard` trait | `timecard.TimeCard` | all 22 methods the family calls, plus what the same `default` block gives free |
| `entity/time_card.rs` — `TimeCardData` | `ActualsTimeCard` + `ScheduleCalcDataSet` | one struct; see divergence 23 |
| `entity/time_card.rs` — `ShiftToDistribution` | `timecard.ShiftToDistribution` | borrows rather than owning |
| `entity/time_card.rs` — `earnings_mut` | the live list `getEarnings()` hands out | added with `CaliforniaOTHrs`, the first rule to write earnings; divergence 22's shape |
| `entity/time_card.rs` — `earning_is_not_salaried_exempt` | `…IsNotSalariedExemptForEarning` | the earning-shaped twin of `shift_is_not_salaried_exempt`; divergence 32 |
| `entity/flsa_data.rs` | `calcshift.flsacalculations.FlsaData` | the value only; see divergence 25 |
| `entity/calc_data_set_stat.rs` | `calcshift.CalcDataSetStat` | 5 constants, no codes |
| `HoursDistribution::{falls_within_period, is_of_type}` | the static `Predicate` factories | |
| `EmployeeShift::{add_hours_distribution, has_distribution_within_period, dates_with_hours_distributions}` | the same | |

Ground truth for the implementation side is `ActualsTimeCard.java` and its four
`ActualsTimeCard*Methods` helper classes (`Shift`, `Earning`, `Validation`,
`Date`), plus `ScheduleCalcDataSet.java`.

### Findings

- **The overtime and double-time buckets are matched on `getName()`**, against
  the literal strings `"Overtime"` and `"Double Time"` — not on the id, and not
  on the premium flag. A site that renames a bucket silently stops getting
  overtime. And the failure is not quiet for long: Java declares
  `getOTHoursDistributionTypeId()` as `int` over a
  `filterHoursDistributionTypes` that ends `.orElse(null)`, so the miss unboxes
  a null and throws NPE inside whichever rule asked. Both lookups return
  `Option<i32>` here, which puts the miss at the call site. Pinned by
  `a_renamed_overtime_bucket_is_not_found`.
- **`getShiftsForPeriod` and `getShiftsWithDistributionsForPeriod` ask different
  questions.** The first filters on `shift.getShiftDate()`, the second on the
  dates of the shift's *distributions*. An overnight shift dated the Saturday
  that distributes hours into the Sunday is outside the new week by the first
  test and inside it by the second — which is why the overtime rules use the
  second one. Pinned by
  `shifts_with_distributions_for_period_filters_on_the_distribution_date`.
- **`ScheduleCalcDataSet.getShifts()` returns the same list as
  `getSchedules()`.** That is what makes one Rust struct cover both
  implementations: its `getShiftsWithDistributionsForPeriod` iterates
  `schedules` where `ActualsTimeCard`'s iterates `shifts`, and the two agree
  because for that class they are one list. `getSchedulesWithDistributionsForPeriod`
  there is a bare delegation for the same reason.
- **`distributionIsPremium` is defined as *not regular***, not as "in a premium
  bucket". So a distribution with no type id, or one naming a bucket this
  property has not configured, counts as premium and is added to premium hours.
- **`getTotalPremiumHours(EmployeeShift)` does not filter by date**, unlike its
  `getTotalPremiumHours(DateRange)` sibling: every distribution the shift owns
  is counted, including ones dated outside the period the caller is working in.
  Same name, two different questions.
- **`isOpenForEditingOn` is abstract but has one implementation.**
  `ActualsTimeCardValidationMethods.isOpenForEditingOn` is
  `date.isOnOrAfter(getCalculationStartDate(employee))` and
  `ScheduleCalcDataSet`'s is the same test against its field, so it is a
  provided method on the trait rather than a required one. Same for the two
  `isOpenForEditingFor` overloads and `hasShiftsOrEarnings`.
- **The trait has to stay dyn-safe.** The punch families take
  `Option<&dyn TimeCard>`, so the stream helpers return `Vec<&T>` rather than
  `impl Iterator`, and no method is generic. Pinned by
  `a_time_card_is_usable_behind_a_trait_object`, which would stop compiling the
  moment something breaks that.

### Deliberate divergences (continued)

21. **`DateRange` is not ported; `date_range_rs::DateRange` is used directly.**
    The rules tree uses three names for one behaviour — `LegacyDatePeriod`
    (what every `HoursDistributionRuleImpl.execute` takes as its work week),
    `ArbitraryDateRange`, and the `DefaultDateRange` both extend — differing
    only in `getDateRangesPerYear` and a deprecation notice. `containsDate` is
    `startDate.isOnOrBefore(date) && endDate.isOnOrAfter(date)`, which is
    `DateRange::contains_date` exactly. The crate is already a dependency and
    `planner` uses it.

22. **The index form is the primitive for the shift filters.**
    `getShiftsWithDistributionsForPeriod` hands back live references that
    `WeeklyOTHrsRuleImpl` writes through *while still reading*
    `timeCard.getOTHoursDistributionTypeId()` in the same loop — the aliasing
    problem the punch cursor settled. So
    `shift_indices_with_distributions_for_period` returns positions into
    `shifts()`, which a rule walks against `shifts_mut()` with the card still
    readable; `shifts_with_distributions_for_period` is a wrapper over it for
    the read-only sites. `getStatMap` gets the same treatment —
    `ContractOTHrsRuleImpl` calls `.put` on the live map, so there is a
    `stat_map_mut`.

23. **`TimeCardData` stands in for both implementations.** They agree on every
    method the trait carries; their differences (the pay-group lock, the
    calculator references, the availability model, shift add/remove) are all
    outside the rules' reach. A second struct would be two copies of the same
    accessors.

24. **`getCalculationStartDate` and `getDatasetStartDate` are fields, not
    derived.** `ActualsTimeCard` walks the employee's pay group and custom data
    for a `lastPPEndFieldID` property-data entry and takes the later of that
    plus a day and the current period start; the dataset start is ten days
    before. That needs `PayGroup`, `PropertyData` and `EmployeeCustomField`,
    none of which any rule touches. `ScheduleCalcDataSet` holds both as plain
    fields, and so does this — `with_calculation_start_date` applies the
    ten-day offset so the pair stays consistent.

25. **`FlsaData` carries the value; the calculators are not ported.** Java's
    constructor is private behind `createFlsaData(timeCard, dateRange)`, which
    runs `TipsCalculator`, `SevenICommissionsCalculator`,
    `AdditionalFlsaEarningsCalculator` and `HoursAndWageCalculator` over the
    period. None of that is in the rules tree — the calc pipeline fills the map
    before any rule runs, and the two call sites
    (`WeeklyOTHrsRuleImpl.passes7iExemptionTests`,
    `TwentyFourHourOTRuleImpl.getFlsaRegularRateForDateRange`) only read
    `getRegularRate`, `getTotalRegularRatePay` and `getSevenICommissions` back
    out. The calculators arrive if a family needs them.

    `getFlsaDataMap(boolean)` is on `TimeCardData`, not the trait: Java's
    version also consults `employee.getProperty().getPayPeriodType()`, and
    `Property` is reached by id here. Both current call sites read the weekly
    map directly.

26. **`HoursDistribution.isOfType` answers `false` for an untyped
    distribution.** Java compares `Integer == int`, which unboxes and throws.
    An untyped distribution is in no bucket under any reading.

27. **`getDatesWithHoursDistributions` returns a sorted `Vec`, not a
    `HashSet`.** Nothing depends on the hash order, and a caller that iterates
    gets a defined one.

**Rules by size**, smallest first — a sensible porting order, since the large
ones are overtime variants that build on the accumulators: `RegHrsOnly` (no-op),
`HolidayDTHrs` (85), `WeeklyOTSecJobHrs` (100), `WeeklyOTHrs` (130),
`PayPeriodOTHrs` (139), `ScheduledShiftOT` (178), `CaliforniaExtendedOTHrs`
(193), `RollingXWeeksOTHrs` (196), `DailyWeekly6thDayOT7thDayDTNonConsec` (200),
`PerMonthOTHrs` (230), `CaliforniaOTHrs` (254), `TwentyFourHourOT` (284),
`ContractOTHrs` (324), `DailyWeekly7thDTHrs` (366),
`CaliforniaExtSpecialJobOTHrs` (414), `MinHrsForFullTimeOT` (430),
`DlyWklyOffConsecOTMinBreak` (439), `DailyWeekly6thOT7thDTHrs` (451),
`DlyWklyConsecOTMinBreakSpanningMidnight` (456).

## `regularhoursdistribution` — done

`RuleType::RegularHoursDistribution` (code `RH`), a 32nd family distinct from
`hoursdistribution` (code `HD`) despite the similar name — see "Where the work
stands" above. Ground truth:
`taps/src/main/java/com/unifocus/watson/server/labor/rules/algorithm/regularhoursdistribution/`
for the rules, and
`taps/src/main/java/com/unifocus/watson/server/labor/calcshift/rules/RegularHoursDistributionRunner.java`
for the runner.

### Scoping

**A clean 1:1 match**: 3 `RuleClass` entries (`RegHoursRhd`, `RegHoursByDayRhd`,
`RegHoursByWorkWeekRhd`), 3 `*RuleImpl` classes, no no-op and no deferred
oddball — unlike `hoursdistribution`'s 19-vs-20 mismatch.

**The interface is shift-scoped, not card-scoped.**
`RegularHoursDistributionRule.execute(EmployeeShift, RuleItem)` — no
`TimeCard`, no work week. The `TimeCard`-level orchestration (which shifts are
open for editing, resetting or clearing distributions, falling back to a
default rule) lives entirely in the runner, not in the rules.

**Two shared helpers this family needed, neither ported before now**:
`BreaksAndAdjustmentsCalculator` (needed a new `EmployeeShift::breaks()`
accessor — positionally-paired punches, mirroring `worked_date_time_ranges`)
and `SingleDistributionTypeRuleConfig` (the shared config base, one parameter:
which bucket a rule writes into). Both close out divergence 34, which had
deferred `HoursDistributionFactory::create_split_distributions` and
`::create_distribution_of_configured_type` until "the family that calls them"
arrived.

### Rules — 3 of 3, plus the runner

| Rust | Java | Ported cases |
|---|---|---:|
| `regularhoursdistribution/reg_hours_on_shift_date.rs` | `RegularHoursOnShiftDateRuleImpl` | no real spec — see below |
| `regularhoursdistribution/reg_hours_by_day.rs` | `RegularHoursByDayRuleImpl` | **7/7** |
| `regularhoursdistribution/reg_hours_by_work_week.rs` | `RegularHoursByWorkWeekRuleImpl` | **7/7** |
| `runner/regular_hours_distribution.rs` | `RegularHoursDistributionRunner` | **3/3**, one strengthened — see below |

**`RegularHoursOnShiftDateRuleImplTest.groovy` and its config test are
misplaced stubs, not a real spec.** Both files live under the
`hoursdistribution` test package instead of `regularhoursdistribution`'s own,
and every method body in both is empty — the same situation as
`hoursdistribution`'s `RegHrsOnlyRuleImpl`. Behavior tests only; nothing to
transcribe.

**`RegularHoursByDayRuleImplTest` and `RegularHoursByWorkWeekRuleImplTest` are
the same spec run through two different boundaries.** Every one of the
by-work-week spec's fixtures sets `periodEndDate: today`, which makes `today`
the last day of its own work week — so the midnight-starting-the-day-after
boundary both rules measure against lands on the identical instant, and all
seven cases assert the identical numbers as their by-day counterparts. Ported
side by side for exactly the reason `PARITY_AUDIT` already recorded for
`hoursdistribution`'s siblings: porting them together is what makes a shared
fixture's meaning legible.

**The runner's own spec is weaker than it looks in its first case.** Its
`ruleImplFactory` is a Spock mock standing in for the rule itself —
`createRuleImpl` returns a mock `RegularHoursOnShiftDateRuleImpl` — and
Spock's default for an unstubbed void method is a no-op. So the "cleared" open
shift's `execute()` call never runs the real rule and never writes a
distribution back, and the spec's `hoursDistributions.isEmpty()` assertion
passes **because nothing rebuilt the list**, not because the runner leaves an
open shift empty (it does not — a real rule always writes one back). The
transcription wires the real `RegularHoursOnShiftDateRule` instead and asserts
what actually happens: cleared *and* rerun. The other two cases don't touch
rule dispatch at all and transcribe as written.

### Deliberate divergences (continued)

51. **The property comes through a new `PropertyPort`, not the entity
    graph.** Java reaches a shift's period end date via
    `shift.getEmployee().getProperty().getPeriodEndDate()`; this crate's
    `EmployeeShift` carries only its property's id (the entity graph is
    one-way, same reasoning as divergence 38's `MinWagePort`). So
    `RegularHoursByWorkWeekRule<P: PropertyPort>` takes the port as a type
    parameter, exactly as `HolidayDTHrsRule<P: HolidayPort>` does.

52. **The runner takes one resolved rule set for the whole run, not one per
    shift.** `RegularHoursDistributionRunner.runRules` calls
    `ruleUtils.getRuleSet(shift.getEmployee(), shift.getJob(),
    shift.getShiftDate(), REGULAR_HOURS_DISTRIBUTION)` **per shift** — a
    different employee, job or date can select a different rule set. Reaching
    a shift's employee and job needs the same entity-graph traversal
    divergence 51 already declined, so `run_rules` takes an
    `Option<&RuleSet>` once for the card, matching
    `PunchRoundingRunner::round_shift`'s precedent of leaving resolution to
    the caller. Faithful to every case in the Groovy spec, which stubs
    `RuleUtils` unconditionally and so always falls back to the same default
    regardless of which shift is being processed.

53. **`shiftSpansCalculationStartDate` is a free function, not a `TimeCard`
    method.** It only needs the scalar `calculation_start_date` divergence 24
    already carries as a plain field, so keeping it free lets the runner
    compute that scalar once before mutably borrowing the card's shifts,
    sidestepping the aliasing problem divergence 22 solved with index forms —
    there is no live `TimeCard` reference to alias against in the first place.

## Wave 1 — punchvalidation

**684 tests. All four rules, and every Groovy case in the family transcribed.**

| Rust | Java | Ported cases |
|---|---|---:|
| `algorithm/punchvalidation/config.rs` | all 5 `*RuleConfig` | — |
| `algorithm/punchvalidation/mod.rs` | `PunchValidationRuleImpl` base | — |
| `no_in.rs` | `NoInPunchValidationRuleImpl` | no Java spec |
| `sched_lockout_in.rs` | `SchedLockoutInPunchValidationRuleImpl` | 19 |
| `sched_lockout_out.rs` | `SchedLockoutOutPunchValidationRuleImpl` | 36 |
| `sched_lock_allow_unsched.rs` | `SchedLockAllowUnschedIPVRuleImpl` | 40 |
| `entity/time_clock_result.rs` | `TimeClockServerResultDTO`, `UFTCResultStatus` | — |
| `entity/punch_log.rs` | `PunchLogDTO` | — |
| `common/enums/uftc_punch_type.rs` | `UFTCPunchType` | — |

### Ported assertions

**120 across the crate** — 25 from `punchrounding`, 95 from `punchvalidation`.
Every `where:` table in the three punch-validation specs is transcribed row for
row. All passed on the first run of their rule.

Two adjustments, applied consistently and changing nothing that is asserted:
the Groovy anchors its times on `new LocalDateTime()` (today at noon) and these
are pinned to a fixed date, since every rule compares a punch against a schedule
and never against the clock; and `messages.contains(ResourceMgr.lookup("res_x"))`
becomes `has_message("res_x")`.

### Findings

- **The two punch-type enums declare their constants in different orders.**
  `UFTCPunchType` reads IN, OUT, BREAK, BACK; `PunchType` reads IN, BREAK, BACK,
  OUT. Same fifteen constants and codes, so Java's "keep these compatible"
  comment holds for the wire format — but **not** for position. That matters
  because `PunchTimeComparator` tiebreaks on the server enum's `ordinal()`, so
  the orders are not interchangeable and neither enum's position may index the
  other. Caught by a test asserting the mirror, which failed.
- **`PunchTimeComparator` has a third tiebreak that Wave 0 dropped.** After
  rounded time and `comparePunchTypes`, Java compares `punchType.ordinal()`.
  The port stopped at the second and left a no-op `.then(Ordering::Equal)`. Now
  restored, with `ordinal()` added to the `coded_enum!` macro. It only decides
  between two punches at the same instant that `comparePunchTypes` called equal
  — two breaks, say — but without it their order was down to the sort's
  stability instead of the rule.
- **`SchedLockoutInPunchValidationRuleImpl` overrides three of the base's
  failure shapes**, and the overrides differ in two ways: the base sets
  `managerOverridable` on two of them and the subclass on none, and
  `jobLockoutWarnFailure`'s reject key is `res_scheduleWarningPeriod` in the
  base but `res_jobRestriction` in the override. `SchedLockAllowUnschedIPVRule`
  does *not* override them and so gets the base behaviour.

  I had first ported only the subclass forms, under the base names. The
  `SchedLockAllowUnsched` spec caught it: its table asserts
  `managerOverridable == true` on exactly the rejections the subclass suppresses.
  Both forms are now spelled out separately — the base three in the family
  module, the three overrides private to `sched_lockout_in`.
- **`SchedLockoutOutPunchValidationRuleImpl` overrides
  `findClosestScheduleToPunch`** to measure from each schedule's **end** rather
  than its start. An out punch belongs to the shift it is ending, not the one
  starting soonest. The base's start-based version is the in-punch form, not a
  shared one — the module doc in the family base said otherwise until the
  out-punch source corrected it.
- **The out-punch rule's `unscheduled` switch inverts both its tests.** With it
  off, "outside the grace window" fails. With it on, only the *bands between*
  the grace window and the wider pre/post punch window fail; beyond those the
  punch is treated as unscheduled and allowed through.
- **`SchedLockAllowUnsched` differs from the plain lockout rule only in the
  tail**: no schedule at all is a manager-overridable *not-scheduled* failure
  rather than a lockout, and so is a punch beyond the allowed window.
- **`withinGracePeriod` is asymmetric.** The warn window is consulted whenever
  `warn` is set; the lockout window only when `warn` is *not* set. So with both
  switches on, the warn window alone decides whether a punch is clean, and the
  lockout window only decides how hard the rejection is.
- **The unscheduled-message parameter key is `unscheduleMessage`**, missing its
  "d". Pinned by a test, so a well-meaning correction cannot silently break
  every configured override at a live site.

### Divergence

19. **Default messages are carried as their resource key.** Java builds each
    with `ResourceMgr.lookup("res_x")`; the engine displays nothing and the i18n
    bundles are not ported, so the key stands in for the resolved string. A
    message the rule set configures is carried literally, as Java does. The
    Groovy assertions stay checkable: `messages.contains(ResourceMgr.lookup(
    "res_jobLockoutMessage"))` becomes `has_message("res_jobLockoutMessage")`
    — the same question about the same branch.

20. **A missing employee counts as an invalid job code.** Java would throw an
    NPE inside `employeeHasInvalidJobCode`; there is no reading under which an
    absent employee holds a valid job, so it takes the rejection branch.

## Wave 1 — punchrounding

**534 tests. The family is complete: all six rules and the runner.**

| Rust | Java | State |
|---|---|---|
| `algorithm/punchrounding/config.rs` | all 7 `*RuleConfig` | 6 catalogue entries + the abstract base's defaults |
| `algorithm/punchrounding/mod.rs` | `PunchRoundingRuleImpl` base | the trait and `canRound` |
| `minute_rounding.rs` | `MinuteRoundingRuleImpl` | **25/25 Java table rows** |
| `worked_hours_rounding.rs` | `WorkedHoursRoundingRuleImpl` | no Java table exists |
| `property_data_rounding.rs` | `PropertyDataRoundingRuleImpl` | over `PropertyDataPort` |
| `back_from_break_with_grace.rs` | `BackFromBreakWithGraceRuleImpl` | with its own break-finding |
| `round_to_schedule.rs` | `PunchRoundToSchedule` + both subclasses | one algorithm, parameterised |
| `runner/punch_rounding.rs` | `PunchRoundingRunner`, `PunchRoundingUtil` | incl. the default-rule fallback |
| `entity/time_card.rs` | `TimeCard` | `schedules()` only at the time; expanded for Wave 3 |

### Ported assertions

**25**, all from `MinuteRoundingRuleImplTest.groovy`'s `@Parameterized` table,
transcribed row for row including its own section comments — defaults rounding
forward and back, each of the eight manual/clock gates, round-to-1 and
round-to-5, and rounding 23:56 across midnight. They passed on the first run,
which also exercises `common/rounding.rs`, `common/dates.rs`, `RuleParams` and
the punch/shift cursor end to end.

The only change to the cases is where the assertion reads from: the Groovy
asserts `punch.getRoundedTime()`, and here it reads through the shift that owns
the punch, because that is where the cursor writes.

### Findings

- **`WorkedHoursRoundingRuleImpl` never calls `canRound`.** It inherits all
  eight manual/clock gates from its config and consults none of them; only the
  punch type gates it, and only on `Out`. A tidier port would have wired the
  gates in and changed behaviour.
- **The four-decimal worked hours leak a second.** `shift.getWorkedHours()` is
  rounded by `TDouble.roundRawHours`, so 488 minutes reads back as `8.1333`
  hours = 29279.88 seconds, not 29280. Java truncates that with an `(int)` cast,
  and the adjustment comes out **421** seconds instead of 420 — an out punch
  landing on 16:15:**01** rather than the clean quarter hour. Three of my own
  test expectations were wrong here before I checked the arithmetic against the
  Java; the port was right. Pinned by
  `the_four_decimal_worked_hours_leak_a_second`.
- **The rule's own final write re-fires the callback**, so the worked hours left
  on the shift afterwards describe the *rounded* span, not the measured one it
  rounded from.
- **A rounding rule is idempotent**, because every rule reads `adjTime` and
  writes `roundedTime`, and none reads `roundedTime` back.
- **The config inheritance is not uniform, and it decides what a rule sees.**
  `MinuteRounding`, `PropertyDataRounding` and `WorkedHoursRounding` extend the
  base and inherit the eight gates; `BackFromBreakWithGrace`,
  `RoundInToSchedule` and `RoundOutToSchedule` build their defaults from scratch
  and carry only the gates they use.
- **`PunchRoundToSchedule` crosses its grace parameters.**
  `createMatchingPeriod` reads
  `preShift = adjTime.minusMinutes(gracePost)` and
  `postShift = adjTime.plusMinutes(gracePre)` — the *post* grace is subtracted
  and the *pre* grace added, the opposite of the names. So
  `gracePreSchedStart` governs how far **after** a punch a schedule may sit.
  Both default to 15, so a site that never changed them cannot tell.
  Reproduced exactly and pinned by `the_grace_parameters_are_crossed`.
- **`BackFromBreakWithGrace` has its own break-finding**, not
  `ShiftUtil.getBreaks`. Same positional pairing and guards, but it takes a
  break's start from `roundedTime` and its end from `adjTime` — deliberate,
  since the end is the punch it is about to move and must still compare
  unrounded.
- **The runner resets before it rounds.** Every punch goes back to its adjusted
  time and the shift date back to the in punch's adjusted date before any rule
  runs, which is what makes a whole recalculation idempotent: rounding twice
  with different thresholds gives the second threshold's answer, not a
  compounded one.

### Wave 0 correction

**`getErrors()` and `getErrorsOld()` are different sets, and Wave 0 conflated
them.** `calcWorkedHours` consults the *derived* set ("we may have not updated
the saved errors yet"), while `hasErrors()` — which
`BackFromBreakWithGraceRuleImpl` gates on — and `ShiftUtil.getBreaks` consult
the *persisted* one. `EmployeeShift` now has both: `errors()`/`has_errors()`
for the persisted set, `derived_errors()` for the recomputed one. Conflating
them changes which shifts get their hours zeroed.

### Settled: f64, not a decimal type

The question came up whether to move the engine off `f64` onto `rust_decimal`
to avoid float rounding problems. Measured before deciding, running
`TDouble.round(v, n)`'s exact algorithm on both substrates:

| Test | Cases | Divergences |
|---|---:|---:|
| `hours × rate` → 4dp currency | 160,000 | 0 |
| Division chains (`/60`, `/7`) → 4dp & 6dp | 60,000 | 0 |
| Large magnitudes (to ~39,595) → 2dp & 4dp | 100,000 | 0 |
| Negatives → 2dp & 4dp | 100,000 | 0 |

Accumulation drift — where float genuinely differs — peaked at **9×10⁻¹¹** over
10,000 rows, against a currency precision of 10⁻⁴.

**Why they agree:** `TDouble`'s epsilon nudge is `0.01/m` — 10⁻⁴ at 2dp, 10⁻⁶ at
4dp — while float representation error at these magnitudes is ~10⁻¹⁴. The nudge
is ten orders of magnitude larger than the error it nominally compensates for,
so it swamps any difference between a binary and a decimal substrate. `TDouble`
is not fighting ULP-level error; it is applying a deliberate "round `.xxx5` up"
bias that is insensitive to the number type.

**Decision: stay on `f64`.** Java is `double` throughout, the results match, and
parity stays easy to argue — a future divergence is then a real porting bug
rather than a substrate difference. Switching would also not remove the need to
reproduce `TDouble`'s quirks exactly; the nudge, `Math.rint`'s half-to-even and
the `-0.0` normalisation would all still be required, just on different numbers.

**When to revisit:** this holds *because* the engine rounds after nearly every
operation. A family that accumulates thousands of unrounded values would change
the calculus — flag it if one appears.

### Divergence

18. **`PropertyDataPort` takes a property id, not a `Property`.** Java's
    `getValue(Property, key, default)` takes the entity but keys off its id, and
    a rule reaching it has navigated
    `punch.getEmployeeShift().getJob().getProperty()` — a chain that needs no
    lazy loading if only the id is wanted.

**Wave 0 is complete. 397 tests, all passing.** Clippy clean.

## Wave 0 — resolution, dispatch, ports

| Rust | Java |
|---|---|
| `rules/rule_utils.rs` | `RuleUtils.getRuleSet`, `getSourcedRuleSets` |
| `rules/dispatch.rs` | `RuleImplFactory`, `RuleConfigFactory` |
| `rules/ports.rs` | `com.unifocus.watson.server.dao.*` |
| `entity/{employee,property,assignment,employee_job_status,earning_type,employee_earning,hours_distribution}.rs` | the corresponding entities |

### Findings

- **The two resolutions disagree on ties, and that is not a bug.**
  `getRuleSet` tests `>` so an equal salience never displaces the incumbent —
  the **first** tied employee set wins. `getSourcedRuleSets` tests `==` to append
  and `>` to reset, so it returns **all** tied sets. Both behaviours are ported.
- **Java's tie-break is non-deterministic.** `getEmployeeSetsForEmployeeAndJob`
  returns a `HashSet<EmployeeSet>`, so "first tied set" means "whichever the
  hash order produced". Here the order is the provider's, which at least makes
  it reproducible. Any site relying on it is relying on unspecified behaviour
  either way.
- **Salience does not compete across levels.** A salience-0 employee set beats a
  salience-999 property default; salience is only consulted *within* the
  employee-set level.
- **`EarningTypeDAO.findByID` is the most-called DAO method in the engine** — 87
  call sites — because rules carry earning type ids in their parameters.

### Deliberate divergences (continued)

14. **Resolution is a pure function over an `EmployeeSetProvider` trait.**
    Java's `RuleUtils` is a `@Scope("request")` Spring bean holding
    `EmployeeSetDAO` and a `SessionFactory`. Only the employee-set lookup is
    actually I/O; putting it behind a trait leaves the cascade itself pure and
    directly testable.

15. **The per-request memo cache is not ported.** Java caches resolutions in a
    `Map<TTuple4<Employee, Assignment, LocalDate, RuleType>, RuleSet>`. It is a
    performance optimisation — same inputs, same output — and reproducing it
    would mean `RefCell` gymnastics to hand out borrowed rule sets. Worth
    revisiting if resolution shows up in a profile.

16. **Dispatch is per-family, so the cast disappears.** Java's
    `RuleImplFactory` returns a raw `RuleImpl` that each caller casts to its own
    family interface, with a missing Spring bean or a bad cast failing at
    runtime. A `RuleRegistry<R>` maps only the rule classes of one `RuleType`,
    so the returned trait object is already the family's trait. It also exposes
    `missing()`, which lists catalogue entries with no implementation — a family
    part-way through porting is visible rather than silently skipping rules.

17. **`PropertyDataKey` is ported as the eight keys the rules read**, not the
    312-line Java catalogue.

### DAO ports — partial by design

**As of Wave 3, eight are defined** — the five Wave 0 started with
(`PropertyDataPort`, `EarningTypePort`, `AssignmentPort`, `EmployeeEarningPort`,
`EmployeeShiftPort`) plus `HolidayPort` and `EmployeeShiftConsecutiveDaysPort`,
which `hoursdistribution` brought, and `MinWagePort`, which is not a DAO port at
all (divergence 38). The remaining eleven DAOs are listed in `rules/ports.rs`
with their call counts, the methods the rules actually call, and the entity each
is blocked on.

They are deliberately not defined yet. A port's method set is only knowable from
its call sites — `EmployeeShiftDAO` is 1,497 lines of which exactly four methods
are reachable from the rules tree — and defining the rest now would mean porting
ten more entities to spell signatures nobody calls yet. Each arrives with the
family that needs it.

## Wave 0 — catalogue, parameters, entities

| Rust | Java | Notes |
|---|---|---|
| `rules/rule_type.rs` | `common.enums.RuleType` | 32 buckets |
| `rules/rule_class.rs` | `common.labor.rules.RuleClass` | 225 entries, generated from the Java |
| `rules/params.rs` | `RuleItem.params` handling | the one parsing helper |
| `rules/rule_config.rs` | `RuleConfig`, `AbstractRuleConfig`, `PriorityRuleConfig` | runtime half only |
| `rules/priority.rs` | `RuleItemPriorityComparator` | post-punch only |
| `entity/rule_item.rs` | `hibernate.entity.RuleItem` | |
| `entity/rule_set.rs` | `hibernate.entity.RuleSet`, `SourcedRuleSet`, `RuleSetSource` | |
| `common/enums/module.rs` | `common.security.Module` | `RuleType`'s licensing metadata |

### Findings

- **`RuleClass` has 225 entries, not ~250.** The larger figure counts
  `*RuleImpl` classes, ~25 of which are per-family abstract bases. Confirmed by
  extracting the enum: 225 constants, no duplicate codes or names, spanning all
  32 rule types. `PUNCH_ROUNDING` holds **6**, not 7 —
  `PunchRoundingRuleConfig` is the abstract base the other six extend.
- **`Module.fromCode` does not throw.** It returns `Module.INVALID`, because old
  licence files carry retired modules. The only coded enum in the port that
  absorbs bad input rather than rejecting it, so it returns `Self`, not `Option`.
- **`calcWorkedHours` uses `TDouble.roundRawHours` then `roundHours`** — the
  4-then-2 decimal sequence. Wave 0 ordering vindicated: the entity model needs
  the rounding layer to already be exact.

### Deliberate divergences (continued from the support layer)

7. **`RuleClass` variants are `PascalCase`**, not Java's
   `SCREAMING_SNAKE_CASE`. 225 non-camel-case variants would need a blanket lint
   suppression; `java_name()` preserves the original spelling, round-trips via
   `from_java_name`, and is asserted unique.

8. **`instanceof PriorityRuleConfig` became a trait method.**
   `RuleItemPriorityComparator` asks Java whether a config is a
   `PriorityRuleConfig` subclass. `RuleConfig::priority` returns
   `Option<i32>` — `None` for configs that are not priority-configurable,
   which the comparator then ranks at 5, exactly as Java does.

9. **One parameter-parsing helper instead of 843 call sites.** Each accessor
   reproduces its Java counterpart's behaviour *including the failure mode*:
   `bool_at` absorbs missing and unparseable values as `false` (Java's
   `Boolean.parseBoolean` never throws, not even on `null`), while `int_at` and
   `double_at` **panic** where Java throws `NumberFormatException`. `try_`
   variants exist for callers that want to handle bad configuration instead.
   Panicking is the faithful choice: Java aborts the calculation here and
   nothing catches it.

10. **`RuleItem` holds `rule_set_id`, not a `RuleSet` reference.** Java
    navigates back up via `getRuleSet()`. With no lazy-loading session there is
    nothing to resolve a cycle, so ownership stays one-way: a rule set owns its
    items.

11. **`PriorityRuleConfig.getPriority` does not mutate.** Java's calls `fixMap`,
    which writes the default back into the caller's map. Nothing depends on that
    side effect; `priority_of` works on a fixed copy.

12. **`SourcedRuleSet` composes rather than extends.** Java subclasses `RuleSet`
    and copies all eight fields across in the constructor.

## The punch/shift ownership shape — settled

13. **The shift is the aggregate; rules reach a punch through a cursor.**

`EmployeeShiftPunch.setRoundedTime` calls
`employeeShift.resetStartAndEndTimesFromPunch(this)` on every change, which
resets the shift's start or end and reruns `calcWorkedHours()`. A punch owned by
a shift cannot hold `&mut` to it.

This is not a detail that could be dropped. Two findings settled it:

- **5 of the 6 punch-rounding rules reach the shift.** Only `MinuteRounding` is
  punch-only. `PropertyDataRounding` walks `punch.getEmployeeShift().getJob()
  .getProperty()`; `RoundInToSchedule`/`RoundOutToSchedule` need the shift's job
  and the time card's schedules; `BackFromBreakWithGrace` reads the shift too.
- **`WorkedHoursRoundingRuleImpl` mutates siblings and then reads state those
  mutations recomputed:**

  ```java
  final EmployeeShift shift = punch.getEmployeeShift();
  for (EmployeeShiftPunch otherPunch : shift.getPunches()) {
     otherPunch.setRoundedTime(otherPunch.getAdjTime());   // each fires the callback
  }
  double workedHours = shift.getWorkedHours();             // correct only because of them
  ```

  No `execute(&mut EmployeeShiftPunch)` signature can express this, however the
  back-reference is modelled. `shiftcorrection` does the same thing across
  punches (`outPunch.setRoundedTime(inPunch.getRoundedTime()...)`), so this is
  the normal case, not an outlier.

**The shape.** `EmployeeShift` owns `Vec<EmployeeShiftPunch>` and owns the
callback. `EmployeeShiftPunch::set_rounded_time` is `pub(crate)`, is the plain
field write, and returns whether the value changed — a rule cannot reach it and
skip the callback. Rules take a `PunchCursor<'a>` (a `&mut EmployeeShift` plus
an index) whose `set_rounded_time` does both halves, and whose
`for_each_punch` reproduces the sibling loop with a callback per punch.

Rejected: an `Rc<RefCell<…>>` object graph mirroring Java's references. It keeps
the Java signature, but the sibling-mutation loop above would re-enter
`shift.borrow_mut()` while the shift is already borrowed for iteration — a
runtime panic the compiler cannot catch — and interior mutability would spread
through every entity relation.

Cost: rule bodies say `punch.set_rounded_time(t)` against a cursor rather than a
punch, and the ported Groovy tables assert through `shift.punch(i)`. Both read
close enough to the Java to stay reviewable against it.

### Entity core so far

`entity/employee_shift_punch.rs` and `entity/employee_shift.rs` bring across the
narrow slice the rules read, plus the shift methods the callback depends on:
`getErrorsOld` (the derived validator `calcWorkedHours` consults rather than the
persisted error set), `calcWorkedHours`, and the `ShiftUtil` helpers
`shiftDurationInMinutes`, `totalBreakTimeInMinutes` and `getBreaks`, along with
`PunchTimeComparator`.

Two faithfulness notes:

- `getErrorsOld` compares punch **object references** (`inPunch !=
  punchList.get(0)`). Ported as index comparisons, which asks the same question
  and stays correct when two punches are equal by value.
- `ShiftUtil.getBreaks` pairs punches **positionally** — indices 1&2, 3&4, … —
  not by punch type, and yields nothing when the shift has errors or has three
  punches or fewer. Kept as-is; it is surprising but it is the behaviour.

`EmployeeShift.workedAdjustmentsEmpty()` is stubbed as a `bool` field until the
adjustments entity arrives. Every punch-rounding test builds a shift with no
adjustments, so the stub is exercised in its true state.

### Settled since

- `HoursDistribution.setHours` recomputes `totalCosts` on every write — the same
  class of problem, but self-contained within one entity, so no cursor was
  needed. `set_hours`, `set_base_rate` and `set_premium_rate` each do both
  halves.

## Wave 0 — support layer

| Rust | Java | Notes |
|---|---|---|
| `common/numbers.rs` | `tbx.core.TDouble` | three rounding modes; see below |
| `common/rounding.rs` | `tbx.core.rounding.*` | 4 strategy classes → one enum |
| `common/dates.rs` | `tbx.core.DTUtil`, `DateUtil` | pure date math only |
| `common/json_range_array.rs` | `watson.common.labor.rules.JsonRangeArray` | banded lookup tables |
| `common/coded_enum.rs` | — | new; carries the shape shared by the enums |
| `common/enums/*` (21 files) | `watson.common.enums.*` | see "Not yet ported" |

### Deliberate divergences

1. **`fromCode` returns `Option`, not a panic.** Java throws
   `IllegalArgumentException` from every `fromCode`. These codes arrive from
   stored data and rule parameters, so an unknown one is a data problem for the
   caller to handle rather than a bug to abort on.

2. **`resourceKey` / `toString()` dropped.** Every coded enum carries an i18n
   key for display. The engine never displays anything; the config UI that did
   is out of scope.

3. **Rounding-strategy reflection dropped.** `RoundingOption.getInstance()`
   reflectively constructs one of four single-method classes. Collapsed to a
   `match`; no behaviour was carried by the indirection.

4. **`parse_bands` extracted** in `json_range_array.rs`. Java reparses the JSON
   independently inside `getValueFromJSONRange` and `isValidJson`. Same results;
   one parser.

5. **`java_signum` written out.** Rust's `f64::signum` returns `±1.0` at zero
   where Java's `Math.signum` returns the zero itself. This feeds `round_to`'s
   nudge term, so Java's behaviour is reproduced explicitly.

6. **`Math.round` written out as `(x + 0.5).floor()`** in `rounding.rs`. Java's
   `Math.round` is half toward *positive infinity*; Rust's `f64::round` is half
   *away from zero*. They agree for the non-negative second-of-day values punch
   rounding produces, but the engine is matching Java, not approximating it.

### `TDouble` — the finding that shaped the wave

`TDouble` has **three mutually inconsistent rounding rules**:

| Java | Semantics |
|---|---|
| `round(double)` | half-up away from zero (hand-rolled, not `Math.rint`) |
| `round(double, 0)` | `Math.rint` — half to even |
| `round(double, n>0)` | half to even **plus** a `signum(v) * 0.01 / m` nudge |

`round(2.5)` is 3; `round(2.5, 0)` is 2. So **each of the ~137 `TDouble` call
sites in the rules tree must be resolved to the overload it actually invoked**
as part of porting its rule. There is no single Rust function to map them all
onto, and `numbers.rs` deliberately exposes them as separate functions to stop
that happening by accident.

`planner/src/workcontent/common/numbers.rs::round_to` is
`(value * factor).round() / factor` — half away from zero, no epsilon — so it
disagrees with `TDouble::round(v, n)` on exactly the boundaries that decide
money. **Do not reuse it here.** That is the drift `planner`'s own doc comments
describe ("only shows up at the fourth decimal").

One correction to the received wisdom: `TDouble`'s comment cites `1.175` storing
as `1.1749999999` and rounding down without the nudge. In f64 it actually stores
just *above* 1.175, so `*100` lands exactly on 117.5 and ties-to-even reaches
1.18 unaided. The nudge does real work at **`1.005`**, which stores low —
without it that rounds to 1.00 instead of 1.01. Both are asserted in
`numbers.rs`.

### Test backing

**205 tests, all passing. None are ported Java cases** — Java has no unit tests
for `TDouble`, `DTUtil`, `DateUtil`, `RoundingOption` or `JsonRangeArray` in
either `taps/src/junit` or the tbx checkout. The tests here were written against
the Java source by reading it, and include:

- the worked examples from `TDouble`'s own javadoc for
  `isOutsideVariance` (8 cases, transcribed);
- rounding boundaries at `.005`/`.0005`, negatives, `-0.0`, and the `1.005`
  representation case;
- three rows lifted from `MinuteRoundingRuleImplTest.groovy` that exercise
  `dates.rs` and `rounding.rs` directly — the 15-minute default, the round-to-1
  no-op, and rounding 23:56 across midnight into the next day.

The real parity evidence arrives in Wave 1, when `punchrounding`'s 636-line test
table runs against this layer.

### Not yet ported

- `AlertType`, `EmployeePointsType` — entangled with each other and with
  `AlertLevel`/`AlertCategory`; they arrive with `employeealerts` and
  `employeepoints`.
- `PropertyDataKey` — a 312-line key catalogue only property-data lookups need.
- `RuleType` — belongs with `RuleClass` in the engine core.
- `JSONUtils` (`getIdsListForKey`, `getIdList`, `getIdsFromJSON`) — 36 uses, all
  multi-select id lists. Deferred until a family needs it.
- `DateUtil`'s `Locale`/`NumberFormat` half, and `TDouble`'s `format*` methods —
  display only, out of scope.

## `schedulerestriction` — done, one deferred

**Ported first among the seven families the user prioritized for scheduling**
(`schedulerestriction`, `schedulelunch`, `shiftadjustment`, `shiftcorrection`,
`dailyearning`, `weeklyearning`, `shiftearning`, in that order — see the
recommendation given before this section started). Measured before porting:
sizes and spec coverage varied more than the family names suggested —
`shiftearning` alone is 23 concrete rules with its own `utility` subpackage,
closer to a second `hoursdistribution` than a quick family, so it is left for
last and will get its own "Scoping —" pass when its turn comes.

**29 tests, 20 of them transcribed Java assertions.** Five of the six
catalogue entries have an algorithm: `NoRestrictionRuleImpl`,
`EarliestStartLatestEndTimeRuleImpl`, `MaxDaysWorkedPerWeekRuleImpl`,
`MaxHoursOnDayRuleImpl`, `MaxHoursPerWeekRuleImpl`.

| Ported | Java spec cases |
|---|---:|
| `NoRestrictionRuleImpl` | 2/2 |
| `EarliestStartLatestEndTimeRuleImpl` | 4/4 (two `where:` tables) |
| `MaxDaysWorkedPerWeekRuleImpl` | 3/3 |
| `MaxHoursOnDayRuleImpl` | 6/6 |
| `MaxHoursPerWeekRuleImpl` | 3/3 |
| `MonthlyRequiredDaysOffRuleImpl` | not ported — see below |
| `ScheduleRestrictionResult` | 2/2 (its own spec file) |

**A new rule shape.** Every family ported before this one either writes a
rate/distribution field or produces an earning.
`ScheduleRestrictionRuleImpl.canEmployeeWorkShift` computes a result instead —
`ScheduleRestrictionResult`, an OK/error value with the `ShiftErrorType` that
explains a failure — and never mutates its arguments. `ScheduleCalcDataSet`
needed no new Rust type: it is reached as `&dyn TimeCard`, the same
one-struct-for-both-implementations shape divergence 23 already settled.

**`MonthlyRequiredDaysOffRuleImpl` is not ported.** It is the only rule in the
family needing genuinely new infrastructure: `Property.getPlanningPeriodType()`/
`getPlanningPeriod()` (a "monthly planning period" concept distinct from the
pay period already carried), a standalone `DaysOffCalculator` class (not a DAO
method), and `EmployeeTimeOff`/`TORDistribution` entities — none ported.
Deferred the way `AnnualSalaryOverHoursRegRateRuleImpl` was deferred from
`regularrate`: a catalogue entry (`MonthlyRequiredDaysOffSrr`) exists, no
algorithm here yet.

**Nothing else needed new Rust-side surface.** `EmployeeShift` already carried
`shift_date`/`net_hours`/`start_date_time`/`end_date_time`;
`translate_dow_from_iso` and `ids_for_key` already existed for day-of-week and
multi-select parameters; `PropertyPort::period_end_date` plus
`WeeklyDateRange::with_end_date(...).range_containing_date(date)` — the same
mechanism `FLSAOTRateRuleImpl`/`CombinationJobsRegRateRuleImpl` already use —
covered `Property.getWorkWeekForDate`. The one addition was
`ShiftErrorType::name()` (below).

### Deliberate divergences (continued)

69. **`ShiftErrorType::name()` added, matching Java's default `Enum.toString()`.**
    `ScheduleRestrictionResult.getMessage()` is the first call site in the
    tree to need an enum's *unparsed* Java constant spelling (`"REQUIRED_DAYS_OFF"`,
    not the `code()` used everywhere else) — an exhaustive match over all 28
    variants rather than a name-mangling transform, so it can't drift silently
    if a future variant's screaming-snake spelling doesn't cleanly reverse its
    Rust `PascalCase` name.

70. **`ScheduleRestrictionRule::is_strict` reads through `.fixed()`, not the
    raw params `BaseScheduleRestrictionRuleImpl.isStrict` reads.** Java
    intentionally skips `fixMap`: `params.get(STRICT_MODE)`, `null` treated as
    `false`. Reading it through this crate's uniform `.fixed()` shape
    (divergence 9) lands on the same answer here because the config's own
    default is also `"false"` — no behavior changes, only the mechanism.
    `NoRestrictionRuleImpl` still overrides to a hardcoded `false` rather than
    reading any parameter, the one override in the family.

71. **`MaxHoursOnDayRuleConfig`/`MaxHoursPerWeekRuleConfig`'s `HOURS_PER_DAY`/
    `HOURS_PER_WEEK` bounds are inlined as `24.0`/`168.0`.**
    `com.unifocus.framework.datetime.DateTimeConstants` is not ported — these
    are the first two rules in the tree to read either constant, and neither
    is used anywhere else yet.

## `schedulelunch` — done

**Second of the seven families the user prioritized for scheduling.** 28
tests, 19 of them transcribed Java assertions, full spec coverage across all
three catalogue entries: `LunchSimpleRuleImpl`, `LunchAdjustEndTimeRuleImpl`,
`LunchStartTimeAndLengthRuleImpl`.

| Ported | Java spec cases |
|---|---:|
| `LunchSimpleRuleImpl` | 7/7 |
| `LunchAdjustEndTimeRuleImpl` | 8/8 |
| `LunchStartTimeAndLengthRuleImpl` | 11/11 |

**The first family to write through the `adj_hours` stand-in, not just read
it.** Every rule inserts an unpaid lunch break by deducting hours from a
shift that was scheduled without one — `ScheduleLunchRuleImpl.addAdjustmentToShift`
builds a full `EmployeeShiftAdjustment` audit record (reason, changing user,
timestamp, the owning `RuleItem`) and appends it to the shift's adjustment
list in Java. `EmployeeShift` already stood in for that whole entity with a
plain `adj_hours: f64` scalar (see that module's own doc), but nothing before
this family had needed to *write* to it — only read the aggregate. The new
[`EmployeeShift::apply_adjustment`](crate::entity::employee_shift::EmployeeShift::apply_adjustment)
reproduces `calcAdjustments`' arithmetic for one `BREAK`-type entry
(`adjHours -= adjustment; netHours = workedHours + adjHours`) with no audit
trail behind it — see divergence 72.

**Two of the three rules also move a punch outright.**
`LunchAdjustEndTimeRuleImpl` and (conditionally)
`LunchStartTimeAndLengthRuleImpl` call `EmployeeShiftPunch.setAllTimes` on
the shift's OUT punch — writing the raw punch time itself, not just a
rounding — to push the shift's end out to cover the new break. Neither the
punch-type lookup nor a three-times-at-once write existed before this
family; see divergence 73.

**`LunchStartTimeAndLengthRuleImpl` needed `shiftearning`'s own
`ShiftTimeWindowUtility`, ported early** — see divergence 74 and
`shiftearning`'s own module doc.

### Deliberate divergences (continued)

72. **`EmployeeShift::apply_adjustment` is the write-side of the
    `adj_hours` stand-in for the unported `EmployeeShiftAdjustment` entity.**
    It reproduces exactly the arithmetic `calcAdjustments` runs for a
    `BREAK`-type adjustment — nothing else, since no ported rule needs the
    `WORKED`/`OT`/`DT` branches that arithmetic also has. There is no
    adjustment list, so `shift.getAdjustments()` (which the Groovy specs
    assert against directly — size, `adjHours`, `reason`, `changedByUser`,
    `source`, `adjType`, `ruleItem`) has nothing to port to; the transcribed
    `java_parity_tests` assert `shift.adj_hours()` and, where relevant, the
    moved punch's three times instead — a strictly narrower set of
    assertions than the Java spec makes, the same reduction the crate's
    general adjustments stand-in already accepted.

73. **`PlannedShift` is a new, narrow entity: two fields of the Java
    entity's several dozen.** `EmployeeShift::punch_index_of_type` (`getPunchOfType`)
    and `PunchCursor::set_all_times` (`EmployeeShiftPunch.setAllTimes`) are
    new surface on the existing punch-cursor design (divergence 13's
    successor) rather than a new abstraction: `set_all_times` writes the raw
    punch time and the adjusted time directly (no callback, matching Java),
    then delegates to the existing `set_rounded_time` for the third write,
    which is the one that fires `resetStartAndEndTimesFromPunch`.

74. **`ShiftTimeWindowUtility` is ported from `shiftearning.utility` ahead of
    the rest of that family.** `LunchStartTimeAndLengthRuleImpl` is the only
    rule needing it so far; `DTUtil.convertStartTime`/`convertEndTime`,
    which it depends on in Java, are not separately ported — their two
    operations (`date.at_time(time)`, and the same "roll to next day if the
    end time is before the start time" pattern `EarliestStartLatestEndTimeRuleImpl`
    already needed in `schedulerestriction`) are inlined as private helpers
    instead of standing up a `DTUtil` module for two call sites.

75. **`ScheduleLunchRule::execute` takes one `RuleItem`, not a `RuleItem`
    plus a separately-passed params map.** Java's abstract method signature
    carries both even though every concrete rule reads only
    `ruleItem.getParams()` — the same redundant shape divergence 9 already
    resolved for the rate families, applied again here.

## `shiftadjustment` — done

**Third of the seven families the user prioritized for scheduling.** 46
tests, 25 of them transcribed Java assertions. All seven catalogue entries
have an algorithm: `NoAdjustmentRuleImpl`, `AutoBreakRuleImpl`,
`MinBreakRuleImpl`, `PaidBreakRuleImpl`, `TotalBreakLengthRuleImpl`,
`DSTAdjustmentRuleImpl`, `MinDailyHrsRuleImpl`.

| Ported | Java spec cases |
|---|---:|
| `NoAdjustmentRuleImpl` | no spec — behaviour test written from the Java |
| `AutoBreakRuleImpl` | 3/3 `where:` tables, narrowed — see below |
| `MinBreakRuleImpl` | 5/5 |
| `PaidBreakRuleImpl` | 5/5 |
| `TotalBreakLengthRuleImpl` | 20/24 rows — the four `shift: null` rows are not transcribed, since `execute` takes `&mut EmployeeShift`, not a nullable reference |
| `DSTAdjustmentRuleImpl` | no spec — behaviour tests written from the Java |
| `MinDailyHrsRuleImpl` | no spec — behaviour tests written from the Java |

**Same adjustment-writing shape as `schedulelunch`, extended to a second
adjustment type.** `EmployeeShift::apply_adjustment` (renamed from
`apply_break_adjustment`, which `schedulelunch` introduced) now takes an
explicit `ShiftAdjustType`: `DSTAdjustmentRuleImpl` and `MinDailyHrsRuleImpl`
write `WORKED`, which *adds* the stored value to the shift's total rather
than subtracting it. See divergence 76's arithmetic note and the family's own
module doc for the full explanation, including the counterintuitive sign
`AutoBreakRuleImpl` stores.

**A new per-shift flag, not a provenance-tagged adjustment list.**
`AutoBreakRuleImpl.hasNoScheduleLunchAdjustment` walks the (unported)
adjustments list for one created by a `schedulelunch` rule.
`EmployeeShift::has_schedule_lunch_adjustment` narrows this to a boolean,
set by `schedulelunch`'s own three rules — see divergence 76.

**`MinBreakRuleImpl`/`PaidBreakRuleImpl`/`TotalBreakLengthRuleImpl` needed no
new surface** beyond what `apply_adjustment` and `EmployeeShift::breaks`
(already ported, matching `ShiftUtil.getBreaks(shift, false)`) already
provide — confirming, the same lesson every prior family's scoping already
taught, that reading the Java before adding surface finds less missing than
expected. `EmployeeShift::total_break_time_in_minutes` moved from private to
`pub` for `TotalBreakLengthRuleImpl`, since it is a direct call site in Java,
not only an internal helper of `calcWorkedHours`.

### Deliberate divergences (continued)

76. **`EmployeeShift::apply_adjustment` takes an explicit `ShiftAdjustType`
    (`WORKED` or `BREAK`) rather than being break-only.** `schedulelunch`
    first wrote it as `apply_break_adjustment`; this family is the first to
    need `WORKED`, so it generalized. The `hours` parameter is always the
    adjustment's own **stored** value, sign included — never a magnitude the
    caller expects added or subtracted by convention. This matters concretely
    for `AutoBreakRuleImpl`: it stores `-hrsAdjustment` on a `BREAK`-type
    adjustment, and `calcAdjustments`' `BREAK` branch is `adjHours -=
    adj.getAdjHours()`, so subtracting a negative number *increases* the
    shift's total — the rule's net effect is to add `hrsAdjustment` to net
    hours, not remove it, despite the name and the negative sign at the call
    site. `OT`/`DT` adjustment types are a no-op in `apply_adjustment` — no
    ported rule anywhere in the tree writes either, and Java folds them into
    separate `adjOTHours`/`adjDTHours` fields nothing here reads.

77. **`EmployeeShift::has_schedule_lunch_adjustment` replaces a
    provenance-tagged adjustments list with a boolean flag.**
    `AutoBreakRuleImpl.hasNoScheduleLunchAdjustment` walks the shift's
    adjustments for one whose owning `RuleItem.getRuleSet().getRuleType() ==
    SCHEDULE_LUNCH`; this crate carries no adjustments list at all (the
    `adj_hours` scalar stand-in predates this family — see `schedulelunch`'s
    own doc), so `schedulelunch`'s three rules set the flag directly via
    `mark_schedule_lunch_adjustment` when they apply their own adjustment,
    and `AutoBreakRuleImpl` reads it. Narrower than Java (it cannot
    distinguish "touched by schedule lunch" from "touched by schedule lunch
    and then something else"), but nothing in the ported rule set needs that
    distinction.

78. **`MinDailyHrsRuleImpl` and `DSTAdjustmentRuleImpl` build their
    adjustment's stored value with each rule's own rounding, not a shared
    helper.** `DSTAdjustmentRuleImpl` does not round its `adjustmentInHours`
    at all; `MinDailyHrsRuleImpl` rounds with `TDouble.round(_, 2)`
    (`round_hours` — the same function `netHours` itself uses, per
    `common/numbers.rs`'s table), not `roundRawHours` the way every
    `BREAK`-type rule in this family does through
    `ShiftAdjustmentRuleImpl.createAdjustment`. Each is reproduced exactly as
    its own rule rounds, not unified.

79. **`MinDailyHrsRuleConfig`'s overlapping-tier-values validation is not
    ported.** Java additionally rejects configurations where two of the
    three `minDailyHrs*`/`minWorkedHrs*` tiers share a nonzero value; nothing
    in the ported algorithm reads the result differently for overlapping
    versus non-overlapping tiers, and no ported behaviour test configures
    more than one nonzero tier at a time.

## `shiftcorrection` — done

Fourth of the seven scheduling-prioritized families. **44 tests, 17 of them
transcribed Java assertions**: all 14 cases of
`SplitToScheduleCorrectionRuleImplTest.groovy`, the config spec's defaults and
validation, and the runner spec's single case. `OutPunchCorrectionRuleImpl`
and `ShiftCategoryCorrectionRuleImpl` have no Groovy spec; their behaviour
tests are written from the Java.

| Ported | Java spec cases |
|---|---:|
| `OutPunchCorrectionRuleImpl` | no spec — behaviour tests written from the Java |
| `ShiftCategoryCorrectionRuleImpl` | no spec — behaviour tests written from the Java |
| `SplitToScheduleCorrectionRuleImpl` | 14/14 |
| `ShiftCorrectionRunner` | 1/1, plus two added |

### Findings

- **`RuleItemPriorityComparator` is used by five runners, not one.** The
  comment in `rules/priority.rs` said only `PostPunchRuleRunner` calls it;
  `grep` shows `ShiftCorrectionRunner`, `ShiftDifferentialRunner`,
  `DailyDifferentialRunner` and `ScheduleChangeValidationRunner` do too. The
  comment is corrected. The two differential runners and
  `ScheduleChangeValidationRunner` will need to sort when they are ported.
- **`ShiftCategoryCorrectionRuleImpl`'s graces are named backwards.** The
  window opens `gracePostSchedStart` minutes before the punch and closes
  `gracePreSchedStart` minutes after it. Invisible at the 60/60 default.
- **This is the first rule family that writes shifts.** `SplitToSchedule`
  creates and persists shifts and appends them to the time card, so its
  `execute` takes `&mut dyn TimeCard` (the one family that does) and the
  family needed `EmployeeShiftSavePort`.
- **`OutPunchCorrectionRuleImpl` gets its end time through a setter-ordering
  side effect.** It sets the new punch's rounded time *after*
  `setEmployeeShift`, which fires the shift callback; `addPunch` itself never
  touches start/end. `EmployeeShift::add_punch` mirrors `addPunch` (no start/end
  change) and the rule then fires the callback explicitly.
- **Neither `EmployeeShift` nor its collections override `equals`**, so
  `findContiguousSchedules`' `!=` and `List.contains` are identity, not value,
  comparisons — the port compares positions.

### Deliberate divergences (continued)

80. **The runner corrects a clone and writes it back at the same index.**
    Java hands each rule the live shift and the live time card. A
    `&mut EmployeeShift` borrowed from the card cannot coexist with the
    `&mut dyn TimeCard` `SplitToSchedule` appends to. Shifts a rule appends
    land after the originals and are not visited this pass, matching Java's
    copied iteration list. During `execute` the card still holds the
    *pre-correction* original; none of the three rules reads it.

81. **`OutPunchCorrectionRuleImpl` backfills its parameter from the defaults.**
    Java never calls `fixMap` there, so a rule item with no `dailyHours`
    throws `NullPointerException` in `Double.parseDouble`. The port reads 8.00.

82. **`ShiftCategoryCorrectionRuleImpl`'s swapped graces are reproduced, not
    fixed** (see Findings).

83. **Missing IN/OUT punches and null times skip rather than throw.** Java
    dereferences `getPunchOfType(OUT)` and `getAdjTime()` unchecked; the port
    returns without changing the shift. Unreachable for shifts that passed the
    rules' own gates.

84. **`EmployeeShiftDAO.save` is its own narrow port, and later changes to a
    saved split are not re-saved.** Java saves the split, then may mutate it
    again when a later schedule cuts it further, relying on the Hibernate
    session to flush. The port calls `EmployeeShiftSavePort::save` once per
    split at creation time; the persisted row for a split that is cut again
    would be stale until the caller saves the card. The spec counts `save`
    calls, which match.

85. **Not modeled on the copied schedule:** `assignment` (the shift has no such
    field here) and `flags`. `setAdjustments().clear()` becomes clearing the
    schedule-lunch flag (divergence 77) — the `adjHours` scalar is left as
    copied, as in Java where `clear()` does not touch it either.

## Scoping — `dailyearning`

`RuleType::DailyDifferential` (`DD`). Java directory `dailyearning`; runner
`DailyDifferentialRunner` (extends `DifferentialRunner`). **9 catalogue rules,
10 `*Impl` files** (the tenth is the abstract base `DailyEarningRuleImpl`),
~900 lines of rules, ~1,100 lines of Groovy rule specs, 8 config specs and one
runner spec. Measured from the Java and the Rust tree; nothing ported yet.

Unlike every family so far these rules are not driven per shift: the runner
calls `execute(TimeCard, LocalDate, RuleItem)` **once per calendar day** from
the calculation start date to `max(maxActualDate, currentPayPeriodEnd)`, and
each rule writes earnings for that day. The runner first clears the card's
earnings that earlier daily runs produced, and sorts items by priority
(`RuleItemPriorityComparator`, see `shiftcorrection` findings).

| Rule | Lines | Spec | What it does |
|---|---:|---|---|
| `NoDailyEarningRuleImpl` | 17 | none | no-op |
| `MinDailyHoursEarningRuleImpl` | 119 | 180 | pays a fixed amount if the day's *original* distribution hours reach a threshold, optionally gated on the day's schedule label |
| `MinDailyWorkedHoursEarningRuleImpl` | 112 | 140 | same gate, but pays the *worked hours* at a rate factor |
| `NoDailyHoursWithLabelEarningRuleImpl` | 108 | 101 | pays when the day has **zero** hours and a matching schedule label |
| `MinimumHoursWorkedDifferentialRuleImpl` | 86 | 192 | tops a short day up to a minimum, priced off the primary shift; uses `MinimumHoursWorkedRuleHelper` |
| `DaysWorkedDailyDifferentialRuleImpl` | 130 | 242 | pays a fixed dollar amount when fewer than N days were worked in the work week |
| `HolidayDifferentialRuleImpl` | 230 | 249 | two-factor holiday pay per shift, with max/min hours; the largest rule |
| `DailySalariedAllowanceRuleImpl` | 87 | 205 (misfiled, see slice 2) | allowance for salaried-exempt employees on configured weekdays |
| `AnniversaryEarningRuleImpl` | 132 | 241 | pays on hire / seniority / birthday anniversaries |

### What is missing on the Rust side

None of these needs a new *kind* of machinery; all of it is entity surface and
three ports:

1. **Employee status.** `Employee.isActiveOnDate` (eight of the nine rules)
   walks `EmployeeStatus` records — `getStatusForDate`, and the
   `ACTIVE`/`REHIRE`/`LEAVE_OF_ABSENCE`/terminated cases. The Rust `Employee`
   holds job statuses but no employee statuses at all.
2. **`TimeCard` day accessors:** `getShiftsForDay`,
   `getShiftsEndingOrStartingOnDay`, `getMaxActualDate`, and **schedule
   labels** (`EmployeeScheduleLabel` entity, `getScheduleLabels`,
   `getScheduleLabelForDate`) — three rules gate on the day's label.
3. **The base class's pricing helpers**, `getEarningRate(shift, ...)` and
   `getEarningRateHomeJob(date, ...)`: hourly rate from the job status,
   floored at minimum wage when the earning type says so. `MinWagePort` covers
   the wage lookup.
4. **Earning construction.** Every rule builds an `EmployeeEarning` with the
   rule item, source `RULE`, note from `RuleUtils.getRuleDescription`, and
   `calcAndSetTotalDollars`. Existing earning-writing rules (`earningrate`,
   `CaliforniaOTHrs`) already do this; a shared builder is warranted.
5. **Ports:** `EarningTypePort`, `EmployeeEarningPort`, `HolidayPort` and
   `MinWagePort` exist. `Property.getWorkWeekForDate` (`DaysWorked`) and
   `PropertyDataPort::value_as_boolean(showDailyDifferentialEarningsOnEmptyTimeCards)`
   (runner) need checking against what `PropertyPort`/`PropertyDataKey` hold.
6. **Smaller:** `ShiftUtil.getNetHours(shift, useActual)` (`Holiday`),
   `AnniversaryDateTypes` enum and a JSON string-array parse
   (`Anniversary`), `DefaultPropertyFactory` parameter-key constants, the
   `DifferentialRunner`'s `ClearTimeCardEarnings`.

### Things to watch

- **`MinDailyWorkedHoursEarningRuleImpl` backfills with the wrong config.**
  It calls `fixMap` on a `MinDailyHoursEarningRuleConfig`, not its own
  `MinDailyWorkedHoursEarningRuleConfig`. The defaults differ where it
  matters: `rateFactor` is `"0"` in the former and `"1"` in the latter, so a
  worked-hours rule item with no stored `rateFactor` prices at **0**, not 1.
  (The former also adds an unused `amount` = `"1"`.) Reproduce the Java and
  record it as a divergence-free quirk; do not "fix" it.
- `HolidayDifferentialRuleImpl` calls `createEarning(..., shift = null, ...)`
  in the make-up branch when no non-home-job shift exists; the null shift
  switches the rate source to the home job.
- Anniversary uses `date.withYear(current.getYear())`; Joda's behaviour for a
  Feb-29 hire date in a non-leap year (clamps to Feb 28) must be matched.
- The runner reads `ruleSet.getRuleItems()` and **sorts it in place**; the
  port sorts a local view (as `shiftcorrection` did).

### Plan

Port in three slices, each with its own commit: (1) the shared surface above
plus `NoDaily`, `MinDailyHours`, `MinDailyWorkedHours`, `NoDailyHoursWithLabel`
(the four label-gated, near-identical rules, ported together per "Port
siblings together"); (2) `MinimumHoursWorked`, `DaysWorked`,
`DailySalariedAllowance`, `Anniversary`; (3) `HolidayDifferential` and the
runner.

## `dailyearning` — slice 1

The shared surface and four rules: `NoDailyEarning`, `MinDailyHoursEarning`,
`MinDailyWorkedHoursEarning`, `NoDailyHoursWithLabelEarning`. **40 tests, 8 of
them transcribed Java assertions** (5 of `MinDailyHoursEarningRuleImplTest`, 3
of `NoDailyHoursWithLabelEarningRuleImplTest`). `NoDaily` and
`MinDailyWorkedHours` have no Groovy spec; their tests are written from the
Java. The config specs' name/class/property-sheet assertions have no
counterpart here (the port has no property sheets); their default-value and
validation behaviour is covered by `config.rs`'s own tests.

New surface: `EmployeeStatusType`, `EmployeeStatus`, `EmployeeScheduleLabel`,
`Employee::{with_statuses, status_for_date, is_active_on_date}`,
`TimeCard::{schedule_labels, schedule_label_for_date}`, and the family's
`DailyEarningRule` trait with its shared helpers (`should_not_run_for_date`,
`original_hours_on_day`, `schedule_label_matches`, `earning_rate_home_job`).
The three paying rules each take `EarningTypePort`, `EmployeeEarningPort` and
`MinWagePort`.

| Ported | Java spec cases |
|---|---:|
| `NoDailyEarningRuleImpl` | no spec |
| `MinDailyHoursEarningRuleImpl` | 5/5 |
| `MinDailyWorkedHoursEarningRuleImpl` | no spec — behaviour tests written from the Java |
| `NoDailyHoursWithLabelEarningRuleImpl` | 3/3 |

### Findings

- **`MinDailyHoursEarningRuleImplTest."rule is not run if employee is not
  active on date"` is vacuous.** It terminates the employee on `today` and
  runs `today`; `isActiveOnDate` counts a terminated employee *on the
  termination day*, so the rule does run, and no earning follows only because
  the card has no shifts. The port's version runs the day after, so the
  inactivity is what withholds the earning.
- **The label-gated rules measure hours differently from the shift rules.**
  They sum `originalHours` over distributions dated that day, not shift
  `netHours`; a day with distributions on another date does not count.
- **`MinDailyWorkedHoursEarningRuleImpl` backfills from the wrong config**
  (confirmed by test): an unset `rateFactor` prices the earning at 0.
- **`MinDailyHoursEarningRuleImpl` never calls `calcAndSetTotalDollars`
  itself**; the totals are right only because `setHours`/`setRate`/
  `setDollars` each recompute it.

### Deliberate divergences (continued)

86. **`Employee::is_active_on_date` reads a new `statuses` list; an employee
    built without statuses is inactive on every date.** Java's `Employee`
    always has them. Existing fixtures that never set statuses are unaffected
    because no earlier rule asks.

87. **A missing earning type, home job, or employee skips the day instead of
    throwing.** Java dereferences `earningTypeDAO.findByID(...)`,
    `getHomeEmployeeJobStatus(date)` and `timeCard.getEmployee()` unchecked.
    Same stance as divergence 83.

88. **The day's hours are summed with a plain left fold, not
    `DoubleStream.sum()`.** Java's stream sum uses Kahan compensation; the two
    can differ in the last bit over many distributions. Every threshold
    comparison here (`> 0`, `>= minQualifyingHours`, `== 0.0`) is exact, so a
    one-ulp difference would matter only at a boundary no fixture reaches.

## `dailyearning` — slice 2

`MinimumHoursWorkedDifferential`, `DaysWorkedDailyDifferential`,
`DailySalariedAllowance` and `AnniversaryEarning`, with their four configs.
**55 tests, 30 of them transcribed Java assertions**: 7 executions of
`MinimumHoursWorkedDifferentialRuleImplTest` (the two `where:` rows of one
case count separately), 16 of `DaysWorkedDailyDifferentialRuleImplTest`, and
all 7 of `AnniversaryEarningRuleImplTest`. `DailySalariedAllowance` was first recorded here as having no Groovy spec;
that was wrong (see the correction in the table and "`weeklyearning` —
done").

| Ported | Java spec cases |
|---|---:|
| `MinimumHoursWorkedDifferentialRuleImpl` (+ `MinimumHoursWorkedRuleHelper`) | 7/7 |
| `DaysWorkedDailyDifferentialRuleImpl` | 16/16 |
| `DailySalariedAllowanceRuleImpl` | 7/7 — **corrected**: this row originally read "no spec"; the spec exists as `weeklyearning/DailySalariedAllowanceRuleImplTest.groovy` and was transcribed with `weeklyearning` |
| `AnniversaryEarningRuleImpl` | 7/7 |

New surface: `TimeCard::shifts_for_day`, `EarningType::is_dollars_type`,
`AnniversaryDateTypes` + `parse_anniversary_types`, and the shared
`earning_rate_for_shift`.

### Findings

- **These four rules do not share slice 1's shape.** Each differs on something
  a copy-paste port would get wrong:
  - `DaysWorked` never checks `isOpenForEditingOn`, and reads the parameters
    without `fixMap`.
  - `DailySalariedAllowance` **saves its earning but never adds it to the
    card**, and sets no note.
  - `Anniversary` takes no `EmployeeEarningPort` at all (it only appends to
    the card) and pays at the **raw** home hourly rate, with no minimum-wage
    floor.
  - `MinimumHoursWorked` prices off the day's *primary shift's* job, not the
    home job, and measures hours by shift `netHours`, not distributions.
- **`Stream.max` keeps the first of equal maxima; Rust's `Iterator::max_by`
  keeps the last.** `MinimumHoursWorked`'s primary-shift choice (latest OUT
  punch) would have flipped on a tie. The port compares with strict `>` and is
  covered by a tie test.
- **`joda_rs::LocalDate::with_year` panics on Feb 29 in a non-leap year;
  Joda's `withYear` clamps to Feb 28.** `Anniversary` does the clamp by hand.
  A leap-day birthday therefore pays on Feb 28 in common years, Feb 29 in leap
  years. Any future caller of `with_year` on a user-supplied date has the same
  trap.
- **`AnniversaryDateTypes` is not in the `taps` checkout.** It lives in
  `com.unifocus.watson.common.ta.anniversarydates`, which a search of the
  monorepo did not find. The three constants, their `toString()` names and
  their declaration order are reconstructed from the rule and its spec (see
  the module doc of `common/enums/anniversary_date_types.rs`).
- **`DaysWorked`'s spec is partly weak.** "Receives no extra fixed Earnings if
  they didn't work at all" passes because zero worked hours yields a zero
  amount, which would also pass if the employee-eligibility branch were
  broken. The other cases do exercise it.
- **The Groovy mocks `ActualsTimeCard` in `MinimumHoursWorked`'s first three
  cases** (`Mock(Employee)`, `getShiftsForDay(_) >> []`). They are transcribed
  against a real `TimeCardData`, which makes them stronger: the "outside the
  calculation period" case now runs against a genuinely closed date.

### Deliberate divergences (continued)

89. **`DaysWorkedDailyDifferentialRuleImpl` backfills its parameters from the
    config defaults.** Java reads them straight off the rule item with no
    `fixMap`, so a missing one throws. Same stance as divergence 81.

90. **Anniversary config parsing is lenient where `JSONArray`/`valueOf` throw.**
    Unknown date-type names are dropped and a value that is not a bracketed
    list yields no types, instead of an exception. A rule with nothing
    configured pays nothing either way.

91. **Earning notes use `RuleItem::name` as a stand-in for the rule
    description.** `RuleUtils.getRuleDescription` is `"Rule: <rule set name> -
    <config display name>"`, built from a rule set and a localized config name
    that the Rust `RuleItem` does not carry; every note in this family
    (and `CaliforniaOTHrs` before it) takes the item's `name`, so a test that
    wants Java's text builds the item with that text as its name. Two notes are
    built differently in Java and reproduced as such: `Anniversary` appends
    `" (<Type>)"`, and `DaysWorked` writes `"Rule: " + ruleItem.toString()`,
    i.e. `"Rule: RuleItem: <name>"`.

## `dailyearning` — slice 3

`HolidayDifferentialRuleImpl`, its config, the nine-class registry, and
`DailyDifferentialRunner` with `ClearTimeCardEarnings`. **20 tests, 25 of them
transcribed Java assertions** (a test function may cover several Groovy
`where:` rows): all 21 rows of `HolidayDifferentialRuleImplTest` (16 + 2 + 3)
and all 4 of `DailyDifferentialRunnerTest`. **The family is complete.**

| Ported | Java spec cases |
|---|---:|
| `HolidayDifferentialRuleImpl` | 21/21 |
| `DailyDifferentialRunner` | 4/4 |
| `ClearTimeCardEarnings` | no spec — behaviour tests written from the Java |

New surface: `EmployeeShift::net_hours_using(use_actual)` (`ShiftUtil.getNetHours`),
`TimeCard::{shifts_ending_or_starting_on_day, max_actual_date}`, and
`DailyEarningRegistry`.

### Findings

- **The minimum can push a day past its cap.** `HolidayDifferential` pays the
  shortfall to `minHrsPaid` after the capped loop without re-checking
  `maxHrsPaid`. The spec's "should only pay the primary factor up to the max
  hrs per day" row (max 5, min 6 by default) therefore writes a *second*
  earning of one hour, taking the day to 6. The spec's mock only matches the
  first earning and has no `0 * makePersistent(_)` in that test, so it passes
  unaware. The config validator rejects `max < min`; the rule does not.
- **The "home property not on a holiday" and "wrong holiday date" scenarios do
  not test what they say.** The employee's *home* job is always the
  property-A job, which does have a holiday today; those rows' shifts are on
  other-property jobs the employee holds no status for, so they are rejected as
  inactive jobs. The holiday-calendar lookup on a property *without* a holiday
  is exercised only by the port's own tests.
- **`actualPunchTime = true` is never exercised by the Groovy** (every row sets
  it `false`). `net_hours_using(true)` is covered by a behaviour test written
  from `ShiftUtil.getNetHours`.
- **`RuleItemPriorityComparator` is inert in this family.** None of the nine
  configs extends `PriorityRuleConfig`, so every item ranks 5 and the stable
  sort leaves rule-set order alone. The port sorts anyway, as Java does.
- **`getMaxActualDate` reads hours-distribution dates, not shift dates**, so a
  shift with no distributions does not extend the run.
- **`Employee.getMinWage` is never reached by the Groovy** for the holiday
  rule (the earning type does not pay at least minimum wage); the floor is
  covered by slice 1's tests of the shared helper.

### Deliberate divergences (continued)

92. **The runner takes `rule_type_of_item` to decide what to clear.** Java walks
    `earning.getRuleItem().getRuleClass().getRuleType()`; an
    `EmployeeEarning` here carries only its rule item's id.

93. **Clearing removes an unpersisted earning (id 0) from the card without a
    delete, and treats an earning whose shift is not on the card as open.**
    Java's entities are always fully linked; ids here are not assigned back by
    `save`, so an id of 0 is the only marker of "never stored".

94. **Missing adjusted punch times count as zero hours in
    `net_hours_using(true)`**, where Java would throw on the unchecked
    `getAdjTime()`. Same stance as divergence 83.

## `weeklyearning` — done

All eight catalogue entries and the runner: `NoWeeklyEarning`, `WeeklyHrsFixed`,
`DOWWeeklyHrsRateFactor`, `DOWHrsFixed`, `DailySpread`, `SalariedMinWage`,
`HoursThresholdFixedWeekly`, `DailyWeeklyGuaranteedHours`, with their configs
and the eight-class `WeeklyEarningRegistry`. **70 tests, 49 of them transcribed
Java assertions.** The shared `ClearTimeCardEarnings` was lifted out of
`DailyDifferentialRunner` into `rules/runner/clear_time_card_earnings.rs`.

| Ported | Java spec cases |
|---|---:|
| `NoWeeklyEarningRuleImpl` | no spec |
| `WeeklyHrsFixedRuleImpl` | no spec — behaviour tests written from the Java |
| `DOWHrsFixedRuleImpl` | no spec — behaviour tests written from the Java |
| `SalariedMinWageRuleImpl` | no spec — behaviour tests written from the Java |
| `DailySpreadRuleImpl` | 4/4 `where:` rows |
| `DOWWeeklyHrsRateFactorRuleImpl` | 18/18 rows |
| `HoursThresholdFixedWeeklyRuleImpl` | 8/8 |
| `DailyWeeklyGuaranteedHoursRuleImpl` | 9/9 |
| `WeeklyDifferentialRuleRunner` | 3/3 (two rows + one case) |
| `DailySalariedAllowanceRuleImpl` (from `dailyearning`) | 7/7 — the misfiled spec |

New surface: `PeriodSalaryPort`, `Employee::is_active_home_job_status_all_period`,
`weeklyearning::net_hours_legacy`.

### Findings

- **`DailySalariedAllowanceRuleImplTest.groovy` is filed under `weeklyearning`
  but tests the `dailyearning` rule.** Slice 2 reported that rule as having no
  spec. Its seven cases are now transcribed in `dailyearning/daily_salaried_allowance.rs`
  and the earlier text is corrected above. One case showed the earning type is
  never inspected — the spec's `EarningTypeDAO` mock answers null and still
  expects one save — so the rule now uses the configured id directly instead of
  requiring the type to resolve.
- **There are two different `getNetHours`.** `WeeklyHrsFixed` and `DOWHrsFixed`
  use the base class's deprecated one; the other two use `ShiftUtil`'s. They
  disagree on rounding (unrounded vs 2 places for stored hours), on how punches
  are read (every in/out pair summed vs first-to-last less breaks), and on
  schedules (`ShiftUtil` exempts them from the errors test). A test pins the
  difference.
- **`DailyWeeklyGuaranteedHours`'s spec barely checks its saves.** Its
  `1 * employeeEarningDAO.save({matcher})` passes a closure that *returns* the
  matcher closure, which Groovy treats as true for any argument, so only the
  call **count** is asserted; the content is asserted separately through
  `timeCard.earnings.any matcher`. The port's transcription checks both.
- **`DailySpread`'s spec only exercises the config defaults** and runs against
  mocks of the card and employee; the `useMinWage = false` and rounded-vs-actual
  punch branches are covered only by tests written from the Java.
- **Negative earnings.** `DailyWeeklyGuaranteedHours` deliberately writes
  *negative* hours to trim a day or week back to its maximum; downstream code
  that assumes positive hours would mishandle them.
- **A zero-hour configured day can pay.** `DOWHrsFixed` seeds every configured
  open weekday at 0 hours; with `minHours` of 0 such days would pay. The config
  validator forbids `minHours <= 0`; the rule does not look.
- **`WeeklyDifferentialRuleRunner` does not sort**, so rule items run in
  rule-set order — it is not one of the five `RuleItemPriorityComparator` users.
- **`SalariedMinWage` and `AnnualSalaryOverHoursRegRate` share a blocker** —
  `PayRateCalculator.getPeriodSalary` — and the one-method `PeriodSalaryPort`
  added here is also what the still-deferred rate rule (divergence 57) needs.
- **Display names in the configs' validation messages are placeholders**, here
  and in `dailyearning` (see divergence 98): Java builds them from localized
  `getName()` strings the crate does not carry. The validation *conditions* are
  exact.

### Deliberate divergences (continued)

95. **`WeeklyHrsFixed` and `DOWHrsFixed` backfill parameters from the defaults.** Java reads `WeeklyHrsFixed`/
    `DOWHrsFixed` straight off the rule item (their `ruleConfig` field is never
    used), so a missing parameter throws. Same stance as divergences 81 and 89.

96. **`DOWHrsFixed` writes its earnings in date order.** Java iterates a
    `HashMap<LocalDate, Double>`, whose order is unspecified.

97. **`DailySpread` skips a day with no home job.** Java throws
    `HomeJobNotFoundException`. Same stance as divergence 83, with the caveat
    that this one is an explicit, typed error rather than an accident.

98. **The description handed to each rule is `"Rule: <rule set> - <rule item
    name>"`**, where Java uses the rule's localized config name. This also
    applies to the placeholder display names in config validation messages.

99. **`SalariedMinWage` takes a `PeriodSalaryPort`** in place of
    `PayRateCalculator`. The port is a single scalar lookup; the calculator's own
    logic (annual rates, pay-period types) stays out of this crate.

## Scoping — `shiftearning`

`RuleType::ShiftDifferential` (`DF`). Java directory `shiftearning`; runner
`ShiftDifferentialRunner` (extends `DifferentialRunner`). The last of the seven
families prioritized for scheduling, and the largest of them: **23 catalogue
rules, 25 `*Impl` files** (the base `ShiftEarningRuleImpl`, plus
`NoEarningRuleImpl`, which has a catalogue entry), a `utility` subpackage, ~3,600
lines of rules, ~2,700 lines of configs and ~3,100 lines of Groovy rule specs
(12 rule specs, 12 config specs, one runner spec, one utility spec).
Measured from the Java and the Rust tree; **nothing ported yet**. The
`ShiftDiffOt` type is a different family: its one rule is the deferred
`ShiftDifferenceOTRuleImpl` and its runner `ShiftDifferenceOTRunner` is
separate (see the deferred list).

Unlike the daily and weekly families these rules are driven **per shift**, as
`punchrounding` and `shiftcorrection` were: `execute(EmployeeShift, TimeCard,
RuleItem)`. The runner skips shifts with errors, resolves the rule set per
shift (employee, job, shift date), **sorts by priority**
(`RuleItemPriorityComparator` — one of the five users listed under
`shiftcorrection`) and runs each item. Its clearing step removes earnings of
**three** rule types: `SHIFT_DIFFERENTIAL`, `HOURS_DISTRIBUTION` and
`POST_CALC` — so it also deletes earnings those families wrote.

| Rule | Lines | Spec | What it needs beyond today's surface |
|---|---:|---:|---|
| `NoEarningRuleImpl` | 26 | — | nothing (no-op) |
| `HoursThresholdFixedRuleImpl` | 70 | — | `ShiftUtil` |
| `DOWFlatRuleImpl` | 74 | — | `DateUtil` |
| `DOWRateFactorRuleImpl` | 85 | 130 | `DateUtil`, UOM |
| `WeeklyJobHrsFixedRuleImpl` | 90 | — | `Weeks`, week `DateRange` |
| `MinimumHoursWorkedShiftDifferentialRuleImpl` | 86 | 338 | `MinimumHoursWorkedRuleHelper` (ported in `dailyearning`) |
| `BreakPeriodPenaltyHoursRuleImpl` | 84 | — | break periods (`ShiftUtil.getBreaks`) |
| `PaidBreakFixedHrsRuleImpl` | 100 | — | break periods |
| `HoursThresholdFixedWithMaxCapRuleImpl` | 107 | 227 | `Weeks`, week `DateRange` |
| `ShiftStartTimeFlatRuleImpl` | 116 | — | date-time periods |
| `StartEndBetweenPremiumRuleImpl` | 122 | 107 | `ShiftTimeWindowUtility` (half in Rust already) |
| `ContractToFullTimeRuleImpl` | 117 | — | `Weeks`, contract hours, `RuleUtils` |
| `ContractGuaranteeRuleImpl` | 148 | — | contract hours |
| `TimeBetweenShiftsRuleImpl` | 139 | — | nothing new beyond shifts |
| `DailySpreadShiftDifferentialRuleImpl` | 158 | 143 | `ShiftNetHoursComparator` |
| `HolidayRateFactorRuleImpl` | 190 | 168 | `HolidayDAO` (ported), `JSONUtils` (ported) |
| `AnniversaryWorkedShiftDifferentialRuleImpl` | 195 | 176 | `AnniversaryDateTypes` (reconstructed in `dailyearning`) |
| `BreakPeriodPenaltyRuleImpl` | 211 | — | break periods |
| `HolidayTimeOfDayRuleImpl` | 171 | 206 | `HolidayMatcher`, `RuleUtils` |
| `TimeOfDayDifferentialRateRuleImpl` | 221 | — | date-time periods |
| `DOWHolidayRuleImpl` | 242 | 265 | `HolidayMatcher`, `Holiday`; reads (does not write) shift adjustments — corrected in slice 4 |
| `SplitShiftDifferentialRateRuleImpl` | 308 | 368 | **`SplitShiftPayTypes`**, **`CurrentProperty`** |
| `TimeOfDayPctDifferentialRateRuleImpl` | 343 | 521 | `RoundingOption`, date-time ranges |

(`HoursThresholdFixed`, `DOWFlat` and the other no-spec rules are exactly the
ones the audit's "no spec — behaviour tests from the Java" convention covers.)

### What is missing on the Rust side

1. **The base-class pricing helpers.** `ShiftEarningRuleImpl.getEarningRate`
   has four overloads (shift or job+date, earning type or earning id) and is
   what `dailyearning`'s `earning_rate_for_shift` already is for the first;
   the job+date form is new. A shared `ShiftEarningRule` trait.
2. **`EmployeeShiftAdjustment` as an entity.** *Resolved in slice 4: not needed.*
   This section originally said `DOWHolidayRuleImpl` "writes" an adjustment.
   It does not — `calcWorkedAdjustmentsNotReallocatedToEarning` only **reads**
   them, and the scalars `adj_hours` and `reallocated_to_earning_hours` carry
   exactly what it needs (divergence 110).
3. **Two enums/services not in the `taps` checkout:** `SplitShiftPayTypes`
   (`common.ta.splitshiftpaytypes`) and, from `CurrentProperty`, the "current
   property" the rule reads a fallback minimum wage from. Both will be
   reconstructed from the rule and spec, as `AnniversaryDateTypes` was.
4. **`HolidayMatcher`** (`algorithm/HolidayMatcher.java`), used by
   `HolidayTimeOfDay`; whether `HolidayRateFactor` and `DOWHoliday` use it or
   carry their own copy of the logic is for slice 4 to read.
5. **`Weeks`** (`tbx.core`), used by three weekly-cap rules; the work-week
   arithmetic itself exists (`WeeklyDateRange`).
6. **The runner**, `ShiftDifferentialRunner`, and the shared
   `clear_time_card_earnings` already take a rule-type list — it needs
   `[ShiftDifferential, HoursDistribution, PostCalc]`.

### Plan

Five slices, each its own commit:

1. **Shared base + simple rules (9):** the `ShiftEarningRule` trait and pricing
   helpers, `NoEarning`, `HoursThresholdFixed`, `DOWFlat`, `DOWRateFactor`,
   `WeeklyJobHrsFixed`, `HoursThresholdFixedWithMaxCap`, `TimeBetweenShifts`,
   `MinimumHoursWorkedShiftDifferential`, `ShiftStartTimeFlat`.
2. **Break and window rules (4):** `PaidBreakFixedHrs`, `BreakPeriodPenaltyHours`,
   `BreakPeriodPenalty`, `StartEndBetweenPremium` (+ `ShiftTimeWindowUtility`).
3. **Time-of-day family (3):** `TimeOfDayDifferentialRate`, `TimeOfDayPct`,
   `DailySpreadShiftDifferential` — the 521-line spec makes `TimeOfDayPct` the
   single hardest rule in the family.
4. **Holiday and anniversary (4):** `HolidayRateFactor`, `HolidayTimeOfDay`,
   `DOWHoliday`, `AnniversaryWorkedShiftDifferential`.
5. **Contract, split-shift and the runner (3 + runner):** `ContractToFullTime`,
   `ContractGuarantee`, `SplitShiftDifferentialRate`, `ShiftDifferentialRunner`
   and the registry.

Read the Java for each slice before estimating it: the "Measure before
scheduling" lesson has held for every family so far, and the dependency column
above comes from imports, not from reading the rule bodies.

## `shiftearning` — slice 1

The shared base and nine rules: `NoEarning`, `HoursThresholdFixed`, `DOWFlat`,
`DOWRateFactor`, `WeeklyJobHrsFixed`, `HoursThresholdFixedWithMaxCap`,
`TimeBetweenShifts`, `MinimumHoursWorkedShiftDifferential`,
`ShiftStartTimeFlat`, with their nine configs. **56 tests, 25 of them
transcribed Java assertions.** The registry and runner wait for slice 5.

| Ported | Java spec cases |
|---|---:|
| `NoEarningRuleImpl` | no spec |
| `HoursThresholdFixedRuleImpl` | no spec — behaviour tests written from the Java |
| `DOWFlatRuleImpl` | no spec — behaviour tests written from the Java |
| `DOWRateFactorRuleImpl` | 7/7 rows |
| `WeeklyJobHrsFixedRuleImpl` | no spec — behaviour tests written from the Java |
| `HoursThresholdFixedWithMaxCapRuleImpl` | 6/7 — the seventh asserts `fixMap` plumbing on a mock config |
| `TimeBetweenShiftsRuleImpl` | no spec — behaviour tests written from the Java |
| `MinimumHoursWorkedShiftDifferentialRuleImpl` | 12/12 rows |
| `ShiftStartTimeFlatRuleImpl` | no spec — behaviour tests written from the Java |

New surface: the `ShiftEarningRule` trait (`execute(&EmployeeShift, &mut dyn
TimeCard, &RuleItem)` — the runner passes a clone of the card's shift),
`shift_is_eligible` and `new_shift_earning`; `EmployeeEarning::{from_rule_item,
rule_class, set_earning_date_and_pay_date}`;
`EmployeeShift::{with_reallocated_to_earning_hours,
net_hours_with_reallocated_undone}`; `dailyearning::earning_rate_for_job`.

### Findings

- **Reallocated hours are real, and the Groovy proves it.** `DOWRateFactor`'s
  third `where:` row gives a shift 8 net hours and a `WORKED` adjustment of -2
  hours that was reallocated to an earning, and expects **10** hours: the rule
  prices `net - reallocated`. A first reading ("`getNetHoursWithReallocated...`
  is just `getNetHours`") would have been wrong.
- **`HoursThresholdFixedWithMaxCap` counts by rule *class*; its
  `weeklyearning` sibling counts by rule item.** The spec pins it with an
  earning whose rule item has the *same id* but a different class — which is
  not counted. `EmployeeEarning` therefore now records the item's class
  alongside its id.
- **Two rules check the period differently from the rest.** `WeeklyJobHrsFixed`
  tests only the work week's *end* date for being open, not the shift's own
  date (so it can pay for a week whose early days are closed);
  `HoursThresholdFixedWithMaxCap` checks the shift is open but counts prior
  earnings toward its cap whether their period is open or closed.
- **`WeeklyJobHrsFixed` pays once per job per week**, guarded by an earning of
  the same rule item, date and job already on the card (job compared by id,
  where Java compares object identity).
- **`TimeBetweenShifts`' parameter keys are capitalized phrases** (`Hours
  Between Shifts`, `Dollars`, `Earning`), like `MinBreakRuleConfig`'s. It
  treats a previous shift ending *exactly* at this one's start as a zero-hour
  gap, and it is the only rule here that refuses to write an earning if the
  earning type cannot be found.
- **`MinimumHoursWorkedShiftDifferential` backfills from the daily rule's
  config**, as `MinDailyWorkedHours` did from its sibling's — harmless here,
  since the only key the daily config lacks (`MinimumWage`) reads as `false`
  when absent either way.
- **`TDouble.roundCurrency` keeps four places, not two.** A test that assumed
  "rounded to cents" caught it; `dollars()` is 1.006 and only `totalDollars()`
  is 1.01. (`roundHours` and `round(x, 2)` are the two-place ones.)
- **`HoursThresholdFixed`, `DOWFlat` and `WeeklyJobHrsFixed` read their
  parameters without `fixMap`**, and `actualPunchTime` defaults to `true`: a
  rule item with no parameters computes hours from raw punches, so a shift with
  stored net hours but no punches is worth zero. Easy to trip over in a test.

### Deliberate divergences (continued)

100. **`HoursThresholdFixed`, `DOWFlat` and `WeeklyJobHrsFixed` backfill their
     parameters from the defaults.** Java reads them off the rule item with no
     `fixMap` (their `ruleConfig` field is unused), so a missing one throws.
     Same stance as divergences 81, 89 and 95.

101. **Reallocated-to-earning adjustments are one scalar on the shift.**
     `EmployeeShift::with_reallocated_to_earning_hours` stands in for walking the
     adjustments list for `WORKED` adjustments whose `reallocatedToEarning` is
     set — the same narrowing as `adj_hours` and the schedule-lunch flag
     (divergences 76-77). The sum is rounded to two places at load, where Java
     rounds after each addition.

102. **A shift with no IN punch skips `ShiftStartTimeFlat`**, where Java
     dereferences the null. Same stance as divergence 83.

## `shiftearning` — slice 2

The four break and window rules, with their configs: `PaidBreakFixedHrs`,
`BreakPeriodPenaltyHours`, `BreakPeriodPenalty`, `StartEndBetweenPremium`.
**26 tests, 11 of them transcribed Java assertions** — all 11 rows of
`StartEndBetweenPremiumRuleImplTest`. The other three have no Groovy spec;
their tests are written from the Java.

| Ported | Java spec cases |
|---|---:|
| `PaidBreakFixedHrsRuleImpl` | no spec — behaviour tests written from the Java |
| `BreakPeriodPenaltyHoursRuleImpl` | no spec — behaviour tests written from the Java |
| `BreakPeriodPenaltyRuleImpl` | no spec — behaviour tests written from the Java |
| `StartEndBetweenPremiumRuleImpl` | 11/11 rows |

New surface: `EmployeeShift::breaks_using(use_actual)` (the adjusted-time form
of `ShiftUtil.getBreaks`; `breaks()` is now its `false` case) and
`EmployeeShift::punch_time_of_type` (`ShiftUtil.getPunchTime`).
`ShiftTimeWindowUtility`'s `convert_start_time`/`convert_end_time` became
`pub(crate)`.

### Findings

- **`BreakPeriodPenalty`'s daily cap is shared across the day's shifts.** The
  count it starts from is every RULE earning of *this rule item* dated the
  shift's date — including those written for other shifts that day. Each shift
  is therefore not entitled to `maxDailyPenalty` penalties of its own.
- **A break can only satisfy one of `BreakPeriodPenalty`'s two tests.** The
  matched first break is removed from the list before the second test, so one
  early break covers the first meal period and leaves the second uncovered.
- **`BreakPeriodPenaltyHours` is cancelled by a single long-enough break
  anywhere in the shift**, not just an early one; and with no breaks at all it
  pays nothing (zero break time).
- **`PaidBreakFixedHrs` with `checkMinBreak` on requires the break to start
  strictly less than `breakLimit` hours after the first IN** (a break starting
  exactly at the limit does not qualify), and to last at least `minBreakLength`.
- **`StartEndBetweenPremium` sets `totalDollars` directly** instead of
  calculating it, and uses a fixed note rather than the rule description. Its
  Groovy rows 1-3 pass blank-string parameters; they never reach the parsing
  because an earlier gate (no end time, errors, closed period) exits first.
- **`StartEndBetweenPremium` writes its occurrences with `saveAll`**, one batch,
  where every other rule here saves one at a time; the port uses
  `EmployeeEarningPort::save_all`.
- **Validation messages in `PaidBreakFixedHrsRuleConfig` omit the rule name**
  for four of five checks (`hoursProp.getDisplayName() + " must be …"`); the
  port reproduces the condition, not the missing prefix.

### Deliberate divergences (continued)

103. **`BreakPeriodPenaltyHours` backfills its parameters from the defaults.**
     Java reads them without `fixMap` (its `ruleConfig` field is unused), so a
     missing one throws. Same stance as divergences 81, 89, 95 and 100.

104. **`BreakPeriodPenalty` ignores a RULE earning that has no rule item** when
     counting the day's penalties; Java's `earning.getRuleItem().equals(ruleItem)`
     would throw on it.

## `shiftearning` — slice 3

The three time-of-day rules, with their configs: `TimeOfDayDifferentialRate`,
`TimeOfDayPctDifferentialRate` and `DailySpreadShiftDifferential`. **39 tests,
48 of them transcribed Java assertions** (the count is higher than the number of
test functions because a `where:` table is one function here): all 41 rows of
`TimeOfDayPctDifferentialRateRuleImplTest` and the 7 rows of
`DailySpreadShiftDifferentialRuleImplTest`. `TimeOfDayDifferentialRate` has no
Groovy spec; its tests are written from the Java.

| Ported | Java spec cases |
|---|---:|
| `TimeOfDayDifferentialRateRuleImpl` | no spec — behaviour tests written from the Java |
| `TimeOfDayPctDifferentialRateRuleImpl` | 41/41 rows |
| `DailySpreadShiftDifferentialRuleImpl` | 7/7 rows |

New surface: `TimeCard::work_day_start_time` (default midnight;
`TimeCardData::with_work_day_start_time`), standing in for
`PayGroup.getWorkDayStartTime()`.

### Findings

- **`TimeOfDayPct`'s 41 rows went in green on the first compile**, which is the
  reverse of the audit's usual experience with a large spec. Worth stating the
  reason rather than trusting it: the rule is almost entirely arithmetic on
  `DateTimeRange`s and `RoundingOption`, both already exact in this crate, and
  the day-cut split (the only structurally new part) is covered by the spec's
  last four rows, which check both dates and both rates. The rows that matter
  most for confidence are the rounding table (2.22 / 2.25 / 2.25 / 2.17 from the
  same 7,980 seconds) and the WATSON-11507 table, which pins `DOWN` rounding at
  every five minutes.
- **`TimeOfDayPct` prices each earning for its own date.** The spec's
  employee changes rate at midnight (1 until the shift date, 2 after) so the two
  earnings of a split shift have different rates (1 and 2).
- **A break that "swallows" the premium window zeroes it, but the loop goes
  on**, so in `TimeOfDayDifferentialRate` a *later* break can subtract from zero
  and leave the premium negative; nothing is then paid (it must be above zero).
  `TimeOfDayPct`'s version zeroes the seconds the same way and has the same
  property. Both are reproduced as the Java does them.
- **`TimeOfDayDifferentialRate` clamps *after* the threshold test**:
  `minAmount` can raise a premium of 0.4 hours to a paid 3 hours, but only once
  the 0.4 has passed `minHrsBetween`.
- **`TimeOfDayDifferentialRate` pays nothing for a `HOURS_RATE` earning type**
  (the `switch` has no case for it) and silently drops the earning-less result.
- **`TimeOfDayPct`'s percent gate is `percent <= max || max >= 1`.** With the
  default `maxPctHrsBetween` of 1, a premium that exceeds the shift's hours
  (possible when reallocated hours shrink the shift) still passes.
- **`DailySpreadShiftDifferential`'s spec is contrived on purpose.** Each
  shift's `netHours` is `outHour + inHour`, which exists only to fix the sort
  order (least, home, most-hours-earlier, most-hours-later), and every shift's
  punches are on `today` whatever its shift date — so "tomorrow" and
  "yesterday" shifts are excluded by date, not by punch time.
- **`TimeOfDayPct` and `DailySpread` are the two rules in the family that need
  the rate for a date other than the shift's** (`TimeOfDayPct`) or the shift's
  *employee job status* rather than its job (`DailySpread`); both go through
  `Employee::employee_job_status`, so an employee with no status for that job
  and date pays nothing instead of throwing.

### Deliberate divergences (continued)

105. **`DailySpreadShiftDifferential` skips a shift with no IN or OUT punch** when
     finding the day's first-in/last-out; Java dereferences the null. Same
     stance as divergence 83.

106. **`TimeOfDayPct` writes its `payNetHrs` per-date earnings in date order.**
     Java groups into a `HashMap`, whose order is unspecified. Same as divergence
     96.

107. **An unknown `roundingOption` code is treated as `NONE`.** Java's
     `RoundingOption.fromCode` throws `IllegalArgumentException`.

## `shiftearning` — slice 4

The four holiday and anniversary rules, with their configs and the two shared
helpers they need: `HolidayRateFactor`, `HolidayTimeOfDay`, `DOWHoliday` and
`AnniversaryWorkedShiftDifferential`; `HolidayMatcher` and
`ShiftTimeInPeriodCalculator` (both Java `algorithm/` root, ported into
`shiftearning/utility/` because only this family uses them). **44 tests, 53 of
them transcribed Java assertions** (a `where:` table is one function here).

| Ported | Java spec cases |
|---|---:|
| `AnniversaryWorkedShiftDifferentialRuleImpl` | 6/6 executions (5 tests; the field check runs for `DOLLARS` and `HOURS`) |
| `HolidayRateFactorRuleImpl` | 14/14 rows |
| `HolidayTimeOfDayRuleImpl` | 17/17 (10 acceptance scenarios, 5 rate rows, 2 early exits) |
| `DOWHolidayRuleImpl` | 16/16 (3 invalid-shift rows, 2 no-match rows, 11 paying rows) |
| `HolidayMatcher`, `ShiftTimeInPeriodCalculator` | no spec — behaviour tests from the Java |

New surface: `EmployeeShift::reallocated_to_earning_hours`;
`dailyearning::anniversary_earning::date_for_type` made `pub(crate)`.

### Findings

- **The specs mock `HolidayMatcher`; the port runs the real one.** `HolidayTimeOfDay`
  and `DOWHoliday` stub `findOverlappingHolidays` and `getHolidayPeriod`. The
  transcriptions instead configure a one-type calendar and let the real matcher
  decide. That is a stronger test only if the mock's answers are what the real
  matcher would give; each row was checked to be (every shift overlaps the
  holiday windows the mock returns, and nothing the mock omits overlaps).
- **`HolidayRateFactor`'s Groovy row 1 is vacuous.** It expects zero saves of an
  all-zero earning, but the Mock matcher only counts saves *equal to* that
  earning; the rule really pays 17.5 hours at 9.5 × 0.5 = 4.75. The
  transcription asserts the real earning in its own test.
- **`DOWHoliday` does not write an adjustment.** The scoping section said it did
  and drove an `EmployeeShiftAdjustment` entity; reading the Java showed it only
  sums them (`WORKED` not reallocated counted, every `BREAK` subtracted, `OT`
  ignored). Corrected in the scoping tables above; divergence 110.
- **`HolidayMatcher` overlap is inclusive at both ends**
  (`DateTimePeriod.overlapsPeriod`). A shift that ends exactly when a holiday
  window begins "overlaps" it by zero hours. For `HolidayTimeOfDay` that is
  harmless (no hours, no earning). For `DOWHoliday` it is not: the date then
  takes the *holiday* branch, pays nothing, and so suppresses the weekday
  premium that date would otherwise earn. Reproduced.
- **`HolidayTimeOfDay` matches against the default midnight-to-midnight window
  but measures hours in the configured one.** `findOverlappingHolidays(shift,
  types, useActual)` uses the 3-argument overload, so a holiday whose configured
  `Start Time`–`End Time` window spills into the next day is not found by a shift
  that only touches the spill. `DOWHoliday` passes the configured times to the
  matcher and does not have this problem.
- **`HolidayTimeOfDay`'s rate depends on rule order.** With `includeDiffs`, it
  adds `round_currency(total dollars / net hours)` for each rule-created
  `EARNING`-kind earning already on the shift, so it only sees differentials
  that ran before it (priority).
- **`DOWHoliday`'s spec sets `homeJobRate` on the rule, but `execute` overwrites
  it from `homeAsBase`** (default `true`); its "using earningRate" row therefore
  prices from the home job too. The expected rate is 8 either way.
- **`AnniversaryWorkedShiftDifferential` is the least guarded rule in the
  family.** Only `isOpenForEditingFor`: no error check, no employee-status
  check. It de-duplicates nothing (a type configured twice pays twice, in
  configured order, unlike the daily `AnniversaryEarning`), uses the shift's raw
  stored `netHours` field, takes the earning type's id from the type *found*
  (the spec's DAO returns type 31 for configured id 23), and **never calls the
  earning DAO** — the earnings exist only on the time card.
- **`HolidayRateFactor` matches by date only.** The shift's date must equal the
  holiday date and the holiday's own type must be configured; there is no
  window. The first listed holiday on that date wins.
- **A `DOLLARS` check is `getUOM() == DOLLARS` here, not `isDollarsType()`.** The
  daily `AnniversaryEarning` uses the latter. A `DOLLARS` unit on a non-earning,
  non-memo type pays hours in one rule and dollars in the other.

### Deliberate divergences (continued)

108. **`HolidayMatcher` returns a `Vec` ordered by holiday date then id.** Java
     returns a `HashSet<Holiday>`. `HolidayTimeOfDay` and `DOWHoliday` sum over
     it, so order cannot change the result. Same stance as 96.

109. **Unresolvable lookups end the rule instead of throwing.** A shift whose job
     cannot be resolved (no property, so no calendar), an employee with no home
     job status (home rate, `homeDeptOnly`), or a missing earning type or rate
     all return without writing; Java dereferences null. Same stance as 83.

110. **`DOWHoliday` computes the worked-adjustment total from two scalars.**
     `round_hours(adj_hours − reallocated_to_earning_hours)` equals the Java loop
     over `EmployeeShiftAdjustment`s because `adj_hours` is already Σ `WORKED`
     less Σ `BREAK` and the reallocated figure is the `WORKED` part only.

111. **`HolidayTimeOfDay` looks each card earning's type up through
     `EarningTypePort`** to test `getType().isEarning()`; Java reads it off the
     earning. An earning whose type cannot be resolved is not counted.

## `shiftearning` — slice 5

The last three rules, with their configs, the family's registry and its runner:
`ContractToFullTime`, `ContractGuarantee`, `SplitShiftDifferentialRate`,
`ShiftEarningRegistry` (all 23 rule classes dispatch) and
`ShiftDifferentialRunner`. **29 tests, 42 of them transcribed Java
assertions.**

| Ported | Java spec cases |
|---|---:|
| `ContractToFullTimeRuleImpl` | no spec — behaviour tests written from the Java |
| `ContractGuaranteeRuleImpl` | no spec — behaviour tests written from the Java |
| `SplitShiftDifferentialRateRuleImpl` | 39/39 (13 + 10 table rows, 15 pay-type combinations, 1 prior-shift ordering) |
| `ShiftDifferentialRunner` | 3/3 (`ShiftDifferentialRunnerTest.groovy`) |

New surface: `SplitShiftPayTypes` (reconstructed, `common/enums`);
`JobMinWagePort` (the job's own effective minimum wage and the *current
property's*); `ShiftEarningRegistry`; `employee_contract_hours` as a
crate-private helper (`Employee.getContractHours(date)`); and `HolidayTimeOfDayRuleConfig`
now carries a `priority` (it is the only config in the family that extends
`PriorityRuleConfig`, which `ShiftDifferentialRunnerTest` exercises).

### Findings

- **`SplitShiftPayTypes` is another enum the checkout does not contain.** Its
  four constants are fixed by the rule (the premium map) and the spec (config by
  `name()`): `PAY_FIRST_SHIFT_HOURS`, `PAY_SECOND_SHIFT_HOURS`,
  `PAY_TIME_BETWEEN_SHIFTS`, `FIXED_AMOUNT`. The `taps` checkout has only the
  empty marker interface `SplitShiftPayTypesMultiple`.
- **The split-shift spec has one tricky expected value worth recording.**
  `deductMinWage` with job 2 (whose own minimum wage is 0) falls back to the
  current property's wage of 8 and pays 4.25; with job 3 (7.25) it pays 0.5. Both
  came from the same formula once the two sources of minimum wage were
  separated, which is what `JobMinWagePort` is for.
- **The split-shift spec does not put the executed shift on the card.** The
  "earning is set" table runs the second shift against a card holding only the
  first. The port does the same, so the "prior shift" search never sees the
  shift it runs for even without the identity check.
- **`ContractGuarantee` writes an earning even for zero hours.** Nothing guards a
  zero or negative result (a shift with no net hours, with the contract still
  short, pays a zero-hour earning at the rate gap). `ContractToFullTime`, by
  contrast, only keeps an earning with dollars or hours.
- **`Employee.getContractHours(date)` depends on the schedule mode.** In
  `Weekly` schedule mode with a non-weekly pay period the calculator is the
  `Default` one, which returns the weekly figure unscaled. The first
  `ContractGuarantee` test draft assumed 20 hours for a bi-weekly period and got
  10; the Java was right.
- **`ContractGuarantee`'s home-department test is identity on the parent
  assignment** and the contract hours count the shift's *stored* `netHours`, not
  recomputed ones, while `ContractToFullTime` uses `ShiftUtil.getNetHours`.
- **The runner clears three rule types**, not one: shift differential, hours
  distribution and post-calc. A shift-differential run discards the other two
  families' earnings too.
- **Only `HolidayTimeOfDay` has a priority.** `RuleItemPriorityComparator`
  matters for exactly one class here; the other 22 keep rule-set order.

### Deliberate divergences (continued)

112. **`ContractGuarantee` and `SplitShiftDifferential` return when the shift
     job's rate or the employee's status for it cannot be resolved.** Java
     dereferences the null. Same stance as 83 and 109.

113. **`ContractGuarantee` reads a missing `hireDate` as "not inside the pay
     period"** in `getContractHours`; Java's `containsDate(null)` would throw.

114. **The split-shift rule's `SplitShiftPayTypes` parser drops unknown names**
     where `valueOf` throws, and keeps order and duplicates. Same stance as 90.

115. **`ShiftDifferentialRunner` passes each rule a copy of the shift.** The rules
     take `&EmployeeShift` and none in the family changes it.

## `schedulelabel` — done

`RuleType::ScheduleLabel` (`LB`), five rules and their runner, in one slice:
`NoLabel`, `ApprovedTimeOffLabel`, `AvailableNotScheduled`, `NotScheduledOrOff`
and `UnavailableAllDay` (Java directory `schedulelabel`, 335 lines), with
`ScheduleLabelRunner`. **35 tests, 49 of them transcribed Java assertions**:
every row of all four rule specs plus `ScheduleLabelRunnerTest`'s three cases.

| Ported | Java spec cases |
|---|---:|
| `NoLabelRuleImpl` | none (the rule does nothing) |
| `ApprovedTimeOffLabelRuleImpl` | 17 (5 tests; the "rule checks if employee is off" table is 13 rows) |
| `AvailableNotScheduledRuleImpl` | 7 |
| `NotScheduledOrOffRuleImpl` | 17 |
| `UnavailableAllDayRuleImpl` | 5 |
| `ScheduleLabelRunner` | 3/3 |

The two "defaults are filled before save" tests spy on `fixMap`; the port has no
`fixMap` to spy on, so they assert the observable effect instead (an unset rule
item runs on the defaults without throwing).

New surface: `TimeOffStatus` (already present), `EmployeeAvailType`,
`EmployeeTimeOff` (status, span, type id, `fullDay`, `toDateTimeRange()`),
`EmployeeAvailability` (+ `Employee::is_required_off_for_day`),
`EmployeeScheduleLabel::rule_item_id`, `EmployeeScheduleLabelPort`, and on
`TimeCard`: `time_off_requests`, `schedule_labels_mut`, `date_range`,
`schedule_labels_for_date_range` (`TimeCardData::with_time_off_requests`,
`with_date_range`). Rules take a `TimeCard` rather than a `ScheduleCalcDataSet`
(divergence 23).

### Findings

- **`LabelRuleConfig` defaults `priority` to 1, not the `PriorityRuleConfig`
  default of 5**, and rejects a priority below 1. The five configs override
  `RuleConfig::priority` so the runner's sort uses 1.
- **The two time-off rules disagree about what "covers a day" means.**
  `ApprovedTimeOffLabel` and `NotScheduledOrOff` compare the **date parts** of the
  request's start and end, inclusive. `AvailableNotScheduled` uses the
  *exclusive* overlap of the day with `toDateTimeRange()`, which for a
  `fullDay` request runs to the end of its last day. A request ending exactly at
  midnight blocks the first two from the next day but not `AvailableNotScheduled`.
- **`AvailableNotScheduled` reads `getSchedules()`; `NotScheduledOrOff` reads
  `getShiftsForDay()`** (the actual shifts). A schedule-only employee is
  "scheduled" to the first and "not scheduled" to the second.
- **"Required off" means a whole-day row only** (`startTime == endTime ==
  midnight`); a 09:00-17:00 required-off row is ignored by both rules that ask.
- **I overwrote an existing file while building this and caught it by
  `git diff`.** `common/enums/time_off_status.rs` already existed with tests; the
  first draft replaced it. Restored from git before it was built on; worth
  recording because the lesson is "check `git status` for `M` on a file you
  meant to create".

### Deliberate divergences (continued)

116. **Labels carry the rule item id in a new `rule_item_id` field**, where the
     earlier port had dropped it as unread. The spec asserts `ruleItemID == 1234`.

117. **`Property.getWorkWeekForDate` is `WeeklyDateRange` ending on the property
     period end date**, via `PropertyPort`, as `hours_threshold_fixed_with_max_cap`
     does. The property entity itself is not ported.

118. **`ScheduleLabelRunner` sorts by the config's priority with defaults
     filled.** Java parses the raw `priority` parameter with `Integer.valueOf` and
     throws `NumberFormatException` when it is absent; the port ranks it 1.

## `schedulelabeladdendum` — done

`RuleType::ScheduleLabelAddendum` (`LA`): `NoOp`, `BetweenDatesScheduleLabelCount`
and `AccrualBalanceLabelAddendum`, `ReferenceRangeUtility`, and both runners
(Java directory `schedulelabeladdendum`, 268 lines). **24 tests, 13 of them
transcribed Java assertions**, i.e. every case of the five specs
(`NoOp` 1, `BetweenDates` 2, `AccrualBalance` 4, `ReferenceRangeUtility` 3,
`ScheduleLabelAddendumRunner` 2, `ScheduleLabelAddendumExtraDataRunner` 1).

These are the only rules that **return** a result rather than write one: a map
from label to addendum text. The port returns `(label index, text)` pairs, the
index naming the label in the slice the rule was given — the specs build labels
with no id, so identity cannot be an id.

New surface: `AccrualBalancePort` (`BenefitQueriesAPIJava`) with
`AccrualEndingBalance`; `EmployeeScheduleLabelPort::{count_employee_labels_matching_text,
save_all}`; `EmployeeScheduleLabel::{id, extra_data}`.

### Findings

- **`ReferenceRangeUtility` includes the occurrence already running when the
  period starts**: the start is placed in the period's year and moved back a
  year if that lands after the period begins. The third spec case pins the edge
  where a range starts on the period's last day.
- **`BetweenDates` leaves unnumbered any label in a gap between reference
  ranges**: the spec's 13 April label falls between `[…–12 Apr]` and
  `[15 Apr–…]` and is dropped.
- **`BetweenDates` adds stored labels only for a range that began before the
  period**, counting from the range start to the day before the period.
- **`AccrualBalance` numbers newest-first and counts *up***: the latest label
  shows the base balance, earlier ones higher. Its text match is
  case-insensitive but **not trimmed**, unlike the label-count match in
  `schedulelabel`.
- **`AccrualBalance` sorts the caller's list in place** in Java (divergence 119).
- **`BetweenDates`' default reference dates are today**, so an unconfigured
  rule is valid only on the day it is checked.
- **The accrual spec's "no match" test stubs a method that does not exist**
  (`getEarliestAccrualBalanceAfterDate`); the mock therefore answers null by
  default, which is the behaviour being asserted. Transcribed as a port that
  answers `None`.

### Deliberate divergences (continued)

119. **`AccrualBalance` sorts an index list, not the caller's label list.**

120. **`ScheduleLabelAddendumRunner` keeps the later text when two rule items
     label the same label.** Java's `toMap` throws `IllegalStateException`.

121. **A missing home job in the period returns nothing** where Java dereferences
     null (`getLastHomeEmployeeJobStatusForPeriod`). Same stance as 83 and 109.

## `schedulechangevalidation` — done

`RuleType::ScheduleChangeValidation` (`SV`): `AddHoursPenalty`,
`RemoveHoursPenalty` and `ShiftModificationPenalty`, their three-level config
hierarchy, and `ScheduleChangeValidationRunner` (Java directory
`schedulechangevalidation`, 320 lines). **21 tests, 22 transcribed Java
assertions** (a `where:` table is one function): `AddHoursPenaltyRuleImplTest` 6,
`RemoveHoursPenaltyRuleImplTest` 6, `ShiftModificationPenaltyRuleImplTest` 10.
The runner has no Groovy spec; its tests are written from the Java.

These rules **return** an `EarningRequest` (new, `entity/earning_request.rs`)
instead of writing an earning, for the scheduling UI to show before a user with
the `SCHEDULING_FAIR_WORK_WEEK_CHANGES` action overwrites or accepts it. Either
shift may be absent: no original is an added shift, no update a removed one.

New surface: `EarningRequest`; `CurrentDateTimePort`
(`PropertyDateTimeProvider.currentDateTimeForCurrentProperty`);
`SchedulingPermissionPort` (`ActionSecurityProvider`).

### Findings

- **`usePlanningPeriodStartDate` can never be true.** Java reads it with
  `Boolean.getBoolean(String)`, which returns whether a *system property* named
  by that string is "true", not the string's own value. The planning-period branch
  is unreachable for any real configuration, and the shift's start time is always
  used. Ported as the same dead option (divergence 122).
- **The notice window is open at both ends and in fractional days.** The edit
  qualifies only when `secondThreshold < days to start < firstThreshold`
  (default 1 to 10 days), measured from "now at the current property" to the
  **original** shift's start. One minute past the shift start, or a day of
  notice or less, pays nothing.
- **`ShiftModificationPenalty` needs the stored net hours to match exactly**
  (`== 0`, a floating-point equality) and then any one of: date, start
  time-of-day, end time-of-day or job differs. Moving a shift an hour earlier
  while also making it an hour shorter is *not* a modification.
- **The add and remove rules treat an absent shift as zero hours**, so adding a
  whole shift is "added hours" and deleting one is "removed hours".
- **`percentageOfHours` replaces `earningAmount` with a percentage**, applied to
  the absolute hours changed, then split into hours or cost by the earning type's
  unit again. Without it the amount is a flat figure.
- **None of the three configs extends `PriorityRuleConfig`.** `priority` is an
  ordinary parameter (default 0) the runner's sort never reads, so rule-set order
  is the run order.
- **An unknown earning type counts as `HOURS`** and an absent description with an
  unknown type has no name to fall back on (Java: `NullPointerException`; the port
  uses an empty note, divergence 123).

### Deliberate divergences (continued)

122. **`usePlanningPeriodStartDate` is ignored** (see the finding): the port does
     not carry the planning-period lookup Java could in principle reach.

123. **An absent earning type with no penalty description gives an empty note**
     where Java throws.

124. **`execute` drops the `ScheduleCalcDataSet` argument.** No rule in the family
     reads it. It keeps the separate `params` argument, because the specs pass a
     blank rule item with their own parameter map.

## `ShiftDiffOt` — done

`RuleType::ShiftDiffOt` (`SD`) has one rule, `ShiftDifferenceOTRuleImpl`, with its
config and `ShiftDifferenceOTRunner`. This **resolves the audit's open question**
(it was listed as "unverified whether its rule is the same class as the deferred
`ShiftDifferenceOTRuleImpl`"): the catalogue entry `SHIFT_DIFFERENCE_OT_SDO` names
`ShiftDifferenceOTRuleConfig` and that same class. Java keeps the rule in the
`hoursdistribution` package; here it has its own module, `shiftdifferenceot`.
**20 tests, 21 transcribed Java assertions:** `ShiftDifferenceOTRuleImplTest` 8,
`ShiftDifferenceOTRuleConfigTest` 11 (the validation table's 10 rows and the
defaults), `ShiftDifferenceOTRunnerTest` 2.

A short-rest premium: when the gap between one actual shift's end and the next
one's start is under `hoursBetweenShifts` (default 10), the shortfall is premium
time on the second shift, paid either as an earning (the default) or by converting
regular hours into an overtime distribution (`convertHoursToOT`). The rule's own
doc comment has the full walk.

New surface: `OvertimeRateLookup` (the eight overtime-rate rules, the property's
`OVERTIME_RATE` rule set and the default set), `ShiftDifferenceOT` trait and
registry.

### Findings

- **The specs only exercise the earning mode.** All eight rule cases run with
  `convertHoursToOT` off. The conversion path — the rate lookup, the merge into an
  existing overtime distribution, `reduceSecondShiftOTByFirstShiftOT` — has no
  Groovy coverage; its Rust tests are written from the Java and are the only
  coverage.
- **The earning's rate always floors at minimum wage**, ignoring the earning
  type's `payAtLeastMinWage` flag that the other shift rules honour.
- **The previous shift is found by calendar day, not by adjacency**, and only for
  the first shift on the calculation start date; a shift in a closed period is
  skipped for calculation but is still a candidate "previous shift".
- **A skipped pair still advances the walk**: with `excludeShiftsOnSeparateDays` or
  `excludeShiftsOnSameDay`, the second shift becomes the new previous shift.
- **A non-actual shift does not advance it**, so a schedule shift between two
  actual ones is invisible to the gap calculation.
- **Java's `shiftDiffOTHoursDistribution` is written and never read** — a
  transient field set to the new premium distribution even when that distribution
  is merged away instead of being added. Not ported (divergence 126).
- **The merged overtime rate is a cost-weighted blend**, using the existing
  distribution's `totalCosts` **before** its hours are changed:
  `(existingCosts + newCosts) / newTotalHours − baseRate`.
- **The earning's date and job are the second shift's**; the unit switch covers
  `DAYS` too, which most shift rules omit.

### Deliberate divergences (continued)

125. **The overtime rate is read off a scratch distribution.** Java calls each rate
     rule's `getRate(...)`; the Rust rules expose only `execute_for_shift`, which
     writes the rate to a distribution. The rule prices a scratch distribution
     carrying the shift's date, the overtime hours and the regular distribution's
     base rate, then reads its premium rate. Identical for every rule whose
     `execute` is `setOvertimeRates(getRate(...))`, which the audit checked for the
     job and weighted rules; an unlisted rule class would price 0, as Java's
     `switch` falls through to `rate = 0`.

126. **`shift.setShiftDiffOTHoursDistribution` is not ported**, being a dead write.
