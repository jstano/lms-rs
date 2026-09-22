//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! AbstractByDayPlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/AbstractByDayPlannedShiftSorter.java`. Java is an abstract base class with
//! one abstract method (`sortDates`); ported as two free functions the four "by day" sorters
//! (`DayOfWeekPlannedShiftSorter`, `DefaultPlannedShiftSorter`, `ModifiedPeakPlannedShiftSorter`,
//! `PeakPlannedShiftSorter`) each call from their own `PlannedShiftSorter` impl, passing their own
//! date order in — no trait inheritance needed for two functions shared four ways.

use crate::engine::model::job_data::JobData;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// `createPlannedShiftDateMap(ScheduleModel, JobData)` — groups `job_data`'s planned shifts by
/// date, then orders the groups by `sorted_dates`, dropping any date with no shifts. Java's
/// `throwingMerger` guards against `sorted_dates` containing the same date twice; not reproduced
/// as a panic here since every subclass's `sortDates` visits each date in the schedule model's
/// range exactly once by construction — if a future sorter breaks that, a duplicate date simply
/// overwrites its earlier group's position instead of panicking.
pub fn create_planned_shift_date_map_by_day(
    job_data: &JobData,
    sorted_dates: &[LocalDate],
) -> Vec<(LocalDate, Vec<PlannedShift>)> {
    let mut by_date: HashMap<LocalDate, Vec<PlannedShift>> = HashMap::new();

    for shift in job_data.planned_shifts() {
        by_date.entry(shift.shift_date()).or_default().push(*shift);
    }

    sorted_dates
        .iter()
        .filter_map(|date| by_date.remove(date).map(|shifts| (*date, shifts)))
        .collect()
}

/// `flattenPlannedShiftDateMap(Map)`.
pub fn flatten_planned_shift_date_map_by_day(
    map: Vec<(LocalDate, Vec<PlannedShift>)>,
) -> Vec<PlannedShift> {
    map.into_iter().flat_map(|(_, shifts)| shifts).collect()
}
