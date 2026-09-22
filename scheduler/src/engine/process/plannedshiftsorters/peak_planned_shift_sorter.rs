//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! PeakPlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/PeakPlannedShiftSorter.java`.
//!
//! "determine the day order by sorting the days by projected hours, where the highest day is
//! first and the lowest day is last. ties retain their original order" — `Vec::sort_by` is
//! stable, matching `Collections.sort`'s guarantee.

use crate::common::numbers::tdouble_round;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::abstract_by_day_planned_shift_sorter::{
    create_planned_shift_date_map_by_day, flatten_planned_shift_date_map_by_day,
};
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `PeakPlannedShiftSorter`.
pub struct PeakPlannedShiftSorter;

impl PlannedShiftSorter for PeakPlannedShiftSorter {
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)> {
        let mut peak_dates = schedule_model.date_range().dates();

        peak_dates
            .sort_by_key(|date| -tdouble_round(job_data.projected_hours().hours_for_date(*date)));

        create_planned_shift_date_map_by_day(job_data, &peak_dates)
    }

    fn flatten_planned_shift_date_map(
        &self,
        map: Vec<(LocalDate, Vec<PlannedShift>)>,
    ) -> Vec<PlannedShift> {
        flatten_planned_shift_date_map_by_day(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::entity::assignment::Assignment;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDateTime;

    fn job() -> Assignment {
        Assignment::new(
            1,
            "job",
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

    fn schedule_model(range: DateRange) -> ScheduleModel {
        let property = Property::new(1, range);
        ScheduleModel::new(property, range)
    }

    fn shift(id: i32, date: LocalDate) -> PlannedShift {
        PlannedShift::new(
            id,
            1,
            date,
            LocalDateTime::of(
                date.year(),
                date.month_value(),
                date.day_of_month(),
                9,
                0,
                0,
            ),
            8.0,
            None,
        )
    }

    #[test]
    fn days_sort_highest_projected_hours_first_ties_keep_original_order() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 3));
        let model = schedule_model(range);

        let mut job_data = JobData::new(job());
        job_data
            .projected_hours_mut()
            .set_hours_for_date(LocalDate::of(2024, 1, 1), 4.0);
        job_data
            .projected_hours_mut()
            .set_hours_for_date(LocalDate::of(2024, 1, 2), 8.0);
        job_data
            .projected_hours_mut()
            .set_hours_for_date(LocalDate::of(2024, 1, 3), 8.0);
        job_data
            .planned_shifts_mut()
            .push(shift(1, LocalDate::of(2024, 1, 1)));
        job_data
            .planned_shifts_mut()
            .push(shift(2, LocalDate::of(2024, 1, 2)));
        job_data
            .planned_shifts_mut()
            .push(shift(3, LocalDate::of(2024, 1, 3)));

        let sorted = PeakPlannedShiftSorter.sort_planned_shifts(&model, &job_data);
        let ids: Vec<i32> = sorted.iter().map(PlannedShift::id).collect();

        // 1/2 and 1/3 tie at 8 hours and keep their original (date) order ahead of 1/1's 4 hours.
        assert_eq!(ids, vec![2, 3, 1]);
    }
}
