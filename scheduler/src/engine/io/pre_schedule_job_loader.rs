//! Port of `com.unifocus.watson.server.scheduler.engine.io.PreScheduleJobLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! PreScheduleJobLoader.java`.

use crate::engine::io::ports::PreScheduleJobclassPort;
use crate::engine::model::pre_schedule_parameters::PreScheduleParameters;
use crate::engine::model::schedule_model::ScheduleModel;

/// `PreScheduleJobLoader`.
pub struct PreScheduleJobLoader<'a> {
    pre_schedule_jobclasses: &'a dyn PreScheduleJobclassPort,
}

impl<'a> PreScheduleJobLoader<'a> {
    pub fn new(pre_schedule_jobclasses: &'a dyn PreScheduleJobclassPort) -> Self {
        Self {
            pre_schedule_jobclasses,
        }
    }

    /// `loadPreScheduledJobs(ScheduleModel)`.
    pub fn load_pre_scheduled_jobs(&self, schedule_model: &mut ScheduleModel) {
        let property_id = schedule_model.property().id();
        let pre_schedule_jobclasses = self
            .pre_schedule_jobclasses
            .find_all_for_property(property_id);

        let Some(job_list) = schedule_model.job_list_mut() else {
            return;
        };

        for pre_schedule_jobclass in pre_schedule_jobclasses {
            if let Some(job_data) = job_list.job_data_mut(pre_schedule_jobclass.job_id()) {
                job_data.set_pre_schedule_parameters(Some(PreScheduleParameters::new(
                    pre_schedule_jobclass.order_no(),
                    pre_schedule_jobclass.group_no(),
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::pre_schedule_jobclass::PreScheduleJobclass;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    struct FakePreScheduleJobclassPort {
        rows: Vec<PreScheduleJobclass>,
    }

    impl PreScheduleJobclassPort for FakePreScheduleJobclassPort {
        fn find_all_for_property(&self, _property_id: i32) -> Vec<PreScheduleJobclass> {
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
    fn sets_pre_schedule_parameters_only_for_jobs_present_in_the_job_list() {
        let port = FakePreScheduleJobclassPort {
            rows: vec![
                PreScheduleJobclass::new(1, 10, 20),
                PreScheduleJobclass::new(99, 1, 2),
            ],
        };
        let loader = PreScheduleJobLoader::new(&port);

        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));
        schedule_model.set_job_list(job_list);

        loader.load_pre_scheduled_jobs(&mut schedule_model);

        let job_data = schedule_model.job_list().unwrap().job_data(1).unwrap();
        let params = job_data.pre_schedule_parameters().unwrap();
        assert_eq!(params.order_no(), 10);
        assert_eq!(params.group_no(), 20);
    }
}
