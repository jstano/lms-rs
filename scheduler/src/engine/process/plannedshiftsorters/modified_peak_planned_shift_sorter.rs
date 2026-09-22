//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! ModifiedPeakPlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/ModifiedPeakPlannedShiftSorter.java`. Same "highest projected hours
//! first, stable on ties" shape as `PeakPlannedShiftSorter`, but net of the job's employees'
//! effective available hours on each date.

use crate::common::numbers::tdouble_round;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::abstract_by_day_planned_shift_sorter::{
    create_planned_shift_date_map_by_day, flatten_planned_shift_date_map_by_day,
};
use crate::engine::process::plannedshiftsorters::planned_shift_sorter::PlannedShiftSorter;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `ModifiedPeakPlannedShiftSorter`.
pub struct ModifiedPeakPlannedShiftSorter;

impl PlannedShiftSorter for ModifiedPeakPlannedShiftSorter {
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)> {
        let mut peak_dates = schedule_model.date_range().dates();
        let employee_list = schedule_model.employee_list();

        peak_dates.sort_by_key(|date| {
            let projected_hours = tdouble_round(job_data.projected_hours().hours_for_date(*date));
            let available_hours =
                tdouble_round(Self::available_hours(employee_list, job_data, *date));
            // we want to sort highest to lowest
            -(projected_hours - available_hours)
        });

        create_planned_shift_date_map_by_day(job_data, &peak_dates)
    }

    fn flatten_planned_shift_date_map(
        &self,
        map: Vec<(LocalDate, Vec<PlannedShift>)>,
    ) -> Vec<PlannedShift> {
        flatten_planned_shift_date_map_by_day(map)
    }
}

impl ModifiedPeakPlannedShiftSorter {
    fn available_hours(
        employee_list: Option<&crate::engine::model::employee_list::EmployeeList>,
        job_data: &JobData,
        date: LocalDate,
    ) -> f64 {
        let Some(employee_list) = employee_list else {
            return 0.0;
        };

        employee_list
            .employees_with_job(job_data.job().id(), date)
            .into_iter()
            .map(|employee_data| employee_data.effective_available_hours_for_date(date))
            .sum()
    }
}
