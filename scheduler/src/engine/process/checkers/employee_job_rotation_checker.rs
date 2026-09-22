//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeJobRotationChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeJobRotationChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker_factory::RotationPlanCheckerFactory;
use crate::engine::process::ports::AssignmentPort;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeJobRotationChecker`.
pub struct EmployeeJobRotationChecker<'a> {
    rotation_plan_checker_factory: RotationPlanCheckerFactory,
    assignments: &'a dyn AssignmentPort,
}

impl<'a> EmployeeJobRotationChecker<'a> {
    pub fn new(assignments: &'a dyn AssignmentPort) -> Self {
        Self {
            rotation_plan_checker_factory: RotationPlanCheckerFactory::new(),
            assignments,
        }
    }
}

impl CanWorkChecker for EmployeeJobRotationChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        // `employeeShift.getJob().getJobRotationPlan()` — the job's own field, no chain walk.
        let Some(rotation_plan) = self
            .assignments
            .find_by_id(employee_shift.job_id())
            .and_then(|job| job.job_rotation_plan())
        else {
            return true;
        };

        let can_work = self
            .rotation_plan_checker_factory
            .job_rotation_plan_checker(rotation_plan)
            .can_employee_work_shift(employee_data, employee_shift, rotation_plan);

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Planned shift conflicts with the job rotation plan rules",
            );
        }

        can_work
    }
}
