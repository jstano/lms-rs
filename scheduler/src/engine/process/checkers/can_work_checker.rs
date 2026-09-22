//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.CanWorkChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! CanWorkChecker.java`. `schedule_model`/`employee_data` are `&mut` here — Java's references are
//! implicitly mutable, and two real call sites need that: a failed check writes a note into the
//! model's current `PlannedShiftLog` (`AbstractCanWorkChecker::set_notes_for_employee`), and
//! `EmployeeWeeklyAvailableHoursChecker` mutates the employee's memoized `WeeklyAvailableHours`
//! cache (`EmployeeData::weekly_available_hours`).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee_shift::EmployeeShift;

/// `CanWorkChecker`.
pub trait CanWorkChecker {
    /// `canEmployeeWorkShift(ScheduleModel, EmployeeData, EmployeeShift)`.
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool;
}
