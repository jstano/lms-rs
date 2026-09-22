//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! CascadePlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/CascadePlannedShiftSorter.java`. Round-robins across dates (one shift from
//! each date's bucket in turn) rather than exhausting one date before the next — the only sorter
//! whose `flatten` isn't a plain concatenation, which is why it implements
//! [`PlannedShiftSorter`] directly instead of going through the `abstract_by_day` helpers.

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;
use std::collections::BTreeMap;

/// `CascadePlannedShiftSorter`.
pub struct CascadePlannedShiftSorter;

impl PlannedShiftSorter for CascadePlannedShiftSorter {
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)> {
        let mut by_date: BTreeMap<LocalDate, Vec<PlannedShift>> = BTreeMap::new();

        for shift in job_data.planned_shifts() {
            if schedule_model
                .date_range()
                .contains_date(shift.shift_date())
            {
                by_date.entry(shift.shift_date()).or_default().push(*shift);
            }
        }

        by_date.into_iter().collect()
    }

    fn flatten_planned_shift_date_map(
        &self,
        map: Vec<(LocalDate, Vec<PlannedShift>)>,
    ) -> Vec<PlannedShift> {
        let mut buckets: Vec<Vec<PlannedShift>> =
            map.into_iter().map(|(_, shifts)| shifts).collect();
        let mut planned_shifts = Vec::new();

        while buckets.iter().any(|bucket| !bucket.is_empty()) {
            for bucket in buckets.iter_mut() {
                if !bucket.is_empty() {
                    planned_shifts.push(bucket.remove(0));
                }
            }
        }

        planned_shifts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::entity::assignment::Assignment;
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
    fn round_robins_across_dates_instead_of_exhausting_one_date_first() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 2));
        let property = Property::new(1, range);
        let model = ScheduleModel::new(property, range);

        let mut job_data = JobData::new(job());
        // 1/1 has two shifts, 1/2 has one.
        job_data
            .planned_shifts_mut()
            .push(shift(1, LocalDate::of(2024, 1, 1)));
        job_data
            .planned_shifts_mut()
            .push(shift(2, LocalDate::of(2024, 1, 1)));
        job_data
            .planned_shifts_mut()
            .push(shift(3, LocalDate::of(2024, 1, 2)));

        let sorted = CascadePlannedShiftSorter.sort_planned_shifts(&model, &job_data);
        let ids: Vec<i32> = sorted.iter().map(PlannedShift::id).collect();

        // One from 1/1, one from 1/2, then the leftover from 1/1 — not [1, 2, 3].
        assert_eq!(ids, vec![1, 3, 2]);
    }

    #[test]
    fn shifts_outside_the_schedule_range_are_excluded() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 1));
        let property = Property::new(1, range);
        let model = ScheduleModel::new(property, range);

        let mut job_data = JobData::new(job());
        job_data
            .planned_shifts_mut()
            .push(shift(1, LocalDate::of(2024, 1, 1)));
        job_data
            .planned_shifts_mut()
            .push(shift(2, LocalDate::of(2024, 1, 5)));

        let sorted = CascadePlannedShiftSorter.sort_planned_shifts(&model, &job_data);
        let ids: Vec<i32> = sorted.iter().map(PlannedShift::id).collect();

        assert_eq!(ids, vec![1]);
    }
}
