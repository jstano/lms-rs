//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! WeeklyAssignmentRotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/WeeklyAssignmentRotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_context::WeeklyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_factory::WeeklyRotationPlanCheckerFactory;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `WeeklyAssignmentRotationPlanChecker`.
pub struct WeeklyAssignmentRotationPlanChecker {
    factory: WeeklyRotationPlanCheckerFactory,
}

impl WeeklyAssignmentRotationPlanChecker {
    pub fn new() -> Self {
        Self {
            factory: WeeklyRotationPlanCheckerFactory,
        }
    }
}

impl Default for WeeklyAssignmentRotationPlanChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationPlanChecker for WeeklyAssignmentRotationPlanChecker {
    fn can_employee_work_shift(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
        rotation_plan: RotationPlan,
    ) -> bool {
        self.factory.instance(self).can_employee_work_shift(
            employee_data,
            employee_shift,
            rotation_plan,
        )
    }
}

impl WeeklyRotationPlanCheckerContext for WeeklyAssignmentRotationPlanChecker {
    /// Same null-shortcut divergence as `DailyAssignmentRotationPlanChecker` — see its doc.
    fn has_matching_shift_on_date(
        &self,
        schedules: &Schedules,
        employee_shift: &EmployeeShift,
        date: LocalDate,
    ) -> bool {
        match employee_shift.assignment_id() {
            Some(assignment_id) => schedules.has_shift_with_assignment_on_date(assignment_id, date),
            None => false,
        }
    }
}
