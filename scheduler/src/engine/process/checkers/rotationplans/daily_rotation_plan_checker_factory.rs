//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! DailyRotationPlanCheckerFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/DailyRotationPlanCheckerFactory.java`.

use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_context::DailyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_process::DailyRotationPlanCheckerProcess;

/// `DailyRotationPlanCheckerFactory`.
pub struct DailyRotationPlanCheckerFactory;

impl DailyRotationPlanCheckerFactory {
    /// `getInstance(DailyRotationPlanCheckerContext)`.
    pub fn instance<'a>(
        &self,
        context: &'a dyn DailyRotationPlanCheckerContext,
    ) -> DailyRotationPlanCheckerProcess<'a> {
        DailyRotationPlanCheckerProcess::new(context)
    }
}
