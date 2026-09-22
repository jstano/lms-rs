//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeAssignmentChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeAssignmentChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeAssignmentChecker`.
pub struct EmployeeAssignmentChecker;

impl CanWorkChecker for EmployeeAssignmentChecker {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let can_work = Self::can_employee_work_assignment(
            employee_data.employee(),
            employee_shift.assignment_id(),
        );

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Employee can't work the assignment for the planned shift",
            );
        }

        can_work
    }
}

impl EmployeeAssignmentChecker {
    /// `canEmployeeWorkAssignment(Employee, Assignment)`.
    pub fn can_employee_work_assignment(employee: &Employee, assignment_id: Option<i32>) -> bool {
        let Some(assignment_id) = assignment_id else {
            return true;
        };

        employee
            .assignment(assignment_id)
            .is_some_and(|ea| ea.is_active())
    }
}
