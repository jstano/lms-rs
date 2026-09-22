//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeWeeklyAvailableHoursChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeWeeklyAvailableHoursChecker.java`. Fully portable now — no deferred subsystem, unlike
//! its monthly sibling.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::schedule_mode::ScheduleMode;
use date_range_rs::DateRange;

/// `EmployeeWeeklyAvailableHoursChecker`.
pub struct EmployeeWeeklyAvailableHoursChecker<'a> {
    job_data: &'a JobData,
}

impl<'a> EmployeeWeeklyAvailableHoursChecker<'a> {
    pub fn new(job_data: &'a JobData) -> Self {
        Self { job_data }
    }

    fn scheduled_hours_for_week(
        employee_data: &EmployeeData,
        planned_shift_week: &DateRange,
    ) -> f64 {
        employee_data
            .data_set()
            .shifts_for_date_range(planned_shift_week)
            .iter()
            .map(EmployeeShift::net_hours)
            .sum()
    }
}

impl CanWorkChecker for EmployeeWeeklyAvailableHoursChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        if schedule_model.property().schedule_mode() != Some(ScheduleMode::Weekly) {
            return true;
        }

        let planned_shift_week = schedule_model.week_for_date(employee_shift.shift_date());

        let job = self.job_data.job().clone();
        let job_balance_factor = self.job_data.balance_factor();
        let job_balance_level = self.job_data.balance_level();
        let job_average_shift_length = self.job_data.average_shift_length();
        let time_off_requests = employee_data.data_set().time_off_requests().to_vec();
        let job_id = job.id();

        let employee = employee_data.employee().clone();
        let hours_available = employee_data
            .weekly_available_hours(job_id)
            .hours_available_for_week(
                &employee,
                job_balance_factor,
                job_balance_level,
                job_average_shift_length,
                job.is_balance_schedules(),
                &time_off_requests,
                planned_shift_week.end_date(),
            );

        let hours_scheduled = Self::scheduled_hours_for_week(employee_data, &planned_shift_week);

        hours_scheduled <= hours_available
    }
}
