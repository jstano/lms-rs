//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! DailyRotationPlanCheckerContext`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/DailyRotationPlanCheckerContext.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `DailyRotationPlanCheckerContext`.
pub trait DailyRotationPlanCheckerContext {
    /// `hasMatchingShiftOnDate(Schedules, EmployeeShift, LocalDate)`.
    fn has_matching_shift_on_date(
        &self,
        schedules: &Schedules,
        employee_shift: &EmployeeShift,
        date: LocalDate,
    ) -> bool;

    /// `getRotationInterval(EmployeeData, RotationPlan, LocalDate)`.
    fn rotation_interval(
        &self,
        employee_data: &EmployeeData,
        rotation_plan: RotationPlan,
        date: LocalDate,
    ) -> i32;
}
