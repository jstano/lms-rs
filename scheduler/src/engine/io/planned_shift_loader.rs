//! Port of `com.unifocus.watson.server.scheduler.engine.io.PlannedShiftLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! PlannedShiftLoader.java`. Java's `jobList.getJobData(...)` calls are not null-checked here —
//! it assumes every loaded shift/hours row belongs to a job already in `jobList` (which was
//! loaded from the same property/date scope) and would NPE otherwise. Ported defensively instead
//! (skip rows whose job isn't in `jobList`, per the "don't default to `.unwrap()` without
//! checking the caller" rule — `PARITY_AUDIT.md` finding 7's methodology, applied here too).

use crate::engine::io::forecast_planned_shift_loader::ForecastPlannedShiftLoader;
use crate::engine::io::original_projected_hours_loader::OriginalProjectedHoursLoader;
use crate::engine::model::job_list::JobList;
use date_range_rs::DateRange;

/// `PlannedShiftLoader`.
pub struct PlannedShiftLoader<'a> {
    forecast_planned_shifts: ForecastPlannedShiftLoader<'a>,
    original_projected_hours: OriginalProjectedHoursLoader<'a>,
}

impl<'a> PlannedShiftLoader<'a> {
    pub fn new(
        forecast_planned_shifts: ForecastPlannedShiftLoader<'a>,
        original_projected_hours: OriginalProjectedHoursLoader<'a>,
    ) -> Self {
        Self {
            forecast_planned_shifts,
            original_projected_hours,
        }
    }

    /// `loadPlannedShifts(int, DateRange, JobList)`.
    pub fn load_planned_shifts(
        &self,
        property_id: i32,
        date_range: &DateRange,
        job_list: &mut JobList,
    ) {
        self.load_forecast_planned_shifts(property_id, date_range, job_list);
        self.load_original_projected_hours(property_id, date_range, job_list);
    }

    fn load_forecast_planned_shifts(
        &self,
        property_id: i32,
        date_range: &DateRange,
        job_list: &mut JobList,
    ) {
        let job_ids: Vec<i32> = job_list.job_ids().collect();
        let planned_shifts = self
            .forecast_planned_shifts
            .load_forecast_planned_shifts_for_jobs_and_dates(property_id, &job_ids, date_range);

        for planned_shift in planned_shifts {
            if let Some(job_data) = job_list.job_data_mut(planned_shift.job_id()) {
                job_data.planned_shifts_mut().push(planned_shift);
            }
        }
    }

    fn load_original_projected_hours(
        &self,
        property_id: i32,
        date_range: &DateRange,
        job_list: &mut JobList,
    ) {
        let job_ids: Vec<i32> = job_list.job_ids().collect();
        let results = self
            .original_projected_hours
            .load_original_projected_hours_by_job_and_date(property_id, &job_ids, date_range);

        for result in results {
            if let Some(job_data) = job_list.job_data_mut(result.job_id()) {
                job_data
                    .projected_hours_mut()
                    .add_hours_to_date(result.shift_date(), result.hours());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::io::ports::{OriginalProjectedHoursResult, PlannedShiftQueryPort};
    use crate::engine::model::job_data::JobData;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::planned_shift::PlannedShift;
    use joda_rs::{LocalDate, LocalDateTime};

    struct FakePlannedShiftQueryPort {
        shifts: Vec<PlannedShift>,
        hours: Vec<OriginalProjectedHoursResult>,
    }

    impl PlannedShiftQueryPort for FakePlannedShiftQueryPort {
        fn forecast_planned_shifts_for_jobs_and_dates(
            &self,
            _property_id: i32,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<PlannedShift> {
            self.shifts.clone()
        }

        fn original_projected_hours_by_job_and_date(
            &self,
            _property_id: i32,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<OriginalProjectedHoursResult> {
            self.hours.clone()
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
    fn routes_shifts_and_hours_to_the_matching_job_and_skips_unknown_jobs() {
        let date = LocalDate::of(2024, 1, 1);
        let port = FakePlannedShiftQueryPort {
            shifts: vec![
                PlannedShift::new(
                    1,
                    1,
                    date,
                    LocalDateTime::of(2024, 1, 1, 9, 0, 0),
                    8.0,
                    None,
                ),
                PlannedShift::new(
                    2,
                    99,
                    date,
                    LocalDateTime::of(2024, 1, 1, 9, 0, 0),
                    8.0,
                    None,
                ),
            ],
            hours: vec![
                OriginalProjectedHoursResult::new(1, date, 6.0),
                OriginalProjectedHoursResult::new(99, date, 6.0),
            ],
        };
        let loader = PlannedShiftLoader::new(
            ForecastPlannedShiftLoader::new(&port),
            OriginalProjectedHoursLoader::new(&port),
        );

        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));

        let date_range = DateRange::new(date, date);
        loader.load_planned_shifts(1, &date_range, &mut job_list);

        let job_data = job_list.job_data(1).unwrap();
        assert_eq!(job_data.planned_shifts().len(), 1);
        assert_eq!(job_data.planned_shifts()[0].id(), 1);
        assert_eq!(job_data.projected_hours().hours_for_date(date), 6.0);
    }
}
