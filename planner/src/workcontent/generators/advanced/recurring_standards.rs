//! Adds the work of tasks that come round on a schedule.
//!
//! Recurring tasks — a deep clean, a stock count — are not driven by volume the
//! way standards are; they simply fall due on certain dates. Whatever is due
//! today is totalled and placed at the start of the shift.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::recurring_task_standard::{DurationType, RecurringTaskStandard};
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distributors::non_flowed_distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use joda_rs::constants::MINUTES_PER_HOUR;

/// A single array of the day's recurring work, empty when nothing is due.
///
/// One array rather than one per task: they are totalled before being placed.
pub fn generate(
    params: &GeneratorParameters,
    providers: &Providers,
) -> Vec<DistributionItem> {
    let planner_model = params.planner_model();
    let due_today = params.job().recurring_task_standards_for_shift_and_date(
        params.standard_set_id(),
        params.shift().id(),
        params.shift_date(),
        planner_model.week_ending_day(),
    );

    if due_today.is_empty() {
        return Vec::new();
    }

    let total_work_minutes = total_minutes_for(&due_today, params, providers);

    if total_work_minutes <= 0 {
        return Vec::new();
    }

    // Always at the beginning of the shift, whatever the plan's default shape
    // is — recurring work is not configurable this way.
    non_flowed_distributor(NonFlowedDistributionMethod::BEGINNING).distribute(
        total_work_minutes as f64,
        None,
        params,
        providers,
    )
}

/// The minutes every task due today adds up to.
///
/// Each task's hours are rounded to whole minutes as they are added, rather
/// than the total being rounded once at the end.
fn total_minutes_for(
    standards: &[&RecurringTaskStandard],
    params: &GeneratorParameters,
    providers: &Providers,
) -> i32 {
    let mut total = 0;

    for standard in standards {
        let hours = match standard.duration_type() {
            // Fixed work happens whether or not the driver is trading — it is
            // not driven by volume at all.
            DurationType::Fixed => Some(standard.fixed_hours_with_formula().total_work_hours),
            DurationType::Variable => variable_hours_for(standard, params, providers),
        };

        if let Some(hours) = hours {
            total += numbers::round(hours * MINUTES_PER_HOUR as f64);
        }
    }

    total
}

