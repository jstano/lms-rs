//! Resolver traits for the one-way entity graph — see `entity` module docs.
//!
//! Ground truth: nothing in `taps` corresponds to this file directly; it exists because
//! `Assignment` only carries `parent_assignment_id`, not a live parent reference, so any Java
//! call site that walks `assignment.getParentAssignment()` an unbounded number of times (rather
//! than the one level `JobRankComparator`/`JobSeniorityDateComparator` needed, which compares ids
//! directly) needs a way to fetch the next `Assignment` up the chain. Same shape as `workrules`'s
//! `rules::ports::AssignmentPort`, ported independently (no cross-crate dependency — see
//! `PARITY_AUDIT.md` finding 8).
//!
//! First callers: `EmployeeData::min_hours_off`/`min_days_off` (walks the assignment chain
//! looking for the first non-null override) and
//! `EmployeeAssignmentRotationChecker::rotation_plan` (walks looking for the first assignment
//! with a `RotationPlan` set).
//!
//! `OvertimeForDateRangePort` lives here too, despite not being about the `Assignment` parent
//! chain — it started out defined directly inside `EmployeeOvertimeChecker`'s own file (its only
//! caller at the time), but `EmployeeData::store_pre_schedule_check_overtime`/
//! `CalculateDataSet` (Phase 2 step 6) need the same port, so it moved to this shared module
//! rather than staying checker-local. See `PARITY_AUDIT.md` finding 16 for its origin.

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::assignment::Assignment;
use date_range_rs::DateRange;

/// Jobs / labor-structure nodes by id. `AssignmentDAO`, narrowed to the one method this crate
/// needs.
pub trait AssignmentPort {
    /// `AssignmentDAO.findByID(int)`.
    fn find_by_id(&self, id: i32) -> Option<Assignment>;
}

/// `ScheduleCalcDataSet.getOvertimeForDateRange(DateRange)` — a real overtime calculation
/// `ScheduleCalcDataSet` doesn't ground yet (see that type's doc / `PARITY_AUDIT.md` finding 10's
/// pattern).
pub trait OvertimeForDateRangePort {
    fn overtime_for_date_range(
        &self,
        employee_data: &mut EmployeeData,
        date_range: &DateRange,
    ) -> f64;
}
