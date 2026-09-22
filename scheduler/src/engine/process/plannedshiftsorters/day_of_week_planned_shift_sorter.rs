//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! DayOfWeekPlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/DayOfWeekPlannedShiftSorter.java`.

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::abstract_by_day_planned_shift_sorter::{
    create_planned_shift_date_map_by_day, flatten_planned_shift_date_map_by_day,
};
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `DayOfWeekPlannedShiftSorter` — every date in the range, grouped by the job's configured
/// day-of-week priority order.
pub struct DayOfWeekPlannedShiftSorter;

impl PlannedShiftSorter for DayOfWeekPlannedShiftSorter {
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)> {
        let dates = schedule_model.date_range().dates();
        let mut sorted_dates = Vec::new();

        for day_of_week in job_data.job().day_of_week_order() {
            for date in &dates {
                if date.day_of_week() == *day_of_week {
                    sorted_dates.push(*date);
                }
            }
        }

        create_planned_shift_date_map_by_day(job_data, &sorted_dates)
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
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDateTime};

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
    fn dates_are_grouped_by_the_jobs_day_of_week_priority_order() {
        // 2024-01-01 is a Monday, 2024-01-02 Tuesday, 2024-01-03 Wednesday.
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 3));
        let property = Property::new(1, range);
        let model = ScheduleModel::new(property, range);

        let job = Assignment::new(
            1,
            "job",
            false,
            None,
            None,
            None,
            false,
            Vec::<AssignmentSortOrder>::new(),
            vec![DayOfWeek::Wednesday, DayOfWeek::Monday],
            None,
        );
        let mut job_data = JobData::new(job);
        job_data
            .planned_shifts_mut()
            .push(shift(1, LocalDate::of(2024, 1, 1)));
        job_data
            .planned_shifts_mut()
            .push(shift(2, LocalDate::of(2024, 1, 3)));

        let sorted = DayOfWeekPlannedShiftSorter.sort_planned_shifts(&model, &job_data);
        let ids: Vec<i32> = sorted.iter().map(PlannedShift::id).collect();

        // Wednesday (1/3, id 2) comes before Monday (1/1, id 1); Tuesday has no shift.
        assert_eq!(ids, vec![2, 1]);
    }
}
