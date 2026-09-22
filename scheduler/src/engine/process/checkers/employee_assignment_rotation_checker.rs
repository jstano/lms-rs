//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeAssignmentRotationChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeAssignmentRotationChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker_factory::RotationPlanCheckerFactory;
use crate::engine::process::ports::AssignmentPort;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;

/// `EmployeeAssignmentRotationChecker`.
pub struct EmployeeAssignmentRotationChecker<'a> {
    rotation_plan_checker_factory: RotationPlanCheckerFactory,
    assignments: &'a dyn AssignmentPort,
}

impl<'a> EmployeeAssignmentRotationChecker<'a> {
    pub fn new(assignments: &'a dyn AssignmentPort) -> Self {
        Self {
            rotation_plan_checker_factory: RotationPlanCheckerFactory::new(),
            assignments,
        }
    }

    /// Walks `assignment_id` up its parent chain for the first `RotationPlan` set anywhere,
    /// `getRotationPlan()` — not `getJobRotationPlan()`, see `EmployeeJobRotationChecker`.
    fn rotation_plan(&self, assignment_id: Option<i32>) -> Option<RotationPlan> {
        let mut current_id = assignment_id;

        while let Some(id) = current_id {
            let assignment = self.assignments.find_by_id(id)?;
            if let Some(rotation_plan) = assignment.rotation_plan() {
                return Some(rotation_plan);
            }
            current_id = assignment.parent_assignment_id();
        }

        None
    }
}

impl CanWorkChecker for EmployeeAssignmentRotationChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let Some(rotation_plan) = self.rotation_plan(employee_shift.assignment_id()) else {
            return true;
        };

        let can_work = self
            .rotation_plan_checker_factory
            .assignment_rotation_plan_checker(rotation_plan)
            .can_employee_work_shift(employee_data, employee_shift, rotation_plan);

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Planned shift conflicts with the assignment rotation plan rules",
            );
        }

        can_work
    }
}
