//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! RotationPlanUtils`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/RotationPlanUtils.java`. Java's method is package-private (no modifier); ported
//! as `pub(super)` — used only within `rotationplans`, same visibility shape.

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::employee_type::EmployeeType;
use crate::entity::rotation_plan::RotationPlan;

/// `isRotationPlanApplicable(EmployeeData, RotationPlan)`.
pub(super) fn is_rotation_plan_applicable(
    employee_data: &EmployeeData,
    rotation_plan: RotationPlan,
) -> bool {
    let employee_type = employee_data.employee().employee_type();

    if employee_type == EmployeeType::Regular && !rotation_plan.apply_to_regular_employees() {
        return false;
    }

    if employee_type == EmployeeType::Permanent && !rotation_plan.apply_to_permanent_employees() {
        return false;
    }

    true
}
