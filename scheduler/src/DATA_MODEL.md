# `scheduler` Data Model — Phase 0

> Catalogs `ScheduleModel` and the `engine/model/` package (12 files, including `model/logging/`,
> 6 files) plus `GenerateSchedulesParameters`, ahead of any Rust code. Ground truth is `taps/`
> (`watson/server/scheduler/{engine,autosched}` and
> `watson/common/labor/planner/{GenerateSchedulesParameters,GeneratorParameters}.java`), read
> directly for this document — never a local convenience copy. See
> `scheduler-crate-targets-engine-not-employeescheduler` and `lms-rs-java-port-conventions`
> memories, and `PLAN_SCHEDULER.md` (repo root) for the full phased plan.

## 1. Overview

`ScheduleEngine.generateSchedules()` runs a fixed 10-step pipeline over one `ScheduleModel`: load
→ rotate days off → reduce projected hours (weekly) → balance available hours (weekly) → prepare
shifts → pre-schedule → permanent-schedule → regular-schedule → variable-schedule → save. This
document covers only the data the pipeline operates on (`engine/model/`) and the engine's input
parameters (`GenerateSchedulesParameters`) — not the pipeline steps themselves, which are later
phases.

`ScheduleModel` is the root: it wraps a `Property`, a `DateRange` to schedule over, and a
`Progress` handle, and owns everything the pipeline mutates — the new/old `ShiftList`s, the
`JobList` (one `JobData` per job), the `EmployeeList` (one `EmployeeData` per employee), and a
per-job `JobScheduleLog` map used for auditing/debugging the run.

## 2. Id strategy — **diverges from `planner`**

