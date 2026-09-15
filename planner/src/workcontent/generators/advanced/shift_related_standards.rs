//! Turns a shift's standards into distributed work.
//!
//! Standards fall into two camps and are treated differently in a way worth
//! knowing about: non-flowed ones are **summed by shape first and distributed
//! once**, so five standards all set to BEGINNING produce a single block of
//! work rather than five overlapping ones. Everything else is distributed
//! standard by standard.

use crate::workcontent::domain::distribution_method::DistributionMethod;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::domain::work_type::WorkType;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distributors::{get_distributor, non_flowed_distributor};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::work_total_minutes::calculate_minutes;
use crate::workcontent::generators::error::GenerationError;

/// One distributed array per non-flowed shape, plus one per other standard.
pub fn generate(
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    let standards = params
        .job()
        .non_staff_shift_standards_for_standard_set_and_shift(
            params.standard_set_id(),
            params.shift().id(),
        );

    let mut results = handle_non_flowed_standards(&standards, params, providers)?;

    results.extend(handle_other_standards(&standards, params, providers)?);

    Ok(results)
}

/// Sum each shape's standards into one total, then distribute that once.
fn handle_non_flowed_standards(
    standards: &[&ShiftStandard],
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    // Grouped by the standard's *own* shape, including "none set" as a group
    // of its own — two standards that both leave it unset belong together,
    // even though they will end up distributed by the plan's default.
    let mut shapes: Vec<Option<NonFlowedDistributionMethod>> = Vec::new();
    let mut totals: Vec<f64> = Vec::new();

    for standard in standards
        .iter()
        .filter(|standard| standard.distribution_method() == DistributionMethod::NonFlowed)
    {
        let shape = standard.non_flowed_distribution_method();
        let position = match shapes.iter().position(|existing| *existing == shape) {
            Some(position) => position,
            None => {
                shapes.push(shape);
                totals.push(0.0);
                shapes.len() - 1
            }
        };

        let driver_value = driver_value_for(standard, params);

        // Non-flowed standards only count when their driver is trading.
        if providers.openness.is_open(
            standard.business_driver_id(),
            params.shift_date(),
            driver_value,
        ) {
            totals[position] += calculate_minutes(standard, driver_value, params, providers)?;
        }
    }

    Ok(shapes
        .into_iter()
        .zip(totals)
        .map(|(shape, total)| {
            let method = shape
                .unwrap_or(params.planner_settings().non_flowed_distribution_method);

            non_flowed_distributor(method).distribute(total, None, params, providers)
        })
        .collect())
}

/// Distribute every other standard on its own.
fn handle_other_standards(
    standards: &[&ShiftStandard],
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    let mut results = Vec::new();

    for standard in standards
        .iter()
        .filter(|standard| standard.distribution_method() != DistributionMethod::NonFlowed)
    {
        let driver_value = driver_value_for(standard, params);

        // Note the asymmetry with the non-flowed half above: these are costed
        // whether or not the driver is trading. Confirmed against Java, not an
        // oversight here.
        let work_total = calculate_minutes(standard, driver_value, params, providers)?;

        // Fill-gaps standards are dropped in silence rather than reported —
        // the guard sits before the distributor is ever asked for, so the
        // unimplemented-method error is unreachable from this path.
        if standard.distribution_method() == DistributionMethod::FillGaps {
            continue;
        }

        results.push(
            get_distributor(standard, params)?
                .distribute(work_total, Some(standard), params, providers),
        );
    }

    Ok(results)
}

