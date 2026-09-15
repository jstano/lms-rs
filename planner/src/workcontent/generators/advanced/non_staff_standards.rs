//! Gathers every source of non-staffing work for one shift.
//!
//! Three producers feed this: the shift's own standards, its spread standards,
//! and whatever recurring tasks fall due today. Their arrays are collected
//! unaggregated — combining them is the next stage's job.

use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::{
    recurring_standards, shift_related_standards, spread_standards,
};
use crate::workcontent::generators::error::GenerationError;

/// Every non-staffing array this shift produces.
///
/// The recurring tasks always contribute exactly one array, even when nothing
/// is due — the other two contribute one per shape or per standard.
pub fn process(
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    let mut results = shift_related_standards::generate(params, providers)?;

    results.extend(spread_standards::generate(params, providers));
    results.push(recurring_standards::generate(params, providers));

    Ok(results)
}

/// What the non-staffing half of the pipeline hands on.
///
/// The minutes are kept alongside the head count because the staffing stage
/// needs the bodies while the combining stage needs the minutes — and the two
/// are not recoverable from one another.
#[derive(Debug, Clone, PartialEq)]
pub struct NonStaffResults {
    non_staff_work_minutes_per_period: Vec<DistributionItem>,
    non_staff_bodies_after_breaks_applied: Vec<DistributionItem>,
}

impl NonStaffResults {
    pub fn new(
        non_staff_work_minutes_per_period: Vec<DistributionItem>,
        non_staff_bodies_after_breaks_applied: Vec<DistributionItem>,
    ) -> Self {
        Self {
            non_staff_work_minutes_per_period,
            non_staff_bodies_after_breaks_applied,
        }
    }

    pub fn non_staff_work_minutes_per_period(&self) -> &[DistributionItem] {
        &self.non_staff_work_minutes_per_period
    }

    pub fn non_staff_bodies_after_breaks_applied(&self) -> &[DistributionItem] {
        &self.non_staff_bodies_after_breaks_applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::distribution_method::DistributionMethod;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::{Job, JobId};
    use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::recurring_task_standard::{
        DurationType, FrequencyType, MonthlyIntervalType, RecurringTaskStandard,
    };
    use crate::workcontent::domain::shift_standard::{
        ShiftStandard, ShiftStandardRange, ShiftStandardValue,
    };
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::domain::work_type::WorkType;
    use crate::workcontent::generators::advanced::providers::EnvironmentResolver;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDate, LocalTime};
    use std::collections::HashMap;

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    struct OneEnvironment(EnvironmentId);

    impl EnvironmentResolver for OneEnvironment {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            Some(self.0)
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

    /// An 08:00-16:00 shift carrying one non-flowed standard and, optionally,
    /// one recurring task.
    fn fixture(
        environment_id: EnvironmentId,
        business_driver_id: BusinessDriverId,
        with_recurring_task: bool,
    ) -> Fixture {
        let standard_set_id = StandardSetId::new();
        let detail = JobShiftDefinition::new(
            DayOfWeek::Wednesday,
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            0.0,
            0.0,
            1,
        );
        let shift = JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            vec![detail],
        );

        let standards = vec![
            ShiftStandard::new(
                JobId::new(),
                standard_set_id,
                shift.id(),
                business_driver_id,
                WorkType::Variable,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::with_values(
                    0,
                    1000,
                    vec![ShiftStandardValue::new(environment_id, 1.0)],
                )],
            )
            .distributed_by(DistributionMethod::NonFlowed)
            .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING),
        ];

        let tasks = if with_recurring_task {
            vec![RecurringTaskStandard::new(
                JobId::new(),
                standard_set_id,
                None,
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
                Some(shift.id()),
                DurationType::Fixed,
                1.0,
                Vec::new(),
            )]
        } else {
            Vec::new()
        };

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
                        HashMap::from([(date(), 60)]),
                    ),
                )]),
            ),
            job: Job::new(
                LocationId::new(),
                PlannerSettings::default(),
                Vec::new(),
                Vec::new(),
                standards,
            )
            .with_flowed_standards(Vec::new(), tasks, Vec::new()),
            planner_settings,
            shift_detail: detail,
            shift,
        }
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    /// The recurring array is pushed whether or not anything is due, so a shift
    /// with one standard and no tasks still comes back with two arrays — the
    /// second of them empty.
    #[test]
    fn the_recurring_array_is_always_present_even_when_empty() {
        let environment_id = EnvironmentId::new();
        let fixture = fixture(environment_id, BusinessDriverId::new(), false);
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = process(&fixture.params(), &providers).expect("all supported");

        assert_eq!(results.len(), 2);
        assert_eq!(total_of(&results[0]), 60.0);
        assert!(results[1].is_empty());
    }

    #[test]
    fn every_producer_contributes_its_own_arrays() {
        let environment_id = EnvironmentId::new();
        let fixture = fixture(environment_id, BusinessDriverId::new(), true);
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = process(&fixture.params(), &providers).expect("all supported");

        assert_eq!(results.len(), 2);
        // The standard's sixty minutes...
        assert_eq!(total_of(&results[0]), 60.0);
        // ...and the recurring task's hour.
        assert_eq!(total_of(&results[1]), 60.0);
    }

    #[test]
    fn the_results_carry_both_the_minutes_and_the_bodies() {
        let minutes = vec![DistributionItem::new(
            0,
            date().at_start_of_day(),
            30.0,
        )];
        let bodies = vec![DistributionItem::new(0, date().at_start_of_day(), 1.0)];

        let results = NonStaffResults::new(minutes.clone(), bodies.clone());

        assert_eq!(results.non_staff_work_minutes_per_period(), minutes);
        assert_eq!(results.non_staff_bodies_after_breaks_applied(), bodies);
    }
}
