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
| 1+ | the other 29 families | not started |

## Where the work stands

**1,236 tests, 341 of them transcribed Java assertions.** Clippy clean,
`cargo fmt` clean.

Waves 0 and 1 are complete: the support layer, the catalogue, the parameter and
config machinery, resolution, dispatch, eleven entities, and both
`punchrounding` (6 rules + runner) and `punchvalidation` (4 rules).

Wave 3's `hoursdistribution` is **complete** — all 19 catalogue entries, every
Groovy case in the family transcribed. It was taken out of plan order at the
user's request; the plan puts the rate families (Wave 2) first because the
overtime rules lean on them. **Nothing blocked on that**, which is now settled
rather than provisional: no rule in the family needed a rate.

**Wave 2, the rate families, is the next body of work.** Nothing in this crate
depends on it yet.

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

`ShiftDifferenceOTRuleImpl` exists as a twentieth `*RuleImpl` but has no
catalogue entry; it is deferred with the rules.

### Deliberate divergences — index

Forty-one so far, listed at the end of the section that introduced them. The
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
