//! Fills the shift from its end.
//!
//! The mirror of [`BeginningWorkSpreader`](super::beginning::BeginningWorkSpreader):
//! one period at a time from the back, wrapping to the last period again when
//! the work outlasts the window, so heavy days end up back-loaded.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::{
    WorkSpreader, has_work_left, minutes_for_next_period, spread_bounds,
};

pub struct EndingWorkSpreader;

impl EndingWorkSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for EndingWorkSpreader {
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
        let mut remaining = total_work_minutes;
        let mut current_index = ending_index;

        while has_work_left(remaining) {
            let minutes = minutes_for_next_period(remaining, period_length);

            // Java would throw on a window outside the array; stopping is the
            // closest total behavior.
            let Some(item) = usize::try_from(current_index)
                .ok()
                .and_then(|index| target.get_mut(index))
            else {
                return;
            };

            item.add_to_period_value(numbers::round_raw_hours(minutes));
            remaining -= minutes;
            current_index -= 1;

            if current_index < starting_index {
                current_index = ending_index;
            }
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

        EndingWorkSpreader::new().populate_array_with_work_minutes(
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
    // Ten-minute periods over 48 through 95. 420 minutes fills back from 95 and
    // runs out before reaching 53, leaving the front of the shift empty.
    #[case(420.0, &[(47, 0.0), (53, 0.0), (95, 10.0), (96, 0.0)])]
    #[case(480.0, &[(53, 10.0), (95, 10.0)])]
    #[case(540.0, &[(53, 10.0), (95, 20.0)])]
    #[case(1000.0, &[(53, 20.0), (95, 30.0)])]
    fn a_daytime_shift_at_ten_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(10), total_work), total_work, expected);
    }

    #[rstest]
    #[case(420.0, &[(31, 0.0), (32, 0.0), (35, 0.0), (36, 15.0), (63, 15.0), (64, 0.0)])]
    #[case(480.0, &[(32, 15.0), (35, 15.0), (36, 15.0), (63, 15.0)])]
    #[case(540.0, &[(32, 15.0), (35, 15.0), (36, 15.0), (63, 30.0)])]
    #[case(1000.0, &[(32, 30.0), (35, 30.0), (36, 30.0), (63, 45.0)])]
    fn a_daytime_shift_at_fifteen_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(15), total_work), total_work, expected);
    }

    #[rstest]
    // Fourteen half-hours of work fill 31 back down to 18, leaving 16 and 17
    // untouched.
    #[case(420.0, &[(15, 0.0), (16, 0.0), (17, 0.0), (18, 30.0), (31, 30.0), (32, 0.0)])]
    #[case(480.0, &[(16, 30.0), (17, 30.0), (18, 30.0), (31, 30.0)])]
    #[case(540.0, &[(16, 30.0), (17, 30.0), (18, 30.0), (31, 60.0)])]
    #[case(1000.0, &[(16, 60.0), (17, 60.0), (18, 60.0), (31, 90.0)])]
    fn a_daytime_shift_at_half_hour_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(30), total_work), total_work, expected);
    }

    #[rstest]
    #[case(420.0, &[(137, 0.0), (138, 0.0), (185, 10.0), (186, 0.0)])]
    #[case(480.0, &[(138, 10.0), (185, 10.0)])]
    #[case(540.0, &[(138, 10.0), (185, 20.0)])]
    #[case(1000.0, &[(138, 20.0), (185, 30.0)])]
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
    #[case(420.0, &[(91, 0.0), (92, 0.0), (96, 15.0), (108, 15.0), (123, 15.0), (124, 0.0)])]
    #[case(480.0, &[(92, 15.0), (96, 15.0), (108, 15.0), (123, 15.0)])]
    #[case(540.0, &[(92, 15.0), (96, 15.0), (108, 15.0), (123, 30.0)])]
    #[case(1000.0, &[(92, 30.0), (96, 30.0), (108, 30.0), (123, 45.0)])]
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

    #[rstest]
    #[case(420.0, &[(45, 0.0), (46, 0.0), (48, 30.0), (54, 30.0), (61, 30.0), (62, 0.0)])]
    #[case(480.0, &[(46, 30.0), (48, 30.0), (54, 30.0), (61, 30.0)])]
    #[case(540.0, &[(46, 30.0), (48, 30.0), (54, 30.0), (61, 60.0)])]
    #[case(1000.0, &[(46, 60.0), (48, 60.0), (54, 60.0), (61, 90.0)])]
    fn an_overnight_shift_at_half_hour_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(
            &spread(&overnight_shift(30), total_work),
            total_work,
            expected,
        );
    }

    #[test]
    fn no_work_leaves_the_array_alone() {
        assert_eq!(total_of(&spread(&day_shift(30), 0.0)), 0.0);
    }
}
