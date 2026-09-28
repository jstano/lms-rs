//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators.
//! VariableBySeniorityGenerator`.
//!
//! Ground truth: `taps/.../process/variable/generators/VariableBySeniorityGenerator.java`.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::generators::abstract_variable_scheduling_generator::perform_cleanup_passes;
use crate::engine::process::variable::generators::variable_scheduling_generator::VariableSchedulingGenerator;
use crate::engine::process::variable::variable_job_scheduler::VariableJobScheduler;

/// `VariableBySeniorityGenerator`.
pub struct VariableBySeniorityGenerator<'a> {
    variable_job_scheduler: &'a VariableJobScheduler<'a>,
}

impl<'a> VariableBySeniorityGenerator<'a> {
    pub fn new(variable_job_scheduler: &'a VariableJobScheduler<'a>) -> Self {
        Self {
            variable_job_scheduler,
        }
    }
}

impl VariableSchedulingGenerator for VariableBySeniorityGenerator<'_> {
    /// `generateSchedules(ScheduleModel, JobData)`.
    fn generate_schedules(&self, schedule_model: &mut ScheduleModel, job_id: i32) {
        self.variable_job_scheduler
            .schedule_job(schedule_model, job_id, -1);

        perform_cleanup_passes(self.variable_job_scheduler, schedule_model, job_id, -1);
    }
}
