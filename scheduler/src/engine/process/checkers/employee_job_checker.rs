//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.EmployeeJobChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeJobChecker.java`. Not a `CanWorkChecker` — a different shape (checks a `PlannedShift`,
//! not an `EmployeeShift`, and isn't invoked through the checker dispatch the others are).

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::planned_shift::PlannedShift;

/// `EmployeeJobChecker`.
pub struct EmployeeJobChecker;

impl EmployeeJobChecker {
    /// `employeeHasJob(EmployeeData, PlannedShift, int)`. `job_id` needs to be threaded in
    /// alongside `planned_shift` since `PlannedShift` (unlike `EmployeeShift`) doesn't carry a
    /// job back-reference in this crate's model (`engine::model::job_data::JobData` owns the
    /// shift instead — see `DATA_MODEL.md` §4) — `job_level < 0` selects
    /// `hasJobAtAnyLevel`.
    pub fn employee_has_job(
        employee_data: &EmployeeData,
        job_id: i32,
        planned_shift: PlannedShift,
        job_level: i32,
    ) -> bool {
        if job_level < 0 {
            employee_data.has_job_at_any_level(job_id, planned_shift.shift_date())
        } else {
            employee_data.has_job_at_level(job_id, job_level, planned_shift.shift_date())
        }
    }
}
