use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::JobShift;
use crate::workcontent::domain::labor_data::LaborData;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::salaried::salaried_calculator::{
    SalariedCalculator, SalariedCalculatorImpl,
};
use crate::workcontent::generators::work_generators::WorkResults;
use joda_rs::LocalDate;

pub trait SalariedStandardsProcessor {
    /// Salaried hours for every date the job's settings are effective on.
    fn process(&self, planner_model: &PlannerModel, job: &Job) -> WorkResults;

    /// Salaried hours for a single date, for incremental regeneration.
    fn process_for_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
    ) -> WorkResults;
}

pub struct SalariedStandardsProcessorImpl {
    salaried_calculator: Box<dyn SalariedCalculator>,
}

impl SalariedStandardsProcessorImpl {
    pub fn new() -> Self {
        Self::with_calculator(Box::new(SalariedCalculatorImpl::new()))
    }

    pub fn with_calculator(salaried_calculator: Box<dyn SalariedCalculator>) -> Self {
        Self {
            salaried_calculator,
        }
    }

    fn calculate_work_for_all_shifts(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
    ) -> f64 {
        job.shifts_for_standard_set(planner_model.standard_set_id())
            .iter()
            .map(|shift| self.calculate_work_for_shift(planner_model, job, shift, date))
            .sum()
    }

    fn calculate_work_for_shift(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift: &JobShift,
        date: LocalDate,
    ) -> f64 {
        // A shift that does not run on this date, or runs without times set,
        // generates no salaried hours.
        match shift.shift_detail_for_date(date) {
            Some(shift_detail) if shift_detail.has_times() => {
                self.calc_work_for_shift_detail(planner_model, job, shift, date)
            }
            _ => 0.0,
        }
    }

    fn calc_work_for_shift_detail(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift: &JobShift,
        date: LocalDate,
    ) -> f64 {
        job.salaried_standard_for_standard_set_and_shift(planner_model.standard_set_id(), shift)
            .map(|standard| self.salaried_calculator.calculate_hours(standard, date))
            .unwrap_or(0.0)
    }
}

impl SalariedStandardsProcessor for SalariedStandardsProcessorImpl {
    fn process(&self, planner_model: &PlannerModel, job: &Job) -> WorkResults {
        let labor_data_results = job
            .planner_settings()
            .dates(planner_model)
            .into_iter()
            .map(|date| {
                LaborData::new(
                    job.id(),
                    date,
                    self.calculate_work_for_all_shifts(planner_model, job, date),
                )
            })
            .collect();

        WorkResults::with_labor_data(job.id(), labor_data_results)
    }

    fn process_for_date(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        date: LocalDate,
    ) -> WorkResults {
        let hours = self.calculate_work_for_all_shifts(planner_model, job, date);

        WorkResults::with_labor_data(job.id(), vec![LaborData::new(job.id(), date, hours)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::{JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::salaried_standard::SalariedStandard;
    use crate::workcontent::domain::salary_mode::SalaryMode;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use std::collections::HashMap;

    /// A calculator that reports a fixed number of hours, so the processor's
    /// own filtering and summing is what the assertions exercise.
    struct FixedHoursCalculator {
        hours: f64,
    }

    impl SalariedCalculator for FixedHoursCalculator {
        fn calculate_hours(&self, _standard: &SalariedStandard, _date: LocalDate) -> f64 {
            self.hours
        }
    }

    fn processor(hours: f64) -> SalariedStandardsProcessorImpl {
        SalariedStandardsProcessorImpl::with_calculator(Box::new(FixedHoursCalculator { hours }))
    }

    fn every_day_definitions() -> Vec<JobShiftDefinition> {
        [
            DayOfWeek::Monday,
            DayOfWeek::Tuesday,
            DayOfWeek::Wednesday,
            DayOfWeek::Thursday,
            DayOfWeek::Friday,
            DayOfWeek::Saturday,
            DayOfWeek::Sunday,
        ]
        .into_iter()
        .map(|day_of_week| {
            JobShiftDefinition::new(
                day_of_week,
                LocalTime::of_hour_minute(9, 0),
                LocalTime::of_hour_minute(17, 0),
                0.0,
                0.0,
                1,
            )
        })
        .collect()
    }

    fn shift(standard_set_id: StandardSetId, definitions: Vec<JobShiftDefinition>) -> JobShift {
        JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            definitions,
        )
    }

    fn standard(standard_set_id: StandardSetId, shift_id: JobShiftId) -> SalariedStandard {
        SalariedStandard::new(
            JobId::new(),
            standard_set_id,
            shift_id,
            SalaryMode::WEEKLY,
            Some(40.0),
            None,
            None,
        )
    }

    fn model(standard_set_id: StandardSetId) -> PlannerModel {
        PlannerModel::new(
            // 2025-10-06 is a Monday; a full week.
            DateRange::new(LocalDate::new(2025, 10, 6), LocalDate::new(2025, 10, 12)),
            PlannerMode::Standard,
            LocationId::new(),
            standard_set_id,
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        )
    }

    fn job(shifts: Vec<JobShift>, salaried_standards: Vec<SalariedStandard>) -> Job {
        Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            shifts,
            salaried_standards,
            Vec::new(),
        )
    }

    fn hours(results: &WorkResults) -> Vec<f64> {
        results
            .labor_data()
            .unwrap()
            .iter()
            .map(|labor_data| labor_data.hours())
            .collect()
    }

    #[test]
    fn a_job_with_no_shifts_yields_zero_hours_on_every_date() {
        let standard_set_id = StandardSetId::new();
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(Vec::new(), Vec::new()));

        assert_eq!(hours(&results), vec![0.0; 7]);
    }

    #[test]
    fn a_shift_with_a_standard_yields_the_calculated_hours() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(standard_set_id, every_day_definitions());
        let standard = standard(standard_set_id, shift.id());
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(vec![shift], vec![standard]));

