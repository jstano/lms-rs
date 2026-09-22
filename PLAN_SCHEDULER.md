# Plan: Scope and sequence the `scheduler` crate port

## Context

`lms-rs` ports the UniFocus Java monorepo (`taps/`) crate by crate, following conventions
established by `planner` (the workcontent generator) and `workrules` (the labor rule engine).
`scheduler` is the next crate — currently an untouched `cargo new` stub (`scheduler/src/lib.rs`
still has the default `add()` function) — meant to port "the Java scheduler engine": the
auto-scheduling algorithm that assigns employees to the `PlannedShift`s that `planner` generates.

**The scoping question had a real trap**, the same one `workrules` hit: `taps/scheduler/` (152
files under `employeescheduler-domain` alone) *looks* like the target because of the name, but two
Explore passes confirmed it's a thin hexagonal CRUD/API layer — DTOs, `*QueryAdapter`/
`*CommandAdapter` port interfaces, security checks, and delegation. It contains zero scheduling
computation. Tracing its `generate schedules` call chain
(`EmployeeSchedulerGenerateSchedulesServiceImpl` → `ShiftCommandAdapter` →
`ShiftCommandAdapterImpl` → legacy `com.unifocus.rms.scheduling.services_impl
.GenerateSchedulesServiceImpl` → `TaskManagerContext` → async Task Manager) leads to the real
engine: **`taps/src/java/com/unifocus/watson/server/scheduler/{engine,autosched}`**, confirmed by
reading `ScheduleEngine.java` directly — its `generateSchedules()` method is the literal top-level
orchestrator, and `ScheduleEngineRunnerWithRetries`/`GenerateNewSchedulesService` (under
`watson/server/scheduler/service/generate`) is what the async Task Manager invokes to run it.

This is the same shape as the `workrules`/`work-rules` collision: a Gradle-module-per-bounded-
context name doesn't point at the algorithm; the algorithm hides under the legacy `watson`
package tree. Worth recording in memory once the port starts, alongside the existing
`workrules-crate-targets-rule-engine` note.

**Decisions already made with the user:**
- Scope is the **engine only** — `watson/server/scheduler/engine` (139 files, ~6,950 LOC) +
  `watson/server/scheduler/autosched` (6 files, ~1,183 LOC), ~145 files / ~8,100 LOC of Java. The
  `employeescheduler` CRUD/API layer and the legacy `rms.scheduling`/Task Manager plumbing are out
  of scope — the Rust `scheduler` crate models the pure algorithm, given a loaded model, not the
  REST/persistence/queueing around it.
- `scheduler` will be **self-contained**, like `workrules` — its own `ScheduleModel`/`JobId`/
  `PropertyId`/`PlannedShift`/etc. newtypes, no dependency on the `planner` crate. This matches the
  existing precedent (`workrules` and `planner` have zero cross-crate deps today) and avoids
  entangling two independently-evolving ports. `GenerateSchedulesParameters` (the engine's input,
  currently in `watson/common/labor/planner`, 71 lines) gets its own Rust type in `scheduler`,
  not shared with `planner`.

## The engine's shape

`ScheduleEngine.generateSchedules()` is a fixed 10-step pipeline over a single `ScheduleModel`:

1. `ScheduleModelLoader.load(...)` — builds `ScheduleModel` (io/)
2. `DayOffPlanRotator.rotateDayOffPlans(...)` — conditional on `isRotateDaysOff()` (misc/)
3. `ProjectedHoursReducerFactory` — per-job hour reduction, weekly mode only (process/projectedhoursreducers/, 5 files)
4. `EmployeeAvailableHoursBalancer.computeBalanceFactor(...)` — weekly mode, `isBalanceSchedules()` jobs only (process/schedulebalancers/, 1 file)
5. `SchedulePreparationService.prepareShiftsForScheduling(...)` (io/)
6. `PreScheduleProcess.preScheduleEmployees(...)` — conditional (process/, top-level)
7. `PermanentScheduleProcess.schedulePermanentEmployees(...)` — conditional (process/, top-level)
8. `RegularScheduleProcess.scheduleRegularEmployees(...)` — conditional (process/regularschedules/, 7 files)
9. `VariableScheduleProcess.scheduleVariableEmployees(...)` — conditional (process/variable/, 23 files: generators, filters, comparators, schedulers)
10. `SaveSchedulesService.saveSchedules(...)` (io/)

Shared machinery used across steps 6–9:
- `process/checkers/` (32 files) — `CanWorkChecker`/`AbstractCanWorkChecker` and per-rule
  eligibility checks (overtime, availability, time off, job/assignment rotation, certifications,
  min days off, schedule restrictions).
- `process/checkers/rotationplans/` (13 files) — daily/weekly job & assignment rotation-plan
  checking, its own factory/context/process sub-hierarchy.
