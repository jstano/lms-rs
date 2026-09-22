//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! DefaultPlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/DefaultPlannedShiftSorter.java`.

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::abstract_by_day_planned_shift_sorter::{
    create_planned_shift_date_map_by_day, flatten_planned_shift_date_map_by_day,
};
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `DefaultPlannedShiftSorter` — the schedule model's date range, in order.
pub struct DefaultPlannedShiftSorter;

impl PlannedShiftSorter for DefaultPlannedShiftSorter {
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)> {
        let sorted_dates = schedule_model.date_range().dates();
        create_planned_shift_date_map_by_day(job_data, &sorted_dates)
    }

    fn flatten_planned_shift_date_map(
        &self,
        map: Vec<(LocalDate, Vec<PlannedShift>)>,
    ) -> Vec<PlannedShift> {
        flatten_planned_shift_date_map_by_day(map)
    }
}
