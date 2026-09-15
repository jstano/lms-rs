//! Fills the shift front to back, then bounces.
//!
//! One walker runs from the start of the shift to the end, turns around, runs
//! back, and keeps bouncing until the work is placed. A shift with more work
//! than periods therefore builds up in layers from whichever end it last
//! turned at.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::{
    WorkSpreader, exclusive_spread_bounds, has_work_left, minutes_for_next_period,
};

pub struct VaryingWorkSpreader;

impl VaryingWorkSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for VaryingWorkSpreader {
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

        let period_length = range.period_length_in_minutes();
        let mut remaining = total_work_minutes;
        let mut current_index = starting_index;
        let mut move_forward = true;

        while has_work_left(remaining) {
            let minutes = minutes_for_next_period(remaining, period_length);

            let Some(item) = usize::try_from(current_index)
                .ok()
                .and_then(|index| target.get_mut(index))
            else {
                return;
            };

            item.add_to_period_value(numbers::round_raw_hours(minutes));
            remaining -= minutes;

            // Stepping off either end turns the walker around, leaving it on
            // the period it just filled — so the turn period takes two helpings
            // in a row.
            if move_forward {
                current_index += 1;
                if current_index >= ending_index {
                    current_index -= 1;
                    move_forward = false;
                }
            } else {
                current_index -= 1;
                if current_index < starting_index {
                    current_index += 1;
                    move_forward = true;
                }
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

    /// The Java fixture: an 08:00 to 16:00 shift at quarter-hour periods, so
    /// the shift occupies 32 through 63.
    fn shift(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2016, 8, 21),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        )
    }

    fn spread(context: &Context, total_work_minutes: f64) -> Vec<DistributionItem> {
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        VaryingWorkSpreader::new().populate_array_with_work_minutes(
            total_work_minutes,
            &params,
            &range,
            &mut array,
        );

        array
    }

    /// Asserts every period in `range` holds `value`.
    fn assert_all(items: &[DistributionItem], range: std::ops::RangeInclusive<usize>, value: f64) {
        for index in range {
            assert_eq!(items[index].value_per_period(), value, "period {index}");
        }
    }

    #[test]
    fn work_shorter_than_one_period_all_lands_in_the_first() {
        let items = spread(&shift(15), 12.0);

        assert_all(&items, 0..=31, 0.0);
        assert_eq!(items[32].value_per_period(), 12.0);
        assert_all(&items, 33..=95, 0.0);
    }

    #[test]
    fn work_that_exactly_fills_the_shift_gives_every_period_one_helping() {
        let items = spread(&shift(15), 480.0);

        assert_all(&items, 0..=31, 0.0);
        assert_all(&items, 32..=63, 15.0);
        assert_all(&items, 64..=95, 0.0);
    }

    #[test]
    fn work_that_overruns_turns_around_and_doubles_up_at_the_end() {
        let items = spread(&shift(15), 510.0);

        assert_all(&items, 0..=31, 0.0);
        assert_all(&items, 32..=61, 15.0);
        // The walker turns on 63, so that period and the one before it take a
        // second helping.
        assert_eq!(items[62].value_per_period(), 30.0);
        assert_eq!(items[63].value_per_period(), 30.0);
        assert_all(&items, 64..=95, 0.0);
    }

    #[test]
    fn work_that_overruns_twice_bounces_back_to_the_front() {
        let items = spread(&shift(15), 990.0);

        assert_all(&items, 0..=31, 0.0);
        // A full pass out, a full pass back, then a third pass starting again
        // from the front.
        assert_eq!(items[32].value_per_period(), 45.0);
        assert_eq!(items[33].value_per_period(), 45.0);
        assert_all(&items, 34..=63, 30.0);
        assert_all(&items, 64..=95, 0.0);
    }

    #[test]
    fn capping_at_the_maximum_shift_confines_the_work_to_the_cap() {
        let mut context = shift(30);
        context.planner_settings.limit_shift_to_max_shift = true;
        // Eight half-hour periods.
        context.planner_settings.max_shift_length = 4.0;

        let items = spread(&context, 480.0);

        assert_all(&items, 0..=15, 0.0);
        // Out and back over eight periods, so each takes two helpings.
        assert_all(&items, 16..=23, 60.0);
        assert_all(&items, 24..=47, 0.0);
    }

    #[test]
    fn a_cap_the_shift_already_fits_inside_changes_nothing() {
        let mut context = shift(30);
        context.planner_settings.limit_shift_to_max_shift = true;
        // Sixteen half-hour periods: exactly the shift's own length, so the
        // guard's strict comparison leaves the window alone.
        context.planner_settings.max_shift_length = 8.0;

        let items = spread(&context, 510.0);

        assert_all(&items, 0..=15, 0.0);
        assert_all(&items, 16..=30, 30.0);
        assert_eq!(items[31].value_per_period(), 60.0);
        assert_all(&items, 32..=47, 0.0);
    }

    #[test]
    fn no_work_leaves_the_array_alone() {
        let items = spread(&shift(15), 0.0);

        assert!(items.iter().all(|item| item.value_per_period() == 0.0));
    }
}
