//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.NonPreScheduledJobProcess`.
//!
//! Ground truth: `taps/.../process/variable/NonPreScheduledJobProcess.java`.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::variable_job_scheduling_process::VariableJobSchedulingProcess;

/// `NonPreScheduledJobProcess`.
pub struct NonPreScheduledJobProcess<'a> {
    variable_job_scheduling_process: &'a VariableJobSchedulingProcess<'a>,
}

impl<'a> NonPreScheduledJobProcess<'a> {
    pub fn new(variable_job_scheduling_process: &'a VariableJobSchedulingProcess<'a>) -> Self {
        Self {
            variable_job_scheduling_process,
        }
    }

    /// `generateSchedulesForJobs(ScheduleModel)`.
    pub fn generate_schedules_for_jobs(&self, schedule_model: &mut ScheduleModel) {
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| {
                job_list
                    .non_pre_scheduled_jobs()
                    .into_iter()
                    .map(|job_data| job_data.job().id())
                    .collect()
            })
            .unwrap_or_default();

        self.variable_job_scheduling_process
            .generate_schedules_for_jobs(schedule_model, &job_ids);
    }
}
