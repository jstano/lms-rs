//! Port of `com.unifocus.watson.server.scheduler.engine.model.JobList`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/JobList.java`.
//! Id-keyed collection of `JobData`; see `DATA_MODEL.md` §4.
//!
//! `getNonPreScheduledJobs()`/`getPreScheduledJobs()` are ported as of Phase 2 step 9, using
//! `process/variable/comparators::{NonPreScheduledJobComparator,PreScheduledJobComparator}` (see
//! `PARITY_AUDIT.md` finding 3, now resolved).

use crate::engine::model::job_data::JobData;
use crate::engine::process::variable::comparators::{
    NonPreScheduledJobComparator, PreScheduledJobComparator,
};
use std::collections::HashMap;

/// `JobList`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobList {
    job_data: HashMap<i32, JobData>,
}

impl JobList {
    pub fn new() -> Self {
        Self::default()
    }

    /// `addJobData(JobData)`.
    pub fn add_job_data(&mut self, job_data: JobData) {
        self.job_data.insert(job_data.job().id(), job_data);
    }

    /// `getJobData(Assignment)` / `getJobData(int)`.
    pub fn job_data(&self, job_id: i32) -> Option<&JobData> {
        self.job_data.get(&job_id)
    }

    /// `getJobData(int)` (mutable).
    pub fn job_data_mut(&mut self, job_id: i32) -> Option<&mut JobData> {
        self.job_data.get_mut(&job_id)
    }

    /// Removes and returns one job's data, for a caller that needs simultaneous mutable access to
    /// both this `JobList`'s owner (`ScheduleModel`) and the single `JobData` it holds — same
    /// take/reinsert idiom as [`EmployeeList::take_employee_data`](crate::engine::model::
    /// employee_list::EmployeeList::take_employee_data), used by `ScheduleEngine` (Phase 3) to
    /// call `ProjectedHoursReducer`/`EmployeeAvailableHoursBalancer`, both of which take `&
    /// ScheduleModel` (aliasing this job's own `JobList`) alongside `&mut JobData`. Pair with
    /// [`add_job_data`](Self::add_job_data) to put it back once the caller is done.
    pub fn take_job_data(&mut self, job_id: i32) -> Option<JobData> {
        self.job_data.remove(&job_id)
    }

    /// `containsJob(Assignment)`.
    pub fn contains_job(&self, job_id: i32) -> bool {
        self.job_data.contains_key(&job_id)
    }

    /// `getJobIDs()`.
    pub fn job_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.job_data.keys().copied()
    }

    /// `getAllJobs()` — sorted by job full name, case-insensitive.
    pub fn all_jobs(&self) -> Vec<&JobData> {
        let mut jobs: Vec<&JobData> = self.job_data.values().collect();
        jobs.sort_by_key(|j| j.job().full_name().to_lowercase());
        jobs
    }

    /// `getNonPreScheduledJobs()`.
    pub fn non_pre_scheduled_jobs(&self) -> Vec<&JobData> {
        let mut jobs: Vec<&JobData> = self
            .job_data
            .values()
            .filter(|j| j.pre_schedule_parameters().is_none())
            .collect();
        let comparator = NonPreScheduledJobComparator;
        jobs.sort_by(|a, b| comparator.compare(a, b));
        jobs
    }

    /// `getPreScheduledJobs()`.
    pub fn pre_scheduled_jobs(&self) -> Vec<&JobData> {
        let mut jobs: Vec<&JobData> = self
            .job_data
            .values()
            .filter(|j| j.pre_schedule_parameters().is_some())
            .collect();
        let comparator = PreScheduledJobComparator;
        jobs.sort_by(|a, b| comparator.compare(a, b));
        jobs
    }
}
