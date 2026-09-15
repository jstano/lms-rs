//! Drives the basic (simple non-flowed) pipeline for one job.
//!
//! Ported from Java's `SimpleNonFlowedStandardsProcessor`: for each effective
//! date and each shift on the standard set, work out how much work the
//! standards require and, if there is any, split it into blocks.

use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::JobShift;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::work_content::WorkContent;
use crate::workcontent::generators::basic::basic_calculator::{
    BasicCalculator, BasicCalculatorImpl,
};
use crate::workcontent::generators::basic::basic_work_content_creator::{
    BasicWorkContentCreator, BasicWorkContentCreatorImpl,
};
use crate::workcontent::generators::basic::total_work_minutes_calculator::{
    TotalWorkMinutesCalculator, TotalWorkMinutesCalculatorImpl,
};
use crate::workcontent::generators::error::GenerationError;
use joda_rs::LocalDate;

pub trait BasicStandardsProcessor {
    /// Work blocks for every date the job's settings are effective on.
    fn process(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<Vec<WorkContent>, GenerationError>;

    /// Work blocks for a single date, for incremental regeneration.
    fn process_for_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
    ) -> Result<Vec<WorkContent>, GenerationError>;
}

pub struct BasicStandardsProcessorImpl {
    total_work_minutes_calculator: Box<dyn TotalWorkMinutesCalculator>,
    basic_calculator: Box<dyn BasicCalculator>,
    work_content_creator: Box<dyn BasicWorkContentCreator>,
}

impl BasicStandardsProcessorImpl {
    pub fn new() -> Self {
        Self {
            total_work_minutes_calculator: Box::new(TotalWorkMinutesCalculatorImpl::new()),
            basic_calculator: Box::new(BasicCalculatorImpl::new()),
            work_content_creator: Box::new(BasicWorkContentCreatorImpl::new()),
        }
    }

    fn process_shift_for_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift: &JobShift,
        date: LocalDate,
        work_contents: &mut Vec<WorkContent>,
    ) -> Result<(), GenerationError> {
        // A shift that does not run on this date, or runs without times set,
        // has nowhere to put work.
        let shift_detail = match shift.shift_detail_for_date(date) {
            Some(shift_detail) if shift_detail.has_times() => shift_detail,
            _ => return Ok(()),
        };

        let total_work_minutes = self.total_work_minutes_calculator.total_work_minutes(
            planner_model,
            job,
            shift,
            date,
            shift_detail.shift_length(),
        )?;

        if total_work_minutes <= 0 {
            return Ok(());
        }

        let result = self.basic_calculator.calculate(
            job.planner_settings(),
            shift_detail,
            shift.id(),
            date,
            total_work_minutes,
        );

        work_contents.extend(self.work_content_creator.create_work_content(
            planner_model,
            job,
            shift_detail,
            &result,
            date,
        ));

        Ok(())
    }

    fn process_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
        work_contents: &mut Vec<WorkContent>,
    ) -> Result<(), GenerationError> {
        for shift in job.shifts_for_standard_set(planner_model.standard_set_id()) {
            self.process_shift_for_date(planner_model, job, shift, date, work_contents)?;
        }

        Ok(())
    }
}

