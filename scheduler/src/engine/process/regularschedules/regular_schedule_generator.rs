//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleGenerator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleGenerator.java`. Collects job ids (in `JobList::all_jobs`'s
//! sorted order, matching `getAllJobs()`) and dates up front, as owned `Vec`s, before the inner
//! loop starts calling `RegularScheduleSingleDate::schedule_date` (which needs `&mut
//! ScheduleModel`) — ends the borrow on `schedule_model` those two reads would otherwise hold
//! open across the mutating calls.

use crate::engine::model::regular_schedules::RegularSchedules;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
use crate::engine::process::regularschedules::regular_schedule_single_date::RegularScheduleSingleDate;

/// `RegularScheduleGenerator`.
pub struct RegularScheduleGenerator<'a> {
    regular_schedule_single_date: &'a RegularScheduleSingleDate<'a>,
}

impl<'a> RegularScheduleGenerator<'a> {
    pub fn new(regular_schedule_single_date: &'a RegularScheduleSingleDate<'a>) -> Self {
        Self {
            regular_schedule_single_date,
        }
    }

    /// `scheduleEmployees(ScheduleModel, RegularSchedules)`.
    pub fn schedule_employees(
        &self,
        schedule_model: &mut ScheduleModel,
        regular_schedules: &RegularSchedules,
        active_on_date: &dyn EmployeeActiveOnDatePort,
    ) {
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| {
                job_list
                    .all_jobs()
                    .into_iter()
                    .map(|job_data| job_data.job().id())
                    .collect()
            })
            .unwrap_or_default();
        let dates = schedule_model.date_range().dates();

        for job_id in job_ids {
            for shift_date in &dates {
                self.regular_schedule_single_date.schedule_date(
                    schedule_model,
                    regular_schedules,
                    job_id,
                    *shift_date,
                    active_on_date,
                );
            }
        }
    }
}
