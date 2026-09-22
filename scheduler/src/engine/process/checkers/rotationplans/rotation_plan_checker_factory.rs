//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! RotationPlanCheckerFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/RotationPlanCheckerFactory.java`.

use crate::engine::process::checkers::rotationplans::daily_assignment_rotation_plan_checker::DailyAssignmentRotationPlanChecker;
use crate::engine::process::checkers::rotationplans::daily_job_rotation_plan_checker::DailyJobRotationPlanChecker;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::engine::process::checkers::rotationplans::weekly_assignment_rotation_plan_checker::WeeklyAssignmentRotationPlanChecker;
use crate::engine::process::checkers::rotationplans::weekly_job_rotation_plan_checker::WeeklyJobRotationPlanChecker;
use crate::entity::rotation_plan::RotationPlan;

/// `RotationPlanCheckerFactory`.
#[derive(Default)]
pub struct RotationPlanCheckerFactory {
    daily_job_rotation_plan_checker: DailyJobRotationPlanChecker,
    weekly_job_rotation_plan_checker: WeeklyJobRotationPlanChecker,
    daily_assignment_rotation_plan_checker: DailyAssignmentRotationPlanChecker,
    weekly_assignment_rotation_plan_checker: WeeklyAssignmentRotationPlanChecker,
}

impl RotationPlanCheckerFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getJobRotationPlanChecker(RotationPlan)`.
    pub fn job_rotation_plan_checker(
        &self,
        rotation_plan: RotationPlan,
    ) -> &dyn RotationPlanChecker {
        if rotation_plan.is_rotate_every_day() {
            &self.daily_job_rotation_plan_checker
        } else {
            &self.weekly_job_rotation_plan_checker
        }
    }

    /// `getAssignmentRotationPlanChecker(RotationPlan)`.
    pub fn assignment_rotation_plan_checker(
        &self,
        rotation_plan: RotationPlan,
    ) -> &dyn RotationPlanChecker {
        if rotation_plan.is_rotate_every_day() {
            &self.daily_assignment_rotation_plan_checker
        } else {
            &self.weekly_assignment_rotation_plan_checker
        }
    }
}
