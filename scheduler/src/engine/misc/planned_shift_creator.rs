//! Port of `com.unifocus.watson.server.scheduler.engine.misc.PlannedShiftCreator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! PlannedShiftCreator.java`. `createPlannedShift(PreSchedule)` (Phase 2 step 6) and
//! `createPlannedShift(RegularSchedule, LocalDate)` (Phase 2 steps 7-8) are ported.
//! `createPlannedShift(EmployeeShift)` is `VariableScheduleProcess`'s dependency (step 9), not
//! this one, and lands with that wave per `PLAN_SCHEDULER.md`'s "one wave per step" structure
//! (same deferral shape as finding 20).
//!
//! `getWorkableAssignment` reuses `EmployeeAssignmentChecker::can_employee_work_assignment`
//! (already ported in Phase 1) rather than duplicating its null/active check.
//!
//! `createPlannedShift(RegularSchedule, LocalDate)` takes `job_id` explicitly rather than
//! re-deriving `regularSchedule.getJob(shiftDate)` the way Java does — the caller
//! (`RegularScheduleSingleDate`) already resolved and validated that same id against this exact
//! `regular_schedule`/`shift_date` pair before reaching this call (see that file's doc), so
//! threading it through avoids reintroducing an `Option`/panic risk for a value that's already
//! known. `Property.default_shift_category` is still unmodeled (see
//! `engine::misc::employee_shift_creator`'s doc) — this overload leaves `shift_category_id` unset
//! rather than duplicate that gap undocumented; nothing downstream reads it back yet either
//! (`RegularScheduleEmployeeShiftCreator` always passes `None` explicitly to `EmployeeShiftCreator`
//! regardless of what the created `PlannedShift` itself carries).

use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::process::checkers::employee_assignment_checker::EmployeeAssignmentChecker;
use crate::entity::employee::Employee;
use crate::entity::planned_shift::PlannedShift;
use crate::entity::pre_schedule::PreSchedule;
use joda_rs::LocalDate;

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

    /// `createPlannedShift(RegularSchedule, LocalDate)` — see module doc for the `job_id`
    /// parameter's divergence from Java's literal `regularSchedule.getJob(shiftDate)` re-derive.
    pub fn create_planned_shift_from_regular_schedule(
        &self,
        regular_schedule: &RegularSchedule,
        employee: &Employee,
        job_id: i32,
        shift_date: LocalDate,
    ) -> PlannedShift {
        let date_time_range = regular_schedule.date_time_range(shift_date);
        let assignment_id = Self::workable_assignment(employee, regular_schedule.assignment_id());

        PlannedShift::new(
            0,
            job_id,
            shift_date,
            date_time_range.start(),
            date_time_range.duration().fractional_hours(),
            None,
        )
        .with_end_date_time(date_time_range.end())
        .with_assignment_id(assignment_id)
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
