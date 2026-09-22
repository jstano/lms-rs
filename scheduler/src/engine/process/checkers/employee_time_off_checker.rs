//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeTimeOffChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeTimeOffChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeTimeOffChecker`.
pub struct EmployeeTimeOffChecker;

impl CanWorkChecker for EmployeeTimeOffChecker {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let date_time_range_to_check = employee_shift.to_date_time_range();

        for time_off_request in employee_data.data_set().time_off_requests() {
            if date_time_range_to_check.overlaps(&time_off_request.to_date_time_range()) {
                set_notes_for_employee(
                    schedule_model,
                    employee_data.employee().id(),
                    "Planned shift overlaps with approved time off request",
                );

                return false;
            }
        }

        true
    }
}
