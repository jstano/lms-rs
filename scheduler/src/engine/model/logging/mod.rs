//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/`.
//! `JobScheduleLog`/`ScheduleLog` were blocked since Phase 1 (needed
//! `process/variable/filters::EmployeeFilter` as a plain value, `DATA_MODEL.md` §5/§7) — unblocked
//! by Phase 2 step 9's `EmployeeFilterKey`.

pub mod employee_log_entry;
pub mod employees_with_conflicts;
pub mod job_schedule_log;
pub mod planned_shift_log;
pub mod ranked_employees;
pub mod schedule_log;
