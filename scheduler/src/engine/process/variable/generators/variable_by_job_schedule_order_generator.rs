//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators.
//! VariableByJobScheduleOrderGenerator`.
//!
//! Ground truth: `taps/.../process/variable/generators/VariableByJobScheduleOrderGenerator.java`.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::generators::abstract_variable_scheduling_generator::{
    number_job_levels, perform_cleanup_passes,
};
use crate::engine::process::variable::generators::variable_scheduling_generator::VariableSchedulingGenerator;
use crate::engine::process::variable::variable_job_scheduler::VariableJobScheduler;

/// `VariableByJobScheduleOrderGenerator`.
pub struct VariableByJobScheduleOrderGenerator<'a> {
    variable_job_scheduler: &'a VariableJobScheduler<'a>,
}

impl<'a> VariableByJobScheduleOrderGenerator<'a> {
    pub fn new(variable_job_scheduler: &'a VariableJobScheduler<'a>) -> Self {
        Self {
            variable_job_scheduler,
        }
    }
}

impl VariableSchedulingGenerator for VariableByJobScheduleOrderGenerator<'_> {
    /// `generateSchedules(ScheduleModel, JobData)`.
    fn generate_schedules(&self, schedule_model: &mut ScheduleModel, job_id: i32) {
        let number_job_levels = number_job_levels(schedule_model);

        for job_level in 1..=number_job_levels {
            self.variable_job_scheduler
                .schedule_job(schedule_model, job_id, job_level);
        }

        perform_cleanup_passes(self.variable_job_scheduler, schedule_model, job_id, -1);
    }
}
