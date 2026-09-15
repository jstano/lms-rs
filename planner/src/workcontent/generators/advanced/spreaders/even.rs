//! Spreads work flat across the shift.
//!
//! No iteration and no wrapping: the total is divided by the number of periods
//! and every period gets the same share. A shift needing more than a period's
//! worth per period simply ends up with periods over a period long, which the
//! stages downstream deal with.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::{WorkSpreader, exclusive_spread_bounds};

pub struct EvenWorkSpreader;

impl EvenWorkSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for EvenWorkSpreader {
    fn populate_array_with_work_minutes(
        &self,
        total_work_minutes: f64,
        params: &GeneratorParameters,
        range: &DateTimeRangeWithPeriodLength,
        target: &mut [DistributionItem],
    ) {
        let Some((starting_index, ending_index)) = exclusive_spread_bounds(params, range) else {
            return;
        };

        let shift_period_count = ending_index - starting_index;
        if shift_period_count <= 0 {
            return;
        }

        // Rounded once, so the periods sum to slightly less or more than the
        // total whenever the division does not come out even. Nothing corrects
        // the difference — the shares are what they are.
        let minutes_per_period =
            numbers::round_raw_hours(total_work_minutes / shift_period_count as f64);

        for index in starting_index..ending_index {
            let Some(item) = usize::try_from(index)
                .ok()
                .and_then(|index| target.get_mut(index))
            else {
                return;
            };

            item.add_to_period_value(minutes_per_period);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    fn day_shift(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        )
    }

    fn overnight_shift(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(23, 0, 0),
            LocalTime::new(7, 0, 0),
            period_length,
        )
    }

    fn spread(context: &Context, total_work_minutes: f64) -> Vec<DistributionItem> {
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        EvenWorkSpreader::new().populate_array_with_work_minutes(
            total_work_minutes,
            &params,
            &range,
            &mut array,
        );

        array
    }

    fn assert_periods(items: &[DistributionItem], expected: &[(usize, f64)]) {
        for (index, value) in expected {
            assert_eq!(items[*index].value_per_period(), *value, "period {index}");
        }
    }

    #[rstest]
    // Forty-eight ten-minute periods share the work equally.
    #[case(420.0, 8.75)]
    #[case(480.0, 10.0)]
    #[case(540.0, 11.25)]
    // A total that does not divide evenly leaves each period with the rounded
    // share, so the array sums to a shade under the total.
    #[case(1000.0, 20.8333)]
    fn a_daytime_shift_at_ten_minute_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&day_shift(10), total_work);

        assert_periods(
            &items,
            &[
                (47, 0.0),
                (48, share),
                (49, share),
                (89, share),
                (95, share),
                (96, 0.0),
            ],
        );
    }

    #[rstest]
    #[case(420.0, 13.125)]
    #[case(480.0, 15.0)]
    #[case(540.0, 16.875)]
    #[case(1000.0, 31.25)]
    fn a_daytime_shift_at_fifteen_minute_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&day_shift(15), total_work);

        assert_periods(
            &items,
            &[
                (31, 0.0),
                (32, share),
                (35, share),
                (36, share),
                (63, share),
                (64, 0.0),
            ],
        );
    }

    #[rstest]
    #[case(420.0, 26.25)]
    #[case(480.0, 30.0)]
    #[case(540.0, 33.75)]
    #[case(1000.0, 62.5)]
    fn a_daytime_shift_at_half_hour_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&day_shift(30), total_work);

        assert_periods(
            &items,
            &[
                (15, 0.0),
                (16, share),
                (17, share),
                (18, share),
                (31, share),
                (32, 0.0),
            ],
        );
    }

    #[rstest]
    #[case(420.0, 8.75)]
    #[case(480.0, 10.0)]
    #[case(540.0, 11.25)]
    #[case(1000.0, 20.8333)]
    fn an_overnight_shift_at_ten_minute_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&overnight_shift(10), total_work);

        assert_periods(
            &items,
            &[(137, 0.0), (138, share), (139, share), (185, share), (186, 0.0)],
        );
    }

    #[rstest]
    #[case(420.0, 13.125)]
    #[case(480.0, 15.0)]
    #[case(540.0, 16.875)]
    #[case(1000.0, 31.25)]
    fn an_overnight_shift_at_fifteen_minute_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&overnight_shift(15), total_work);

        assert_periods(
            &items,
            &[
                (91, 0.0),
                (92, share),
                (96, share),
                (108, share),
                (123, share),
                (124, 0.0),
            ],
        );
    }

    #[rstest]
    #[case(420.0, 26.25)]
    #[case(480.0, 30.0)]
    #[case(540.0, 33.75)]
    #[case(1000.0, 62.5)]
    fn an_overnight_shift_at_half_hour_periods(#[case] total_work: f64, #[case] share: f64) {
        let items = spread(&overnight_shift(30), total_work);

        assert_periods(
            &items,
            &[
                (45, 0.0),
                (46, share),
                (48, share),
                (54, share),
                (61, share),
                (62, 0.0),
            ],
        );
    }

    /// Not covered by the Java suite, which never enables the cap for this
    /// spreader. Pinned here because the clamped window is deliberately one
    /// period wider than the other spreaders produce.
    #[test]
    fn capping_at_the_maximum_shift_keeps_one_more_period_than_elsewhere() {
        let mut context = day_shift(30);
        context.planner_settings.limit_shift_to_max_shift = true;
        // A four-hour maximum is eight half-hour periods.
        context.planner_settings.max_shift_length = 4.0;

        let items = spread(&context, 480.0);

        // Eight periods, 16 through 23, each taking an eighth of the work.
        assert_eq!(items[16].value_per_period(), 60.0);
        assert_eq!(items[23].value_per_period(), 60.0);
        assert_eq!(items[24].value_per_period(), 0.0);
    }

    #[test]
    fn no_work_leaves_every_period_empty() {
        let items = spread(&day_shift(30), 0.0);

        assert!(items.iter().all(|item| item.value_per_period() == 0.0));
    }
}
