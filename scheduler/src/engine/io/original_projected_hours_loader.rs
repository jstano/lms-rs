//! Port of `com.unifocus.watson.server.scheduler.engine.io.OriginalProjectedHoursLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! OriginalProjectedHoursLoader.java`. `Results` is ported as
//! `engine::io::ports::OriginalProjectedHoursResult` (it's a plain projection row, not tied to
//! this loader's identity, and `PlannedShiftQueryPort` needs to name it in its own signature).

use crate::engine::io::ports::{OriginalProjectedHoursResult, PlannedShiftQueryPort};
use date_range_rs::DateRange;

/// `OriginalProjectedHoursLoader`.
pub struct OriginalProjectedHoursLoader<'a> {
    planned_shifts: &'a dyn PlannedShiftQueryPort,
}

impl<'a> OriginalProjectedHoursLoader<'a> {
    pub fn new(planned_shifts: &'a dyn PlannedShiftQueryPort) -> Self {
        Self { planned_shifts }
    }

    /// `loadOriginalProjectedHoursByJobAndDate(int, Collection<Integer>, DateRange)`.
    pub fn load_original_projected_hours_by_job_and_date(
        &self,
        property_id: i32,
        job_ids: &[i32],
        date_range: &DateRange,
    ) -> Vec<OriginalProjectedHoursResult> {
        self.planned_shifts
            .original_projected_hours_by_job_and_date(property_id, job_ids, date_range)
    }
}
