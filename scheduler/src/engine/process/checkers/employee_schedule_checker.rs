//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeScheduleChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeScheduleChecker.java`. Java's `scheduleShift != employeeShift` is reference identity;
//! `EmployeeShift` here has no identity beyond its field values, so this compares by value
//! (`PartialEq`) instead — two genuinely distinct shifts with every field equal (same id, same
//! times, same job) would be indistinguishable, which shouldn't occur for real data but is worth
//! flagging as a divergence from Java's stricter identity check.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeScheduleChecker`.
pub struct EmployeeScheduleChecker;

impl CanWorkChecker for EmployeeScheduleChecker {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        schedule_shift: &EmployeeShift,
    ) -> bool {
        let date_time_range_to_check = schedule_shift.to_date_time_range();

        for employee_shift in employee_data.data_set().shifts() {
            if schedule_shift != employee_shift
                && date_time_range_to_check.overlaps_exclusive(&employee_shift.to_date_time_range())
            {
                set_notes_for_employee(
                    schedule_model,
                    employee_data.employee().id(),
                    "Planned shift overlaps with existing schedules",
                );

                return false;
            }
        }

        true
    }
}
