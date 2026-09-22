//! Port of `com.unifocus.watson.server.scheduler.engine.model.JobList`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/JobList.java`.
//! Id-keyed collection of `JobData`; see `DATA_MODEL.md` §4.
//!
//! `getNonPreScheduledJobs()`/`getPreScheduledJobs()` are **not ported** — their sort order comes
//! from `process/variable/comparators::{NonPreScheduledJobComparator,PreScheduledJobComparator}`,
//! Phase 2 step 9. See `DATA_MODEL.md` §7 / `PARITY_AUDIT.md` finding 3.

use crate::engine::model::job_data::JobData;
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
}
