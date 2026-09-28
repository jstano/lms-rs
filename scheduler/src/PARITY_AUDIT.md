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
| 2 | `PermanentScheduleProcess` (step 7) + `process/regularschedules/` + `RegularScheduleProcess` (step 8, 7 files) — **merged into one wave**, see finding 31 | **done** |
| 2 | `process/variable/` + `VariableScheduleProcess` (step 9, 23 files) | **done** — all 23 files plus `VariableScheduleProcess` itself; unblocked `JobList::non_pre_scheduled_jobs`/`pre_scheduled_jobs` and `JobScheduleLog`/`ScheduleLog` (findings 38-41) |
| 2 | `io::SaveSchedulesService` (step 10) | **done** — plus `CancelShiftRequestsService` (real no-op), `SaveScheduleSnapshotService`, `SaveScheduleLogService` (findings 44-48) |
| 3 | `ScheduleEngine` orchestrator | **done** — `JobList::take_job_data` new (finding 49) |
| 3 | `autosched/` (6 files) | **done** — `AvailPeriod` already ported (Phase 1 prerequisite); `ScheduleEmployee` not ported (dead code, finding 51); the other four (`ScheduleHoursDistributionValidator`, `ExceedsAvailableHoursConflictValidator`, `ScheduleChecker`, `ScheduleMatcher`) ported for real (findings 50, 52-55) |

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

31. **Steps 7-8 (`PermanentScheduleProcess`/`RegularScheduleProcess`) merged into one wave.**
    `PermanentScheduleProcess.schedulePermanentEmployees` is a 3-line facade whose entire real
    dependency graph — `RegularScheduleLoader` and the whole `process/regularschedules/`
    subpackage (`RegularScheduleGenerator` → `RegularScheduleSingleDate` →
    `RegularScheduleSingleShift` → checker factory/creator/matcher/helper machinery) — is step 8's
    stated scope; `RegularScheduleProcess` is the same facade shape, ~5 lines different
    (`EmployeeType::Regular` vs. `::Permanent`). Delivering step 7 without step 8 would mean
    building ~95% of step 8's files and reporting them under the wrong step. Decided with the user
    ahead of this wave.
