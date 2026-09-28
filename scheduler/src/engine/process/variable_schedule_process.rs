//! Port of `com.unifocus.watson.server.scheduler.engine.process.VariableScheduleProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! VariableScheduleProcess.java`. `ScheduleEngine`'s step 9 facade — a 2-line delegation, same
//! shape as `PermanentScheduleProcess`/`RegularScheduleProcess` (`PARITY_AUDIT.md` finding 31).

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::non_pre_scheduled_job_process::NonPreScheduledJobProcess;
use crate::engine::process::variable::pre_scheduled_job_process::PreScheduledJobProcess;

/// `VariableScheduleProcess`.
pub struct VariableScheduleProcess<'a> {
    pre_scheduled_job_process: &'a PreScheduledJobProcess<'a>,
    non_pre_scheduled_job_process: &'a NonPreScheduledJobProcess<'a>,
}

impl<'a> VariableScheduleProcess<'a> {
    pub fn new(
        pre_scheduled_job_process: &'a PreScheduledJobProcess<'a>,
        non_pre_scheduled_job_process: &'a NonPreScheduledJobProcess<'a>,
    ) -> Self {
        Self {
            pre_scheduled_job_process,
            non_pre_scheduled_job_process,
        }
    }

    /// `scheduleVariableEmployees(ScheduleModel)`.
    pub fn schedule_variable_employees(&self, schedule_model: &mut ScheduleModel) {
        self.pre_scheduled_job_process
            .generate_schedules_for_jobs(schedule_model);

        self.non_pre_scheduled_job_process
            .generate_schedules_for_jobs(schedule_model);
    }
}
