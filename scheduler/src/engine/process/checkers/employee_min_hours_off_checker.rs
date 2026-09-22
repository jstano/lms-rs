//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeMinHoursOffChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeMinHoursOffChecker.java`.

use crate::common::datetime::duration_in_fractional_hours;
use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::ports::AssignmentPort;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeMinHoursOffChecker`.
pub struct EmployeeMinHoursOffChecker<'a> {
    assignments: &'a dyn AssignmentPort,
}

impl<'a> EmployeeMinHoursOffChecker<'a> {
    pub fn new(assignments: &'a dyn AssignmentPort) -> Self {
        Self { assignments }
    }
}

impl CanWorkChecker for EmployeeMinHoursOffChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        schedule_shift: &EmployeeShift,
    ) -> bool {
        let assignment_id = schedule_shift
            .assignment_id()
            .unwrap_or(schedule_shift.job_id());
        let min_hours_off = employee_data.min_hours_off(assignment_id, self.assignments);

        let schedule_shift_range = schedule_shift.to_date_time_range();
        let schedule_shift_start = schedule_shift_range.start();
        let schedule_shift_end = schedule_shift_range.end();

        for employee_shift in employee_data.data_set().shifts() {
            let employee_shift_range = employee_shift.to_date_time_range();
            let employee_shift_start = employee_shift_range.start();
            let employee_shift_end = employee_shift_range.end();

            let hours_between = if employee_shift_end.is_on_or_before(schedule_shift_start) {
                duration_in_fractional_hours(employee_shift_end, schedule_shift_start)
            } else if employee_shift_start.is_on_or_after(schedule_shift_end) {
                duration_in_fractional_hours(schedule_shift_end, employee_shift_start)
            } else {
                f64::MAX
            };

            if hours_between < min_hours_off {
                set_notes_for_employee(
                    schedule_model,
                    employee_data.employee().id(),
                    "Planned shift conflicts with minimum hours off rules",
                );

                return false;
            }
        }

        true
    }
}
