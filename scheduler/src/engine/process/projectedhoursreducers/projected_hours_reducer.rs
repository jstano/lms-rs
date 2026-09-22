//! Port of `com.unifocus.watson.server.scheduler.engine.process.projectedhoursreducers.
//! ProjectedHoursReducer`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! projectedhoursreducers/ProjectedHoursReducer.java`. `job_data` takes `&mut` — Java mutates
//! `jobData.getProjectedHours()` in place; `schedule_model` stays `&` — every implementation
//! only reads it (`getDateRange()`/`getEmployeeList()`), never mutates it.

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;

/// `ProjectedHoursReducer`.
pub trait ProjectedHoursReducer {
    /// `reduceProjectedHours(ScheduleModel, JobData)`.
    fn reduce_projected_hours(&self, schedule_model: &ScheduleModel, job_data: &mut JobData);
}
