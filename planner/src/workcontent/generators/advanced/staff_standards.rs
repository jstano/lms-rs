//! Work that scales with how many people are on, rather than with volume.
//!
//! Handover, briefings, till counts — work each body on shift does once. So the
//! standards are costed per shift and then multiplied by however many shifts
//! the day's coverage actually needs, which is only known once the non-staffing
//! work has been turned into bodies.

use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::plan_type::PlanType;
use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distributors::non_flowed_distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::work_total_minutes::calculate_minutes;
use crate::workcontent::generators::error::GenerationError;

/// The staffing work implied by a day's planned shifts.
///
/// `planned_shifts` are the provisional shifts the non-staffing bodies would
/// need — Java builds them inside this stage; here they are passed in, so this
/// does not have to know how they were arrived at.
pub fn process(
    params: &GeneratorParameters,
    providers: &Providers,
    planned_shifts: &[PlannedShift],
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    match number_of_shifts(planned_shifts) {
        Some(number_of_shifts) => handle_staff_work(params, providers, number_of_shifts),
        // No shifts means nobody to do the work.
        None => Ok(Vec::new()),
    }
}

/// How many shifts the staffing work should be multiplied by.
///
/// A projected plan writes the same shift under more than one plan type, so
/// counting them all would double up. The **largest** single plan type's count
/// is the real number of shifts; `None` when nothing is planned at all.
pub fn number_of_shifts(planned_shifts: &[PlannedShift]) -> Option<i32> {
    if planned_shifts.is_empty() {
        return None;
    }

    let mut counts: Vec<(Option<PlanType>, i32)> = Vec::new();

    for shift in planned_shifts {
        match counts.iter_mut().find(|(kind, _)| *kind == shift.shift_type()) {
            Some((_, count)) => *count += 1,
            None => counts.push((shift.shift_type(), 1)),
        }
    }

    counts.into_iter().map(|(_, count)| count).max()
}