        assert_eq!(hours(&results), vec![8.0; 7]);
    }

    #[test]
    fn hours_from_several_shifts_are_summed() {
        let standard_set_id = StandardSetId::new();
        let first = shift(standard_set_id, every_day_definitions());
        let second = shift(standard_set_id, every_day_definitions());
        let standards = vec![
            standard(standard_set_id, first.id()),
            standard(standard_set_id, second.id()),
        ];
        let planner_model = model(standard_set_id);

        let results =
            processor(8.0).process(&planner_model, &job(vec![first, second], standards));

        assert_eq!(hours(&results), vec![16.0; 7]);
    }

    #[test]
    fn a_date_the_shift_does_not_run_on_yields_zero_hours() {
        let standard_set_id = StandardSetId::new();
        // Runs on Mondays only.
        let shift = shift(
            standard_set_id,
            vec![JobShiftDefinition::new(
                DayOfWeek::Monday,
                LocalTime::of_hour_minute(9, 0),
                LocalTime::of_hour_minute(17, 0),
                0.0,
                0.0,
                1,
            )],
        );
        let standard = standard(standard_set_id, shift.id());
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(vec![shift], vec![standard]));

        assert_eq!(hours(&results), vec![8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_shift_definition_without_times_yields_zero_hours() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(
            standard_set_id,
            vec![JobShiftDefinition::without_times(DayOfWeek::Monday)],
        );
        let standard = standard(standard_set_id, shift.id());
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(vec![shift], vec![standard]));

        assert_eq!(hours(&results), vec![0.0; 7]);
    }

    #[test]
    fn a_shift_without_a_matching_standard_yields_zero_hours() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(standard_set_id, every_day_definitions());
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(vec![shift], Vec::new()));

        assert_eq!(hours(&results), vec![0.0; 7]);
    }

    #[test]
    fn a_shift_on_another_standard_set_is_ignored() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(StandardSetId::new(), every_day_definitions());
        let standard = standard(standard_set_id, shift.id());
        let planner_model = model(standard_set_id);

        let results = processor(8.0).process(&planner_model, &job(vec![shift], vec![standard]));

        assert_eq!(hours(&results), vec![0.0; 7]);
    }

    #[test]
    fn processing_a_single_date_yields_one_entry_for_that_date() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(standard_set_id, every_day_definitions());
        let standard = standard(standard_set_id, shift.id());
        let planner_model = model(standard_set_id);
        let date = LocalDate::new(2025, 10, 8);

        let results = processor(8.0).process_for_date(
            &planner_model,
            &job(vec![shift], vec![standard]),
            date,
        );

        let labor_data = results.labor_data().unwrap();
        assert_eq!(labor_data.len(), 1);
        assert_eq!(labor_data[0].date(), date);
        assert_eq!(labor_data[0].hours(), 8.0);
    }

    #[test]
    fn results_are_labelled_with_the_job() {
        let standard_set_id = StandardSetId::new();
        let planner_model = model(standard_set_id);
        let job = job(Vec::new(), Vec::new());

        let results = processor(8.0).process(&planner_model, &job);

        assert_eq!(results.job_id(), job.id());
        assert!(results.work_content().is_none());
    }
}
