//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! PlannedShiftSorterFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/PlannedShiftSorterFactory.java`.

use crate::engine::model::job_data::JobData;
use crate::engine::process::plannedshiftsorters::cascade_planned_shift_sorter::CascadePlannedShiftSorter;
use crate::engine::process::plannedshiftsorters::day_of_week_planned_shift_sorter::DayOfWeekPlannedShiftSorter;
use crate::engine::process::plannedshiftsorters::default_planned_shift_sorter::DefaultPlannedShiftSorter;
use crate::engine::process::plannedshiftsorters::modified_peak_planned_shift_sorter::ModifiedPeakPlannedShiftSorter;
use crate::engine::process::plannedshiftsorters::peak_planned_shift_sorter::PeakPlannedShiftSorter;
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift_sorting_method::PlannedShiftSortingMethod;

/// `PlannedShiftSorterFactory`.
pub struct PlannedShiftSorterFactory;

impl PlannedShiftSorterFactory {
    /// `getPlannedShiftSorter(JobData)`.
    pub fn planned_shift_sorter(job_data: &JobData) -> &'static dyn PlannedShiftSorter {
        match job_data.job().planned_shift_sorting_method() {
            Some(PlannedShiftSortingMethod::ByDay) => &DayOfWeekPlannedShiftSorter,
            Some(PlannedShiftSortingMethod::Cascade) => &CascadePlannedShiftSorter,
            Some(PlannedShiftSortingMethod::ModifiedPeak) => &ModifiedPeakPlannedShiftSorter,
            Some(PlannedShiftSortingMethod::Peak) => &PeakPlannedShiftSorter,
            None => &DefaultPlannedShiftSorter,
        }
    }
}
