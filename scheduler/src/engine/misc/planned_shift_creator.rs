//! Port of `com.unifocus.watson.server.scheduler.engine.misc.PlannedShiftCreator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! PlannedShiftCreator.java`. Only `createPlannedShift(PreSchedule)` is ported this wave (Phase 2
//! step 6, `PreScheduleProcess`'s dependency) — `createPlannedShift(RegularSchedule, LocalDate)`
//! and `createPlannedShift(EmployeeShift)` are used by `RegularScheduleProcess`/
//! `VariableScheduleProcess` (steps 8-9), not this one, and land with those waves per
//! `PLAN_SCHEDULER.md`'s "one wave per step" structure (same deferral shape as finding 20).
//!
//! `getWorkableAssignment` reuses `EmployeeAssignmentChecker::can_employee_work_assignment`
//! (already ported in Phase 1) rather than duplicating its null/active check.

use crate::engine::process::checkers::employee_assignment_checker::EmployeeAssignmentChecker;
use crate::entity::employee::Employee;
use crate::entity::planned_shift::PlannedShift;
use crate::entity::pre_schedule::PreSchedule;

/// `PlannedShiftCreator`.
pub struct PlannedShiftCreator;

impl PlannedShiftCreator {
    /// `createPlannedShift(PreSchedule)`. `preSchedule.getEmployee()` is passed explicitly
    /// (rather than read off `pre_schedule` itself) since `PreSchedule` flattens its employee
    /// reference to `employee_id` — see that type's doc.
    pub fn create_planned_shift_from_pre_schedule(
        &self,
        pre_schedule: &PreSchedule,
        employee: &Employee,
    ) -> PlannedShift {
        let date_time_range = pre_schedule.to_date_time_range();
        let assignment_id = Self::workable_assignment(employee, pre_schedule.assignment_id());

        PlannedShift::new(
            0,
            pre_schedule.job_id(),
            pre_schedule.start_date_time().to_local_date(),
            date_time_range.start(),
            date_time_range.duration().fractional_hours(),
            None,
        )
        .with_end_date_time(date_time_range.end())
        .with_assignment_id(assignment_id)
        .with_shift_category_id(pre_schedule.shift_category_id())
    }

    /// `getWorkableAssignment(Employee, Assignment)`.
    fn workable_assignment(employee: &Employee, assignment_id: Option<i32>) -> Option<i32> {
        if EmployeeAssignmentChecker::can_employee_work_assignment(employee, assignment_id) {
            assignment_id
        } else {
            None
        }
    }
}
