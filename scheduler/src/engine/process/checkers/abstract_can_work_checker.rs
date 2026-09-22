//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! AbstractCanWorkChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! AbstractCanWorkChecker.java`. Java is an abstract base class with one protected helper; ported
//! as a free function every concrete `CanWorkChecker` impl calls, the same treatment
//! `AbstractByDayPlannedShiftSorter` got in `process/plannedshiftsorters/`.

use crate::engine::model::schedule_model::ScheduleModel;

/// `setNotesForEmployee(ScheduleModel, EmployeeData, String)`.
pub fn set_notes_for_employee(
    schedule_model: &mut ScheduleModel,
    employee_id: i32,
    notes: impl Into<String>,
) {
    if let Some(planned_shift_log) = schedule_model.current_planned_shift_log_mut() {
        planned_shift_log
            .employees_with_conflicts_mut()
            .set_notes_for_employee(employee_id, notes);
    }
}
