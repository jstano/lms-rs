//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! RotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/RotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;

/// `RotationPlanChecker`.
pub trait RotationPlanChecker {
    /// `canEmployeeWorkShift(EmployeeData, EmployeeShift, RotationPlan)`.
    fn can_employee_work_shift(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
        rotation_plan: RotationPlan,
    ) -> bool;
}
