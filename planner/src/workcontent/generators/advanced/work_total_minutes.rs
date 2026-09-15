//! How many minutes of work one standard asks for on one date.
//!
//! The flowed path's counterpart to the non-flowed
//! [`total_work_minutes_calculator`](crate::workcontent::generators::basic::total_work_minutes_calculator):
//! same per-unit arithmetic underneath, but the standard's value is resolved
//! against the operating environment rather than taken flat.

use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::domain::work_type::WorkType;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::basic::total_work_minutes_calculator::calculate_work_minutes_per_unit;
use crate::workcontent::generators::error::GenerationError;

/// The work `standard` requires at `driver_value`.
///
/// Zero when no band covers the volume, or none of the bands that do hold a
/// value for the date's environment.
pub fn calculate_minutes(
    standard: &ShiftStandard,
    driver_value: i32,
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<f64, GenerationError> {
    // Task standards are real production behaviour but their frequency and
    // expectancy model is not ported. Reported rather than silently costed at
    // nothing — which is what Java does, and would be wrong-but-quiet here.
    if standard.work_type() == WorkType::Task {
        return Err(GenerationError::TaskStandardsNotSupported);
    }

    let Some(environment_id) =
        providers.environment_for(standard.business_driver_id(), params.shift_date())
    else {
        return Ok(0.0);
    };

    let Some(standard_value) =
        standard.value_for_volume_in_environment(driver_value, environment_id)
    else {
        return Ok(0.0);
    };

    calculate_work_minutes_per_unit(
        standard.units(),
        standard_value,
        standard.suppressed_driver_value(driver_value),
        shift_length_for_standards(params),
    )
}

/// The shift length standards are costed against: the template's, but never
/// longer than a shift is allowed to be.
///
/// Shared with the staffing generator, which asks the same question.
pub fn shift_length_for_standards(params: &GeneratorParameters) -> f64 {
    params
        .planner_settings()
        .max_shift_length
        .min(params.shift_detail().shift_length())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::shift_standard::{ShiftStandardRange, ShiftStandardValue};
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use crate::workcontent::generators::advanced::providers::EnvironmentResolver;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// Every date resolves to the same environment.
    struct OneEnvironment(EnvironmentId);

    impl EnvironmentResolver for OneEnvironment {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            Some(self.0)
        }
    }

    /// No environment resolves at all.
    struct NoEnvironment;

    impl EnvironmentResolver for NoEnvironment {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            None
        }
    }

    /// An eight-hour shift, so the maximum never binds.
    fn context() -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        )
    }

    fn standard(
        work_type: WorkType,
        units: Units,
        suppress_value: i32,
        ranges: Vec<ShiftStandardRange>,
    ) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            work_type,
            units,
            suppress_value,
            ranges,
        )
    }

    /// A band covering every volume used here, valued for `environment_id`.
    fn band(environment_id: EnvironmentId, value: f64) -> ShiftStandardRange {
        ShiftStandardRange::with_values(
            0,
            1000,
            vec![ShiftStandardValue::new(environment_id, value)],
        )
    }

    #[test]
    fn a_standard_with_a_value_for_the_environment_is_costed() {
        let environment_id = EnvironmentId::new();
        let context = context();
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        // Two minutes per unit at a volume of ten.
        let standard = standard(
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![band(environment_id, 2.0)],
        );

        assert_eq!(
            calculate_minutes(&standard, 10, &context.params(), &providers),
            Ok(20.0)
        );
    }

    #[test]
    fn a_volume_no_band_covers_costs_nothing() {
        let environment_id = EnvironmentId::new();
        let context = context();
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        let standard = standard(
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::with_values(
                0,
                9,
                vec![ShiftStandardValue::new(environment_id, 2.0)],
            )],
        );

        assert_eq!(
            calculate_minutes(&standard, 500, &context.params(), &providers),
            Ok(0.0)
        );
    }

    #[test]
    fn a_band_with_no_value_for_this_environment_costs_nothing() {
        let context = context();
        let environments = OneEnvironment(EnvironmentId::new());
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        // The band is valued for a different environment entirely.
        let standard = standard(
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![band(EnvironmentId::new(), 2.0)],
        );

        assert_eq!(
            calculate_minutes(&standard, 10, &context.params(), &providers),
            Ok(0.0)
        );
    }

    #[test]
    fn an_unresolvable_environment_costs_nothing() {
        let context = context();
        let environments = NoEnvironment;
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        let standard = standard(
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![band(EnvironmentId::new(), 2.0)],
        );

        assert_eq!(
            calculate_minutes(&standard, 10, &context.params(), &providers),
            Ok(0.0)
        );
    }

    /// Java costs a task standard with no detail at zero and one with a detail
    /// through its own frequency model. Neither is ported, so both are
    /// reported — a deliberate divergence from Java's `0.0`, chosen so a job
    /// configured with task work cannot be silently planned as empty.
    #[test]
    fn task_standards_are_reported_rather_than_costed_at_nothing() {
        let environment_id = EnvironmentId::new();
        let context = context();
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        let standard = standard(
            WorkType::Task,
            Units::MinutesPerUnit,
            0,
            vec![band(environment_id, 2.0)],
        );

        assert_eq!(
            calculate_minutes(&standard, 10, &context.params(), &providers),
            Err(GenerationError::TaskStandardsNotSupported)
        );
    }

    #[rstest]
    // `WorkTotalMinutesCalculatorTest."calculation uses suppress value"`, and
    // the same table the non-flowed path is pinned against. At one minute per
    // unit the minutes read back as the suppressed volume.
    #[case(100, 101, 0.0)]
    #[case(100, 100, 0.0)]
    #[case(101, 100, 1.0)]
    #[case(100, 0, 100.0)]
    fn the_suppressed_volume_is_what_gets_costed(
        #[case] driver_value: i32,
        #[case] suppress_value: i32,
        #[case] expected: f64,
    ) {
        let environment_id = EnvironmentId::new();
        let context = context();
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };
        let standard = standard(
            WorkType::Variable,
            Units::MinutesPerUnit,
            suppress_value,
            vec![band(environment_id, 1.0)],
        );

        assert_eq!(
            calculate_minutes(&standard, driver_value, &context.params(), &providers),
            Ok(expected)
        );
    }

    #[rstest]
    // The shift is costed at its own length until the maximum is shorter.
    #[case(12.0, 8.0)]
    #[case(8.0, 8.0)]
    #[case(4.0, 4.0)]
    fn standards_are_costed_against_the_shorter_of_shift_and_maximum(
        #[case] max_shift_length: f64,
        #[case] expected: f64,
    ) {
        let mut context = context();
        context.planner_settings.max_shift_length = max_shift_length;

        assert_eq!(shift_length_for_standards(&context.params()), expected);
    }
}
