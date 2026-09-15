//! Fills the shift from its start.
//!
//! One period at a time from the front, a full period's worth each. If there is
//! more work than the window can hold it wraps back to the start and layers
//! another pass on top, so heavy days end up front-loaded.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::{
    WorkSpreader, has_work_left, minutes_for_next_period, spread_bounds,
};

pub struct BeginningWorkSpreader;

impl BeginningWorkSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for BeginningWorkSpreader {
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
        let mut current_index = starting_index;

        while has_work_left(remaining) {
            let minutes = minutes_for_next_period(remaining, period_length);

            // Java indexes the list directly and would throw on a window that
            // falls outside the array; stopping is the closest total behavior.
            let Some(item) = usize::try_from(current_index)
                .ok()
                .and_then(|index| target.get_mut(index))
            else {
                return;
            };

            item.add_to_period_value(numbers::round_raw_hours(minutes));
            remaining -= minutes;
            current_index += 1;

            if current_index > ending_index {
                current_index = starting_index;
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

    /// A daytime shift, 08:00 to 16:00 on 2013-07-24.
    fn day_shift(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        )
    }

    /// An overnight shift, 23:00 to 07:00 the next morning.
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

        BeginningWorkSpreader::new().populate_array_with_work_minutes(
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

    /// Asserts the values at the indexes the Java table checks, and that the
    /// whole of the work was placed.
    fn assert_periods(items: &[DistributionItem], total_work: f64, expected: &[(usize, f64)]) {
        for (index, value) in expected {
            assert_eq!(items[*index].value_per_period(), *value, "period {index}");
        }
        assert_eq!(total_of(items), total_work);
    }

    #[rstest]
    // Ten-minute periods: the shift occupies 48 through 95. 420 minutes leaves
    // the tail empty; 480 fills it exactly; beyond that the work wraps and
    // layers up from the front.
    #[case(420.0, &[(47, 0.0), (48, 10.0), (49, 10.0), (89, 10.0), (95, 0.0), (96, 0.0)])]
    #[case(480.0, &[(47, 0.0), (48, 10.0), (49, 10.0), (89, 10.0), (95, 10.0), (96, 0.0)])]
    #[case(540.0, &[(48, 20.0), (49, 20.0), (89, 10.0), (95, 10.0)])]
    #[case(1000.0, &[(48, 30.0), (49, 30.0), (89, 20.0), (95, 20.0)])]
    fn a_daytime_shift_at_ten_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(10), total_work), total_work, expected);
    }

    #[rstest]
    #[case(420.0, &[(31, 0.0), (32, 15.0), (35, 15.0), (36, 15.0), (63, 0.0), (64, 0.0)])]
    #[case(480.0, &[(32, 15.0), (35, 15.0), (36, 15.0), (63, 15.0)])]
    #[case(540.0, &[(32, 30.0), (35, 30.0), (36, 15.0), (63, 15.0)])]
    #[case(1000.0, &[(32, 45.0), (35, 30.0), (36, 30.0), (63, 30.0)])]
    fn a_daytime_shift_at_fifteen_minute_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(15), total_work), total_work, expected);
    }

    #[rstest]
    #[case(420.0, &[(15, 0.0), (16, 30.0), (17, 30.0), (18, 30.0), (31, 0.0), (32, 0.0)])]
    #[case(480.0, &[(16, 30.0), (17, 30.0), (18, 30.0), (31, 30.0)])]
    #[case(540.0, &[(16, 60.0), (17, 60.0), (18, 30.0), (31, 30.0)])]
    // The second wrap runs out part-way through, so period 17 takes only the
    // ten minutes that were left rather than a full third layer.
    #[case(1000.0, &[(16, 90.0), (17, 70.0), (18, 60.0), (31, 60.0)])]
    fn a_daytime_shift_at_half_hour_periods(
        #[case] total_work: f64,
        #[case] expected: &[(usize, f64)],
    ) {
        assert_periods(&spread(&day_shift(30), total_work), total_work, expected);
    }

    #[rstest]
    // The overnight shift runs 23:00 to 07:00, so at ten-minute periods it
    // occupies 138 through 185 — on into the array's second day.
    #[case(420.0, &[(137, 0.0), (138, 10.0), (185, 0.0), (186, 0.0)])]
    #[case(480.0, &[(138, 10.0), (185, 10.0)])]
    #[case(540.0, &[(138, 20.0), (185, 10.0)])]
    #[case(1000.0, &[(138, 30.0), (185, 20.0)])]
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
    #[case(420.0, &[(91, 0.0), (92, 15.0), (96, 15.0), (108, 15.0), (123, 0.0), (124, 0.0)])]
    #[case(480.0, &[(92, 15.0), (96, 15.0), (108, 15.0), (123, 15.0)])]
    #[case(540.0, &[(92, 30.0), (96, 15.0), (108, 15.0), (123, 15.0)])]
    #[case(1000.0, &[(92, 45.0), (96, 30.0), (108, 30.0), (123, 30.0)])]
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
    #[case(420.0, &[(45, 0.0), (46, 30.0), (48, 30.0), (54, 30.0), (61, 0.0), (62, 0.0)])]
    #[case(480.0, &[(46, 30.0), (48, 30.0), (54, 30.0), (61, 30.0)])]
    #[case(540.0, &[(46, 60.0), (48, 30.0), (54, 30.0), (61, 30.0)])]
    #[case(1000.0, &[(46, 90.0), (48, 60.0), (54, 60.0), (61, 60.0)])]
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
        let items = spread(&day_shift(30), 0.0);

        assert_eq!(total_of(&items), 0.0);
    }
}
