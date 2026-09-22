//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! WeeklyRotationPlanCheckerContext`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/WeeklyRotationPlanCheckerContext.java`.

use crate::engine::model::schedules::Schedules;
use crate::entity::employee_shift::EmployeeShift;
use joda_rs::LocalDate;

/// `WeeklyRotationPlanCheckerContext`.
pub trait WeeklyRotationPlanCheckerContext {
    /// `hasMatchingShiftOnDate(Schedules, EmployeeShift, LocalDate)`.
    fn has_matching_shift_on_date(
        &self,
        schedules: &Schedules,
        employee_shift: &EmployeeShift,
        date: LocalDate,
    ) -> bool;
}