32. **`RegularSchedule`/`EmployeeRegularPeriod.assignment_id` corrected from non-optional `i32` to
    `Option<i32>`.** Both types were already ported (Phase 1's `engine/model/` prerequisite wave)
    with `assignment_id: i32` — nothing read the field yet, so the gap was invisible. This wave's
    `PlannedShiftMatcher` (`getRegularScheduleAssignment`) is the first real reader, and it
    explicitly null-checks the assignment; ported as `Option<i32>` instead of carrying the bug
    forward, same "first real caller turns a placeholder into a real field" pattern as findings 22
    and `EmployeeData::store_pre_schedule_check_overtime`.
33. **`RegularScheduleSingleDate` carries `job_id: i32`, not `&JobData` — a new instance of the
    `&mut ScheduleModel`/nested-borrow conflict**, distinct from finding 30's `EmployeeData` case.
    Java holds a `JobData` reference across the whole per-date loop, re-reading it after each
    shift to decide whether to stop scheduling — safe there since `ScheduleSaver`/
    `CalculateDataSet` mutate that same shared object in place. Rust can't hold `&JobData` across a
    call needing `&mut ScheduleModel`, and unlike `EmployeeData` it can't be taken out either:
    `ScheduleSaver` looks the job up *by id* through `schedule_model.job_list_mut()`, so a
    taken-out job would silently fail to record newly-scheduled hours. Resolved by carrying
    `job_id` through the whole `regularschedules/` chain and re-fetching a fresh, short-lived
    `&JobData` at each read point, always dropped before the next mutating call — simpler than a
    take/reinsert dance, and correct here specifically because nothing in this flow needs to hand
    out `&mut JobData` to an external API.
34. **`RegularScheduleCanWorkChecker` clones the resolved `JobData` before running its checker
    list — a third simultaneous-borrow shape**, on top of 30 and 33.
    `EmployeeAvailabilityChecker` (one of the ten checkers this wave's factory assembles) holds a
    `&'a JobData` field for the duration of one check, but `RegularScheduleCanWorkChecker::
    can_employee_work_shift` also needs `&mut ScheduleModel` (every checker can write a failure
    note into the model's log). Rust can't alias a `&JobData` borrowed out of `schedule_model`
    with `&mut ScheduleModel` at the same time. Resolved with a `.cloned()` — cheap and
    behaviorally identical to Java's live reference, since nothing in this checker list mutates
    the job's scheduled data (only `ScheduleSaver`, downstream, after every check passes, does
    that). Also takes the already-taken `&mut EmployeeData` directly from
    `RegularScheduleSingleShift`'s one take/reinsert scope, rather than Java's own
    `scheduleModel.getEmployeeList().getEmployeeData(...)` re-lookup — same idiom finding 30
    established, applied to a second caller.
35. **`RegularScheduleCanWorkCheckerFactory` collapses Java's Spring-DI checker-class lookup into a
    plain ordered `Vec<Box<dyn CanWorkChecker>>`**, same idiom as `RotationPlanCheckerFactory`/
    `SeniorityComparatorFactory` (no bean-lookup shim, since this crate has no DI container) — the
    list is rebuilt fresh per call (not cached on `self`) specifically because
    `EmployeeAvailabilityChecker`'s `&JobData` borrow only lives for one check (finding 34).
36. **`PlannedShiftCreator::create_planned_shift_from_regular_schedule` and
    `PlannedShiftHelper`/`RegularScheduleEmployeeShiftCreator` take `job_id: i32` explicitly**
    rather than re-deriving `regularSchedule.getJob(shiftDate)` at every level the way Java does.
    The caller (`RegularScheduleSingleDate`) already resolved and validated that same id against
    this exact `regular_schedule`/`shift_date` pair before reaching these calls (via
    `RegularSchedules::regular_schedules_for_job_and_date`'s own job-match filter), so threading it
    through is behaviorally identical without reintroducing an `Option`/panic risk for an already-
    known value.
37. **`Property.default_shift_category` is now reached by two call sites instead of zero, still
    deferred.** `engine::misc::employee_shift_creator`'s existing doc already flagged this gap
    (nothing modeled it, and `PreScheduleProcess` never triggered the fallback);
    `PlannedShiftCreator::create_planned_shift_from_regular_schedule` and
    `RegularScheduleEmployeeShiftCreator` (which always passes `None` for `shift_category_id`,
    Java's explicit `ShiftCategory shiftCategory = null`) both leave it unset too, rather than
    duplicate the gap undocumented a second time.

38. **`EmployeeFilter` is a trait, not the plain value type Phase 1's finding 3 / `DATA_MODEL.md`
    §5 expected** — its seven implementations (six unit structs plus `JobLevelEmployeeFilter`,
    which carries a level) genuinely differ in behavior, so a trait matches Java's polymorphic
    `EmployeeFilter` interface better than a single enum with a `match`-based body would. The map-
    keying problem finding 3/§5 anticipated is solved with a companion `EmployeeFilterKey` enum
    (`Copy`/`Eq`/`Hash`, one variant per concrete filter, `JobLevel(i32)` carrying the level) — a
    `key()` trait method every impl provides. This is needed because Java's `HashMap` keys these
    by object identity, and each `JobLevelEmployeeFilter` bean is a distinct instance per job
    level despite all reporting the same `getName() == "Default"` — keying by name (or address)
    would either collide across levels or need unsafe pointer comparison. `EmployeeFilterKey` is
    exactly the plain value type finding 5's map-identity treatment calls for, applied to the key
    rather than to `EmployeeFilter` itself.
39. **`JobScheduleLog`/`ScheduleLog` (`engine::model::logging`), blocked since Phase 1 (finding 3),
    are unblocked and ported for real** — `JobScheduleLog` is keyed by job id, not an owned
    `JobData` (finding 5's entity-identity-keyed-map treatment, consistent with
    `ScheduleModel.cleared_employee_shifts_map`); `ScheduleLog::current_planned_shift_log`
    `.expect()`s a non-empty list, faithful to Java's `IllegalStateException` (finding 7, now
    resolved the same direction as `EmployeeOvertimeChecker` etc. rather than softened to
    `Option`) — every real caller (`VariableJobScheduler`, the generators) adds a log immediately
    before reading it back. Adding `job_schedule_log_map` broke `ScheduleModel`'s
    `#[derive(Clone, PartialEq)]` (its values now hold `Box<dyn EmployeeFilter>`, neither `Clone`
    nor `PartialEq`) — removed both derives; nothing ported clones or compares a whole
    `ScheduleModel` (call sites already compared individual fields), so this cost nothing. Added
    a manual `Debug` impl for `JobScheduleLog` instead (needed for `ScheduleModel`'s own derived
    `Debug`), printing everything except the trait-object map values.
40. **`VariableChecker`/`VariableCanWorkChecker`/`VariableCanWorkCheckerFactory` extend the
    simultaneous-borrow idiom findings 30/33/34/35 established, applied to a second, larger
    checker list.** `VariableCanWorkCheckerFactory` adds `EmployeeAvailableHoursChecker` (itself
    composing `EmployeeMonthlyAvailableHoursChecker`+`EmployeeWeeklyAvailableHoursChecker`, the
    latter also borrowing `&JobData`) and `EmployeeDayOffRotationPlanChecker`/
    `EmployeeOvertimeChecker` to the checker list beyond what `RegularScheduleCanWorkCheckerFactory`
    assembles — same `.cloned()`-the-`JobData`-before-the-checker-list resolution (finding 34),
    same DI-free `Vec<Box<dyn CanWorkChecker>>` rebuilt per call (finding 35). `VariableChecker`
    (a plain comparator, not a `CanWorkChecker`) has its own version of the same shape:
    `PlannedShiftScheduler` resolves and clones the job's `Assignment` once up front, since
    `VariableChecker::compare_employees` needs `&Assignment` (for `job.sort_order()`) at the same
    time as the loop needs `&mut ScheduleModel`. `VariableChecker::compare_employees` also always
    passes `None` for `SeniorityComparator`'s `assignment` parameter (Java passes
    `plannedShift.getAssignment()`) — this crate's `PlannedShift` only carries `assignment_id:
    Option<i32>`, and nothing resolves an arbitrary assignment id back to a full `Assignment`
    (`JobList` only maps *job* ids). `AssignmentOrderComparator`/`AssignmentRankComparator`
    already treat `None` as an automatic tie, so only those two tie-breakers are silently skipped;
    every other `JcSortOrderType` variant is unaffected. Flagged as a real, not guessed,
    divergence — revisit if an assignment-id lookup gets built for another caller.
41. **`PlannedShiftScheduler` is the hardest file in this wave**: Java holds a running "best
    employee" candidate across a loop over every unsorted employee, each iteration needing both
    `&mut ScheduleModel` and `&mut EmployeeData` (finding 30's shape) — but unlike every prior
    caller of `EmployeeList::take_employee_data`/`add_employee_data`, this loop needs the pattern
    extended to hold **at most one `EmployeeData` taken out across multiple iterations**, not just
    within one. Solved by tracking `best: Option<(EmployeeData, EmployeeShift)>`: every employee
    that loses (fails a check, or is out-competed) is reinserted immediately; a new best displaces
    the old one (which gets `removeShiftAndCalculate`'d and reinserted on the spot); the final
    winner is only reinserted once, after `ScheduleSaver::save_schedule`. `EmployeeShiftCreator::
    create_shift`'s `shiftCategory` fallback (`plannedShift.getJob().getProperty().
    getDefaultShiftCategory()`) is a third call site left deferred per finding 37 — still nothing
    models `Property.default_shift_category`.
42. **`VariableJobSchedulingProcess`'s lazily-built `schedulingMethodMap`** collapses to a `match`
    over `entity::scheduling_method::SchedulingMethod`'s three fixed variants (new entity, plus a
    new `Assignment.scheduling_method` field) — same "fixed small map needs no cache" treatment as
    `ProjectedHoursReducerFactory` (finding 26). An unmapped job (`getSchedulingMethod() == null`,
    an implicit Java NPE at `.get(...)`) is modeled as `Option::None` and skipped instead of
    panicking — a deliberate divergence from the literal NPE, flagged rather than silently copied.
    `scheduleModel.getProgress().setMessage(...)` is dropped, not stubbed — `Progress` isn't
    modeled in this crate at all (`ScheduleModel`'s own doc already says so).
43. **`PreScheduledJobProcess::combine_planned_shifts_for_grouped_jobs`/`grouped_jobs` are
    associated functions, not methods** — unlike every other file in this wave, they never read
    `self` (`self.variable_job_scheduling_process` is only used by the public
    `generate_schedules_for_jobs` entry point), so making them `Self::`-qualified keeps them
    directly unit-testable without constructing the full generator dependency chain just to reach
    a method that ignores it. Moving shifts between two `JobData`s in the same `JobList` (Java
    holds two live references; this map only exposes one `&mut JobData` at a time) is done by
    taking the loser's `planned_shifts` out (`std::mem::take`, leaving it empty in place — matches
    Java's explicit `.clear()`) before appending them to the group's first job, one
    `job_data_mut` call at a time rather than two simultaneous ones.

44. **`CancelShiftRequestsService.cancelShiftRequests`'s entire body is commented out in the real
    Java source** (a `//TODO: need to implement this correctly` block, dated well before this
    port started) — ported as a real no-op, not a stub awaiting implementation. There's nothing
    to port; the method genuinely does nothing in production today.
45. **Step 10's DAOs needed real write-side port traits for the first time** — every prior loader
    (steps 1-9) only ever *read*. `EmployeeShiftPort` (already existed for `evict`, step 5) grew
    `bulk_delete_by_shift_id`/`save`/`bulk_delete_employee_shifts_for_jobs_in_current_property`/
    `employee_schedule_shifts_for_period`; a new `PlannedShiftPort` (`save`/
    `bulk_delete_planned_shifts_for_current_property`) was added alongside the existing
    `PlannedShiftQueryPort` rather than folded into it, since the query/write surfaces of
    `PlannedShiftDAO` have no callers in common (finding 21's "one trait per DAO" rule, applied
    with a deliberate split when the two halves never meet). `ShiftType.GENERATED`/
    `PlanType.GENERATED` aren't modeled as enums — both are fixed constants at their one call site
    each, same "fixed constant, not a field" treatment `entity::planned_shift::PlannedShift`
    already established for its own undocumented constants, extended here to enum arguments.
    `SchedulesTimeCardCalculatorPort` (step 6) grew a second method,
    `calculate_schedule_calc_data_set` — a different real Java overload
    (`calculateScheduleCalcDataSet`, not `calculateOvertimeForScheduleCalcDataSet`) on the same
    external class, not a second trait.
46. **`SaveScheduleSnapshotService`'s `EmployeeShiftCloner` is deferred behind a new
    `EmployeeShiftClonerPort`**, the same "narrow entity slice" treatment as findings 2/8 rather
    than the usual "deferred rule/calculation engine" shape (findings 10/16/37): Java's real clone
    copies ~30 fields (adjustments, errors, punches, pay rates and dollar amounts) this crate's
    `EmployeeShift`/`PlannedShift` never modeled, since no other ported file reads any of them
    back. Growing both entities for this one write-only caller would be pure churn for a step that
    otherwise has real sequencing logic (delete old generated snapshots for the job list's ids,
    load the current period's scheduled shifts, clone-as-generated, save) — that sequencing is
    ported for real; only the clone's actual field copying is stubbed. One consequence:
    `cloneEmployeeShiftAsGenerated`'s own two `setShiftType(GENERATED)` calls (on the cloned
    `EmployeeShift` and its `PlannedShift`, right after the clone — confirmed by
    `SaveScheduleSnapshotServiceTest.groovy`'s final assertions) aren't ported either, since
    `shift_type` isn't a modeled field on either entity (`entity::employee_shift`'s own doc already
    says so) — nothing ported reads it back, so this is the same "inert until a reader appears"
    gap as `PlannedShift`'s other unmodeled fixed-constant fields, not a new one.
47. **`SaveSchedulesService.auditPlannedShiftChanges`'s two `getXPlannedShiftAudits` helpers
    resolve the "old" side by id, not by full entity, unlike Java.** Java's
    `getClearedEmployeeShiftsMap()` is `Map<PlannedShift, EmployeeShift>` — real Hibernate-managed
    entities as both key and value; this crate's `ScheduleModel.cleared_employee_shifts` has
    always been `HashMap<i32, i32>` (finding 5's entity-identity-keyed-map treatment, decided back
    at step 5, well before this wave needed the full entities for an audit-trail row). Rather than
    grow that map into a third shape this late, `PlannedShiftAuditPort::create_modify_audit_*`
    take the ids directly — a real DAO would resolve them from the DB by id anyway, so nothing is
    lost, but it's a genuine interface divergence from Java's literal object-reference signature,
    not just an implementation detail. Also: `getScheduledPlannedShiftAudits`'s `.map(it ->
    ...it.getPlannedShift().getID()...)` NPEs in Java on a `null` planned shift; every real
    `new_shift_list` entry always has one (finding 30), but since this is an audit/reporting path
    rather than core scheduling logic, this port uses `filter_map` (skip, don't panic) instead of
    the literal `.expect()` this crate uses elsewhere for guaranteed-non-null NPE sites — a
    deliberate, documented softening, not a guess.
48. **`SaveScheduleLogService`'s actual XML/gzip serialization (`ScheduleLogWriter`,
    `java.util.zip.GZIPOutputStream`, and the temp-file round-trip in between) is entirely out of
    this wave's scope** — `ScheduleLogWriterPort` wraps the three calls
    (`outputHeader`/`outputScheduleLog`/`outputFooter`) into an in-memory `Vec<u8>` buffer instead
    of a temp file, and the encoding itself is deferred. What *is* real: `ScheduleModel::
    job_schedule_logs()` (Phase 2 step 9 had left this sorted-plural accessor unported, flagged in
    its own doc as deferred to step 10) sorts by job full name case-insensitively, same as
    `JobList::all_jobs`; the nested per-job, per-filter-key traversal that decides which
    `ScheduleLog`s get written and in what order is ported for real, not stubbed alongside the
    encoding. `DateTimeServices.currentDateTime()` becomes `joda_rs::LocalDateTime::now()` called
    directly, not a port — same "cheap to port faithfully, nothing needs to control it in a test"
    treatment as `common::java_random::JavaRandom` (finding 23).
49. **`ScheduleEngine`'s steps 3-4 need the same take/reinsert idiom as `PreScheduleProcess`/
    `RegularScheduleSingleShift`, one level up.** `ProjectedHoursReducer::reduce_projected_hours`/
    `EmployeeAvailableHoursBalancer::compute_balance_factor` both take `&ScheduleModel` alongside
    `&mut JobData` — but here the `&mut JobData` comes from `schedule_model.job_list_mut()` itself,
    so the aliasing is with the *same* `ScheduleModel` the immutable parameter borrows, not a
    sibling collection (contrast finding 30's `EmployeeList`/`ScheduleModel` case). Resolved with a
    new `JobList::take_job_data` (removes and returns one `JobData`, mirroring `EmployeeList::
    take_employee_data` exactly): each per-job loop iteration takes the job out, calls the reducer/
    balancer with the now-unaliased `&schedule_model` + `&mut job_data`, then reinserts via the
    existing `add_job_data`. `adjust_projected_hours_for_jobs`/`compute_balance_factors_for_jobs`
    are ported as free functions rather than `ScheduleEngine` methods specifically so they're
    testable without constructing all nine of `ScheduleEngine`'s other collaborators.
50. **`EmployeeShift` drops `Copy` — a deliberate, user-confirmed structural change touching ~55
    files.** `ScheduleChecker` needs each shift to carry a live, mutable `errors: Vec<
    EmployeeShiftError>` (Java: `shift.getErrors().add(...)`, a list the shift owns and every
    holder of that shift shares by reference). Two options were on the table: keep `EmployeeShift`
    `Copy`/unchanged and have `ScheduleChecker` hold a side table (`HashMap<shift_id,
    Vec<EmployeeShiftError>>`) instead, or make `EmployeeShift` own its errors directly and drop
    `Copy` crate-wide. Chosen (explicitly, after flagging the blast radius): the latter — matches
    Java's object model exactly, at the cost of fixing up every `.copied()`/implicit-copy call site
    across Phase 1-2's already-tested files (mechanical: `.copied()` → `.cloned()` on iterators,
    `*shift` → `shift.clone()` on individual values; `PARITY_AUDIT.md`'s own methodology doesn't
    call for a step-by-step log of each of the ~12 touched files since none changed behavior, only
    ownership). `EmployeeShift` also grew `hours_distributions: Vec<HoursDistribution>` in the same
    wave (`ScheduleHoursDistributionValidator`/`ScheduleCalcDataSet`'s `TimeCard` default methods'
    first real reader) — see `entity::hours_distribution`'s doc for why `HoursDistribution` itself
    is only a 3-field slice (`date`/`hours`/`is_premium`) of Java's real 10-field entity.
51. **`ScheduleEmployee` is not ported.** Nothing in the entire `taps` checkout — not `taps/`'s
    other `autosched` files, not any other module — constructs or references
    `autosched.ScheduleEmployee`; it's dead code in the Java source. Confirmed via a full-checkout
    grep before deciding to skip it, not assumed.
52. **`ScheduleCalcDataSet`'s `TimeCard`-interface default methods (`hasOvertime`,
    `getOvertimeForDateRange`, `getTotalPremiumHours`, `getNetHoursForWorkWeek`,
    `getDistributionHoursForDateRange`) are ported as real sums over `EmployeeShift::
    hours_distributions`, not stubbed** — resolving the "real calculation this crate hasn't
    grounded yet" gap this type's doc flagged since Phase 2 step 1. The one piece still not
    reproduced is Java's `distributionIsPremium(HoursDistribution)`, which classifies a
    distribution as regular-vs-premium via `TimeCard.getRegularHoursDistributionTypeIds()` — a
    property-level configuration lookup, not a calculation. `HoursDistribution::is_premium` is a
    pre-resolved stand-in for that one classification, same "narrow entity slice, defer the
    classification lookup" treatment as `entity::employee_pay_type`/`entity::scheduling_method`.
    `Assignment::min_shift`/`max_shift` got the same treatment for a different reason: Java's real
    `getMinShift()`/`getMaxShift()` are themselves `@Deprecated`, resolving through
    `InheritedPlannerSettingsService`/`StandardSet` — the `planner` crate's domain, which
    `scheduler` deliberately doesn't depend on (`PLAN_SCHEDULER.md`) — so they're flat pre-resolved
    `f64` fields here instead of a re-derived settings-inheritance chain.
53. **`ScheduleRestrictionRuleChecker` gains its other two Java methods
    (`runStrictRestrictions`/`runNonStrictRestrictions`), unblocked by `EmployeeShiftError` now
    existing** (finding 50) — its own doc had left them unported specifically because their return
    types weren't modeled yet. Rather than adding a `ScheduleCalcDataSet`-based dependency to the
    existing `ScheduleRestrictionRuleChecker` struct (which the two existing `CanWorkChecker`-only
    factories never construct), the two methods live on a small sibling type,
    `ScheduleRestrictionRules`, backed by its own `ScheduleRestrictionRulesPort` — same
    `RuleUtils`/`RuleImplFactory`/`RuleSet`/`RuleItem`/`ScheduleRestrictionRuleImpl` rule-dispatch
    machinery kept out of scope, for the same reason as the original method.
54. **`ScheduleChecker`'s `validateMinHoursOff`/`validateMinShiftLength`/`validateMaxShiftLength`/
    `validateAgainstTimeOffRequests`/`validateAgainstAvailabilityRequests`/
    `validateScheduleRestrictionRules` each split into a read pass and a write pass** — Java reads
    every shift in the dataset while deciding which *other* shift(s) to flag, then mutates in the
    same loop, which Rust can't do once `self.dataset` needs both an immutable borrow (reading
    every shift to compute flags) and a mutable one (`shifts_mut()`, applying them) at once. Each
    method first collects the ids to flag into a `HashSet<i32>`, then a second loop over
    `shifts_mut()` applies `add_error` to the matching ones — same result, same order-independent
    outcome (nothing here depends on *which* shift is visited first), just two passes instead of
    one. `ScheduleChecker` itself holds `&'a mut ScheduleCalcDataSet` (not `&'a ScheduleCalcDataSet`)
    for exactly this reason — every mutating method needs to reborrow it.
55. **`ScheduleMatcher.getEmployeesForPlannedShift`'s `List<TPair<ScheduleCalcDataSet, Integer>>`
    becomes `Vec<(i32, i32)>`** (employee id, weight) rather than carrying the dataset itself —
    `datasetMap` is already keyed by employee id, so a caller that needs the dataset back looks it
    up there, and returning the id sidesteps holding a second alias into `datasets` while the
    ported sort comparator needs to read two entries by id at once. That comparator is ported
    exactly as Java wrote it — `(pair1, pair2) -> isBetterMatch(...) ? 1 : 0` never returns a
    negative value, so it was never a strict total order in Java either; preserved as-is
    (`Ordering::Greater`/`Ordering::Equal`) rather than "fixed," to keep the same resulting order.
    `ScheduleUtils.createEmployeeShiftFromPlannedShift` is inlined as a free function — its
    `ShiftCategoryDAO.findByID` round-trips the same id back out, so the DAO call is skipped, and
    its punch creation isn't modeled (nothing ported reads `EmployeeShift`'s punches).

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

**Phase 2 steps 7-8 (`PermanentScheduleProcess`/`RegularScheduleProcess` + the shared
`process/regularschedules/` machinery) are done — merged into one wave (finding 31).** New files:
`engine::io::regular_schedule_loader::RegularScheduleLoader` (deferred from step 1 per finding 20,
its first real caller, plus a new `EmployeeRegularPeriodDAOPort`); `engine::misc::
projected_hours_checker::ProjectedHoursChecker` (thin wrapper, both overloads ported for 1:1
parity though only one is reached this wave); `engine::misc::planned_shift_matcher::
PlannedShiftMatcher` and `engine::misc::planned_shift_helper::PlannedShiftHelper` (new, unit-struct
`PlannedShiftMatcher` since `EmployeeAssignmentChecker::can_employee_work_assignment` is already a
plain associated function); `PlannedShiftCreator`'s `RegularSchedule` overload (finding 36);
all seven `process/regularschedules/` files (`EmployeeRegularPeriodComparator` — thin delegation to
the already-ported `EmployeeSeniorityComparator`; `RegularScheduleCanWorkCheckerFactory`/
`RegularScheduleCanWorkChecker` — findings 34-35; `RegularScheduleEmployeeShiftCreator`;
`RegularScheduleSingleShift` — the take/mutate/reinsert point, mirroring `PreScheduleProcess`
exactly; `RegularScheduleSingleDate` — findings 33 and the early-return/`isPermanent()` hazards
called out in its own module doc; `RegularScheduleGenerator`); and `PermanentScheduleProcess`/
`RegularScheduleProcess` themselves. `RegularSchedule`/`RegularSchedules` (`engine::model`, ported
in the Phase 1 prerequisite wave) needed one correction (finding 32) and one net-new method
(`regular_schedules_for_job_and_date`) to become real rather than blocked.

Findings 32-37 cover this wave's hard parts: the `assignment_id` optionality bug (32), three
distinct simultaneous-borrow shapes beyond finding 30's `EmployeeData` case — `job_id`-not-
`&JobData` in the per-date loop (33), a `JobData` clone for the checker-list construction (34),
and the DI-free checker factory (35) — plus two `job_id`-threading/deferred-fallback divergences
from Java's literal re-derivation (36-37).

69 tests, all passing (12 new tests: two end-to-end `RegularScheduleSingleDate` cases proving the
early-return-not-continue and per-employee `isPermanent()` hazards; a `RegularScheduleCanWorkChecker`
short-circuit case with `Cell<bool>` spies per finding 26's convention; `PlannedShiftMatcher`
matching/fallback/no-match cases; `RegularSchedules` filter/sort cases; a `RegularScheduleLoader`
wiring case), `cargo build --workspace`/`clippy --all-targets`/`fmt --check` all clean.

**Phase 2 step 9 (`process/variable/` + `VariableScheduleProcess`, 23 files — the largest remaining
pipeline wave) is done.** Ported in dependency order: `comparators/` (`NonPreScheduledJobComparator`/
`PreScheduledJobComparator`, wired into `JobList::non_pre_scheduled_jobs`/`pre_scheduled_jobs`,
resolving finding 3's first half); `filters/` (`EmployeeFilter` trait + 7 impls, plus the new
`EmployeeFilterKey` — finding 38); `generators/` (`VariableSchedulingGenerator` trait,
`AbstractVariableSchedulingGenerator`'s two helpers as free functions since this crate has no
inheritance, and the three concrete generators); then the top-level orchestration files
(`VariableChecker`, `VariableCanWorkChecker`/`Factory`, `PlannedShiftScheduler`,
`VariableJobScheduler`, `VariableJobSchedulingProcess`, `NonPreScheduledJobProcess`/
`PreScheduledJobProcess`) and finally `VariableScheduleProcess` itself (`engine::process`, the
step-9 facade, same shape as `PermanentScheduleProcess`/`RegularScheduleProcess`).

Unblocked, not just ported this wave: `JobList::non_pre_scheduled_jobs`/`pre_scheduled_jobs`
(finding 3's first half) and `JobScheduleLog`/`ScheduleLog` (`engine::model::logging`, finding 3's
second half, blocked since Phase 1 — see finding 39). New supporting types: `entity::
employee_pay_type::EmployeePayType`, `entity::employee_job_status::EmployeeJobStatus.pay_type`
(the six contract/salaried filters' first real reader), `entity::scheduling_method::
SchedulingMethod` + `Assignment.scheduling_method` (`VariableJobSchedulingProcess`'s dispatch key),
and `Property.max_scheduler_passes`/`max_employee_skills`/`max_balance_levels`
(`AbstractVariableSchedulingGenerator`'s two helpers). Findings 38-43 cover this wave's hard
parts: the `EmployeeFilter`-trait-plus-`EmployeeFilterKey` design (38), `JobScheduleLog`/
`ScheduleLog` unblocking and `ScheduleModel` losing its `Clone`/`PartialEq` derives (39), two more
instances of the simultaneous-borrow family plus a new "can't resolve an assignment id" divergence
in `VariableChecker` (40), `PlannedShiftScheduler`'s "hold at most one taken-out `EmployeeData`
across a whole loop" extension of the take/reinsert idiom (41), the DI-map-to-`match` idiom applied
to `SchedulingMethod` dispatch (42), and `PreScheduledJobProcess`'s grouped-job shift-merging (43).

`process/variable/` had comparatively little Java/Groovy test value to transcribe as
`java_parity_tests`: `NonPreScheduledJobComparatorTest`/`PreScheduledJobComparatorTest` (real
value tables, both ported via `rstest`) were the only two with genuine value assertions;
`JobLevelEmployeeFilterTest`, `VariableJobSchedulerTest`, and the rest of the top-level
orchestration suite are pure interaction/mock-call-count tests (same shape as finding 24), so
their outcomes are covered through real state instead — `PlannedShiftScheduler`'s own test proves
the better-schedule-order employee wins and gets saved; `VariableJobScheduler`'s proves a shift
that would exceed projected hours is skipped while one that wouldn't gets logged;
`PreScheduledJobProcess`'s proves grouped jobs' shifts get merged onto the lowest-`order_no` job in
the group.

112 tests, all passing (43 new tests this wave), `cargo build --workspace`/`clippy -p scheduler
--all-targets`/`fmt -p scheduler --check` all clean.

**Phase 2 step 10 (`io::SaveSchedulesService`) is done — Phase 2 is now fully complete.**
`SaveSchedulesService` itself, plus its three collaborators (`CancelShiftRequestsService`,
`SaveScheduleSnapshotService`, `SaveScheduleLogService`) are all ported. `CancelShiftRequestsService`
is a real no-op (finding 44 — its Java body is entirely commented out already). `SaveSchedulesService`'s
own real logic — deleting prior schedules, saving new shifts, the two audit-diffing passes
(`auditScheduleChanges`'s old-vs-new shift comparison, `auditPlannedShiftChanges`'s cleared-shift
bookkeeping), clearing the old shift list, recalculating each employee's data set, and the final
cancel/log/snapshot sequence — is ported for real against grounded `ScheduleModel` state, not
stubbed; only the actual DAO/external-subsystem calls at the leaves (deletes, saves, audit-row
creation, the time-card recalculation, alert refresh) go behind new port traits (finding 45).
`SaveScheduleSnapshotService`'s clone step and `SaveScheduleLogService`'s XML/gzip serialization
are both genuinely out of scope (findings 46, 48) — same "narrow entity slice"/"deferred external
subsystem" treatment this crate has used throughout, not new gaps introduced by this wave.
`ScheduleModel::job_schedule_logs()` (the sorted plural accessor Phase 2 step 9 explicitly deferred
to this step) and `JobScheduleLog::schedule_log_for_key`/`ShiftList::clear_employee_shifts` are new
supporting methods this wave's first real reader/writer needed.

All three of step 10's Groovy test files (`SaveSchedulesServiceTest`,
`SaveScheduleSnapshotServiceTest`, `SaveScheduleLogServiceIntegrationTest`) are either pure
interaction/mock-call-order tests (same shape as finding 24) or a full DB-backed integration test
(`BaseWatsonTXSpockIntegrationTest`, no unit-test equivalent this crate can run against) — none
transcribed as `java_parity_tests`; each service's own `mod tests` covers the same real-state
outcomes instead (deleted ids, saved shifts, audit call counts, the written report's real fields).

116 tests, all passing (4 new tests this wave), `cargo build --workspace`/`clippy -p scheduler
--all-targets`/`fmt -p scheduler --check` all clean.

**`ScheduleEngine` (Phase 3's orchestrator) is done.** `generate_schedules` is the straight-line
10-step pipeline: `ScheduleModelLoader::load` (returning `None` on failure, matching the loader's
own contract — `Progress` isn't threaded, per `ScheduleModelLoader`'s own doc), each of the other
nine steps gated exactly as `ScheduleEngine.java` gates them
(`isRotateDaysOff`/`ScheduleMode::Weekly`/`isBalanceSchedules`/the four `isGenerate*Schedules`
flags), ending in `SaveSchedulesService::save_schedules`. `rotateDayOffPlans`'s Java body is
`void` (Hibernate dirty-checking persists the rotation without the `ScheduleModel` ever seeing the
result); `DayOffPlanRotator::rotate_day_off_plans`'s returned `(Vec<DayOffPlan>, Vec<Employee>)` is
called and discarded here, matching that void-ness for the in-memory model exactly (finding 49's
doc). Steps 3-4's per-job loops needed the new `JobList::take_job_data` (finding 49) — otherwise a
direct translation.

`ScheduleEngineTest`/`ScheduleEngineImplTest`-shaped Groovy tests (if any exist) would be pure
interaction/mock-call-count tests over nine already-tested collaborators (same shape as finding
24), so nothing was transcribed; `mod tests` instead covers the two new take/reinsert loops
directly as free functions (`adjust_projected_hours_for_jobs`/`compute_balance_factors_for_jobs`),
gating on `ScheduleMode`/`is_balance_schedules` and proving every job survives the round trip — an
end-to-end `generate_schedules` test would mostly re-verify existing per-step wiring rather than
new behavior, so it wasn't added.

119 tests, all passing (3 new tests this wave), `cargo build --workspace`/`clippy -p scheduler
--all-targets`/`fmt -p scheduler --check` all clean.

**`autosched/` (Phase 3's last wave) is done — the `scheduler` crate port is now complete per
`PLAN_SCHEDULER.md`.** Scoped to full depth (decided with the user ahead of this wave, after
flagging that `ScheduleChecker`/`ScheduleMatcher` sit on a second aggregate,
`ScheduleCalcDataSet`, that this crate had deliberately left partial since Phase 2 step 1):
`ScheduleHoursDistributionValidator` and `ExceedsAvailableHoursConflictValidator` are pure/near-pure
ports with one new port each (none, and `CurrentUserPort` respectively); `ScheduleChecker` and
`ScheduleMatcher` are both ported for real against grounded `ScheduleCalcDataSet`/`Employee`/
`Assignment` state, not stubbed — only the genuinely external leaves (`EmployeeCertificationValidator`
via `CertificationPort`, the schedule-restriction rule engine via `ScheduleRestrictionRulesPort`,
`CurrentUser`'s security check via `CurrentUserPort`) stay behind ports, same treatment as every
other deferred subsystem this crate has used since Phase 1. `AvailPeriod` was already ported
(a Phase 1 prerequisite, `EmployeeAvailabilityChecker`'s dependency); `ScheduleEmployee` is not
ported — confirmed dead code, referenced nowhere in the entire `taps` checkout (finding 51).

Getting there required one structural change to already-completed work, confirmed with the user
first: `EmployeeShift` drops `Copy` to own its errors directly (`errors: Vec<EmployeeShiftError>`,
matching Java's `shift.getErrors()`), rippling a mechanical `.copied()` → `.cloned()`/`*shift` →
`shift.clone()` fixup across roughly a dozen Phase 1-2 files (finding 50) — no behavior changed in
any of them, confirmed by the full existing suite staying green throughout. `ScheduleCalcDataSet`'s
`TimeCard`-interface default methods (`hasOvertime`, `getOvertimeForDateRange`,
`getTotalPremiumHours`, `getNetHoursForWorkWeek`, `getDistributionHoursForDateRange`) are now real,
resolving the "real calculation not grounded yet" gap flagged since Phase 2 step 1 (finding 52).
`ScheduleRestrictionRuleChecker` gained its other two Java methods via a small sibling type,
`ScheduleRestrictionRules` (finding 53). Findings 54-55 cover the remaining adaptations: splitting
`ScheduleChecker`'s six validation passes into a read pass then a write pass (the borrow-checker
version of Java's single mutate-while-reading loop), and `ScheduleMatcher`'s `TPair` → `(id,
weight)` tuple plus its intentionally-non-strict sort comparator preserved as-is.

No Groovy/Java test file was found for `ScheduleHoursDistributionValidator`/
`ExceedsAvailableHoursConflictValidator`/`ScheduleMatcher`; `ScheduleCheckerTest.java` exists but
depends on test-helper infrastructure not present in this checkout (`TestableScheduleChecker`,
`LaborTestUtils`, `TestUtils`) — none transcribed as `java_parity_tests`. Each new file's own `mod
tests` instead proves the same real-state outcomes independently derived from the production
methods (status/job/overlap/negative-length fatal conflicts, min/max shift length overridable
errors, home-job/job-not-active/not-active hours-distribution errors, weekly-mode-gated available-
hours conflicts, job-eligibility filtering, and the `EmployeeType` sort-order branch of
`isBetterMatch`).

133 tests, all passing (14 new tests this wave), `cargo build --workspace`/`clippy -p scheduler
--all-targets`/`fmt -p scheduler --check` all clean. This closes out every wave in
`PLAN_SCHEDULER.md` — Phase 0 through Phase 3, including `autosched/`.