/// Cost each staffing shape once, then multiply by the shift count.
pub fn handle_staff_work(
    params: &GeneratorParameters,
    providers: &Providers,
    number_of_shifts: i32,
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    let standards = params
        .job()
        .staff_shift_standards_for_standard_set_and_shift(
            params.standard_set_id(),
            params.shift().id(),
        );

    let mut shapes: Vec<Option<NonFlowedDistributionMethod>> = Vec::new();
    let mut totals: Vec<i32> = Vec::new();

    for standard in &standards {
        let shape = standard.non_flowed_distribution_method();
        let position = match shapes.iter().position(|existing| *existing == shape) {
            Some(position) => position,
            None => {
                shapes.push(shape);
                totals.push(0);
                shapes.len() - 1
            }
        };

        let driver_value = params
            .planner_model()
            .business_driver_value(standard.business_driver_id(), params.shift_date());

        // Note there is no openness gate here, unlike the non-flowed half of
        // the shift-related generator. Staffing work is owed for whoever is on
        // shift whether or not the driver is trading.
        //
        // Each standard's contribution is truncated as it is added, matching
        // Java's accumulation into an `int`.
        totals[position] += calculate_minutes(standard, driver_value, params, providers)? as i32;
    }

    Ok(shapes
        .into_iter()
        .zip(totals)
        .map(|(shape, total)| {
            let method =
                shape.unwrap_or(params.planner_settings().non_flowed_distribution_method);

            non_flowed_distributor(method).distribute(
                (total * number_of_shifts) as f64,
                None,
                params,
                providers,
            )
        })
        .collect())
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

    /// An 08:00-16:00 shift whose driver records ten covers.
    fn fixture(
        environment_id: EnvironmentId,
        business_driver_id: BusinessDriverId,
        make_standards: impl Fn(StandardSetId, JobShiftId, EnvironmentId) -> Vec<ShiftStandard>,
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
        let standards = make_standards(standard_set_id, shift.id(), environment_id);

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
            shift_detail: detail,
            shift,
        }
    }

    /// A staffing standard worth `minutes_per_unit` per unit of volume.
    fn staff_standard(
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        environment_id: EnvironmentId,
        business_driver_id: BusinessDriverId,
        minutes_per_unit: f64,
        shape: NonFlowedDistributionMethod,
    ) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            business_driver_id,
            WorkType::Staff,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::with_values(
                0,
                1000,
                vec![ShiftStandardValue::new(environment_id, minutes_per_unit)],
            )],
        )
        .with_non_flowed_distribution_method(shape)
    }

    /// A planned shift of the given plan type.
    fn planned_shift(shift_type: PlanType) -> PlannedShift {
        let mut shift = PlannedShift::new();
        shift.set_shift_type(Some(shift_type));
        shift
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    #[test]
    fn nothing_planned_means_no_staffing_work() {
        assert_eq!(number_of_shifts(&[]), None);
    }

    /// `StaffStandardsProcessorTest` — two forecast shifts and two original
    /// ones are the *same* two shifts written twice, so the count is two.
    #[test]
    fn the_shift_count_is_the_largest_plan_types_not_the_sum() {
        let shifts = vec![
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Original),
            planned_shift(PlanType::Original),
        ];

        assert_eq!(number_of_shifts(&shifts), Some(2));
    }

    #[test]
    fn an_uneven_split_takes_the_larger_side() {
        let shifts = vec![
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Original),
        ];

        assert_eq!(number_of_shifts(&shifts), Some(3));
    }

    #[test]
    fn staffing_work_is_multiplied_by_the_number_of_shifts() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            |standard_set_id, job_shift_id, environment_id| {
                // Two minutes per cover, at ten covers: twenty minutes a shift.
                vec![staff_standard(
                    standard_set_id,
                    job_shift_id,
                    environment_id,
                    business_driver_id,
                    2.0,
                    NonFlowedDistributionMethod::BEGINNING,
                )]
            },
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = handle_staff_work(&fixture.params(), &providers, 3)
            .expect("staffing standards are supported");

        assert_eq!(results.len(), 1);
        // Twenty minutes each across three shifts.
        assert_eq!(total_of(&results[0]), 60.0);
    }

    #[test]
    fn staffing_standards_are_totalled_by_shape_like_the_non_flowed_ones() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            |standard_set_id, job_shift_id, environment_id| {
                vec![
                    staff_standard(
                        standard_set_id,
                        job_shift_id,
                        environment_id,
                        business_driver_id,
                        1.0,
                        NonFlowedDistributionMethod::BEGINNING,
                    ),
                    staff_standard(
                        standard_set_id,
                        job_shift_id,
                        environment_id,
                        business_driver_id,
                        2.0,
                        NonFlowedDistributionMethod::BEGINNING,
                    ),
                    staff_standard(
                        standard_set_id,
                        job_shift_id,
                        environment_id,
                        business_driver_id,
                        3.0,
                        NonFlowedDistributionMethod::END,
                    ),
                ]
            },
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results =
            handle_staff_work(&fixture.params(), &providers, 1).expect("all supported");

        assert_eq!(results.len(), 2);
        // Thirty minutes at the beginning, thirty at the end.
        assert_eq!(total_of(&results[0]), 30.0);
        assert_eq!(total_of(&results[1]), 30.0);
    }

    #[test]
    fn a_shift_with_no_staffing_standards_produces_nothing() {
        let environment_id = EnvironmentId::new();
        let fixture = fixture(environment_id, BusinessDriverId::new(), |_, _, _| Vec::new());
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        assert!(
            handle_staff_work(&fixture.params(), &providers, 3)
                .expect("nothing to do")
                .is_empty()
        );
    }

    #[test]
    fn processing_with_no_planned_shifts_does_no_staffing_work() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            |standard_set_id, job_shift_id, environment_id| {
                vec![staff_standard(
                    standard_set_id,
                    job_shift_id,
                    environment_id,
                    business_driver_id,
                    2.0,
                    NonFlowedDistributionMethod::BEGINNING,
                )]
            },
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        assert!(
            process(&fixture.params(), &providers, &[])
                .expect("nothing to do")
                .is_empty()
        );
    }

    #[test]
    fn processing_multiplies_by_the_shifts_it_was_given() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(
            environment_id,
            business_driver_id,
            |standard_set_id, job_shift_id, environment_id| {
                vec![staff_standard(
                    standard_set_id,
                    job_shift_id,
                    environment_id,
                    business_driver_id,
                    2.0,
                    NonFlowedDistributionMethod::BEGINNING,
                )]
            },
        );
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        // Two shifts, each written under both plan types.
        let planned = vec![
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Forecast),
            planned_shift(PlanType::Original),
            planned_shift(PlanType::Original),
        ];

        let results =
            process(&fixture.params(), &providers, &planned).expect("all supported");

        // Twenty minutes a shift across the two real shifts, not four.
        assert_eq!(total_of(&results[0]), 40.0);
    }
}
