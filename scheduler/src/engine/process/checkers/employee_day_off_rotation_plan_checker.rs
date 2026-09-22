//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeDayOffRotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeDayOffRotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::day_off_pattern::DayOffPattern;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeDayOffRotationPlanChecker`.
pub struct EmployeeDayOffRotationPlanChecker;

impl EmployeeDayOffRotationPlanChecker {
    fn day_off_pattern(employee_data: &EmployeeData) -> Option<&DayOffPattern> {
        let day_off_plan = employee_data.employee().day_off_plan()?;
        day_off_plan.day_off_pattern(employee_data.employee().current_pattern_no())
    }
}

impl CanWorkChecker for EmployeeDayOffRotationPlanChecker {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let Some(day_off_pattern) = Self::day_off_pattern(employee_data) else {
            return true;
        };

        let can_work =
            day_off_pattern.can_work_on_day_of_week(employee_shift.shift_date().day_of_week());

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Planned shift conflicts with the day off rotation plan rules",
            );
        }

        can_work
    }
}
