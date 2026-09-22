//! Port of `com.unifocus.watson.server.scheduler.engine.io.PreScheduleLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! PreScheduleLoader.java`. Deferred from Phase 2 step 1 (see `PARITY_AUDIT.md` finding 20) —
//! not reachable from `ScheduleModelLoader.load()`'s call graph; `PreScheduleProcess` (step 6) is
//! its first real caller.

use crate::engine::io::ports::PreScheduleDAOPort;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::pre_schedule::PreSchedule;

/// `PreScheduleLoader`.
pub struct PreScheduleLoader<'a> {
    pre_schedules: &'a dyn PreScheduleDAOPort,
}

impl<'a> PreScheduleLoader<'a> {
    pub fn new(pre_schedules: &'a dyn PreScheduleDAOPort) -> Self {
        Self { pre_schedules }
    }

    /// `loadPreSchedules(ScheduleModel)`.
    pub fn load_pre_schedules(&self, schedule_model: &ScheduleModel) -> Vec<PreSchedule> {
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| job_list.job_ids().collect())
            .unwrap_or_default();

        self.pre_schedules
            .load_pre_schedules_for_jobs_and_date_range(&job_ids, schedule_model.date_range())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalDateTime};

    struct FakePreScheduleDAOPort {
        rows: Vec<PreSchedule>,
    }

    impl PreScheduleDAOPort for FakePreScheduleDAOPort {
        fn load_pre_schedules_for_jobs_and_date_range(
            &self,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<PreSchedule> {
            self.rows.clone()
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
    fn loads_pre_schedules_for_the_job_lists_job_ids_and_the_models_date_range() {
        let start = LocalDateTime::of(2024, 1, 1, 9, 0, 0);
        let end = LocalDateTime::of(2024, 1, 1, 17, 0, 0);
        let pre_schedule = PreSchedule::new(1, 4001, 1, start, end);
        let port = FakePreScheduleDAOPort {
            rows: vec![pre_schedule],
        };
        let loader = PreScheduleLoader::new(&port);

        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));
        schedule_model.set_job_list(job_list);

        let pre_schedules = loader.load_pre_schedules(&schedule_model);

        assert_eq!(pre_schedules.len(), 1);
        assert_eq!(pre_schedules[0].employee_id(), 4001);
    }

    #[test]
    fn returns_empty_when_the_schedule_model_has_no_job_list_yet() {
        let port = FakePreScheduleDAOPort { rows: Vec::new() };
        let loader = PreScheduleLoader::new(&port);

        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let schedule_model = ScheduleModel::new(property, range);

        assert!(loader.load_pre_schedules(&schedule_model).is_empty());
    }
}
