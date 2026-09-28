//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.ScheduleLog`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! ScheduleLog.java`. Blocked since Phase 1 (`PARITY_AUDIT.md` finding 3) on
//! `process/variable/filters::EmployeeFilter` existing as a plain value — now unblocked by
//! `EmployeeFilterKey` (Phase 2 step 9).
//!
//! `employee_filter` is stored as `Box<dyn EmployeeFilter>` — the trait object Java's field
//! literally is (`private final EmployeeFilter employeeFilter`) — not flattened to just the key,
//! since `getEmployeeFilter()` hands the real filter back to callers.

use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
use crate::engine::process::variable::filters::EmployeeFilter;

/// `ScheduleLog`.
pub struct ScheduleLog {
    employee_filter: Box<dyn EmployeeFilter>,
    planned_shift_logs: Vec<PlannedShiftLog>,
}

impl ScheduleLog {
    pub fn new(employee_filter: Box<dyn EmployeeFilter>) -> Self {
        Self {
            employee_filter,
            planned_shift_logs: Vec::new(),
        }
    }

    /// `getEmployeeFilter()`.
    pub fn employee_filter(&self) -> &dyn EmployeeFilter {
        self.employee_filter.as_ref()
    }

    /// `addPlannedShiftLog(PlannedShiftLog)`.
    pub fn add_planned_shift_log(&mut self, planned_shift_log: PlannedShiftLog) {
        self.planned_shift_logs.push(planned_shift_log);
    }

    /// `getCurrentPlannedShiftLog()`. Java throws `IllegalStateException` on an empty list
    /// (finding 7); every real call site adds a log for the current shift immediately before
    /// reading it back, so a panic here is equally faithful — not silently defaulting to `None`.
    pub fn current_planned_shift_log(&self) -> &PlannedShiftLog {
        self.planned_shift_logs
            .last()
            .expect("getCurrentPlannedShiftLog called with an empty plannedShiftLogs list")
    }

    /// `getCurrentPlannedShiftLog()` (mutable).
    pub fn current_planned_shift_log_mut(&mut self) -> &mut PlannedShiftLog {
        self.planned_shift_logs
            .last_mut()
            .expect("getCurrentPlannedShiftLog called with an empty plannedShiftLogs list")
    }

    /// `getPlannedShiftLogs()`.
    pub fn planned_shift_logs(&self) -> &[PlannedShiftLog] {
        &self.planned_shift_logs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::process::variable::filters::SalariedHomeJobOnlyEmployeeFilter;
    use crate::entity::planned_shift::PlannedShift;
    use joda_rs::{LocalDate, LocalDateTime};

    fn shift() -> PlannedShift {
        PlannedShift::new(
            1,
            500,
            LocalDate::of(2024, 1, 1),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            8.0,
            None,
        )
    }

    #[test]
    #[should_panic(expected = "empty plannedShiftLogs list")]
    fn current_planned_shift_log_panics_when_empty() {
        let log = ScheduleLog::new(Box::new(SalariedHomeJobOnlyEmployeeFilter));

        log.current_planned_shift_log();
    }

    #[test]
    fn current_planned_shift_log_returns_the_most_recently_added() {
        let mut log = ScheduleLog::new(Box::new(SalariedHomeJobOnlyEmployeeFilter));
        log.add_planned_shift_log(PlannedShiftLog::new(shift()));

        assert_eq!(log.current_planned_shift_log().planned_shift(), shift());
    }
}
