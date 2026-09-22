//! Port of `com.unifocus.watson.server.scheduler.engine.io.ForecastPlannedShiftLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! ForecastPlannedShiftLoader.java`. The `jobIDs.isEmpty()` short-circuit (avoiding an invalid
//! empty `IN ()` clause) is a query-construction concern that belongs to whatever real
//! `PlannedShiftQueryPort` implementation eventually issues the query, not to this wrapper — see
//! `engine::io::ports`'s doc on what these traits do and don't cover.

use crate::engine::io::ports::PlannedShiftQueryPort;
use crate::entity::planned_shift::PlannedShift;
use date_range_rs::DateRange;

/// `ForecastPlannedShiftLoader`.
pub struct ForecastPlannedShiftLoader<'a> {
    planned_shifts: &'a dyn PlannedShiftQueryPort,
}

impl<'a> ForecastPlannedShiftLoader<'a> {
    pub fn new(planned_shifts: &'a dyn PlannedShiftQueryPort) -> Self {
        Self { planned_shifts }
    }

    /// `loadForecastPlannedShiftsForJobsAndDates(int, Collection<Integer>, DateRange)`.
    pub fn load_forecast_planned_shifts_for_jobs_and_dates(
        &self,
        property_id: i32,
        job_ids: &[i32],
        date_range: &DateRange,
    ) -> Vec<PlannedShift> {
        self.planned_shifts
            .forecast_planned_shifts_for_jobs_and_dates(property_id, job_ids, date_range)
    }
}
