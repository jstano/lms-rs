//! Places work along a configured intraday curve.
//!
//! Three stages: smear the arrival curve over how long guests stay, apply the
//! curve's percentages to the work, then hold the result down to what the
//! operation can actually serve.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::distributors::capacity::apply_capacity;
use crate::workcontent::generators::advanced::distributors::retention::apply_retention;
use crate::workcontent::generators::advanced::distributors::Distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;

/// Curve values are percentages of the day's work.
const PERCENTAGE_CONVERSION: f64 = 100.0;

pub struct FlowPlanDistributor {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl FlowPlanDistributor {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl Distributor for FlowPlanDistributor {
    fn distribute(
        &self,
        work_minutes: f64,
        standard: Option<&ShiftStandard>,
        params: &GeneratorParameters,
        providers: &Providers,
    ) -> Vec<DistributionItem> {
        let mut distribution = self.list_creator.create_array_for(params);

        let (Some(standard), Some(range)) = (standard, params.shift_date_range()) else {
            return distribution;
        };

        // No curve configured means no flowed work at all.
        let Some(curve) = providers
            .patterns
            .pattern_data(standard, params.shift_date(), &range)
        else {
            return distribution;
        };

        let business_driver_id = standard.business_driver_id();

        let curve = if standard.ignore_retention() {
            curve
        } else {
            apply_retention(
                &curve,
                business_driver_id,
                range.period_length_in_minutes(),
                providers.retention_capacity,
            )
        };

        let Some(shift_periods) = range.index_range() else {
            return distribution;
        };

        for index in shift_periods {
            // A shift reaching past the end of the curve falls back to its
            // first period rather than running out.
            let curve_index = if (index as usize) < curve.len() {
                index as usize
            } else {
                0
            };

            let Some(curve_value) = curve.get(curve_index).map(|item| item.value_per_period())
            else {
                continue;
            };
            let Some(item) = distribution.get_mut(index as usize) else {
                continue;
            };

            let percentage = numbers::round_percent(curve_value / PERCENTAGE_CONVERSION);

            item.add_to_period_value(round_fractional_minutes(
                work_minutes * percentage,
                params,
            ));
        }

        apply_capacity(
            &distribution,
            business_driver_id,
            work_minutes,
            params,
            &range,
            providers.retention_capacity,
        )
    }
}

/// How finely a period's minutes are kept.
///
/// The engine used to round to whole minutes, at the half; plans still on that
/// setting keep doing so, everything else keeps ten-thousandths.
fn round_fractional_minutes(minutes: f64, params: &GeneratorParameters) -> f64 {
    if params.planner_model().legacy_flow_pattern_rounding() {
        let whole = numbers::truncate(minutes);

        if minutes - whole as f64 >= 0.5 {
            (whole + 1) as f64
        } else {
            whole as f64
        }
    } else {
        numbers::round_raw_hours(minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::distribution_method::DistributionMethod;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::shift_standard::ShiftStandardRange;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::domain::work_type::WorkType;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use crate::workcontent::generators::advanced::providers::{
        DistributionPatternProvider, RetentionCapacityProvider, RetentionCapacityUtilization,
    };
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    /// An 08:00-12:00 shift at half-hour periods: indexes 16 through 23.
    fn context() -> Context {
        Context::new(
            date(),
            LocalTime::new(8, 0, 0),
            LocalTime::new(12, 0, 0),
            30,
        )
    }

    /// A curve putting a fixed percentage in each of the shift's periods.
    struct FlatCurve(f64);

    impl DistributionPatternProvider for FlatCurve {
        fn pattern_data(
            &self,
            _standard: &ShiftStandard,
            _date: LocalDate,
            _range: &DateTimeRangeWithPeriodLength,
        ) -> Option<Vec<DistributionItem>> {
            let mut array: Vec<_> = (0..96)
                .map(|index| {
                    DistributionItem::new(
                        index,
                        date().at_start_of_day().plus_minutes(index as i64 * 30),
                        0.0,
                    )
                })
                .collect();

            for item in array.iter_mut().take(24).skip(16) {
                item.set_value_per_period(self.0);
            }

            Some(array)
        }
    }

    /// No curve configured at all.
    struct NoCurve;

    impl DistributionPatternProvider for NoCurve {
        fn pattern_data(
            &self,
            _standard: &ShiftStandard,
            _date: LocalDate,
            _range: &DateTimeRangeWithPeriodLength,
        ) -> Option<Vec<DistributionItem>> {
            None
        }
    }

    /// Retention long enough to reach one period further on. Half an hour
    /// would not: it has to *exceed* a period to spill into the next.
    struct OnePeriodRetention;

    impl RetentionCapacityProvider for OnePeriodRetention {
        fn utilization_for(
            &self,
            _business_driver_id: BusinessDriverId,
            _date_time: LocalDateTime,
        ) -> Option<RetentionCapacityUtilization> {
            Some(RetentionCapacityUtilization {
                retention_hours: 0.75,
                capacity: 0.0,
                utilization: 0.0,
            })
        }
    }

    fn standard(ignore_retention: bool) -> ShiftStandard {
        let standard = ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::Minutes,
            0,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        )
        .distributed_by(DistributionMethod::Flowed);

        if ignore_retention {
            standard.ignoring_retention()
        } else {
            standard
        }
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        numbers::round_raw_hours(items.iter().map(|item| item.value_per_period()).sum())
    }

    #[test]
    fn with_no_curve_configured_nothing_is_planned() {
        let context = context();
        let providers = Providers {
            patterns: &NoCurve,
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(false)),
            &context.params(),
            &providers,
        );

        assert_eq!(total_of(&items), 0.0);
    }

    #[test]
    fn the_curves_percentages_decide_each_periods_share() {
        // Eight periods at 12.5% each accounts for the whole of the work.
        let context = context();
        let providers = Providers {
            patterns: &FlatCurve(12.5),
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(true)),
            &context.params(),
            &providers,
        );

        for (index, item) in items.iter().enumerate().take(24).skip(16) {
            assert_eq!(item.value_per_period(), 60.0, "period {index}");
        }
        assert_eq!(total_of(&items), 480.0);
    }

