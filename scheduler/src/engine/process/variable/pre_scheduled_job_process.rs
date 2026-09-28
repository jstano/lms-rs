//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.PreScheduledJobProcess`.
//!
//! Ground truth: `taps/.../process/variable/PreScheduledJobProcess.java`. `combinePlannedShiftsForGroupedJobs`
//! moves shifts between two `JobData`s held in the same `JobList` map — Rust can't hold two
//! `&mut JobData` at once the way Java holds two live references, so each job's shifts are taken
//! out (cleared in place) before being appended to the group's first job, one `job_data_mut` call
//! at a time rather than two simultaneous ones.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::comparators::PreScheduledJobComparator;
use crate::engine::process::variable::variable_job_scheduling_process::VariableJobSchedulingProcess;
use std::collections::HashMap;

/// `PreScheduledJobProcess`.
pub struct PreScheduledJobProcess<'a> {
    variable_job_scheduling_process: &'a VariableJobSchedulingProcess<'a>,
}

impl<'a> PreScheduledJobProcess<'a> {
    pub fn new(variable_job_scheduling_process: &'a VariableJobSchedulingProcess<'a>) -> Self {
        Self {
            variable_job_scheduling_process,
        }
    }

    /// `generateSchedulesForJobs(ScheduleModel)`.
    pub fn generate_schedules_for_jobs(&self, schedule_model: &mut ScheduleModel) {
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| {
                job_list
                    .pre_scheduled_jobs()
                    .into_iter()
                    .map(|job_data| job_data.job().id())
                    .collect()
            })
            .unwrap_or_default();

        Self::combine_planned_shifts_for_grouped_jobs(schedule_model, &job_ids);

        self.variable_job_scheduling_process
            .generate_schedules_for_jobs(schedule_model, &job_ids);
    }

    /// `combinePlannedShiftsForGroupedJobs(List<JobData>)`. An associated function, not a
    /// method — it never touches `self`.
    fn combine_planned_shifts_for_grouped_jobs(
        schedule_model: &mut ScheduleModel,
        job_ids: &[i32],
    ) {
        let grouped_job_ids = Self::grouped_jobs(schedule_model, job_ids);

        let comparator = PreScheduledJobComparator;

        for mut group_job_ids in grouped_job_ids.into_values() {
            let Some(job_list) = schedule_model.job_list() else {
                continue;
            };

            group_job_ids.sort_by(|&id1, &id2| {
                let (Some(job_data1), Some(job_data2)) =
                    (job_list.job_data(id1), job_list.job_data(id2))
                else {
                    return std::cmp::Ordering::Equal;
                };
                comparator.compare(job_data1, job_data2)
            });

            let Some((&first_job_id, rest)) = group_job_ids.split_first() else {
                continue;
            };

            for &job_id in rest {
                let taken_shifts = schedule_model
                    .job_list_mut()
                    .and_then(|job_list| job_list.job_data_mut(job_id))
                    .map(|job_data| std::mem::take(job_data.planned_shifts_mut()))
                    .unwrap_or_default();

                if let Some(first_job_data) = schedule_model
                    .job_list_mut()
                    .and_then(|job_list| job_list.job_data_mut(first_job_id))
                {
                    first_job_data.planned_shifts_mut().extend(taken_shifts);
                }
            }
        }
    }

    /// `getGroupedJobs(List<JobData>)`.
    fn grouped_jobs(schedule_model: &ScheduleModel, job_ids: &[i32]) -> HashMap<i32, Vec<i32>> {
        let Some(job_list) = schedule_model.job_list() else {
            return HashMap::new();
        };

        let mut grouped: HashMap<i32, Vec<i32>> = HashMap::new();

        for &job_id in job_ids {
            let Some(job_data) = job_list.job_data(job_id) else {
                continue;
            };
            let Some(pre_schedule_parameters) = job_data.pre_schedule_parameters() else {
                continue;
            };

            let group_no = pre_schedule_parameters.group_no();

            if group_no > 0 {
                grouped.entry(group_no).or_default().push(job_id);
            }
        }

        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::model::pre_schedule_parameters::PreScheduleParameters;
    use crate::entity::assignment::Assignment;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalDateTime};

    fn job(id: i32) -> Assignment {
        Assignment::new(
            id,
            format!("Job {id}"),
            false,
            None,
            None,
            None,
            false,
            vec![],
            vec![],
            None,
        )
    }

    fn shift(id: i32, job_id: i32) -> PlannedShift {
        PlannedShift::new(
            id,
            job_id,
            LocalDate::of(2024, 1, 1),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            8.0,
            None,
        )
    }

    /// Two jobs share group 1; job 20 (order_no 1) sorts before job 10 (order_no 2), so job 20
    /// becomes the group's "first" job and absorbs job 10's shifts. A third, ungrouped job (30)
    /// is untouched.
    #[test]
    fn merges_grouped_jobs_planned_shifts_into_the_lowest_order_no_job() {
        let date_range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 1));
        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);

        let mut job_data_10 = JobData::new(job(10));
        job_data_10.set_pre_schedule_parameters(Some(PreScheduleParameters::new(2, 1)));
        job_data_10.planned_shifts_mut().push(shift(1, 10));

        let mut job_data_20 = JobData::new(job(20));
        job_data_20.set_pre_schedule_parameters(Some(PreScheduleParameters::new(1, 1)));
        job_data_20.planned_shifts_mut().push(shift(2, 20));

        let mut job_data_30 = JobData::new(job(30));
        job_data_30.set_pre_schedule_parameters(Some(PreScheduleParameters::new(1, 0)));
        job_data_30.planned_shifts_mut().push(shift(3, 30));

        let mut job_list = JobList::new();
        job_list.add_job_data(job_data_10);
        job_list.add_job_data(job_data_20);
        job_list.add_job_data(job_data_30);
        schedule_model.set_job_list(job_list);

        PreScheduledJobProcess::combine_planned_shifts_for_grouped_jobs(
            &mut schedule_model,
            &[10, 20, 30],
        );

        let job_list = schedule_model.job_list().unwrap();
        assert_eq!(job_list.job_data(20).unwrap().planned_shifts().len(), 2);
        assert_eq!(job_list.job_data(10).unwrap().planned_shifts().len(), 0);
        assert_eq!(job_list.job_data(30).unwrap().planned_shifts().len(), 1);
    }
}
