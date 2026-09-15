//! Fills the shift outward from its middle.
//!
//! Two walkers start either side of the centre and alternate, one stepping
//! back toward the start and one forward toward the end, so the busiest part of
//! the day sits in the middle of the shift. Each walker wraps to the centre
//! again when it runs off its end.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::{
    WorkSpreader, has_work_left, minutes_for_next_period, spread_bounds,
};

pub struct MiddleWorkSpreader;

impl MiddleWorkSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for MiddleWorkSpreader {
    fn populate_array_with_work_minutes(
        &self,
        total_work_minutes: f64,
        params: &GeneratorParameters,
        range: &DateTimeRangeWithPeriodLength,
        target: &mut [DistributionItem],
    ) {
        let Some((starting_index, ending_index)) = spread_bounds(params, range) else {
            return;
        };

        let period_length = range.period_length_in_minutes();
        let middle_index = (ending_index - starting_index) / 2;
        let front_home = starting_index + middle_index;
        let back_home = front_home + 1;

        let mut remaining = total_work_minutes;
        let mut period_front = front_home;
        let mut period_back = back_home;
        let mut move_front = true;

        while has_work_left(remaining) {
            let minutes = minutes_for_next_period(remaining, period_length);

            if move_front && period_front >= starting_index {
                // Unlike every other spreader, the added value is *not* rounded
                // on the way in — only `DistributionItem`'s own rounding
                // applies. A confirmed asymmetry in Java; do not "fix" it.
                if !add_to(target, period_front, minutes) {
                    return;
                }
                period_front -= 1;
                if period_front < starting_index {
                    period_front = front_home;
                }
                move_front = false;
            } else if !move_front && period_back <= ending_index {
                if !add_to(target, period_back, minutes) {
                    return;
                }
                period_back += 1;
                if period_back > ending_index {
                    period_back = back_home;
                }
                move_front = true;
            }

            // Deducted whether or not a walker could place it: on a degenerate
            // window neither branch fires and the work is dropped, which is
            // what Java does too.
            remaining -= minutes;
        }
    }
}

/// Add to one period, reporting whether the index was inside the array.
fn add_to(target: &mut [DistributionItem], index: i32, minutes: f64) -> bool {
    match usize::try_from(index)
        .ok()
        .and_then(|index| target.get_mut(index))
    {
        Some(item) => {
            item.add_to_period_value(minutes);
            true
        }
        // Java would throw here; stopping is the closest total behavior.
        None => false,
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

        MiddleWorkSpreader::new().populate_array_with_work_minutes(
            total_work_minutes,
            &params,
            &range,
            &mut array,
        );

        array
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    fn assert_periods(items: &[DistributionItem], total_work: f64, expected: &[(usize, f64)]) {
        for (index, value) in expected {
            assert_eq!(items[*index].value_per_period(), *value, "period {index}");
        }
        assert_eq!(total_of(items), total_work);
    }

    #[rstest]
    // Ten-minute periods over 48 through 95, centred on 72/73. 390 minutes runs
    // out before reaching either end.
    #[case(390.0, &[(47, 0.0), (48, 0.0), (52, 10.0), (72, 10.0), (73, 10.0), (91, 0.0), (96, 0.0)])]
    #[case(480.0, &[(48, 10.0), (52, 10.0), (72, 10.0), (73, 10.0), (91, 10.0)])]
    #[case(540.0, &[(48, 10.0), (52, 10.0), (72, 20.0), (73, 20.0), (91, 10.0)])]
    #[case(1000.0, &[(48, 20.0), (52, 20.0), (72, 30.0), (73, 30.0), (91, 20.0)])]
    fn a_daytime_shift_at_ten_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(10), total_work), total_work, expected);
    }

    #[rstest]
    #[case(420.0, &[(31, 0.0), (32, 0.0), (35, 15.0), (48, 15.0), (49, 15.0), (63, 0.0), (64, 0.0)])]
    #[case(480.0, &[(32, 15.0), (35, 15.0), (48, 15.0), (49, 15.0), (63, 15.0)])]
    #[case(540.0, &[(32, 15.0), (35, 15.0), (48, 30.0), (49, 30.0), (63, 15.0)])]
    // The third pass runs out mid-way, so the centre takes a full period and
    // the one after it only what was left.
    #[case(1000.0, &[(32, 30.0), (35, 30.0), (48, 45.0), (49, 30.0), (63, 30.0)])]
    fn a_daytime_shift_at_fifteen_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(15), total_work), total_work, expected);
    }

    #[rstest]
    #[case(390.0, &[(15, 0.0), (16, 0.0), (17, 30.0), (23, 30.0), (24, 30.0), (31, 0.0), (32, 0.0)])]
    #[case(480.0, &[(16, 30.0), (17, 30.0), (23, 30.0), (24, 30.0), (31, 30.0)])]
    #[case(540.0, &[(16, 30.0), (17, 30.0), (23, 60.0), (24, 60.0), (31, 30.0)])]
    #[case(1000.0, &[(16, 60.0), (17, 60.0), (23, 90.0), (24, 70.0), (31, 60.0)])]
    fn a_daytime_shift_at_half_hour_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(30), total_work), total_work, expected);
    }

    #[rstest]
    #[case(390.0, &[(137, 0.0), (138, 0.0), (139, 0.0), (162, 10.0), (163, 10.0), (185, 0.0), (186, 0.0)])]
    #[case(480.0, &[(138, 10.0), (139, 10.0), (162, 10.0), (163, 10.0), (185, 10.0)])]
    #[case(540.0, &[(138, 10.0), (139, 10.0), (162, 20.0), (163, 20.0), (185, 10.0)])]
    #[case(1000.0, &[(138, 20.0), (139, 20.0), (162, 30.0), (163, 30.0), (185, 20.0)])]
    fn an_overnight_shift_at_ten_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(
            &spread(&overnight_shift(10), total_work),
            total_work,
            expected,
        );
    }

    #[rstest]
    #[case(420.0, &[(91, 0.0), (92, 0.0), (95, 15.0), (108, 15.0), (109, 15.0), (123, 0.0), (124, 0.0)])]
    #[case(480.0, &[(92, 15.0), (95, 15.0), (108, 15.0), (109, 15.0), (123, 15.0)])]
    #[case(540.0, &[(92, 15.0), (95, 15.0), (108, 30.0), (109, 30.0), (123, 15.0)])]
    #[case(1000.0, &[(92, 30.0), (95, 30.0), (108, 45.0), (109, 30.0), (123, 30.0)])]
    fn an_overnight_shift_at_fifteen_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(
            &spread(&overnight_shift(15), total_work),
            total_work,
            expected,
        );
    }

    /// The value added is not rounded on the way in, so a total that does not
    /// divide evenly leaves a fraction the other spreaders would have rounded.
    #[test]
    fn the_added_value_is_not_rounded_before_it_lands() {
        // A third of a minute into a single period: the item's own rounding to
        // four places is all that applies.
        let items = spread(&day_shift(30), 1.0 / 3.0);

        assert_eq!(items[23].value_per_period(), 0.3333);
    }

    #[test]
    fn no_work_leaves_the_array_alone() {
        assert_eq!(total_of(&spread(&day_shift(30), 0.0)), 0.0);
    }
}
