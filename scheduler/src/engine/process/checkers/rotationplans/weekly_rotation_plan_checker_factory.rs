//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! WeeklyRotationPlanCheckerFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/WeeklyRotationPlanCheckerFactory.java`.

use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_context::WeeklyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_process::WeeklyRotationPlanCheckerProcess;

/// `WeeklyRotationPlanCheckerFactory`.
pub struct WeeklyRotationPlanCheckerFactory;

impl WeeklyRotationPlanCheckerFactory {
    /// `getInstance(WeeklyRotationPlanCheckerContext)`.
    pub fn instance<'a>(
        &self,
        context: &'a dyn WeeklyRotationPlanCheckerContext,
    ) -> WeeklyRotationPlanCheckerProcess<'a> {
        WeeklyRotationPlanCheckerProcess::new(context)
    }
}
