//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! DailyAssignmentRotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/DailyAssignmentRotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_context::DailyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_factory::DailyRotationPlanCheckerFactory;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `DailyAssignmentRotationPlanChecker`.
pub struct DailyAssignmentRotationPlanChecker {
    factory: DailyRotationPlanCheckerFactory,
}

impl DailyAssignmentRotationPlanChecker {
    pub fn new() -> Self {
        Self {
            factory: DailyRotationPlanCheckerFactory,
        }
    }
}

impl Default for DailyAssignmentRotationPlanChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationPlanChecker for DailyAssignmentRotationPlanChecker {
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

impl DailyRotationPlanCheckerContext for DailyAssignmentRotationPlanChecker {
    /// Java passes `employeeShift.getAssignment()` (possibly `null`) straight into
    /// `Schedules.hasShiftWithAssignmentOnDate`, which NPEs on `assignment.getID()` the moment it
    /// finds a shift with a non-null assignment to compare against a null one. A shift with no
    /// assignment can't "match" any assignment, so this short-circuits to `false` instead of
    /// reproducing the crash.
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

    fn rotation_interval(
        &self,
        employee_data: &EmployeeData,
        rotation_plan: RotationPlan,
        _date: LocalDate,
    ) -> i32 {
        rotation_plan
            .rotate_interval()
            .min(employee_data.employee().assignments().len() as i32)
    }
}
