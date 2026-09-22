# Scheduler engine — parity audit

Running record of what has been ported, what was deliberately changed, and how much of the Java
test suite each piece is backed by. Companion to `DATA_MODEL.md` (catalog, written first) and
`PLAN_SCHEDULER.md` (repo root, phased plan).

## Methodology

**Ground truth is the `taps` checkout** (`taps/src/java/com/unifocus/watson/server/scheduler/
{engine,autosched}`, `taps/src/java/com/unifocus/watson/common/labor/planner/
{GenerateSchedulesParameters,GeneratorParameters}.java`, and — as of Phase 2 step 1 —
`taps/src/java/com/unifocus/watson/server/dao/` + `watson/server/common/engine/JobLoader.java`
for the DAOs `engine/io/ports.rs` narrows), never a local convenience copy — this crate has none
yet, and shouldn't grow one without a note here if it does (`planner`'s and `workrules`'s local
copies both drifted from production; see their own audits).

Ported Java test cases go in a `#[cfg(test)] mod java_parity_tests` alongside the regular
`mod tests`, following `planner`/`workrules`. `process/projectedhoursreducers/` (Phase 2 step 3)
is the first family with a Java/Groovy suite carrying real value assertions
(`{Default,Flat,Percent}ProjectedHoursReducerTest.groovy`, `ProjectedHoursReducerFactoryTest.groovy`)
— all four transcribed into `java_parity_tests`, exact expected values preserved (`10.71`, `25.0`,
`50.0`, etc. — see finding 26). `EmployeeAvailableHoursBalancerTest.groovy` (step 4) is a Spock
`where:` table (4 rows); ported via `rstest` (added as a dev-dependency, matching `workrules`'s
existing use — `PLAN_SCHEDULER.md`'s "Ongoing" section already called for it). `DayOffPlanRotatorTest.groovy`
(step 2) is a pure interaction test (mock call-count verification, no value assertions), so
nothing from it was transcribed (finding 24). `SchedulePreparationServiceTest.groovy` (step 5,
two cases) transcribed with real value/state assertions (finding 29's mutation-order trap
discovered while transcribing it). Assertion-macro call sites: 98 (one of them an `rstest` case
covering 4 transcribed rows).

## Status

| Phase | Piece | State |
|---|---|---|
| 0 | `DATA_MODEL.md` — `ScheduleModel` + `engine/model/` (12 files) + `model/logging/` (6 files) + `GenerateSchedulesParameters` | **done** |
| 0 | `src/` scaffolding — `mod.rs` tree mirroring `engine/{model,io,misc,process/...}` + `autosched/` | **done** |
| 0 | `Cargo.toml` deps (`joda_rs`, `date_range_rs`, `uuid`) | **done** |
| — | `entity/` — Hibernate entity stubs (see finding 8 for the full list, now ~25 files) + `ScheduleCalcDataSet` (partial, growing) | **done, narrow slice** — only what Phase 1's ported files read; grows with every wave |
| — | `engine::process::ports::AssignmentPort` | **done** — resolves the `Assignment` parent-chain walk finding 9 flagged as missing; first used by `EmployeeData.min_hours_off`/`min_days_off` and two `process/checkers/` files |
| 1 (prereq) | `engine/model/` — `ScheduleModel`, `JobData`, `JobList`, `EmployeeData`, `EmployeeList`, `ShiftList`, `HoursByDate`, `PreScheduleParameters`, `Schedules`, `WeeklyAvailableHours`, `RegularSchedule` | **done** — `RegularSchedules` still blocked (finding 3); `store_pre_schedule_check_overtime`/`ScheduleCalcDataSet.getOvertimeForDateRange`/`getTotalAccrualHours` (real calculations not yet grounded) deliberately not ported; everything else, including `min_hours_off`/`min_days_off` and `getShiftsForDateRange`, **is** ported (findings 9, 12) |
| 1 (prereq) | `engine/model/logging/` — `EmployeeLogEntry`, `EmployeesWithConflicts`, `PlannedShiftLog`, `RankedEmployees` | **done** — `JobScheduleLog`/`ScheduleLog` still blocked (finding 3) |
| 1 | `process/comparators/` (13 files) | **done** |
| 1 | `process/plannedshiftsorters/` (8 files) | **done** |
| 1 | `process/checkers/` (19 files, top level) | **done** — 15 fully real, 4 wrap a deferred external subsystem behind a port trait (finding 16) |
| 1 | `process/checkers/rotationplans/` (13 files) | **done** |
| 2 | `io/` loaders + `ScheduleModelLoader` (step 1) | **done** — `ScheduleModelCreator`, `JobListLoader`, `PreScheduleJobLoader`, `EmployeeListLoader`, `ForecastPlannedShiftLoader`, `OriginalProjectedHoursLoader`, `PlannedShiftLoader`, `ScheduleModelLoader`; `engine::io::ports` stubs the DAOs behind them. `RegularScheduleLoader`/`PreScheduleLoader` deferred to the steps that call them (6-8), not this wave — see finding 20 |
| 2 | `misc::DayOffPlanRotator` (step 2) | **done** — `DayOffPlan.rotateEmployees` ported onto the entity itself (real logic, not a stub); `engine::misc::ports::DayOffPlanPort` new, `EmployeePort` (from `engine::io::ports`) reused for its second `EmployeeDAO` call site |
| 2 | `process/projectedhoursreducers/` (step 3, 5 files) | **done** — all 5 files; `engine::misc::EmployeeDataServices` (its first real caller) and `entity::projected_hours_reduction_method::ProjectedHoursReductionMethod` new; `common::numbers::round_hours` (`TDouble.roundHours`) independently re-ported from `workrules`'s implementation |
| 2 | `process/schedulebalancers/` (step 4, 1 file) | **done** — `EmployeeAvailableHoursBalancer`; `rstest` added to `Cargo.toml` for its 4-row Spock table |
| 2 | `io::SchedulePreparationService` (step 5) | **done** — `ScheduleModel::shift_preparation_fields` new (split-borrow accessor, finding 28); `EmployeeShift` grew `planned_shift`; new `EmployeeShiftPort` |
| 2 | `PreScheduleProcess` (step 6) | **done** — `PreScheduleLoader`, `PlannedShiftCreator` (one overload), `EmployeeShiftCreator` (one overload), `CalculateDataSet`, `ScheduleSaver`, `PreScheduleProcess` itself; new `entity::pre_schedule::PreSchedule`; `EmployeeData::store_pre_schedule_check_overtime` now real (finding 16's `OvertimeForDateRangePort`, moved to `engine::process::ports`); `EmployeeList::take_employee_data` new (finding 30) |
| 2 | `PermanentScheduleProcess` (step 7) | not started |
| 2 | `process/regularschedules/` + `RegularScheduleProcess` (step 8, 7 files) | not started |
| 2 | `process/variable/` + `VariableScheduleProcess` (step 9, 23 files) | not started |
| 2 | `io::SaveSchedulesService` (step 10) | not started |
| 3 | `ScheduleEngine` orchestrator | not started |
| 3 | `autosched/` (6 files) | not started |

## Findings

1. **Id strategy diverges from `planner`.** `planner`'s `id_type!` uuid macro doesn't apply here:
   every entity `scheduler` touches (`Assignment`/job, `Employee`, `PlannedShift`,
   `EmployeeShift`, `Property`) has a legacy Hibernate `int` primary key
   (`Assignment.getID()`/`Employee.getID()` return `int`; `JobList`/`EmployeeList` key their maps
   on `Integer`; `GeneratorParameters` declares `propertyID`/`divisionID`/`departmentID`/`jobID`
   as `int`/`List<Integer>`). Following `workrules`'s precedent
   (`workrules/src/entity/time_card.rs`, bare `i32` ids, no newtype), this crate uses plain `i32`
   for `JobId`/`AssignmentId`/`EmployeeId`/`PlannedShiftId`/`EmployeeShiftId`/`PropertyId` — no
   `id_type!` usage. See `DATA_MODEL.md` §2. `uuid` is still in `Cargo.toml` per the plan's
   explicit instruction (matching `planner`'s dep set) even though nothing in the current catalog
   needs it — flagging so a future reader doesn't assume it implies uuid ids anywhere in this
   crate.
2. **`Assignment`/`Employee`/`PlannedShift`/`EmployeeShift`/`Property` are stubbed, not ported.**
   `Assignment` alone is 1133 lines of Hibernate entity. `DATA_MODEL.md` §3 catalogs only the
   fields `engine/model/` reads directly; every later wave that reads a new getter off one of
   these should extend that table rather than silently assuming a field exists.
3. **Several `engine/model/` types have hard forward dependencies** that block a faithful port
   before Phase 1/2 land: `JobList`'s pre/non-pre-scheduled sort order needs
   `process/variable/comparators::{NonPreScheduledJobComparator,PreScheduledJobComparator}`;
   `RegularSchedules.get_regular_schedules_for_job_and_date` needs
   `process/checkers::EmployeeJobStatusChecker` and
   `process/regularschedules::EmployeeRegularPeriodComparator`; the `model/logging/` package needs
   `process/variable/filters::EmployeeFilter` as a map key type before `JobScheduleLog`/
   `ScheduleLog` can be ported at all. Listed in `DATA_MODEL.md` §7 so Phase 1 knows what it's
   unblocking, not just what it's porting.
4. **`JobData.average_shift_length` is a lazy cache, not a real default** — Java uses `<= 0.0` as
   a "not yet computed" sentinel on a mutable field. Port as computed-on-demand or `Option`, not
   copied verbatim as a mutable f64 cache.
5. **Entity-keyed Java `HashMap`s are identity/id-keyed, not value-keyed.** `ScheduleModel`'s
   `clearedEmployeeShiftsMap`, `EmployeesWithConflicts`'s `employeeDataLogEntryMap`, and
   `JobScheduleLog`'s `employeeFilterScheduleLogMap` all use a Hibernate-entity type as a
   `HashMap` key — Java's `equals`/`hashCode` on these entities is id-based. Rust ports of these
   maps should key on the corresponding id newtype (`PlannedShiftId`, `EmployeeId`, `EmployeeFilter`
   itself once that's a plain value type), not on an owned struct value.
6. **`ShiftList.getPlannedShiftIDs()` null-checks each element** — the Java list can contain
   `null` `PlannedShift`s. Port the Rust equivalent over `Vec<Option<PlannedShift>>` (or filter
   at the boundary) rather than assuming every element is present.
7. **`ScheduleLog.getCurrentPlannedShiftLog()` throws `IllegalStateException` on an empty list.**
   Decide when ported whether the calling step (Phase 2, step 9) genuinely guarantees non-empty at
   every call site (in which case an assertion is faithful) or whether Rust should return
   `Option`/`Result` instead — don't default to `.unwrap()` without checking the caller.
8. **`entity/` holds only what Phase 1's first two waves (`engine/model/`, `process/comparators/`)
   read.** No DAO/port-trait layer was needed this wave — unlike `workrules`'s `rules/ports.rs`,
   every value a comparator needs is either on the entity itself or passed as a parameter (see
   finding 9). Follows `workrules`'s `entity/` precedent (`workrules/src/entity/mod.rs`: "only
   that slice comes across, as plain owned structs") applied to a different Hibernate entity set —
   `workrules` and `scheduler` do **not** share a crate dependency (per the plan's
   zero-cross-crate-dep decision), so `Assignment`/`Employee`/`EmployeeShift`/`Property` are
   independently re-stubbed here, narrower than `workrules`'s own slice of the same Java classes.
   Two fields (`EmployeeType`'s variant list/sort order, `JcSortOrderType`'s variant list) are
   **unconfirmed placeholders** — inferred from call sites, not from reading the real Java enums.
   Read `com.unifocus.watson.common.enums.{EmployeeType,JCSortOrderType}` before this ships;
   `EmployeeTypeComparator`'s sort order is currently a guess (see `entity/employee_type.rs`).
9. **Several `EmployeeData`/`WeeklyAvailableHours`/`RegularSchedule` methods take as parameters
   what Java reads off a live, mutable field.** `WeeklyAvailableHours` doesn't own its
   `EmployeeData`/`JobData` (Java captures them once at construction and reads them live on every
   call, including `JobData.balance_factor`/`balance_level` which step 4 mutates *after*
   construction — an owned Rust copy would go stale); `RegularSchedule.job()` takes `&Employee`
   rather than holding one. `EmployeeData.min_hours_off`/`min_days_off` **are now ported** — the
   parent-chain walk resolves through `engine::process::ports::AssignmentPort`, the trait
   `PARITY_AUDIT.md` (this note, originally) proposed; `EmployeeMinHoursOffChecker`/
   `EmployeeMinDaysOffChecker` are its first callers. Everything touching
   `ScheduleCalcDataSet.getOvertimeForDateRange`/`getTotalAccrualHours` is still not ported — see
   finding 16.
10. **`ScheduleRestrictionRuleChecker`'s rule-dispatch dependency will be stubbed, not built.**
    Decided with the user ahead of the `process/checkers/` wave: `RuleUtils`/`RuleImplFactory`/
    `RuleSet`/`RuleItem`/`ScheduleRestrictionRuleImpl` are the same shape of rule-dispatch
    machinery `workrules` ports (and `workrules` already has a `schedulerestriction` rule family,
    5 of 6 rules, **done** — see `workrules/src/PARITY_AUDIT.md`), but for a different rule type
    (`SCHEDULE_RESTRICTION`) not itself in `workrules`'s or `scheduler`'s scope. Port the
    checker's shape with the dispatch left as an unimplemented trait dependency rather than
    building or wiring a cross-crate dependency on `workrules`'s rule engine — flag that
    cross-crate option to the user again if/when this checker's real body is needed.
11. **`Employee.getActiveEmployeeJobStatusesForDate` is misleadingly named.** Per
    `BasicEmployee.java`, it applies the exact same `containsDate(date)` filter as
    `getEmployeeJobStatuses(date)` — no further "is this status active" check despite the name.
    Ported as a wrapper that calls the other method, not duplicated logic, so the two can't drift.
12. **`ScheduleCalcDataSet` grew from an empty placeholder to a partial real type this wave.**
    `ModifiedPeakPlannedShiftSorter` needed `EmployeeData.getEffectiveAvailableHoursForDate`,
    which needed `dataSet.getAvailability().getAvailPeriodsRequiredOffOnly()` — so
    `ScheduleCalcDataSet` now holds real `shifts`/`time_off_requests`/`availability` fields (new
    `entity::{avail_period,availability}`) instead of being a zero-field stub, and
    `EmployeeData.add_employee_shift`/`remove_employee_shift`/`effective_available_hours_for_date`
    are ported. `getOvertimeForDateRange`/`getShiftsForDateRange`/`getTotalAccrualHours` are
    still not — those are real calculations (rolling overtime, accrual totals), not just data
    plumbing, and nothing ported so far grounds them. This unblocks part of
    `process/checkers/`'s next wave for free (`EmployeeAvailabilityChecker`,
    `EmployeeScheduleChecker`, `EmployeeTimeOffChecker` all only need the fields already here).
13. **`nullEnding(...)` composition collapses to one `Option` comparison.**
    `PlannedShiftSorter.compareAssignments`/`compareAssignmentOrders` each wrap a possibly-null
    Java value in `ComparatorUtil.nullEnding` (nulls sort last); because
    `PlannedShift.assignment_order_no` already flattens `getAssignment().getAssignmentOrder()
    .getOrderNo()` into one `Option<i32>` (see that field's doc), the two nested null-handling
    passes become a single custom `Ordering` (not `Option`'s derived `Ord`, which sorts `None`
    *first* — the opposite of `nullEnding`). See
    `plannedshiftsorters::planned_shift_sorter::assignment_order_no_nulls_last`.
14. **`ModifiedPeakPlannedShiftSorter.available_hours` returns `0.0` when `ScheduleModel` has no
    `EmployeeList` set.** Java's `scheduleModel.getEmployeeList()` is never null by the time this
    sorter runs in the real pipeline (step 5 loads it before step 9 calls this), so this can't be
    exercised yet in isolation — flagged rather than silently assumed safe.
15. **`CanWorkChecker::can_employee_work_shift` takes `employee_data: &mut EmployeeData`, not
    `&EmployeeData`.** Discovered porting `EmployeeWeeklyAvailableHoursChecker`: Java's
    `EmployeeData` reference is implicitly mutable, and this checker needs to call the
    memoizing `EmployeeData.weekly_available_hours(job_id)` (which mutates the employee's cache
    map) through it. Every other `CanWorkChecker` impl just gets a mutable reference it doesn't
    use — no behavior change for them, but it's the reason the trait signature isn't `&EmployeeData`.
16. **Four `process/checkers/` files wrap a deferred external subsystem behind a narrow port
    trait, same treatment as finding 10:** `ScheduleRestrictionRuleChecker`
    (`ScheduleRestrictionRulePort`, rule dispatch), `EmployeeCertificationsChecker`
    (`EmployeeCertificationPort`, `EmployeeCertificationValidator`),
    `EmployeeMonthlyAvailableHoursChecker` (`MonthlyContractHoursAvailablePort`,
    `ContractCalculatorProvider`/`PropertyDataDAO` — only the contract-hours half; the
    scheduled-hours half is real), `EmployeeOvertimeChecker` (`OvertimeForDateRangePort`,
    `ScheduleCalcDataSet.getOvertimeForDateRange`), and `EmployeeJobStatusChecker`
    (`EmployeeActiveOnDatePort`, `Employee.isActiveOnDate`/`EmployeeStatus`). Each port's shape is
    inferred from its one call site's inputs/outputs, not from reading the real Java
    subsystem — treat the trait signatures as provisional until whichever one is implemented for
    real gets checked against `ContractCalculator.java`/`RuleImplFactory.java`/etc.
17. **`Schedules.determine_consecutive_days_*` is reused directly by `EmployeeMinDaysOffChecker`
    and both rotation-plan `*Process` types**, all three constructing a fresh `Schedules` from
    `employee_data.data_set().shifts()` filtered to exclude the candidate shift. No new logic —
    flagged only because three near-identical "filter out this shift, build a `Schedules`" blocks
    now exist; a shared helper would be reasonable if a fourth call site appears.
18. **`WeeklyRotationPlanCheckerProcess` does not build a `date_range_rs::DateRange` for its
    prior/future windows at all.** Java's `ArbitraryDateRange.of(later, earlier)` relies on
    reversed-argument iteration order — the "how many consecutive days back" loop must check the
    day immediately adjacent first. `date_range_rs::DateRange` doesn't have that behavior, so the
    dates are stepped by hand instead of risking a silent iterate-the-wrong-direction bug; a test
    (`weekly_job_rotation_plan_checker::tests`) exercises the boundary this would get wrong.
19. **`DailyAssignmentRotationPlanChecker`/`WeeklyAssignmentRotationPlanChecker` return `false`
    for a shift with no assignment, where Java would NPE** the moment any other shift in the
    comparison window *does* have one (`Schedules.hasShiftWithAssignmentOnDate`'s null check is
    one-sided). Documented at each `has_matching_shift_on_date` impl.
20. **Phase 2 step 1's scope is exactly `ScheduleModelLoader.load()`'s call graph, not all 16
    `io/` files.** `RegularScheduleLoader` (used by `PermanentScheduleProcess`/
    `RegularScheduleProcess`, steps 7-8) and `PreScheduleLoader` (used by `PreScheduleProcess`,
    step 6) live in `engine/io/` but aren't reachable from `ScheduleModelLoader` — confirmed by
    grepping their callers in `taps/`. Left unported until the step that actually calls them,
    per `PLAN_SCHEDULER.md`'s "one wave per step" structure; `CancelShiftRequestsService` is
    `SaveSchedulesService`'s dependency (step 10), also out of this wave's scope.
21. **`engine::io::ports` follows `workrules::rules::ports`'s "one trait per DAO, narrowed to the
    methods actually called" pattern**, not `engine::process::ports::AssignmentPort`'s
    one-trait-per-use-case shape — `PlannedShiftQueryPort` carries both
    `ForecastPlannedShiftLoader`'s and `OriginalProjectedHoursLoader`'s queries because both wrap
    the same `PlannedShiftDAO`. `com.unifocus.watson.server.common.engine.JobLoader` (not a DAO,
    but the same "external subsystem behind a narrow port" treatment as findings 10/16) is also
    here as `JobLoaderPort`, since `JobListLoader` is otherwise a pure Phase-2 file with a single
    external dependency.
22. **Two Phase 1 types needed real constructor changes to accept their first genuine caller's
    data**, not just new methods: `EmployeeData::new` now takes `ScheduleCalcDataSet` as its
    second parameter (Java's actual signature — `EmployeeListLoader` is the crate's first caller
    that has a real dataset to pass; Phase 1's `EmployeeData::new(employee, random)` was a
    placeholder for callers with none), and `entity::planned_shift::PlannedShift` gained a
    `job_id` field (`PlannedShiftLoader` is the first caller of `getJob()`, flattened to the id
    per the existing `assignment_order_no` precedent). All Phase 1 test call sites updated
    (`ScheduleCalcDataSet::default()`, a fixed `job_id` in the three sorter test helpers) rather
    than left on the old signature.
23. **`common::java_random::JavaRandom` ports `java.util.Random`'s actual 48-bit LCG**, not a
    different Rust RNG, for `EmployeeListLoader.createEmployeeList`'s per-employee tiebreak
    value (read only by `DefaultSeniorityComparator`, never persisted or compared across
    processes) — cheap to port faithfully, so it was, even though bit-exact parity isn't
    load-bearing here. The seed itself (`dateRange.getStartDate()` at start of day) is computed
    in UTC (`joda_rs` has no JVM-default-time-zone concept), a documented divergence from Java's
    actual seed value that doesn't affect behavior for the same reason.
24. **`DayOffPlanRotator`'s own Groovy/Spock test is a pure interaction test** — it mocks every
    collaborator and asserts `rotateEmployees` is called once per loaded plan with the full
    employee list (`1 * dayOffPlan1.rotateEmployees(employees)`), never checking what
    `rotateEmployees` actually does. The real logic — `DayOffPlan.rotateEmployees` itself, not
    previously ported — got its own `mod tests` in `entity::day_off_plan` instead (first
    rotation, wrong-plan no-op, "too soon to rotate again" cases); `DayOffPlanRotator`'s own test
    covers the wiring (every loaded plan gets rotated against the full list) that the Groovy test
    checks.
25. **`DayOffPlanRotator`/`DayOffPlan.rotateEmployees` mutate state Java persists implicitly**
    (Hibernate dirty-checking on managed `DayOffPlan`/`Employee` entities inside a transaction —
    `PARITY_AUDIT.md`'s Methodology section already flags this crate has no persistence layer of
    its own). `DayOffPlan::rotate_employees` mutates its `&mut self`/`&mut [Employee]` arguments
    in place (matching Java's mutation shape); `DayOffPlanRotator::rotate_day_off_plans` then
    returns the mutated `(Vec<DayOffPlan>, Vec<Employee>)` rather than discarding them once the
    fetched owned copies go out of scope, so a future save layer has something to write back.
    First case in this crate of a step whose *point* is a side effect, not a value flowing into
    `ScheduleModel` — worth watching for again in later `misc::` files (`ScheduleSaver`,
    `EmployeeDataServices`) and the save-service trio in `io/` (step 10).
26. **`ProjectedHoursReducerFactory`'s lazy `HashMap` cache is ported as a direct `match`**, not
    a real cache — same treatment as `JobData.average_shift_length` (finding 4): the map only
    ever holds two fixed entries (`FLAT`/`PERCENT`), so a `match` gives the same answer with no
    mutable-state-on-first-call machinery to get wrong. Its Groovy test asserts reference
    identity (`getProjectedHoursReducer(jobData) == flatProjectedHoursReducer`); ported as three
    spies with a `Cell<bool>` each (call the returned reducer, assert exactly one spy was
    invoked) rather than raw pointer comparison — `DefaultProjectedHoursReducer` is a
    zero-sized unit struct, and comparing addresses of two ZST values is not guaranteed to
    distinguish them in Rust. `FlatProjectedHoursReducer`/`PercentProjectedHoursReducer`'s own
    Groovy tests mock `EmployeeDataServices.sumEmployeeAvailableHours` directly (`>> 75.0`)
    rather than exercising real employee data; ported the same way — one real employee whose
    available hours happen to sum to 75.0, not a mock substitution (this crate has no mocking
    layer), so the exact numbers (`10.71`, `25.0`/`50.0`) still match the Groovy suite's expected
    values.
27. **`EmployeeAvailableHoursBalancer.computeBalanceFactor` never sets `JobData.balanceFactor` to
    `1.0` itself** — that's `JobData::new`'s already-ported default (Phase 1). The method only
    ever assigns a value when `availableHours > projectedHours`; the Spock `where:` table's first
    two rows (`ahours <= phours`) pass because nothing touches the field, not because the method
    computed `1.0`. Worth remembering if a later step's parity test for the same field looks like
    it's asserting behavior that's actually just the untouched default.
28. **`SchedulePreparationService` needs simultaneous mutable access to four independent
    `ScheduleModel` fields per shift** (`job_list`, `employee_list`, `old_shift_list`,
    `cleared_employee_shifts`) — Java's single mutable object graph gives every method that for
    free; Rust's borrow checker can't, since each field is already behind its own single-field
    accessor and none of the existing `_mut()` methods can be called more than once
    simultaneously. Added `ScheduleModel::shift_preparation_fields`, a purpose-built accessor
    that borrows `self` once and returns a tuple of disjoint mutable references — not a
    general-purpose pattern, just what this one caller needs; add narrower single-field
    accessors instead for anything that doesn't need this many pieces at once.
29. **Identity-shared mutation reorders under a `Copy` port.** Java's `removeEmployeeShift` calls
    `oldShiftList.addEmployeeShift(employeeShift)` *before* `employeeShift.setPlannedShift(null)`
    — harmless in Java because both references point at the same Hibernate-managed object, so
    nulling it after adding still leaves `oldShiftList`'s copy null by the time anything reads it.
    `EmployeeShift` here is a `Copy` value type with no shared identity, so preserving Java's
    literal statement order would leave `old_shift_list` holding a shift whose `planned_shift` is
    still set — wrong final state. Ported with `set_planned_shift(None)` moved *before* the
    `add_employee_shift` call, landing on the same final state Java's shared mutation produces.
    Watch for this shape (`objectA.mutate(objectB); objectB.setField(x);` where `objectA` holds a
    live reference to `objectB`) at any other call site that copies a mutable Java entity into a
    collection before finishing its mutations.
30. **`PreScheduleProcess` is the first real caller needing simultaneous `&mut ScheduleModel` and
    `&mut EmployeeData`, where `EmployeeData` is nested inside `ScheduleModel`'s `EmployeeList`.**
    `CanWorkChecker::can_employee_work_shift` (Phase 1) takes both `&mut ScheduleModel` and
    `&mut EmployeeData` — Java's shared object graph gives every checker both for free, but Rust
    can't alias a `&mut EmployeeData` borrowed out of `schedule_model` with `&mut ScheduleModel`
    at the same time. Finding 28's `shift_preparation_fields` doesn't fit here — that split-borrow
    accessor was built for a fixed *set* of fields read every call, where this loop needs exactly
    one dynamically-chosen `EmployeeData` per iteration (keyed by the loaded `PreSchedule`'s
    employee id). Solved instead with `EmployeeList::take_employee_data`/`add_employee_data`:
    remove that one employee's data from the map, operate on it as a fully independent owned
    value alongside `&mut ScheduleModel` for the rest of the iteration, then reinsert it. Also
    surfaced `CalculateDataSet.storeOvertimeAddShiftAndCalculate`'s own finding-29-shaped trap:
    Java adds `employeeShift` to the data set *before* `adjustShiftForLunch` mutates it (safe
    there via shared identity); ported with the lunch adjustment moved before the add, and
    `&mut EmployeeShift` threaded through so the caller's own copy (later passed to
    `EmployeeTimeOffChecker`/`ScheduleSaver`/`removeShiftAndCalculate`) reflects the same
    mutation the stored copy got. `EmployeeData::store_pre_schedule_check_overtime` is real now
    too — `OvertimeForDateRangePort` (originally `EmployeeOvertimeChecker`-local, finding 16)
    moved to the shared `engine::process::ports` module once `CalculateDataSet` needed it too,
    since `CalculateDataSet.storeOvertimeAddShiftAndCalculate` needs a dependency Java's class
    doesn't visibly have (the entity-level calculation is opaque in Java; this port models it as
    an external port instead).

## Where the work stands

**Phase 1 ("shared machinery") is complete.** Phase 0 (scaffolding + `DATA_MODEL.md`),
`engine/model/` (all 11 non-blocked types, plus 4 of 6 `model/logging/` types),
`process/comparators/` (13 files), `process/plannedshiftsorters/` (8 files),
`process/checkers/` (19 top-level files), and `process/checkers/rotationplans/` (13 files) are all
ported — 53 of 53 files in the wave `PLAN_SCHEDULER.md` scoped as "the largest single wave,"
plus the `engine/model/` prerequisite it turned out to also need and the `entity/`/`ports.rs`
surface all of it reads from (now ~25 entity files + `AssignmentPort`).

25 tests, all passing, `cargo build --workspace`/`clippy --all-targets`/`fmt --check` all clean.
Coverage highlights: the date-walking logic in `Schedules` (one caught a wrong *test* expectation
during authoring, not a code bug — Java's "last scheduled date" collapse for an in-progress
time-off period); the department-seniority branch of `JobRankComparator`; the day-ordering logic
of `PeakPlannedShiftSorter`/`CascadePlannedShiftSorter`/`DayOfWeekPlannedShiftSorter`; and the
daily/weekly rotation-plan boundary conditions (finding 18's reversed-iteration fix included).

Of the 19 top-level checkers, 15 are fully real logic against grounded data. Four
(`ScheduleRestrictionRuleChecker`, `EmployeeCertificationsChecker`,
`EmployeeMonthlyAvailableHoursChecker`, `EmployeeOvertimeChecker`) wrap a deferred external
subsystem behind a port trait per finding 16 — same treatment finding 10 established, extended
consistently rather than one-off. `EmployeeJobStatusChecker` does too, for the same reason
(`Employee.isActiveOnDate` needs unmapped `EmployeeStatus`).

Still blocked, unchanged from before this wave: `RegularSchedules`, `JobScheduleLog`,
`ScheduleLog` (finding 3 — wait on Phase 2's `process/variable/{comparators,filters}` and
`process/regularschedules/`). `ScheduleCalcDataSet.getOvertimeForDateRange`/`getTotalAccrualHours`
remain real calculations nothing has grounded (finding 16's `OvertimeForDateRangePort` stub covers
the checker that needs the former).

**Phase 2 step 1 (`io/` loaders + `ScheduleModelLoader`) is done.** `ScheduleModelCreator`,
`JobListLoader`, `PreScheduleJobLoader`, `EmployeeListLoader`, `ForecastPlannedShiftLoader`,
`OriginalProjectedHoursLoader`, `PlannedShiftLoader`, and `ScheduleModelLoader` itself are ported
against `engine::io::ports`'s six new DAO/loader port traits (finding 21), plus
`engine::generate_schedules_parameters::GenerateSchedulesParameters` (Phase 0's DATA_MODEL §6
catalog, not yet built until this wave needed it as `ScheduleModelLoader.load`'s parameter) and
`common::java_random::JavaRandom` (finding 23). `entity::pre_schedule_jobclass::
PreScheduleJobclass` and `entity::employee_calculation_mode::EmployeeCalculationMode` are new
minimal entity slices; `ScheduleCalcDataSet` grew `employee`/`dataset_start_date`/
`calculation_mode`; `EmployeeData::new` and `PlannedShift::new` both picked up new required
parameters their first real caller needed (finding 22). `RegularScheduleLoader`/`PreScheduleLoader`
are intentionally not part of this wave (finding 20) — they land with steps 6-8.

46 tests, all passing (21 new asserts across 10 new tests), `cargo build --workspace`/
`clippy --all-targets`/`fmt --check` all clean. `ScheduleModelLoader`'s own test wires all six
fake ports together end-to-end (property → job list → planned shifts routed to the right job →
employee list), plus a "property doesn't resolve" `None` path.

**Phase 2 step 2 (`misc::DayOffPlanRotator`) is done.** `DayOffPlan.rotateEmployees` — real
rotation logic, not a stub — is now ported onto `entity::day_off_plan::DayOffPlan`, which grew
`id`/`number_weeks`/`last_rotated` fields for it (finding 25 covers the "mutates state Java
persists implicitly" shape). `engine::misc::ports::DayOffPlanPort` is the one new DAO port;
`EmployeeDAO`'s `findAllForProperty` was added as a second method on the existing `EmployeePort`
(from `engine::io::ports`, finding 21's "one trait per DAO" rule applied across module
boundaries — see that module's updated doc). `Property` grew `period_start_date`; `Employee`
grew `set_current_pattern_no`.

55 tests, all passing (9 new asserts across 4 new tests — 3 for `DayOffPlan::rotate_employees`
itself, 1 for `DayOffPlanRotator`'s wiring), `cargo build --workspace`/`clippy --all-targets`/
`fmt --check` all clean.

**Phase 2 step 3 (`process/projectedhoursreducers/`) is done.** All 5 files ported —
`ProjectedHoursReducer` (trait), `DefaultProjectedHoursReducer` (no-op), `FlatProjectedHoursReducer`,
`PercentProjectedHoursReducer`, `ProjectedHoursReducerFactory` (finding 26 covers the
lazy-cache-to-`match` divergence). This step's Java test suite is the first with real value
assertions this crate found — all four Groovy tests transcribed into `java_parity_tests` with
their exact expected numbers preserved. New supporting pieces: `engine::misc::
EmployeeDataServices` (`FlatProjectedHoursReducer`/`PercentProjectedHoursReducer`'s dependency,
its own Groovy test also transcribed), `entity::projected_hours_reduction_method::
ProjectedHoursReductionMethod` (confirmed by direct read, not a guess — two variants), and
`common::numbers::round_hours`/`round_to`/`java_signum` (`TDouble.roundHours` /
`TDouble.round(double,int)`, independently re-ported from `workrules::common::numbers`'s
already-battle-tested implementation, narrowed to the one precision `scheduler` needs). `Assignment`
grew `projected_hours_reduction_method`.

48 tests, all passing (78 total assertions, 22 new this wave across 9 new tests), `cargo build
--workspace`/`clippy --all-targets`/`fmt --check` all clean.

**Phase 2 step 4 (`process/schedulebalancers/`, the one file) is done.**
`EmployeeAvailableHoursBalancer.compute_balance_factor` — real logic, reusing
`engine::misc::EmployeeDataServices` (step 3's dependency) and `EmployeeList::employees_with_job`
the same way `FlatProjectedHoursReducer`/`PercentProjectedHoursReducer` do. Its Groovy test is a
4-row Spock `where:` table, the first parameterized case found in this crate's Java suites —
transcribed with `rstest`, added to `Cargo.toml` as a dev-dependency (`PLAN_SCHEDULER.md`'s
"Ongoing" section already called for it, matching `workrules`'s existing use). Finding 27 flags a
subtlety worth remembering: two of the four rows pass because `JobData.balance_factor`'s Phase-1
default (`1.0`) is never touched, not because the method computed that value.

52 tests, all passing (83 assertion-macro call sites, one an `rstest` case covering the table's 4
rows), `cargo build --workspace`/`clippy --all-targets`/`fmt --check` all clean.

**Phase 2 step 5 (`io::SchedulePreparationService`) is done.** Real logic — for each
in-date-range, in-job-list employee shift, either clears its planned shift off `JobData`
(`clear_schedules == false`) or moves the shift itself to `old_shift_list` and records it in
`cleared_employee_shifts` (`clear_schedules == true`), evicting via the new `EmployeeShiftPort`.
Findings 28-29 cover the two hardest parts: `ScheduleModel::shift_preparation_fields`, a new
split-borrow accessor letting this method touch four independent `ScheduleModel` fields at once
(no Java equivalent needed since its object graph is one big mutable structure), and reordering
`set_planned_shift(None)` before `old_shift_list.add_employee_shift(...)` to land on the same
final state Java's shared-identity mutation produces despite `EmployeeShift` being a `Copy` value
type here. Both Groovy test cases (`ClearSchedulesFalse`/`ClearSchedulesTrue`) transcribed into
`java_parity_tests`. `EmployeeShift` grew a `planned_shift: Option<PlannedShift>` field (full
value, not flattened to an id — see the file's own doc for why); `EmployeeList` grew
`employee_data_list_mut`; `EmployeeData` grew `data_set_mut`; `ScheduleCalcDataSet` grew
`shifts_mut`.

54 tests, all passing (98 assertion-macro call sites), `cargo build --workspace`/
`clippy --all-targets`/`fmt --check` all clean.

**Phase 2 step 6 (`PreScheduleProcess`) is done.** `PreScheduleLoader` (`engine::io`, deferred
from step 1 per finding 20 — its first real caller), `PlannedShiftCreator`/`EmployeeShiftCreator`
(`engine::misc`, one overload each — the other overloads are `RegularSchedule`/`VariableSchedule`
process dependencies, deferred to steps 8-9), `CalculateDataSet` (`engine::misc`, all four ported
methods real except `distributeHoursForShift`, unreachable from this wave and left unported),
`ScheduleSaver` (`engine::misc`), and `PreScheduleProcess` itself (`engine::process`) are all
ported against real logic, not stubs — except the two genuinely external subsystems
`CalculateDataSet` reaches into (`SchedulesTimeCardCalculatorPort`/`ScheduleLunchRunnerPort`, new
in `engine::misc::ports`, same treatment as findings 10/16). New `entity::pre_schedule::
PreSchedule`; `PlannedShift`/`EmployeeShift` both grew a few `with_*` fields for their first real
writer (finding 30's doc covers exactly what and why). `EmployeeData::
store_pre_schedule_check_overtime` — deferred since Phase 1 (status table's `engine/model/` row)
— is real now too, via the relocated `OvertimeForDateRangePort`. Finding 30 covers the two hard
parts: the `EmployeeList::take_employee_data` remove-then-reinsert pattern for the
`&mut ScheduleModel`/`&mut EmployeeData` aliasing conflict (a new structural shape, not
`shift_preparation_fields` reused), and `CalculateDataSet`'s own finding-29-shaped reordering
trap. `PreScheduleProcessTest.groovy` is a pure interaction test (same shape as finding 24) — its
one real-world outcome ("only employees who pass `EmployeeTimeOffChecker` get saved") is covered
through real state instead, not transcribed as a `java_parity_tests` case since there's no mock
call-count assertion to preserve.

57 tests, all passing (1 new test, `PreScheduleProcess`'s own end-to-end case — two employees, one
blocked by an overlapping time-off request), `cargo build --workspace`/`clippy --all-targets`/
`fmt --check` all clean.

**Next up per `PLAN_SCHEDULER.md`: Phase 2 step 7**, `PermanentScheduleProcess`. Every step should
re-check `DATA_MODEL.md` §7's dependency list against what's actually shipped so far — several
entries (`EmployeeJobStatusChecker`, `AssignmentPort`) are now resolved.