/// The hours a volume-driven task adds, or `None` when its driver is closed.
fn variable_hours_for(
    standard: &RecurringTaskStandard,
    params: &GeneratorParameters,
    providers: &Providers,
) -> Option<f64> {
    let business_driver_id = standard.business_driver_id()?;
    let driver_value = params
        .planner_model()
        .business_driver_value(business_driver_id, params.shift_date());

    providers
        .openness
        .is_open(business_driver_id, params.shift_date(), driver_value)
        .then(|| {
            standard
                .total_variable_hours_with_formula(driver_value)
                .total_work_hours
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::job::{Job, JobId};
    use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::recurring_task_standard::{
        FrequencyType, MonthlyIntervalType, RecurringVariableWork,
    };
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::generators::advanced::providers::BusinessDriverOpenness;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDate, LocalTime};
    use std::collections::HashMap;

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    struct Trading(bool);

    impl BusinessDriverOpenness for Trading {
        fn is_open(
            &self,
            _business_driver_id: BusinessDriverId,
            _date: LocalDate,
            _driver_value: i32,
        ) -> bool {
            self.0
        }
    }

    struct Fixture {
        planner_model: PlannerModel,
        job: Job,
        planner_settings: PlannerSettings,
        shift: JobShift,
        shift_detail: JobShiftDefinition,
    }

    impl Fixture {
        fn params(&self) -> GeneratorParameters<'_> {
            GeneratorParameters::new(
                &self.planner_model,
                &self.job,
                &self.planner_settings,
                date(),
                &self.shift,
                &self.shift_detail,
            )
        }
    }

    /// A task due every day from the start of the month.
    fn task(
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: Option<BusinessDriverId>,
        duration_type: DurationType,
        fixed_hours: f64,
        variable_works: Vec<RecurringVariableWork>,
    ) -> RecurringTaskStandard {
        RecurringTaskStandard::new(
            JobId::new(),
            standard_set_id,
            business_driver_id,
            "Task".to_string(),
            LocalDate::new(2013, 7, 1),
            FrequencyType::Daily,
            1,
            1,
            Vec::new(),
            MonthlyIntervalType::DayNOfEveryMonth,
            1,
            1,
            None,
            Vec::new(),
            Some(job_shift_id),
            duration_type,
            fixed_hours,
            variable_works,
        )
    }

    /// An 08:00-16:00 shift whose driver records a hundred covers.
    fn fixture(
        business_driver_id: BusinessDriverId,
        make_tasks: impl Fn(StandardSetId, JobShiftId) -> Vec<RecurringTaskStandard>,
    ) -> Fixture {
        let standard_set_id = StandardSetId::new();
        let shift = JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            vec![JobShiftDefinition::new(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
                0.0,
                0.0,
                1,
            )],
        );
        let tasks = make_tasks(standard_set_id, shift.id());

        let mut planner_settings = PlannerSettings::default();
        planner_settings.period_length = 30;

        Fixture {
            planner_model: PlannerModel::new(
                DateRange::new(date(), date()),
                PlannerMode::Standard,
                LocationId::new(),
                standard_set_id,
                Vec::new(),
                Vec::new(),
                HashMap::from([(
                    business_driver_id,
                    BusinessDriverValues::new(
                        business_driver_id,
                        HashMap::from([(date(), 100)]),
                    ),
                )]),
            ),
            job: Job::test().with_flowed_standards(Vec::new(), tasks, Vec::new()),
            planner_settings,
            shift_detail: JobShiftDefinition::new(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
                0.0,
                0.0,
                1,
            ),
            shift,
        }
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    #[test]
    fn nothing_due_today_generates_nothing() {
        let fixture = fixture(BusinessDriverId::new(), |_, _| Vec::new());

        assert!(generate(&fixture.params(), &Providers::none()).is_empty());
    }

    #[test]
    fn a_fixed_task_adds_its_hours_at_the_start_of_the_shift() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![task(
                standard_set_id,
                job_shift_id,
                None,
                DurationType::Fixed,
                2.0,
                Vec::new(),
            )]
        });

        let items = generate(&fixture.params(), &Providers::none());

        assert_eq!(total_of(&items), 120.0);
        // Placed from 08:00, which is period 16.
        assert_eq!(items[16].value_per_period(), 30.0);
    }

    #[test]
    fn several_tasks_are_totalled_into_one_block() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![
                task(
                    standard_set_id,
                    job_shift_id,
                    None,
                    DurationType::Fixed,
                    1.0,
                    Vec::new(),
                ),
                task(
                    standard_set_id,
                    job_shift_id,
                    None,
                    DurationType::Fixed,
                    0.5,
                    Vec::new(),
                ),
            ]
        });

        let items = generate(&fixture.params(), &Providers::none());

        assert_eq!(total_of(&items), 90.0);
    }

    #[test]
    fn a_variable_task_scales_with_its_drivers_volume() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            // One base hour plus half an hour per hundred covers; the driver
            // records exactly a hundred.
            vec![task(
                standard_set_id,
                job_shift_id,
                Some(business_driver_id),
                DurationType::Variable,
                0.0,
                vec![RecurringVariableWork::new(0, 1000, 1.0, 0.5, 100)],
            )]
        });

        let items = generate(&fixture.params(), &Providers::none());

        assert_eq!(total_of(&items), 90.0);
    }

    /// Volume-driven tasks fall away when the driver is not trading; fixed ones
    /// do not, because they were never driven by volume.
    #[test]
    fn a_closed_driver_silences_variable_tasks_but_not_fixed_ones() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![
                task(
                    standard_set_id,
                    job_shift_id,
                    Some(business_driver_id),
                    DurationType::Variable,
                    0.0,
                    vec![RecurringVariableWork::new(0, 1000, 1.0, 0.5, 100)],
                ),
                task(
                    standard_set_id,
                    job_shift_id,
                    None,
                    DurationType::Fixed,
                    2.0,
                    Vec::new(),
                ),
            ]
        });
        let closed = Trading(false);
        let providers = Providers {
            openness: &closed,
            ..Providers::none()
        };

        let items = generate(&fixture.params(), &providers);

        // Only the fixed task's two hours survive.
        assert_eq!(total_of(&items), 120.0);
    }

    #[test]
    fn a_task_worth_no_time_generates_nothing() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![task(
                standard_set_id,
                job_shift_id,
                None,
                DurationType::Fixed,
                0.0,
                Vec::new(),
            )]
        });

        assert!(generate(&fixture.params(), &Providers::none()).is_empty());
    }

    #[test]
    fn a_task_on_another_shift_is_not_due_here() {
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, _| {
            vec![task(
                standard_set_id,
                JobShiftId::new(),
                None,
                DurationType::Fixed,
                2.0,
                Vec::new(),
            )]
        });

        assert!(generate(&fixture.params(), &Providers::none()).is_empty());
    }
}
