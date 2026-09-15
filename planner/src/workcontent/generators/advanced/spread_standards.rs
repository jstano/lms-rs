//! Turns spread standards into distributed work.
//!
//! A spread standard carries its own intraday shape rather than borrowing one
//! from a distributor. Fixed ones have that shape configured directly; dynamic
//! ones derive it from volumes observed per period.

use crate::workcontent::domain::spread_standard::{SpreadStandard, SpreadStandardType};
use crate::workcontent::domain::units::Units;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::spread_values_conversion::{
    SpreadValuesConverter, SpreadValuesConverterImpl,
};

/// One array per spread standard that produced anything.
///
/// A standard on another shift, one whose driver is closed, or one that
/// resolves to nothing all drop out — so the result may be shorter than the
/// number of standards configured.
pub fn generate(
    params: &GeneratorParameters,
    providers: &Providers,
) -> Vec<Vec<DistributionItem>> {
    let Some(range) = params.shift_date_range() else {
        return Vec::new();
    };

    params
        .job()
        .spread_standards_for_standard_set_and_shift(params.standard_set_id(), params.shift().id())
        .into_iter()
        .filter(|standard| driver_is_trading(standard, params, providers))
        .filter_map(|standard| match standard.spread_standard_type() {
            SpreadStandardType::Fixed => process_fixed(standard, params, providers, &range),
            SpreadStandardType::Dynamic => process_dynamic(standard, params, providers, &range),
        })
        .collect()
}

fn driver_is_trading(
    standard: &SpreadStandard,
    params: &GeneratorParameters,
    providers: &Providers,
) -> bool {
    let driver_value = params
        .planner_model()
        .business_driver_value(standard.business_driver_id(), params.shift_date());

    providers
        .openness
        .is_open(standard.business_driver_id(), params.shift_date(), driver_value)
}

/// A fixed standard's shape is read straight off its configured values.
fn process_fixed(
    standard: &SpreadStandard,
    params: &GeneratorParameters,
    providers: &Providers,
    range: &crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength,
) -> Option<Vec<DistributionItem>> {
    let driver_value = params
        .planner_model()
        .business_driver_value(standard.business_driver_id(), params.shift_date());
    let environment_id =
        providers.environment_for(standard.business_driver_id(), params.shift_date())?;

    let spread_value = standard.fixed_value_for(driver_value, environment_id)?;

    Some(SpreadValuesConverterImpl::new().convert(spread_value, range))
}

/// A dynamic standard's shape comes from the volumes observed per period.
fn process_dynamic(
    standard: &SpreadStandard,
    params: &GeneratorParameters,
    providers: &Providers,
    range: &crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength,
) -> Option<Vec<DistributionItem>> {
    let observed = providers
        .dynamic_spread
        .spread_values(standard.business_driver_id(), range)?;

    if observed.is_empty() {
        return None;
    }

    let mut array = DistributionItemListCreatorImpl::new().create_array(range);
    let start_index = range.start_index()? as usize;
    let environment_id =
        providers.environment_for(standard.business_driver_id(), params.shift_date());

    for (offset, volume) in observed.iter().enumerate() {
        let Some(item) = array.get_mut(start_index + offset) else {
            break;
        };

        item.add_to_period_value(work_for_volume(
            standard,
            *volume,
            environment_id,
            params.period_length(),
        ));
    }

    Some(array)
}