impl BasicStandardsProcessor for BasicStandardsProcessorImpl {
    fn process(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<Vec<WorkContent>, GenerationError> {
        let mut work_contents = Vec::new();

        for date in job.planner_settings().dates(planner_model) {
            self.process_date(planner_model, job, date, &mut work_contents)?;
        }

        Ok(work_contents)
    }

    fn process_for_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
    ) -> Result<Vec<WorkContent>, GenerationError> {
        let mut work_contents = Vec::new();

        self.process_date(planner_model, job, date, &mut work_contents)?;

        Ok(work_contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::JobShiftDefinition;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::shift_standard::{ShiftStandard, ShiftStandardRange};
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::domain::work_type::WorkType;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use std::collections::HashMap;

    // 2025-10-06 is a Monday; the window runs Monday to Wednesday.
    fn monday() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    fn settings() -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.period_length = 15;
        settings.min_shift_length = 0.0;
        settings.max_shift_length = 8.0;
        settings.rounding_threshold_below_one = 0.0;
        settings.rounding_threshold_above_one = 0.0;
        settings
    }

    /// A 24-hour template running on Mondays only, so several 8-hour shifts fit.
    fn monday_only_shift(standard_set_id: StandardSetId) -> JobShift {
        JobShift::new(
            JobId::new(),
            standard_set_id,
            "All day".to_string(),
            1,
            vec![JobShiftDefinition::new(
                DayOfWeek::Monday,
                LocalTime::of_hour_minute(0, 0),
                LocalTime::of_hour_minute(0, 0),
                0.0,
                0.0,
                1,
            )],
        )
    }

    /// A standard yielding `minutes` of work per unit of driver volume.
    fn standard(
        standard_set_id: StandardSetId,
        job_shift_id: crate::workcontent::domain::job_shift::JobShiftId,
        business_driver_id: BusinessDriverId,
        minutes_per_unit: f64,
    ) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::new(0, 100_000, minutes_per_unit)],
        )
    }

    fn model(
        standard_set_id: StandardSetId,
        business_driver_id: BusinessDriverId,
        volumes: Vec<(LocalDate, i32)>,
    ) -> PlannerModel {
        PlannerModel::new(
            DateRange::new(monday(), LocalDate::new(2025, 10, 8)),
            PlannerMode::Standard,
            LocationId::new(),
            standard_set_id,
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                business_driver_id,
                BusinessDriverValues::new(business_driver_id, volumes.into_iter().collect()),
            )]),
        )
    }

    fn job(shifts: Vec<JobShift>, shift_standards: Vec<ShiftStandard>) -> Job {
        Job::new(
            LocationId::new(),
            settings(),
            shifts,
            Vec::new(),
            shift_standards,
        )
    }

    /// A job whose single Monday shift carries one standard.
    fn job_with_standard(
        standard_set_id: StandardSetId,
        business_driver_id: BusinessDriverId,
        minutes_per_unit: f64,
    ) -> Job {
        let shift = monday_only_shift(standard_set_id);
        let shift_id = shift.id();

        job(
            vec![shift],
            vec![standard(
                standard_set_id,
                shift_id,
                business_driver_id,
                minutes_per_unit,
            )],
        )
    }

    #[test]
    fn work_is_planned_only_on_the_dates_the_shift_runs() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        // 480 units at 1 minute each = 8 hours, one full shift.
        let job = job_with_standard(standard_set_id, business_driver_id, 1.0);
        let planner_model = model(
            standard_set_id,
            business_driver_id,
            vec![
                (monday(), 480),
                (LocalDate::new(2025, 10, 7), 480),
                (LocalDate::new(2025, 10, 8), 480),
            ],
        );

        let work_contents = BasicStandardsProcessorImpl::new()
            .process(&planner_model, &job)
            .expect("the fixture is configured to plan");

        // The shift runs on Mondays only, so the Tuesday and Wednesday volume
        // generates nothing.
        assert_eq!(work_contents.len(), 1);
        assert_eq!(work_contents[0].shift_date(), monday());
        assert_eq!(work_contents[0].calculated_hours(), 8.0);
    }

    #[test]
    fn a_job_with_no_standards_plans_nothing() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let job = job(vec![monday_only_shift(standard_set_id)], Vec::new());
        let planner_model = model(standard_set_id, business_driver_id, vec![(monday(), 480)]);

        assert!(
            BasicStandardsProcessorImpl::new()
                .process(&planner_model, &job)
                .expect("the fixture is configured to plan")
                .is_empty()
        );
    }

    #[test]
    fn a_driver_with_no_volume_plans_nothing() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let job = job_with_standard(standard_set_id, business_driver_id, 1.0);
        let planner_model = model(standard_set_id, business_driver_id, Vec::new());

        assert!(
            BasicStandardsProcessorImpl::new()
                .process(&planner_model, &job)
                .expect("the fixture is configured to plan")
                .is_empty()
        );
    }

    #[test]
    fn required_work_is_split_into_full_shifts_and_a_remainder() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let job = job_with_standard(standard_set_id, business_driver_id, 1.0);
        // 18 hours: two 8-hour shifts plus 2 hours.
        let planner_model = model(standard_set_id, business_driver_id, vec![(monday(), 18 * 60)]);

        let work_contents = BasicStandardsProcessorImpl::new()
            .process(&planner_model, &job)
            .expect("the fixture is configured to plan");

        assert_eq!(work_contents.len(), 3);
        assert_eq!(work_contents[0].calculated_hours(), 8.0);
        assert_eq!(work_contents[1].calculated_hours(), 8.0);
        assert_eq!(work_contents[2].calculated_hours(), 2.0);
    }

    #[test]
    fn a_shift_on_another_standard_set_is_ignored() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let job = job_with_standard(StandardSetId::new(), business_driver_id, 1.0);
        let planner_model = model(standard_set_id, business_driver_id, vec![(monday(), 480)]);

        assert!(
            BasicStandardsProcessorImpl::new()
                .process(&planner_model, &job)
                .expect("the fixture is configured to plan")
                .is_empty()
        );
    }

    #[test]
    fn a_shift_definition_without_times_plans_nothing() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = JobShift::new(
            JobId::new(),
            standard_set_id,
            "No times".to_string(),
            1,
            vec![JobShiftDefinition::without_times(DayOfWeek::Monday)],
        );
        let shift_id = shift.id();
        let job = job(
            vec![shift],
            vec![standard(standard_set_id, shift_id, business_driver_id, 1.0)],
        );
        let planner_model = model(standard_set_id, business_driver_id, vec![(monday(), 480)]);

        assert!(
            BasicStandardsProcessorImpl::new()
                .process(&planner_model, &job)
                .expect("the fixture is configured to plan")
                .is_empty()
        );
    }

    #[test]
    fn processing_a_single_date_covers_only_that_date() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let job = job_with_standard(standard_set_id, business_driver_id, 1.0);
        let planner_model = model(standard_set_id, business_driver_id, vec![(monday(), 480)]);

        let work_contents = BasicStandardsProcessorImpl::new()
            .process_for_date(&planner_model, &job, monday())
            .expect("the fixture is configured to plan");
        assert_eq!(work_contents.len(), 1);

        // A date the shift does not run on yields nothing.
        assert!(
            BasicStandardsProcessorImpl::new()
                .process_for_date(&planner_model, &job, LocalDate::new(2025, 10, 7))
                .expect("the fixture is configured to plan")
                .is_empty()
        );
    }
}