`planner` mints `uuid`-backed newtype ids via `id_type!` (`common/id_type.rs`) for every entity,
because `workcontent` entities are new domain concepts with no legacy PK. `scheduler`'s model is
different: every entity it touches (`Assignment`/job, `Employee`, `PlannedShift`, `EmployeeShift`,
`Property`) is a **Hibernate entity with a legacy `int` primary key** — confirmed by
`Assignment.getID()`/`Employee.getID()` (both `int`), by `JobList`/`EmployeeList` keying their
maps on `Integer`, and by `GeneratorParameters` (`GenerateSchedulesParameters`'s base class)
declaring `propertyID`/`divisionID`/`departmentID`/`jobID`/`jobIDs` all as `int`/`List<Integer>`.

This is exactly the case `lms-rs-java-port-conventions` calls out: *"`planner`'s UUID `id_type!`
macro does not transfer to crates whose ids are legacy integer DB keys."* `workrules` already hit
this and settled on bare `i32` with no newtype wrapper (see `workrules/src/entity/time_card.rs`,
e.g. `fn employee_id(&self) -> i32`). `scheduler` follows that precedent, not `planner`'s:

- `JobId` = `i32` (an `Assignment`'s id, when that assignment is a job leaf — `Assignment` and
  "job" are the same Java type; Rust may still want a `JobId`/`AssignmentId` distinction at the
  type level, TBD when `Assignment` itself is ported)
- `AssignmentId` = `i32`
- `EmployeeId` = `i32`
- `PlannedShiftId` = `i32`
- `EmployeeShiftId` = `i32`
- `PropertyId` = `i32`

No `id_type!` macro usage in this crate. If later phases want stronger typing than bare `i32`
(there's real risk of mixing up job/employee/shift ids, all `i32`), that's a newtype-without-uuid
decision to make then — noted here so it isn't silently skipped, not decided yet.

## 3. Referenced Hibernate entities — minimal stubs only

`Assignment` (1133 lines), `Employee`, `PlannedShift`, `EmployeeShift`, and `Property` are large
Hibernate entities out of scope for a full port in Phase 0. The fields below are only what
`engine/model/` and `GenerateSchedulesParameters` touch directly; expect this table to grow every
time a later wave (checkers, comparators, pipeline steps) reads a new getter off one of these
types. Do not treat this as a complete entity — it isn't.

#### `Assignment` (doubles as "job" — `Assignment.getJob()` on a leaf returns itself; used as both)
| Field (Java getter) | Type | Used by |
|---|---|---|
| `getID()` | `int` | `JobList`/`EmployeeList` map keys, equality-by-id throughout |
| `getFullName()` | `String` | `JobList.getAllJobs()`/`ScheduleModel.getJobScheduleLogs()` sort |
| `isBalanceSchedules()` | `bool` | `WeeklyAvailableHours` (only for `EmployeeType` variable path) |
| `getMinHoursOff()` | `Double` (nullable, walks parent chain) | `EmployeeData.getMinHoursOff` |
| `getMinDaysOff()` | `Integer` (nullable, walks parent chain) | `EmployeeData.getMinDaysOff` |
| `getParentAssignment()` | `Assignment` (nullable) | parent-chain walks above |

#### `Employee`
| Field (Java getter) | Type | Used by |
|---|---|---|
| `getID()` | `int` | `EmployeeList` map key |
| `getName()` | `String` | `EmployeeList.getEmployeeDataList()` sort |
| `getEmployeeType()` | `EmployeeType` enum (`REGULAR`/`PERMANENT`/…) | `WeeklyAvailableHours` regular-vs-variable branch |
| `getHoursAvailable()` | `Double` (nullable, falls back to `WorkClass`) | `WeeklyAvailableHours.getBaseAvailableHours` |
| `getWorkClass()` | `WorkClass` (has its own `getHoursAvailable()`) | same fallback |
| `getMinHoursOff()` / `getMinDaysOff()` | `Double`/`Integer` (nullable) | `EmployeeData`, checked before walking assignment parent chain |
| `getEmployeeJobStatus(job, date)` / `getEmployeeJobStatuses(date)` | `EmployeeJobStatus`/`List<EmployeeJobStatus>` | `EmployeeData.hasJobAtAnyLevel`/`hasJobAtLevel` |
| `getHomeEmployeeJobStatus(date)` | `EmployeeJobStatus` (nullable) | `RegularSchedule.getJob` fallback when the regular period has no explicit job |

#### `PlannedShift`
| Field (Java getter) | Type | Used by |
|---|---|---|
| `getID()` | `int` | `ShiftList.getPlannedShiftIDs` |
| `getShiftDate()` | `LocalDate` | `JobData.getPlannedShiftsForDate` |
| `getStartDateTime()` | `LocalDateTime` | sort in `getPlannedShiftsForDate` |
| `getDuration()` | `double` | `JobData.getAverageShiftLength` |

#### `EmployeeShift`
| Field (Java getter) | Type | Used by |
|---|---|---|
| `getID()` | `int` | `ShiftList.getEmployeeShiftIDs` |
| `getShiftDate()` | `LocalDate` | `Schedules` (has-shift/find-first/find-last-on-date) |
| `getStartDateTime()` | `LocalDateTime` | `Schedules.findFirstShiftOnDate`/`findLastShiftOnDate` compare |
| `getJob()` | `Assignment` | `Schedules.hasShiftWithJobOnDate` |
| `getAssignment()` | `Assignment` (nullable) | `Schedules.hasShiftWithAssignmentOnDate` |

#### `Property`
| Field (Java getter) | Type | Used by |
|---|---|---|
| `getCurrentWeek()` | has `getDateRangeContainingDate(date)` | `ScheduleModel.getWeekForDate` |

Also referenced but not cataloged yet (deferred to the wave that first needs them):
`EmployeeRegularPeriod`, `EmployeeJobStatus`, `EmployeeTimeOff`, `EmployeeType` (enum),
`WorkClass`, `ScheduleCalcDataSet` (`watson/server/labor/calcshift`), `AvailPeriod`
(`autosched/AvailPeriod.java`).

## 4. `engine/model/` types

Conventions: `joda_rs` for dates/times, `date_range_rs::DateRange` for ranges, per
`lms-rs-java-port-conventions`. Getters drop the `get_`/`is_` prefix. No `id_type!` — see §2.

### `ScheduleModel`
*Java: `ScheduleModel.java`* — the pipeline's root aggregate, threaded through every step.

| Field | Type | Notes |
|---|---|---|
| `property` | `Property` | stub, §3 |
| `date_range` | `DateRange` | the range being scheduled |
| `progress` | `Progress` | `com.unifocus.tbx.core.progress.Progress` — not yet modeled; likely a trait/callback, not a struct, when ported |
| `new_shift_list` | `ShiftList` | owned, always-present (Java: `final`, default-constructed) |
| `old_shift_list` | `ShiftList` | same |
| `cleared_employee_shifts_map` | `HashMap<PlannedShiftId, EmployeeShiftId>` | Java keys/values by entity identity via `equals`/`hashCode`, which for Hibernate entities is id-based — map by id, not by owned value |
| `job_schedule_log_map` | `HashMap<JobId, JobScheduleLog>` | lazily populated by `get_job_schedule_log` |
| `job_list` | `Option<JobList>` | Java: settable after construction, null until `set` |
| `employee_list` | `Option<EmployeeList>` | same |
| `current_planned_shift_log` | `Option<PlannedShiftLog>` | set/read during the variable-schedule step |

Methods of note (not fields): `get_week_for_date(date)` delegates to
`property.getCurrentWeek().getDateRangeContainingDate(date)`; `get_job_schedule_log` is
get-or-insert (lazy map population — matches Rust `entry().or_insert_with()`);
`get_job_schedule_logs()` returns all logs sorted by job full name, case-insensitive.

### `JobData`
*Java: `JobData.java`* — per-job scheduling state for one pipeline run.

| Field | Type | Notes |
|---|---|---|
| `job` | `Assignment` (job) | `Job` reference — see §3 |
| `projected_hours` | `HoursByDate` | owned, always-present |
| `scheduled_hours` | `HoursByDate` | owned, always-present |
| `planned_shifts` | `Vec<PlannedShift>` | |
| `balance_factor` | `f64` | default `1.0` |
| `balance_level` | `i32` | default `0`, `increment`/`reset` methods |
| `average_shift_length` | `f64` | lazily computed and cached on first read (`<= 0.0` sentinel means "not yet computed") — a cache, not a real default; port as computed-on-demand or an `Option`/`OnceCell`, not a mutable cache field copied verbatim |
| `pre_schedule_parameters` | `Option<PreScheduleParameters>` | presence distinguishes pre-scheduled vs. not, used by `JobList` |

Methods of note: `get_planned_shifts_for_date` filters + sorts by start time;
`will_scheduled_hours_exceed_projected_hours(date, additional_hours)` and
`has_exceeded_projected_hours(date)` are plain comparisons — no `TDouble` rounding involved here,
but downstream hour arithmetic (steps 3/4/8/9) should still be checked per
`tdouble-rounding-parity-trap` when ported.

### `JobList`
*Java: `JobList.java`* — id-keyed collection of `JobData`, with pre-scheduled/non-pre-scheduled
partitioning.

| Field | Type | Notes |
|---|---|---|
| `job_data_map` | `HashMap<JobId, JobData>` | |

Methods: `get_all_jobs()` — all, sorted by job full name (case-insensitive);
`get_non_pre_scheduled_jobs()` / `get_pre_scheduled_jobs()` — partitioned on
`pre_schedule_parameters.is_none()`/`is_some()`, each sorted by a comparator that lives in
`process/variable/comparators/` (`NonPreScheduledJobComparator`/`PreScheduledJobComparator`,
Phase 1 — these sorts can't be ported until that wave lands).

### `EmployeeData`
*Java: `EmployeeData.java`* — per-employee scheduling state for one pipeline run.

| Field | Type | Notes |
|---|---|---|
| `employee` | `Employee` | §3 |
| `data_set` | `ScheduleCalcDataSet` | not yet modeled (`watson/server/labor/calcshift`); holds shifts/time-off/availability/overtime, deferred |
| `weight` | `i32` | constant per instance: `MINUTES_PER_DAY * DAYS_PER_WEEK` = `1440 * 7` = `10080`; set once in the constructor from `DateTimeConstants`, not truly per-employee data — worth double-checking whether this is dead/unused weight before porting it as a stored field vs. a constant |
| `random` | `i32` | passed into the constructor; used as randomization input elsewhere in the engine (tie-breaking?), not resolved yet |
| `weekly_available_hours_map` | `HashMap<JobId, WeeklyAvailableHours>` | lazily populated get-or-insert, keyed by the `JobData` it was computed for |
| `pre_schedule_check_overtime` | `f64` | default `0.0`, set via `store_pre_schedule_check_overtime(date_range)` from `data_set.getOvertimeForDateRange` |

Methods of note: `has_job_at_any_level`/`has_job_at_level` read `EmployeeJobStatus` (not modeled
yet); `get_min_hours_off`/`get_min_days_off` walk the assignment's parent chain when the employee
itself has no override — same nullable-walk-up-the-tree shape as `Assignment`'s own
`getMinHoursOff`/`getMinDaysOff`, i.e. employee override wins, else assignment-chain override,
else a hardcoded default (`0.0`/`0`); `get_effective_available_hours_for_date` starts from
`HOURS_PER_DAY` (24.0) and subtracts required-off `AvailPeriod` durations for that date — note this
is calendar hours, not a working-hours default, easy to misport.

### `EmployeeList`
*Java: `EmployeeList.java`* — id-keyed collection of `EmployeeData`.

| Field | Type | Notes |
|---|---|---|
| `employee_data_map` | `HashMap<EmployeeId, EmployeeData>` | |

Methods: `get_employee_data_list()` sorted by employee name (case-insensitive);
`get_unsorted_employee_data_list()` insertion/map order (Java: `HashMap` iteration order is
unspecified — port as whatever Rust `HashMap` gives, this method never claimed a stable order in
Java either); `get_employees_with_job(job, date)` filters via `has_job_at_any_level`.

### `ShiftList`
*Java: `ShiftList.java`* — flat holder of two independent shift collections (not one list of a
shared supertype — `EmployeeShift` and `PlannedShift` are unrelated Java types here).

| Field | Type | Notes |
|---|---|---|
| `employee_shifts` | `Vec<EmployeeShift>` | |
| `planned_shifts` | `Vec<PlannedShift>` | |

`get_planned_shift_ids()` explicitly null-checks each element before taking its id (Java list may
contain `null` planned shifts) — port as filtering `Option<PlannedShift>`/skip-if-absent, not an
`.unwrap()`.

### `RegularSchedule`
*Java: `RegularSchedule.java`* — one weekly recurring work period for an employee, derived from an
`EmployeeRegularPeriod` (not modeled yet).

| Field | Type | Notes |
|---|---|---|
| `employee_data` | `EmployeeData` (by id/ref) | |
| `job` | `Option<Assignment>` | may be null in Java — `get_job(shift_date)` falls back to the employee's *home* job status on that date when null |
| `assignment` | `Assignment` | distinct from `job` — the regular period's assignment, not necessarily the job leaf |
| `day_of_week` | `DayOfWeek` (`tbx.core.DayOfWeek`, not `joda`'s) | |
| `time_range` | `TimeRange` (`tbx.core.timerange.TimeRange`) | built from the period's start/end time |
| `duration` | `f64` | |

`get_date_time_range(shift_date)` projects `time_range` onto a concrete date via
`DateTimeRange.fromTimeRangeOnDate` — the Rust equivalent needs a `joda_rs`/`date_range_rs`
combinator or a small helper, TBD when ported.

### `RegularSchedules`
*Java: `RegularSchedules.java`* — all `RegularSchedule`s for a `ScheduleModel` run, built from
`List<EmployeeRegularPeriod>` at construction (skips periods whose employee isn't in the model's
`EmployeeList`).

| Field | Type | Notes |
|---|---|---|
| `regular_schedules` | `Vec<RegularSchedule>` | |

Depends on `EmployeeJobStatusChecker` (`process/checkers/`, Phase 1) and
`EmployeeRegularPeriodComparator` (`process/regularschedules/`, Phase 2 step 8) for its core
method, `get_regular_schedules_for_job_and_date` — filter by day-of-week + job match +
`canEmployeeWorkJobOnDate`, then sort by seniority. **Cannot be ported before those two land** —
noted here for completeness of the model catalog, not as a Phase 0 deliverable.

### `WeeklyAvailableHours`
*Java: `WeeklyAvailableHours.java`* — per-(employee, job) cache of hours available in a given week,
keyed by week-end date.

| Field | Type | Notes |
|---|---|---|
| `employee_data` | `EmployeeData` (by id/ref) | |
| `job_data` | `JobData` (by id/ref) | |
| `weekly_hours_available_map` | `HashMap<LocalDate, f64>` | week-end date → hours, memoized on first computation |

`HOURS_PER_TIME_OFF_REQUEST_DAY = 8.0` is a named constant, not a config value — carry it as one.
Computation branches on `EmployeeType`: `REGULAR`/`PERMANENT` employees get base hours minus 8.0
per time-off day that falls in the target week; everyone else (variable) gets base hours scaled by
`JobData.balance_factor` when `job.is_balance_schedules()`, plus a `balance_level`-driven bump
capped at the unscaled base — this is the one call site in `engine/model/` that reads
`JobData.balance_factor`/`balance_level`, produced by step 4 (`EmployeeAvailableHoursBalancer`,
Phase 2).

### `HoursByDate`
*Java: `HoursByDate.java`* — simple date→hours accumulator, no surprises.

| Field | Type | Notes |
|---|---|---|
| `hours_by_date_map` | `HashMap<LocalDate, f64>` | |

`get_hours_for_date` defaults to `0.0` for a missing date. `add_hours_to_date`/
`subtract_hours_from_date` are get-or-default-then-write, not `entry().and_modify()` with a
zero-default — same effect, note only for translation fidelity. `sum_hours_for_date_range` iterates
the `DateRange` and sums present entries (absent dates contribute `0.0`, same as `get_hours_for_date`
implicitly).

### `PreScheduleParameters`
*Java: `PreScheduleParameters.java`* — a plain immutable pair.

| Field | Type | Notes |
|---|---|---|
| `order_no` | `i32` | |
| `group_no` | `i32` | |

Trivial — straight-line struct + constructor + getters, no logic.

### `Schedules`
*Java: `Schedules.java`* — read-only query surface over one employee's shifts + time-off for the
run, built either from a `ScheduleCalcDataSet` or directly from lists. Distinct from `ScheduleModel`
and not stored on it — constructed on demand by callers (checkers, comparators — Phase 1/2) that
need to ask "did this employee work/have time off on date X" style questions.

| Field | Type | Notes |
|---|---|---|
| `employee_shifts` | `Vec<EmployeeShift>` (by ref/id) | |
| `employee_time_off_list` | `Vec<EmployeeTimeOff>` | `EmployeeTimeOff` not modeled yet |

Methods: `has_shift_on_date`, `find_first_shift_on_date`/`find_last_shift_on_date` (linear scan +
compare start time — no assumption the input list is sorted), `has_time_off_on_date`,
`get_number_of_prior_days_off` (walks backward combining last-scheduled-date and last-time-off-date
logic — the two `getLastScheduledDate`/`getLastTimeOffDate` private helpers have subtly different
"before" semantics for the time-off case: an in-progress time-off period spanning `shift_date`
collapses to `shift_date.minusDays(1)` rather than its own end date), `has_shift_with_job_on_date`,
`has_shift_with_assignment_on_date`,
`determine_consecutive_days_prior_dates`/`determine_consecutive_days_future_dates` (bounded walk up
to `min_days_off` days looking for a run of consecutive scheduled days). This class is a strong
candidate for parity tests once ported — the date-walking logic is exactly the kind of thing that's
easy to off-by-one.

## 5. `engine/model/logging/` types

Audit trail built up during the variable-schedule step (step 9) and surfaced via
`ScheduleModel.job_schedule_log_map`. All six files are small; cataloged together.

- **`JobScheduleLog`** (`Java: JobScheduleLog.java`) — one per `JobData`; holds an ordered
  `Vec<EmployeeFilter>` (Phase 1's `process/variable/filters/EmployeeFilter` — not modeled yet) and
  a `HashMap<EmployeeFilter, ScheduleLog>`, lazily populated by `get_schedule_log` (get-or-insert,
  same pattern as `EmployeeData.weekly_available_hours_map`). **Blocked on `EmployeeFilter`**
  (Phase 1) to port meaningfully — the map key is that type.
- **`ScheduleLog`** — one per `(JobScheduleLog, EmployeeFilter)`; an ordered `Vec<PlannedShiftLog>`.
  `get_current_planned_shift_log()` panics (`IllegalStateException`) if the list is empty — port as
  returning `Option`/`Result`, not replicating the panic, unless a caller genuinely expects
  "always non-empty when called" as an invariant (verify when the calling step is ported).
- **`PlannedShiftLog`** — one per `PlannedShift`; owns an `EmployeesWithConflicts` and a
  `RankedEmployees`, both default-constructed and always present.
- **`EmployeesWithConflicts`** — parallel `Vec<EmployeeLogEntry>` + `HashMap<EmployeeData,
  EmployeeLogEntry>` (same list+map-for-lookup shape as `JobScheduleLog`); `EmployeeData` used as a
  map key again confirms entity-identity-based `equals`/`hashCode` — Rust should key these maps by
  `EmployeeId`, not by an owned `EmployeeData` value (see `ScheduleModel.cleared_employee_shifts_map`
  note in §4, same pattern repeats here).
- **`EmployeeLogEntry`** — `EmployeeData` (by id/ref) + mutable `Option<String>` notes.
- **`RankedEmployees`** — a `Vec<EmployeeData>` used as a stack (`Stack<EmployeeData>` in Java, only
  ever pushed) — port as `Vec` with `push`, no need for a real stack type unless pop turns up in a
  later-ported caller.

## 6. `GenerateSchedulesParameters` (engine input)

*Java: `watson/common/labor/planner/{GenerateSchedulesParameters,GeneratorParameters}.java`* — the
engine's top-level input, extending a shared `GeneratorParameters` base (also used by other
generators outside `scheduler`'s scope, e.g. `workcontent`). `scheduler` gets its own flat Rust
struct per the plan's decision (§ PLAN_SCHEDULER.md) — not shared with `planner`.

| Field | Type | Source | Notes |
|---|---|---|---|
| `date_range` | `DateRange` | `GeneratorParameters` | range to generate schedules over |
| `property_id` | `PropertyId` (`i32`) | `GeneratorParameters` | |
| `division_id` | `i32` | `GeneratorParameters` | no newtype seen elsewhere in this catalog for division — plain `i32` unless/until a `DivisionId` earns its keep |
| `department_id` | `i32` | `GeneratorParameters` | same |
| `job_id` | `JobId` (`i32`) | `GeneratorParameters` | singular — likely "generate for this one job" mode |
| `job_ids` | `Vec<JobId>` | `GeneratorParameters` | plural — "generate for these jobs" mode; Java defaults to an empty list, never null (`setJobIDs(null)` resets to empty, not null) |
| `clear_schedules` | `bool` | `GenerateSchedulesParameters` | |
| `generate_pre_schedules` | `bool` | `GenerateSchedulesParameters` | gates pipeline step 6 |
| `generate_permanent_schedules` | `bool` | `GenerateSchedulesParameters` | gates step 7 |
| `generate_regular_schedules` | `bool` | `GenerateSchedulesParameters` | gates step 8 |
| `generate_variable_schedules` | `bool` | `GenerateSchedulesParameters` | gates step 9 |
| `rotate_days_off` | `bool` | `GenerateSchedulesParameters` | gates step 2 (`isRotateDaysOff()` in `PLAN_SCHEDULER.md`'s pipeline sketch) |

All seven `bool`/id fields default `false`/`0` in Java (primitive defaults, no explicit
initialization) — a Rust `Default` impl mirrors that directly.

## 7. Open dependencies for later phases (not blocking Phase 0)

Types referenced by `engine/model/` but owned by other packages, listed so later waves know what
they're unblocking:

- `process/variable/filters::EmployeeFilter` — unblocks `JobScheduleLog`/`ScheduleLog` logging
  types.
- `process/variable/comparators::{NonPreScheduledJobComparator,PreScheduledJobComparator}` —
  unblocks `JobList.get_non_pre_scheduled_jobs`/`get_pre_scheduled_jobs` sort order.
- `process/regularschedules::EmployeeRegularPeriodComparator` and
  `process/checkers::{EmployeeJobStatusChecker,EmployeeCertificationsChecker}` — unblock
  `RegularSchedules.get_regular_schedules_for_job_and_date`.
- `autosched::AvailPeriod` — unblocks `EmployeeData.get_effective_available_hours_for_date`.
- `watson/server/labor/calcshift::ScheduleCalcDataSet` — unblocks most of `EmployeeData`'s shift/
  time-off/overtime accessors; large enough to warrant its own stub-then-grow treatment like the
  Hibernate entities in §3.
- `com.unifocus.tbx.core.progress.Progress` — unblocks `ScheduleModel.progress`; likely a
  callback/trait rather than a data struct.

## 8. Status

Nothing in this catalog is implemented in Rust yet. This document is written ahead of code per
`workrules-port-read-audit-first`'s methodology (`DATA_MODEL.md` before any struct exists). Phase 0
scaffolding (mod tree, `Cargo.toml` deps, `PARITY_AUDIT.md`) follows this doc; Phase 1 (checkers/
comparators/sorters) is explicitly deferred pending review.
