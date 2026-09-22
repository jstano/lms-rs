//! Port of `com.unifocus.watson.server.scheduler.engine.io.JobListLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! JobListLoader.java`.

use crate::engine::generate_schedules_parameters::GenerateSchedulesParameters;
use crate::engine::io::ports::JobLoaderPort;
use crate::engine::model::job_data::JobData;
use crate::engine::model::job_list::JobList;

/// `JobListLoader`.
pub struct JobListLoader<'a> {
    jobs: &'a dyn JobLoaderPort,
}

impl<'a> JobListLoader<'a> {
    pub fn new(jobs: &'a dyn JobLoaderPort) -> Self {
        Self { jobs }
    }

    /// `loadJobList(GenerateSchedulesParameters)`.
    pub fn load_job_list(&self, params: &GenerateSchedulesParameters) -> JobList {
        let mut job_list = JobList::new();

        for job in self.jobs.load_jobs(params) {
            job_list.add_job_data(JobData::new(job));
        }

        job_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    struct FakeJobLoaderPort {
        jobs: Vec<Assignment>,
    }

    impl JobLoaderPort for FakeJobLoaderPort {
        fn load_jobs(&self, _params: &GenerateSchedulesParameters) -> Vec<Assignment> {
            self.jobs.clone()
        }
    }

    fn job(id: i32) -> Assignment {
        Assignment::new(
            id,
            format!("Job {id}"),
            false,
            None,
            None,
            None,
            false,
            Vec::<AssignmentSortOrder>::new(),
            Vec::new(),
            None,
        )
    }

    #[test]
    fn adds_job_data_for_every_loaded_job() {
        let port = FakeJobLoaderPort {
            jobs: vec![job(1), job(2)],
        };
        let loader = JobListLoader::new(&port);
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let params = GenerateSchedulesParameters::new(range, 1);

        let job_list = loader.load_job_list(&params);

        assert!(job_list.contains_job(1));
        assert!(job_list.contains_job(2));
        assert!(!job_list.contains_job(3));
    }
}
