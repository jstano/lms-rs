//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeMinDaysOffChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeMinDaysOffChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::ports::AssignmentPort;
use crate::entity::employee_shift::EmployeeShift;

const DAYS_PER_WEEK: i32 = 7;

/// `EmployeeMinDaysOffChecker`.
pub struct EmployeeMinDaysOffChecker<'a> {
    assignments: &'a dyn AssignmentPort,
}

impl<'a> EmployeeMinDaysOffChecker<'a> {
    pub fn new(assignments: &'a dyn AssignmentPort) -> Self {
        Self { assignments }
    }

    fn min_days_off(
        &self,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> i32 {
        let assignment_id = employee_shift
            .assignment_id()
            .unwrap_or(employee_shift.job_id());
        let min_days_off = employee_data.min_days_off(assignment_id, self.assignments);

        min_days_off.min(3)
    }

    fn are_consecutive_days_off_in_bounds(
        &self,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
        min_days_off: i32,
    ) -> bool {
        let shifts: Vec<EmployeeShift> = employee_data
            .data_set()
            .shifts()
            .iter()
            .filter(|s| *s != employee_shift)
            .cloned()
            .collect();
        let schedules = Schedules::new(
            shifts,
            employee_data.data_set().time_off_requests().to_vec(),
        );

        let shift_date = employee_shift.shift_date();

        let consecutive_days_before_date =
            schedules.determine_consecutive_days_prior_dates(shift_date, min_days_off);
        let consecutive_days_after_date =
            schedules.determine_consecutive_days_future_dates(shift_date, min_days_off);

        consecutive_days_before_date + consecutive_days_after_date + min_days_off < DAYS_PER_WEEK
    }
}

impl CanWorkChecker for EmployeeMinDaysOffChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let min_days_off = self.min_days_off(employee_data, employee_shift);

        if min_days_off <= 0 {
            return true;
        }

        let can_work =
            self.are_consecutive_days_off_in_bounds(employee_data, employee_shift, min_days_off);

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Planned shift conflicts with the minimum days off rules",
            );
        }

        can_work
    }
}