/// The volume a standard is costed against.
///
/// Weekly standards are stated per week, so they read the whole week's volume
/// rather than the day's.
fn driver_value_for(standard: &ShiftStandard, params: &GeneratorParameters) -> i32 {
    let business_driver_id = standard.business_driver_id();
    let planner_model = params.planner_model();

    if standard.work_type() == WorkType::Weekly {
        planner_model.business_driver_value_for_week(
            business_driver_id,
            params.shift_date(),
            planner_model.week_ending_day(),
        )
    } else {
        planner_model.business_driver_value(business_driver_id, params.shift_date())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::{Job, JobId};
    use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::shift_standard::{ShiftStandardRange, ShiftStandardValue};
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::generators::advanced::providers::{
        BusinessDriverOpenness, EnvironmentResolver,
    };
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

    /// Every driver trading, or none of them.
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

    /// The pieces a generation borrows, kept alive together.
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

    /// An 08:00-16:00 shift whose driver records ten covers a day.
    fn fixture(
        environment_id: EnvironmentId,
        business_driver_id: BusinessDriverId,
        standards: Vec<ShiftStandard>,
    ) -> Fixture {
        let standard_set_id = StandardSetId::new();
        let job_id = JobId::new();
        let shift = JobShift::new(
            job_id,
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

        // Every standard is attached to this shift and standard set.
        let standards = standards
            .into_iter()
            .map(|standard| rebind(standard, standard_set_id, shift.id(), environment_id))
            .collect();

        let mut planner_settings = PlannerSettings::default();
        planner_settings.period_length = 30;
        planner_settings.non_flowed_distribution_method = NonFlowedDistributionMethod::BEGINNING;

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
                        HashMap::from([(date(), 10)]),
                    ),
                )]),
            ),
            job: Job::new(
                LocationId::new(),
                PlannerSettings::default(),
                Vec::new(),
                Vec::new(),
                standards,
            ),
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

    /// Rebuild a standard against the fixture's ids, keeping its settings.
    fn rebind(
        standard: ShiftStandard,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        environment_id: EnvironmentId,
    ) -> ShiftStandard {
        let rebound = ShiftStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            standard.business_driver_id(),
            standard.work_type(),
            standard.units(),
            standard.suppress_value(),
            vec![ShiftStandardRange::with_values(
                0,
                1000,
                vec![ShiftStandardValue::new(
                    environment_id,
                    standard.value_for_volume(10).unwrap_or(0.0),
                )],
            )],
        )
        .distributed_by(standard.distribution_method());

        match standard.non_flowed_distribution_method() {
            Some(method) => rebound.with_non_flowed_distribution_method(method),
            None => rebound,
        }
    }

    /// A standard worth `minutes_per_unit` minutes for each unit of volume.
    fn standard(
        business_driver_id: BusinessDriverId,
        minutes_per_unit: f64,
        method: DistributionMethod,
        shape: Option<NonFlowedDistributionMethod>,
    ) -> ShiftStandard {
        let standard = ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::new(0, 1000, minutes_per_unit)],
        )
        .distributed_by(method);

        match shape {
            Some(shape) => standard.with_non_flowed_distribution_method(shape),
            None => standard,
        }
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    #[test]
    fn a_shift_with_no_standards_generates_nothing() {
        let environment_id = EnvironmentId::new();
        let fixture = fixture(environment_id, BusinessDriverId::new(), Vec::new());
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        assert!(
            generate(&fixture.params(), &providers)
                .expect("nothing to generate")
                .is_empty()
        );
    }

    /// Non-flowed standards sharing a shape are totalled *before* they are
    /// distributed, so each shape yields one array rather than one per
    /// standard. Everything else gets an array to itself.
    #[test]
    fn non_flowed_standards_are_totalled_by_shape_and_other_standards_are_not() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            vec![
                // Two at the beginning: 10 units × 1 and × 2 = 30 minutes.
                standard(
                    business_driver_id,
                    1.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::BEGINNING),
                ),
                standard(
                    business_driver_id,
                    2.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::BEGINNING),
                ),
                // Two at the end: 10 × 3 and × 4 = 70 minutes.
                standard(
                    business_driver_id,
                    3.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::END),
                ),
                standard(
                    business_driver_id,
                    4.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::END),
                ),
                // One opening standard, distributed on its own: 10 × 5.
                standard(business_driver_id, 5.0, DistributionMethod::Opening, None),
            ],
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = generate(&fixture.params(), &providers).expect("all supported");

        // Two non-flowed shapes plus the one opening standard.
        assert_eq!(results.len(), 3);
        assert_eq!(total_of(&results[0]), 30.0);
        assert_eq!(total_of(&results[1]), 70.0);
    }

    /// A driver that is not trading contributes nothing to the non-flowed
    /// total — but the other standards are costed regardless, which is the
    /// asymmetry the implementation flags.
    #[test]
    fn a_closed_driver_silences_non_flowed_standards_but_not_the_others() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            vec![
                standard(
                    business_driver_id,
                    1.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::BEGINNING),
                ),
                standard(business_driver_id, 5.0, DistributionMethod::Opening, None),
            ],
        );
        let environments = OneEnvironment(environment_id);
        let closed = Trading(false);
        let providers = Providers {
            environments: &environments,
            openness: &closed,
            ..Providers::none()
        };

        let results = generate(&fixture.params(), &providers).expect("all supported");

        assert_eq!(results.len(), 2);
        // The non-flowed group is silenced...
        assert_eq!(total_of(&results[0]), 0.0);
        // ...while the opening standard is still costed at 50 minutes.
        assert_eq!(total_of(&results[1]), 50.0);
    }

    #[test]
    fn fill_gaps_standards_are_dropped_without_being_reported() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            vec![standard(
                business_driver_id,
                1.0,
                DistributionMethod::FillGaps,
                None,
            )],
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        // No error, and nothing planned.
        assert!(
            generate(&fixture.params(), &providers)
                .expect("fill-gaps is skipped, not reported")
                .is_empty()
        );
    }

    #[test]
    fn a_weekly_standard_is_costed_against_the_whole_week() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let mut fixture = fixture(
            environment_id,
            business_driver_id,
            vec![{
                let standard = standard(
                    business_driver_id,
                    1.0,
                    DistributionMethod::NonFlowed,
                    Some(NonFlowedDistributionMethod::BEGINNING),
                );
                // Re-made as a weekly standard.
                ShiftStandard::new(
                    JobId::new(),
                    StandardSetId::new(),
                    JobShiftId::new(),
                    business_driver_id,
                    WorkType::Weekly,
                    standard.units(),
                    0,
                    vec![ShiftStandardRange::new(0, 1000, 1.0)],
                )
                .distributed_by(DistributionMethod::NonFlowed)
                .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING)
            }],
        );

        // Three days of the same week record ten covers each.
        fixture.planner_model = PlannerModel::new(
            DateRange::new(date(), date()),
            PlannerMode::Standard,
            LocationId::new(),
            fixture.shift.standard_set_id(),
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                business_driver_id,
                BusinessDriverValues::new(
                    business_driver_id,
                    HashMap::from([
                        (date(), 10),
                        (date().plus_days(1), 10),
                        (date().plus_days(2), 10),
                    ]),
                ),
            )]),
        );

        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = generate(&fixture.params(), &providers).expect("all supported");

        // Thirty covers across the week at a minute each, not the day's ten.
        assert_eq!(total_of(&results[0]), 30.0);
    }

    #[test]
    fn a_task_standard_anywhere_in_the_shift_is_reported() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            vec![ShiftStandard::new(
                JobId::new(),
                StandardSetId::new(),
                JobShiftId::new(),
                business_driver_id,
                WorkType::Task,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 1000, 1.0)],
            )],
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        assert_eq!(
            generate(&fixture.params(), &providers),
            Err(GenerationError::TaskStandardsNotSupported)
        );
    }
}
