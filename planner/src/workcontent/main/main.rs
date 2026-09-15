use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::work_generators;
use crate::workcontent::generators::work_generators::WorkResults;

/// Plan every job in the model.
///
/// A job the engine cannot plan fails the whole run rather than being skipped:
/// a misconfigured standard is a problem to surface, not to quietly drop work
/// over.
pub fn generate_work_content(
    planner_model: PlannerModel,
) -> Result<Vec<WorkResults>, GenerationError> {
    planner_model
        .jobs()
        .iter()
        .map(|job| {
            let work_generator = work_generators::create(job.planner_settings().standard_type);
            work_generator.generate_work(&planner_model, job)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::Job;
    use crate::workcontent::domain::location::{Location, LocationId};
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::standard_set::{StandardSet, StandardSetId};
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    fn planner_model(jobs: Vec<Job>) -> PlannerModel {
        let dates = DateRange::new(LocalDate::new(2025, 10, 1), LocalDate::new(2025, 10, 31));
        let location = Location::new(LocationId::new());
        let standard_set = StandardSet::new(StandardSetId::new());

        PlannerModel::new(
            dates,
            PlannerMode::Standard,
            location.id(),
            standard_set.id(),
            jobs,
            vec![],
            HashMap::new(),
        )
    }

    #[test]
    fn should_generate_no_work_results_if_no_jobs() {
        let results = generate_work_content(planner_model(vec![])).expect("the fixture is configured to plan");

        assert!(results.is_empty());
    }

    #[test]
    fn should_generate_one_work_result_per_job() {
        let results = generate_work_content(planner_model(vec![Job::test(), Job::test()])).expect("the fixture is configured to plan");

        assert_eq!(results.len(), 2);
    }

    /// The generators are chosen per job, so one model can mix them.
    mod end_to_end {
        use super::*;
        use crate::workcontent::domain::business_driver::BusinessDriverId;
        use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
        use crate::workcontent::domain::job::JobId;
        use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition};
        use crate::workcontent::domain::planner_settings::PlannerSettings;
        use crate::workcontent::domain::salaried_standard::SalariedStandard;
        use crate::workcontent::domain::salary_mode::SalaryMode;
        use crate::workcontent::domain::shift_standard::{ShiftStandard, ShiftStandardRange};
        use crate::workcontent::domain::standard_type::StandardType;
        use crate::workcontent::domain::units::Units;
        use crate::workcontent::domain::work_type::WorkType;
        use joda_rs::{DayOfWeek, LocalTime};

        // 2025-10-06 is a Monday; the window is that single week.
        fn monday() -> LocalDate {
            LocalDate::new(2025, 10, 6)
        }

        fn settings(standard_type: StandardType) -> PlannerSettings {
            let mut settings = PlannerSettings::default();
            settings.standard_type = standard_type;
            settings.period_length = 15;
            settings.min_shift_length = 0.0;
            settings.max_shift_length = 8.0;
            settings.rounding_threshold_below_one = 0.0;
            settings.rounding_threshold_above_one = 0.0;
            settings
        }

        /// A 24-hour template running on Mondays only.
        fn monday_shift(standard_set_id: StandardSetId) -> JobShift {
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

        fn basic_job(
            standard_set_id: StandardSetId,
            business_driver_id: BusinessDriverId,
        ) -> Job {
            let shift = monday_shift(standard_set_id);
            let shift_id = shift.id();

            Job::new(
                LocationId::new(),
                settings(StandardType::BASIC),
                vec![shift],
                Vec::new(),
                // One minute of work per unit of driver volume.
                vec![ShiftStandard::new(
                    JobId::new(),
                    standard_set_id,
                    shift_id,
                    business_driver_id,
                    WorkType::Variable,
                    Units::MinutesPerUnit,
                    0,
                    vec![ShiftStandardRange::new(0, 100_000, 1.0)],
                )],
            )
        }

        fn salaried_job(standard_set_id: StandardSetId) -> Job {
            let shift = monday_shift(standard_set_id);
            let shift_id = shift.id();

            Job::new(
                LocationId::new(),
                settings(StandardType::SALARIED),
                vec![shift],
                vec![SalariedStandard::new(
                    JobId::new(),
                    standard_set_id,
                    shift_id,
                    SalaryMode::WEEKLY,
                    Some(42.0),
                    None,
                    None,
                )],
                Vec::new(),
            )
        }

        fn model(
            standard_set_id: StandardSetId,
            business_driver_id: BusinessDriverId,
            jobs: Vec<Job>,
        ) -> PlannerModel {
            PlannerModel::new(
                DateRange::new(monday(), LocalDate::new(2025, 10, 12)),
                PlannerMode::Standard,
                LocationId::new(),
                standard_set_id,
                jobs,
                Vec::new(),
                HashMap::from([(
                    business_driver_id,
                    BusinessDriverValues::new(
                        business_driver_id,
                        // 18 hours of work on the Monday.
                        HashMap::from([(monday(), 18 * 60)]),
                    ),
                )]),
            )
        }

        #[test]
        fn a_basic_job_produces_work_content_and_matching_planned_shifts() {
            let standard_set_id = StandardSetId::new();
            let business_driver_id = BusinessDriverId::new();
            let job = basic_job(standard_set_id, business_driver_id);
            let job_id = job.id();

            let results =
                generate_work_content(model(standard_set_id, business_driver_id, vec![job])).expect("the fixture is configured to plan");

            assert_eq!(results.len(), 1);
            assert_eq!(results[0].job_id(), job_id);

            // 18 hours against 8-hour shifts: two full shifts plus 2 hours.
            let work_content = results[0].work_content().unwrap();
            assert_eq!(work_content.len(), 3);
            assert_eq!(
                work_content
                    .iter()
                    .map(|wc| wc.calculated_hours())
                    .collect::<Vec<_>>(),
                vec![8.0, 8.0, 2.0]
            );
            assert!(work_content.iter().all(|wc| wc.shift_date() == monday()));

            // One planned shift mirrors each block.
            let planned_shifts = results[0].planned_shifts().unwrap();
            assert_eq!(planned_shifts.len(), 3);
            for (shift, wc) in planned_shifts.iter().zip(work_content) {
                assert_eq!(shift.duration(), wc.calculated_hours());
                assert_eq!(shift.start_date_time(), Some(wc.calculated_start_date_time()));
                assert_eq!(shift.end_date_time(), Some(wc.calculated_end_date_time()));
            }

            assert!(results[0].labor_data().is_none());
        }

        #[test]
        fn a_salaried_job_produces_labor_data_for_every_date() {
            let standard_set_id = StandardSetId::new();
            let business_driver_id = BusinessDriverId::new();
            let job = salaried_job(standard_set_id);
            let job_id = job.id();

            let results =
                generate_work_content(model(standard_set_id, business_driver_id, vec![job])).expect("the fixture is configured to plan");

            assert_eq!(results.len(), 1);
            assert_eq!(results[0].job_id(), job_id);

            // One entry per date in the window, with hours only on the Monday.
            let labor_data = results[0].labor_data().unwrap();
            assert_eq!(labor_data.len(), 7);
            assert_eq!(
                labor_data
                    .iter()
                    .map(|ld| ld.hours())
                    .collect::<Vec<_>>(),
                vec![6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
            );

            assert!(results[0].work_content().is_none());
        }

        #[test]
        fn each_job_is_generated_by_the_generator_its_settings_name() {
            let standard_set_id = StandardSetId::new();
            let business_driver_id = BusinessDriverId::new();

            let results = generate_work_content(model(
                standard_set_id,
                business_driver_id,
                vec![
                    basic_job(standard_set_id, business_driver_id),
                    salaried_job(standard_set_id),
                    Job::test(),
                ],
            )).expect("the fixture is configured to plan");

            assert_eq!(results.len(), 3);
            // Basic plans blocks, salaried records hours, none does neither.
            assert_eq!(results[0].work_content().unwrap().len(), 3);
            assert_eq!(results[1].labor_data().unwrap().len(), 7);
            assert!(results[2].work_content().unwrap().is_empty());
        }

        #[test]
        fn a_job_whose_shift_is_on_another_standard_set_plans_nothing() {
            let standard_set_id = StandardSetId::new();
            let business_driver_id = BusinessDriverId::new();
            let job = basic_job(StandardSetId::new(), business_driver_id);

            let results =
                generate_work_content(model(standard_set_id, business_driver_id, vec![job])).expect("the fixture is configured to plan");

            assert!(results[0].work_content().unwrap().is_empty());
            assert!(results[0].planned_shifts().unwrap().is_empty());
        }
    }
}