- `process/comparators/` (13 files) — seniority/hire-date/assignment-rank ordering.
- `process/plannedshiftsorters/` (8 files) — shift ranking/ordering for assignment.
- `engine/model/` (12 files) — `ScheduleModel`, `JobData`, `JobList`, `EmployeeData`,
  `EmployeeList`, `ShiftList`, `RegularSchedule(s)`, `WeeklyAvailableHours`, `HoursByDate`,
  `PreScheduleParameters` (+ `model/logging`).
- `engine/misc/` (10 files) — `PlannedShiftCreator`/`Matcher`/`Helper`,
  `EmployeeShiftCreator`/`Cloner`, `DayOffPlanRotator`, `CalculateDataSet`,
  `EmployeeDataServices`, `ScheduleSaver`, `ProjectedHoursChecker`.
- `engine/io/` (16 files) — all the loaders (`ScheduleModelLoader`, `EmployeeListLoader`,
  `JobListLoader`, `RegularScheduleLoader`, `PlannedShiftLoader`, `ForecastPlannedShiftLoader`,
  `PreScheduleLoader`/`PreScheduleJobLoader`, `OriginalProjectedHoursLoader`) and the
  save/log/snapshot services.
- `autosched/` (6 files) — `ScheduleMatcher`, `ScheduleChecker`,
  `ScheduleHoursDistributionValidator`, `ExceedsAvailableHoursConflictValidator` — matching/
  validation logic layered on top of the engine, used by the auto-scheduling path.

## Recommended approach

Mirror the `workrules` porting methodology (see `workrules-port-read-audit-first` memory):
running `scheduler/src/PARITY_AUDIT.md` (status, divergences, findings, assertion count,
"where the work stands") plus a per-crate `DATA_MODEL.md` written up front (like `planner`'s),
both living next to the code — no code before the data model doc exists in enough detail to
build against. Ground truth is always `taps/`, never any local convenience copy.

### Phase 0 — Data model & scaffolding (do first, single wave)
Write `scheduler/src/DATA_MODEL.md`: catalog `ScheduleModel` and its constituent
`model/` types (§ above), decide the `Job`/`Employee`/`PlannedShift`/id-type shapes (uuid via
`id_type!` macro, matching `planner`'s convention, even though the crate itself is
self-contained), and the `GenerateSchedulesParameters` struct. Stand up `mod.rs` layout mirroring
`engine/{model,io,misc,process/...}` → `src/{model,io,misc,process/...}`. Add `joda_rs`,
`date_range_rs`, `uuid` to `scheduler/Cargo.toml` (matching `planner`'s deps — the engine is
date/time-heavy: rotation plans, availability windows, weekly balancing).

### Phase 1 — Shared machinery (checkers, comparators, sorters)
Port `process/checkers/` (32 files, including `rotationplans/`), `process/comparators/` (13),
`process/plannedshiftsorters/` (8) before any pipeline step, since steps 6–9 all depend on them.
This is the largest single wave (~53 files) but has no forward dependency on the pipeline steps
themselves — same "port the leaf machinery before the orchestrator" order `workrules` used per
rule family.

### Phase 2 — Pipeline steps, in `ScheduleEngine` order
One wave per step, each wave = the step's own class(es) + its private subpackage:
1. `io/` loaders + `ScheduleModelLoader` (step 1)
2. `misc::DayOffPlanRotator` (step 2)
3. `process/projectedhoursreducers/` (step 3, 5 files)
4. `process/schedulebalancers/` (step 4, 1 file)
5. `io::SchedulePreparationService` (step 5)
6. `PreScheduleProcess` (step 6)
7. `PermanentScheduleProcess` (step 7)
8. `process/regularschedules/` + `RegularScheduleProcess` (step 8, 7 files)
9. `process/variable/` + `VariableScheduleProcess` (step 9, 23 files — largest pipeline wave)
10. `io::SaveSchedulesService` (step 10)

### Phase 3 — Orchestrator + autosched
Port `ScheduleEngine` itself (the 10-step pipeline, straight translation once every step exists),
then `autosched/` (6 files) as a final wave — it's validation/matching layered on top, not a
pipeline dependency.

### Ongoing
- Java parity tests transcribed into `#[cfg(test)] mod java_parity_tests` with provenance comments
  (per `lms-rs-java-port-conventions`), `rstest` for tabular cases.
- Watch for `TDouble`-style rounding traps in any hours math (weekly balancing, hour reduction,
  availability) — resolve each call site to the specific Java rounding overload rather than
  reusing a single Rust helper; see `tdouble-rounding-parity-trap` memory.
- Update `scheduler/src/PARITY_AUDIT.md` continuously (divergences, hard-won findings, assertion
  count, "where the work stands"), not retroactively.

## Verification

Since this is a pure-algorithm crate with no I/O boundary of its own (loaders/save services will
initially be stubs/traits pending real persistence), verification per wave is:
- `cargo test -p scheduler` passing, including transcribed `java_parity_tests`.
- `cargo build --workspace` staying green (crate is additive, no shared-type coupling to break).
- Periodic re-read of `ScheduleEngine.java` and the current wave's Java source under `taps/` to
  confirm behavior wasn't drifted from during translation (ground truth is always `taps/`).