/// The work one period's observed volume amounts to.
///
/// A volume no band covers contributes nothing — the gaps between configured
/// bands are real, not an error.
fn work_for_volume(
    standard: &SpreadStandard,
    volume: i32,
    environment_id: Option<crate::workcontent::domain::environment::EnvironmentId>,
    period_length: i32,
) -> f64 {
    let Some(environment_id) = environment_id else {
        return 0.0;
    };
    let Some(dynamic) = standard.dynamic_value_for(volume, environment_id) else {
        return 0.0;
    };

    match standard.dynamic_spread_unit_type() {
        // How long a period's worth of people takes to serve.
        Units::UnitsPerPerson => volume as f64 / dynamic.value() * period_length as f64,
        Units::MinutesPerUnit => volume as f64 * dynamic.value(),
        // Every other unit is meaningless for a dynamic spread.
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::{Job, JobId};
    use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::spread_standard::{
        DynamicSpreadStandard, SpreadStandardRange, SpreadStandardValue,
    };
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::generators::advanced::providers::{
        BusinessDriverOpenness, DynamicSpreadProvider, EnvironmentResolver,
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

    /// The volumes the Java fixture observes, one per period from the start.
    struct Observed(Vec<i32>);

    impl DynamicSpreadProvider for Observed {
        fn spread_values(
            &self,
            _business_driver_id: BusinessDriverId,
            _range: &DateTimeRangeWithPeriodLength,
        ) -> Option<Vec<i32>> {
            Some(self.0.clone())
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

    /// A 01:00-03:00 shift at quarter-hour periods, so the shift starts at
    /// index 4 — the Java fixture's shape.
    fn fixture(
        business_driver_id: BusinessDriverId,
        make_standards: impl Fn(StandardSetId, JobShiftId) -> Vec<SpreadStandard>,
    ) -> Fixture {
        let standard_set_id = StandardSetId::new();
        let detail = JobShiftDefinition::new(
            DayOfWeek::Wednesday,
            LocalTime::new(1, 0, 0),
            LocalTime::new(3, 0, 0),
            0.0,
            0.0,
            1,
        );
        let shift = JobShift::new(
            JobId::new(),
            standard_set_id,
            "Night".to_string(),
            1,
            vec![detail],
        );
        let standards = make_standards(standard_set_id, shift.id());

        let mut planner_settings = PlannerSettings::default();
        planner_settings.period_length = 15;

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
                        HashMap::from([(date(), 50)]),
                    ),
                )]),
            ),
            job: Job::test().with_flowed_standards(standards, Vec::new(), Vec::new()),
            planner_settings,
            shift_detail: detail,
            shift,
        }
    }

    /// The Java fixture's three volume bands, valued 20, 30 and 50. Note the
    /// deliberate gap between 200 and 400.
    fn dynamic_standard(
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: BusinessDriverId,
        environment_id: EnvironmentId,
        units: Units,
    ) -> SpreadStandard {
        SpreadStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            business_driver_id,
            units,
            SpreadStandardType::Dynamic,
            vec![
                SpreadStandardRange::new(
                    1,
                    100,
                    Vec::new(),
                    vec![DynamicSpreadStandard::new(environment_id, 20.0)],
                ),
                SpreadStandardRange::new(
                    101,
                    200,
                    Vec::new(),
                    vec![DynamicSpreadStandard::new(environment_id, 30.0)],
                ),
                SpreadStandardRange::new(
                    400,
                    i32::MAX,
                    Vec::new(),
                    vec![DynamicSpreadStandard::new(environment_id, 50.0)],
                ),
            ],
        )
    }

    fn providers_for<'a>(
        environments: &'a OneEnvironment,
        observed: &'a Observed,
    ) -> Providers<'a> {
        Providers {
            environments,
            dynamic_spread: observed,
            ..Providers::none()
        }
    }

    /// The worked example: four periods of observed volume, one of which falls
    /// in the gap between configured bands and so costs nothing.
    #[test]
    fn dynamic_volumes_are_costed_band_by_band() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![dynamic_standard(
                standard_set_id,
                job_shift_id,
                business_driver_id,
                environment_id,
                Units::UnitsPerPerson,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let observed = Observed(vec![40, 161, 350, 428, 0, 0, 0, 0]);
        let providers = providers_for(&environments, &observed);

        let results = generate(&fixture.params(), &providers);

        assert_eq!(results.len(), 1);
        let items = &results[0];
        assert_eq!(items.len(), 192);
        // Before the shift starts, nothing.
        for item in items.iter().take(4) {
            assert_eq!(item.value_per_period(), 0.0);
        }
        assert_eq!(items[4].value_per_period(), 30.0);
        assert_eq!(items[5].value_per_period(), 80.5);
        // 350 falls between the 101-200 and 400+ bands, so it costs nothing.
        assert_eq!(items[6].value_per_period(), 0.0);
        assert_eq!(items[7].value_per_period(), 128.4);
        for item in items.iter().skip(8) {
            assert_eq!(item.value_per_period(), 0.0);
        }
    }

    #[test]
    fn minutes_per_unit_costs_the_volume_directly() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![dynamic_standard(
                standard_set_id,
                job_shift_id,
                business_driver_id,
                environment_id,
                Units::MinutesPerUnit,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let observed = Observed(vec![40]);
        let providers = providers_for(&environments, &observed);

        let results = generate(&fixture.params(), &providers);

        // Forty units at twenty minutes each.
        assert_eq!(results[0][4].value_per_period(), 800.0);
    }

    #[test]
    fn a_unit_type_that_means_nothing_here_costs_nothing() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![dynamic_standard(
                standard_set_id,
                job_shift_id,
                business_driver_id,
                environment_id,
                Units::Hours,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let observed = Observed(vec![40]);
        let providers = providers_for(&environments, &observed);

        assert_eq!(generate(&fixture.params(), &providers)[0][4].value_per_period(), 0.0);
    }

    #[test]
    fn with_no_observed_volumes_a_dynamic_standard_produces_nothing() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![dynamic_standard(
                standard_set_id,
                job_shift_id,
                business_driver_id,
                environment_id,
                Units::UnitsPerPerson,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        assert!(generate(&fixture.params(), &providers).is_empty());
    }

    #[test]
    fn a_fixed_standard_uses_its_configured_shape() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            // Five-minute slots, each worth three, across the whole day.
            vec![SpreadStandard::new(
                JobId::new(),
                standard_set_id,
                job_shift_id,
                business_driver_id,
                Units::MinutesPerUnit,
                SpreadStandardType::Fixed,
                vec![SpreadStandardRange::new(
                    0,
                    1000,
                    vec![SpreadStandardValue::new(
                        environment_id,
                        vec![Some(3); 288],
                    )],
                    Vec::new(),
                )],
            )]
        });
        let environments = OneEnvironment(environment_id);
        let providers = Providers {
            environments: &environments,
            ..Providers::none()
        };

        let results = generate(&fixture.params(), &providers);

        assert_eq!(results.len(), 1);
        // Three per five-minute slot, sampled once per quarter-hour period and
        // scaled by the period length.
        assert_eq!(results[0][4].value_per_period(), 45.0);
    }

    #[test]
    fn a_standard_on_another_shift_is_ignored() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, _| {
            vec![dynamic_standard(
                standard_set_id,
                JobShiftId::new(),
                business_driver_id,
                environment_id,
                Units::UnitsPerPerson,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let observed = Observed(vec![40]);
        let providers = providers_for(&environments, &observed);

        assert!(generate(&fixture.params(), &providers).is_empty());
    }

    #[test]
    fn a_closed_driver_silences_its_spread_standard() {
        let business_driver_id = BusinessDriverId::new();
        let environment_id = EnvironmentId::new();
        let fixture = fixture(business_driver_id, |standard_set_id, job_shift_id| {
            vec![dynamic_standard(
                standard_set_id,
                job_shift_id,
                business_driver_id,
                environment_id,
                Units::UnitsPerPerson,
            )]
        });
        let environments = OneEnvironment(environment_id);
        let observed = Observed(vec![40]);
        let closed = Trading(false);
        let providers = Providers {
            environments: &environments,
            dynamic_spread: &observed,
            openness: &closed,
            ..Providers::none()
        };

        assert!(generate(&fixture.params(), &providers).is_empty());
    }
}