    #[test]
    fn retention_widens_the_curve_before_the_work_is_applied() {
        // The same flat curve, but each period's demand also counts in the
        // next, so every period bar the first carries twice the percentage.
        let context = context();
        let providers = Providers {
            patterns: &FlatCurve(12.5),
            retention_capacity: &OnePeriodRetention,
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(false)),
            &context.params(),
            &providers,
        );

        assert_eq!(items[16].value_per_period(), 60.0);
        for (index, item) in items.iter().enumerate().take(24).skip(17) {
            assert_eq!(item.value_per_period(), 120.0, "period {index}");
        }
    }

    #[test]
    fn a_standard_that_ignores_retention_skips_the_smear() {
        let context = context();
        let providers = Providers {
            patterns: &FlatCurve(12.5),
            retention_capacity: &OnePeriodRetention,
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(true)),
            &context.params(),
            &providers,
        );

        // Unwidened: every period keeps its own share.
        for (index, item) in items.iter().enumerate().take(24).skip(16) {
            assert_eq!(item.value_per_period(), 60.0, "period {index}");
        }
    }

    #[test]
    fn legacy_plans_round_each_period_to_a_whole_minute() {
        // A third of a percent of 480 minutes is 1.6 minutes a period.
        let context = context().with_legacy_flow_pattern_rounding();
        let providers = Providers {
            patterns: &FlatCurve(0.3333),
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(true)),
            &context.params(),
            &providers,
        );

        // 1.584 rounds up to 2 under the legacy rule.
        assert_eq!(items[16].value_per_period(), 2.0);
    }

    /// The percentage is rounded to four places *before* it is applied, so
    /// 0.3333% becomes 0.0033 and 480 minutes yields 1.584 rather than 1.5998.
    #[test]
    fn current_plans_keep_the_fraction() {
        let context = context();
        let providers = Providers {
            patterns: &FlatCurve(0.3333),
            ..Providers::none()
        };

        let items = FlowPlanDistributor::new().distribute(
            480.0,
            Some(&standard(true)),
            &context.params(),
            &providers,
        );

        assert_eq!(items[16].value_per_period(), 1.584);
    }
}
