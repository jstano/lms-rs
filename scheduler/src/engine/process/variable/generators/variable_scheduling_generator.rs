//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators.
//! VariableSchedulingGenerator`.
//!
//! Ground truth: `taps/.../process/variable/generators/VariableSchedulingGenerator.java`. Takes
//! `job_id: i32` rather than `&JobData` — same "re-fetch by id, don't hold a borrow across a
//! mutating call" idiom as `VariableJobScheduler::schedule_job` (this wave's other caller of the
//! same shape).

use crate::engine::model::schedule_model::ScheduleModel;

/// `VariableSchedulingGenerator`.
pub trait VariableSchedulingGenerator {
    /// `generateSchedules(ScheduleModel, JobData)`.
    fn generate_schedules(&self, schedule_model: &mut ScheduleModel, job_id: i32);
}
