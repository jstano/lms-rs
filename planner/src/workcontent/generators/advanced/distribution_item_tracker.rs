//! Peels blocks of work off a head-count array.
//!
//! Output is produced by repeatedly taking the deepest layer that runs
//! unbroken: find where coverage starts, find where it stops, take the
//! shallowest depth across that run, and subtract it. What remains is a
//! shallower array, and the process repeats.
//!
//! The endpoint and minimum-finding rules differ between work content and
//! planned shifts — deliberately, and in ways that look like duplication until
//! you read them side by side. Each is documented where it differs.

use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::work_content_tracker::WorkContentTrackerBean;
use joda_rs::LocalDateTime;
use joda_rs::constants::MINUTES_PER_HOUR;

/// Periods holding this are placeholders, not real coverage, and are passed
/// over when looking for the shallowest depth.
const SENTINEL: f64 = i32::MAX as f64;

/// Where the next block of coverage starts, and how deep it is there.
///
/// `None` when nothing is covered. Depth is truncated, not rounded — a period
/// holding 0.9 of a body covers nobody.
pub fn find_first_non_zero(
    bodies: &[DistributionItem],
    params: &GeneratorParameters,
) -> Option<WorkContentTrackerBean> {
    if bodies.is_empty() {
        return None;
    }

    bodies
        .iter()
        .find(|item| numbers::truncate(item.value_per_period()) > 0)
        .map(|item| {
            let mut bean = WorkContentTrackerBean::new(params);
            bean.set_start_date_time(item.date_time());
            bean.set_min_value(numbers::truncate(item.value_per_period()));
            bean
        })
}

/// Where the block ends: the first period with no coverage at all.
///
/// Unbounded — a work-content block may run as long as the coverage does,
/// however long that is.
pub fn find_work_content_endpoint(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
    params: &GeneratorParameters,
) {
    let starting_index = bean.starting_index().max(0) as usize;
    let mut ending_index = starting_index;

    while ending_index < bodies.len() && bodies[ending_index].value_per_period() > 0.0 {
        ending_index += 1;
    }

    set_end_after(bean, bodies, ending_index, params);
}

/// Where the shift ends: the first period with no coverage, or the maximum
/// shift length, whichever comes first.
pub fn find_planned_shift_endpoint(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
    params: &GeneratorParameters,
) {
    let starting_index = bean.starting_index().max(0) as usize;
    let max_shift_in_periods = params.max_shift_length_in_periods().max(0) as usize;
    let mut ending_index = starting_index;

    while ending_index < bodies.len()
        && ending_index - starting_index < max_shift_in_periods
        && bodies[ending_index].value_per_period() > 0.0
    {
        ending_index += 1;
    }

    set_end_after(bean, bodies, ending_index, params);
}

/// Where a long shift ends: the maximum shift length, backed off past any
/// trailing periods with no coverage.
///
/// Unlike [`find_planned_shift_endpoint`] this does **not** stop at the first
/// gap — a long shift may span one — it only trims empty periods off the end.
pub fn find_planned_long_shift_endpoint(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
    params: &GeneratorParameters,
) {
    let starting_index = bean.starting_index().max(0) as usize;
    let max_shift_in_periods = params.max_shift_length_in_periods().max(0) as usize;
    let period_length = params.period_length() as i64;

    // A shift that would run past the end of the array is cut to the array,
    // and is not trimmed — there is nothing beyond it to trim toward.
    let endpoint_outside_periods = starting_index + max_shift_in_periods >= bodies.len();
    let max_ending_index = if endpoint_outside_periods {
        bodies.len()
    } else {
        starting_index + max_shift_in_periods
    };

    let Some(anchor) = bodies.get(if endpoint_outside_periods {
        max_ending_index.saturating_sub(1)
    } else {
        max_ending_index
    }) else {
        return;
    };
    let mut end_date_time = anchor.date_time();

    if !endpoint_outside_periods {
        let mut index = max_ending_index - 1;

        while index > starting_index {
            if numbers::truncate(bodies[index].value_per_period()) > 0 {
                break;
            }

            end_date_time = end_date_time.minus_minutes(period_length);
            index -= 1;
        }
    }

    bean.set_end_date_time(end_date_time);
}

/// The end of the period *after* the last covered one.
fn set_end_after(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
    ending_index: usize,
    params: &GeneratorParameters,
) {
    let Some(last) = bodies.get(ending_index.saturating_sub(1)) else {
        return;
    };

    bean.set_end_date_time(last.date_time().plus_minutes(params.period_length() as i64));
}

/// The shallowest depth across the block — the layer that can be peeled whole.
///
/// Starts from the block's first period and only skips the sentinel, so a
/// period with no coverage inside the block *does* pull the depth to zero.
/// That is the difference from
/// [`find_planned_shift_minimum_value_in_range`]; keep them apart.
pub fn find_minimum_value_in_range(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
) {
    let starting_index = bean.starting_index().max(0) as usize;

    let Some(first) = bodies.get(starting_index) else {
        return;
    };
    let mut min_value = numbers::truncate(first.value_per_period());
    bean.set_min_value(min_value);

    for index in starting_index..bean.ending_index().max(0) as usize {
        let Some(item) = bodies.get(index) else {
            break;
        };
        let value = item.value_per_period();

        if value < SENTINEL && value < min_value as f64 {
            min_value = numbers::truncate(value);
            bean.set_min_value(min_value);
        }
    }
}

/// The shallowest depth across the block, ignoring uncovered periods.
///
/// Two differences from [`find_minimum_value_in_range`]: it starts from the
/// depth the bean already holds rather than resetting to the first period, and
/// it skips periods with no coverage — a shift can span a gap without its
/// depth collapsing to nothing.
pub fn find_planned_shift_minimum_value_in_range(
    bean: &mut WorkContentTrackerBean,
    bodies: &[DistributionItem],
) {
    let starting_index = bean.starting_index().max(0) as usize;
    let mut min_value = bean.min_value();

    for index in starting_index..bean.ending_index().max(0) as usize {
        let Some(item) = bodies.get(index) else {
            break;
        };
        let value = item.value_per_period();

        if value < SENTINEL && value < min_value as f64 && value > 0.0 {
            min_value = numbers::truncate(value);
            bean.set_min_value(min_value);
        }
    }
}

/// Take the peeled layer off the array.
///
/// Only periods that actually reach the depth are reduced — a shallower one is
/// left alone rather than going negative. Subtracted unrounded, which is the
/// one place in the engine that path is used.
pub fn deduct_minimum_value_from_list_items(
    bodies: &mut [DistributionItem],
    bean: &WorkContentTrackerBean,
) {
    let starting_index = bean.starting_index().max(0) as usize;
    let ending_index = bean.ending_index().max(0) as usize;
    let min_value = bean.min_value() as f64;

    for index in starting_index..ending_index {
        let Some(item) = bodies.get_mut(index) else {
            break;
        };

        if item.value_per_period() >= min_value {
            item.subtract_from_period_value(min_value);
        }
    }
}

/// Stretch a block out to the shortest shift the plan allows.
///
/// The end is pushed out first, but never past the shift's own end; if that is
/// not enough the start is pulled back — and that *can* take the block outside
/// the shift window entirely, which is intended.
pub fn adjust_for_min_shift(
    bean: &mut WorkContentTrackerBean,
    shift_end_date_time: LocalDateTime,
    params: &GeneratorParameters,
) {
    let min_shift = params
        .planner_settings()
        .min_shift_length
        .min(params.shift_detail().shift_length());

    let work_range = bean.date_time_range();
    let max_end = if shift_end_date_time > work_range.end() {
        shift_end_date_time
    } else {
        work_range.end()
    };

    if work_range.duration().fractional_hours() >= min_shift {
        return;
    }

    let min_shift_minutes = (min_shift * MINUTES_PER_HOUR as f64) as i64;
    let minutes_to_extend_end = min_shift_minutes - work_range.duration().to_minutes();
    let extended_end = work_range.end().plus_minutes(minutes_to_extend_end);

    bean.set_end_date_time(if extended_end < max_end {
        extended_end
    } else {
        max_end
    });

    let extended = bean.date_time_range();

    if extended.duration().fractional_hours() < min_shift {
        let minutes_to_extend_start = min_shift_minutes - extended.duration().to_minutes();

        bean.set_start_date_time(extended.start().minus_minutes(minutes_to_extend_start));
    }
}


#[cfg(test)]
mod java_parity_tests {
    //! Ported verbatim from `DistributionItemTrackerTest.groovy`.
    //!
    //! Java asserts array indexes where the algorithm works in times, so these
    //! do the same — an index assertion pins both the time and the way the bean
    //! counts on past midnight.

    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// The Java fixture arrays.
    const EXAMPLE_1: [i32; 9] = [1, 2, 1, 3, 4, 4, 3, 2, 1];
    const EXAMPLE_2: [i32; 9] = [0, 2, 1, 3, 4, 4, 3, 2, 1];
    const EXAMPLE_3: [i32; 9] = [6, 7, 8, 9, 10, 3, 9, 8, 7];
    const EXAMPLE_4: [i32; 9] = [0, 0, 8, 9, 10, 3, 9, 8, 7];
    const ONE_AT_EIGHT: [i32; 9] = [0, 1, 0, 0, 0, 0, 0, 0, 0];

    fn date() -> LocalDate {
        LocalDate::new(2013, 8, 26)
    }

    fn time(hour: i32, minute: i32) -> LocalTime {
        LocalTime::new(hour, minute, 0)
    }

    /// The Java setup's shift: 07:30–12:00 at half-hour periods.
    fn context() -> Context {
        Context::new(date(), time(7, 30), time(12, 0), 30)
    }

    /// The canonical two-day array with `values` laid in from `start_index`,
    /// standing in for Java's `arrayHelper.populateTestArray`.
    fn bodies(context: &Context, start_index: usize, values: &[i32]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value as f64);
        }

        array
    }

    fn at(date: LocalDate, hour: i32, minute: i32) -> LocalDateTime {
        date.at_time(time(hour, minute))
    }

    fn on(day: i32, hour: i32, minute: i32) -> LocalDateTime {
        at(LocalDate::new(2013, 1, day), hour, minute)
    }

    #[test]
    fn find_first_with_empty_bodies_per_period_returns_null() {
        let context = context();

        assert!(find_first_non_zero(&[], &context.params()).is_none());
    }

    #[test]
    fn find_first_returns_null_if_no_values_greater_than_zero_are_found() {
        let context = context();
        let array = bodies(&context, 15, &[]);

        assert!(find_first_non_zero(&array, &context.params()).is_none());
    }

    #[rstest]
    #[case(15, EXAMPLE_1, at(date(), 7, 30))]
    #[case(20, EXAMPLE_2, at(date(), 10, 30))]
    fn find_first_returns_the_first_non_zero_distribution_item(
        #[case] start_index: usize,
        #[case] array: [i32; 9],
        #[case] start_date_time: LocalDateTime,
    ) {
        let context = context();
        let array = bodies(&context, start_index, &array);

        let bean = find_first_non_zero(&array, &context.params()).expect("a non-zero item");

        assert_eq!(bean.start_date_time(), start_date_time);
    }

    #[rstest]
    #[case(15, EXAMPLE_1, at(date(), 12, 0))]
    #[case(20, EXAMPLE_2, at(date(), 14, 30))]
    #[case(15, ONE_AT_EIGHT, at(date(), 8, 30))]
    fn find_end(
        #[case] start_index: usize,
        #[case] array: [i32; 9],
        #[case] end_date_time: LocalDateTime,
    ) {
        let context = context();
        let array = bodies(&context, start_index, &array);
        let params = context.params();
        let mut bean = find_first_non_zero(&array, &params).expect("a non-zero item");

        find_work_content_endpoint(&mut bean, &array, &params);

        assert_eq!(bean.end_date_time(), end_date_time);
    }

    #[rstest]
    #[case(15, EXAMPLE_1, 1, at(date(), 7, 30), at(date(), 12, 0))]
    #[case(15, EXAMPLE_3, 3, at(date(), 7, 30), at(date(), 12, 0))]
    #[case(20, EXAMPLE_2, 1, at(date(), 10, 30), at(date(), 14, 30))]
    fn find_minimum(
        #[case] start_index: usize,
        #[case] array: [i32; 9],
        #[case] min_value: i32,
        #[case] start_date_time: LocalDateTime,
        #[case] end_date_time: LocalDateTime,
    ) {
        let context = context();
        let array = bodies(&context, start_index, &array);
        let params = context.params();
        let mut bean = find_first_non_zero(&array, &params).expect("a non-zero item");
        find_work_content_endpoint(&mut bean, &array, &params);

        find_minimum_value_in_range(&mut bean, &array);

        assert_eq!(bean.start_date_time(), start_date_time);
        assert_eq!(bean.end_date_time(), end_date_time);
        assert_eq!(bean.min_value(), min_value);
    }

    /// Java's one row: the array after deductions, laid in at 15, minus 3.
    #[test]
    fn deduction_of_a_minimum_value_from_each_item_in_an_array() {
        let context = context();
        let mut array = bodies(&context, 15, &EXAMPLE_3);
        let params = context.params();
        let mut bean = find_first_non_zero(&array, &params).expect("a non-zero item");
        find_work_content_endpoint(&mut bean, &array, &params);
        find_minimum_value_in_range(&mut bean, &array);

        deduct_minimum_value_from_list_items(&mut array, &bean);

        for (offset, original) in EXAMPLE_3.iter().enumerate() {
            assert_eq!(
                array[15 + offset].value_per_period(),
                (*original - 3) as f64,
                "index {}",
                15 + offset
            );
        }
    }

    #[rstest]
    #[case(4.0, 8.0, 16, EXAMPLE_1, 25, at(date(), 8, 0), (time(8, 0), time(16, 0)))]
    #[case(4.0, 8.0, 46, EXAMPLE_1, 55, at(date(), 23, 0), (time(23, 0), time(4, 0)))]
    #[case(4.0, 8.0, 16, EXAMPLE_3, 25, at(date(), 8, 0), (time(8, 0), time(16, 0)))]
    #[case(4.0, 8.0, 46, EXAMPLE_3, 55, at(date(), 23, 0), (time(23, 0), time(4, 0)))]
    #[case(0.0, 0.0, 16, EXAMPLE_1, 16, at(date(), 8, 0), (time(8, 0), time(16, 0)))]
    #[case(4.0, 8.0, 46, EXAMPLE_4, 55, at(date().plus_days(1), 0, 0), (time(23, 0), time(4, 0)))]
    fn find_last_non_zero_distribution_item_with_min_max_shift_for_planned_shifts(
        #[case] min_shift: f64,
        #[case] max_shift: f64,
        #[case] start_index: usize,
        #[case] array: [i32; 9],
        #[case] ending_index: i32,
        #[case] start_date_time: LocalDateTime,
        #[case] shift_times: (LocalTime, LocalTime),
    ) {
        let mut context = Context::new(date(), shift_times.0, shift_times.1, 30);
        context.planner_settings.min_shift_length = min_shift;
        context.planner_settings.max_shift_length = max_shift;
        let array = bodies(&context, start_index, &array);
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(start_date_time);

        find_planned_shift_endpoint(&mut bean, &array, &params);

        assert_eq!(bean.ending_index(), ending_index);
    }

    #[rstest]
    #[case(16, EXAMPLE_1, 25, at(date(), 8, 0), time(16, 0))]
    #[case(46, EXAMPLE_1, 55, at(date(), 23, 0), time(4, 0))]
    #[case(16, EXAMPLE_3, 25, at(date(), 8, 0), time(16, 0))]
    #[case(46, EXAMPLE_3, 55, at(date(), 23, 0), time(4, 0))]
    fn find_planned_long_shift_endpoint_table(
        #[case] start_index: usize,
        #[case] array: [i32; 9],
        #[case] ending_index: i32,
        #[case] start_date_time: LocalDateTime,
        #[case] shift_end_time: LocalTime,
    ) {
        let mut context = Context::new(date(), start_date_time.to_local_time(), shift_end_time, 30);
        context.planner_settings.min_shift_length = 4.0;
        context.planner_settings.max_shift_length = 8.0;
        let array = bodies(&context, start_index, &array);
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(start_date_time);

        find_planned_long_shift_endpoint(&mut bean, &array, &params);

        assert_eq!(bean.ending_index(), ending_index);
    }

    /// A block starting deep into the array's second day: the shift's maximum
    /// would run past the end, so it is cut to the array and not trimmed.
    #[test]
    fn find_planned_long_shift_endpoint_period_length_15_and_endpoint_outside_periods() {
        let shift_date = LocalDate::new(2023, 12, 9);
        let mut context = Context::new(shift_date, time(6, 30), time(22, 0), 15);
        context.planner_settings.min_shift_length = 4.0;
        context.planner_settings.max_shift_length = 8.0;
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);
        array
            .iter_mut()
            .for_each(|item| item.set_value_per_period(1.0));

        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(at(shift_date.plus_days(1), 22, 30));

        find_planned_long_shift_endpoint(&mut bean, &array, &params);

        assert_eq!(bean.start_date_time(), at(shift_date.plus_days(1), 22, 30));
        assert_eq!(bean.end_date_time(), at(shift_date.plus_days(1), 23, 45));
        assert_eq!(params.periods_per_day(), 96);
        assert_eq!(bean.min_value(), 0);
        assert_eq!(bean.starting_index(), 186);
        assert_eq!(bean.ending_index(), 191);
    }

    /// Java's guard that a block starting past the end of the array does not
    /// run off it.
    #[test]
    fn find_planned_long_shift_endpoint_with_index_limits() {
        let shift_date = LocalDate::new(2024, 12, 6);
        let mut context = Context::new(shift_date, time(6, 30), time(22, 0), 15);
        context.planner_settings.min_shift_length = 4.0;
        context.planner_settings.max_shift_length = 8.0;
        let params = context.params();
        let array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(at(shift_date.plus_days(1), 23, 15));

        find_planned_long_shift_endpoint(&mut bean, &array, &params);

        assert_ne!(bean.end_date_time(), at(shift_date.plus_days(1), 23, 15));
    }

    #[rstest]
    #[case(32, at(date(), 8, 0), at(date(), 16, 0), 4.0, 8.0)]
    #[case(22, at(date(), 8, 0), at(date(), 11, 0), 4.0, 8.0)]
    #[case(32, at(date(), 8, 0), at(date(), 16, 0), 8.0, 8.0)]
    #[case(22, at(date(), 8, 0), at(date(), 11, 0), 8.0, 10.0)]
    fn adjusting_for_min_shift(
        #[case] ending_index: i32,
        #[case] start_date_time: LocalDateTime,
        #[case] end_date_time: LocalDateTime,
        #[case] min_shift: f64,
        #[case] max_shift: f64,
    ) {
        let mut context = Context::new(
            date(),
            start_date_time.to_local_time(),
            end_date_time.to_local_time(),
            30,
        );
        context.planner_settings.min_shift_length = min_shift;
        context.planner_settings.max_shift_length = max_shift;
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(start_date_time);
        bean.set_end_date_time(end_date_time);

        adjust_for_min_shift(&mut bean, end_date_time, &params);

        assert_eq!(bean.ending_index(), ending_index);
    }

    /// The minimum shift is capped at the shift's own length, so the last three
    /// rows — a sixteen-hour minimum against a six-hour shift — stretch the
    /// block to six hours and, doing so, take it outside the shift window.
    /// That is the intended behaviour and must not be clamped away.
    #[rstest]
    #[case(8.0, on(1, 8, 0), on(1, 10, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 8, 0), on(1, 12, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 8, 0), on(1, 14, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 8, 0), on(1, 16, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 10, 0), on(1, 12, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 10, 0), on(1, 14, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(8.0, on(1, 10, 0), on(1, 16, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(4.0, on(1, 8, 0), on(1, 10, 0), on(1, 8, 0), on(1, 16, 0), 16, 24)]
    #[case(4.0, on(1, 8, 0), on(1, 12, 0), on(1, 8, 0), on(1, 16, 0), 16, 24)]
    #[case(4.0, on(1, 8, 0), on(1, 14, 0), on(1, 8, 0), on(1, 16, 0), 16, 28)]
    #[case(4.0, on(1, 8, 0), on(1, 16, 0), on(1, 8, 0), on(1, 16, 0), 16, 32)]
    #[case(4.0, on(1, 14, 0), on(1, 16, 0), on(1, 11, 0), on(1, 16, 0), 24, 32)]
    #[case(4.0, on(1, 13, 0), on(1, 15, 0), on(1, 11, 0), on(1, 16, 0), 24, 32)]
    #[case(4.0, on(1, 12, 0), on(1, 14, 0), on(1, 11, 0), on(1, 16, 0), 24, 32)]
    #[case(4.0, on(1, 11, 0), on(1, 13, 0), on(1, 11, 0), on(1, 16, 0), 22, 30)]
    #[case(8.0, on(1, 22, 0), on(2, 0, 0), on(1, 22, 0), on(2, 6, 0), 44, 60)]
    #[case(16.0, on(1, 21, 30), on(1, 22, 30), on(1, 22, 0), on(2, 4, 0), 43, 55)]
    #[case(16.0, on(2, 2, 0), on(2, 4, 30), on(1, 22, 0), on(2, 4, 0), 45, 57)]
    #[case(16.0, on(1, 21, 30), on(2, 4, 30), on(1, 22, 0), on(2, 4, 0), 43, 57)]
    fn test_adjust_for_min_shift(
        #[case] min_shift: f64,
        #[case] work_start: LocalDateTime,
        #[case] work_end: LocalDateTime,
        #[case] shift_start: LocalDateTime,
        #[case] shift_end: LocalDateTime,
        #[case] starting_index: i32,
        #[case] ending_index: i32,
    ) {
        let mut context = Context::new(
            shift_start.to_local_date(),
            shift_start.to_local_time(),
            shift_end.to_local_time(),
            30,
        );
        context.planner_settings.min_shift_length = min_shift;
        context.planner_settings.max_shift_length = 8.0;
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(work_start);
        bean.set_end_date_time(work_end);

        adjust_for_min_shift(&mut bean, shift_end, &params);

        assert_eq!(bean.starting_index(), starting_index);
        assert_eq!(bean.ending_index(), ending_index);
    }
}

#[cfg(test)]
mod tests {
    //! Behaviour the Java suite left uncovered, in particular the two
    //! minimum-finding rules whose difference no Java table exercises.

    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2013, 8, 26)
    }

    fn context() -> Context {
        Context::new(
            date(),
            LocalTime::new(7, 30, 0),
            LocalTime::new(12, 0, 0),
            30,
        )
    }

    fn bodies(context: &Context, start_index: usize, values: &[f64]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value);
        }

        array
    }

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        date().at_time(LocalTime::new(hour, minute, 0))
    }

    /// The two minimum-finding rules are genuinely different, and this is where
    /// it shows: an uncovered period inside the block pulls the work-content
    /// depth to zero, but the planned-shift rule steps over it.
    #[test]
    fn an_uncovered_period_inside_a_block_is_treated_differently_by_each_rule() {
        let context = context();
        let array = bodies(&context, 15, &[5.0, 5.0, 0.0, 5.0, 5.0]);
        let params = context.params();

        let mut work_content = WorkContentTrackerBean::new(&params);
        work_content.set_start_date_time(at(7, 30));
        work_content.set_end_date_time(at(10, 0));
        find_minimum_value_in_range(&mut work_content, &array);

        let mut planned_shift = WorkContentTrackerBean::new(&params);
        planned_shift.set_start_date_time(at(7, 30));
        planned_shift.set_end_date_time(at(10, 0));
        planned_shift.set_min_value(5);
        find_planned_shift_minimum_value_in_range(&mut planned_shift, &array);

        assert_eq!(work_content.min_value(), 0);
        assert_eq!(planned_shift.min_value(), 5);
    }

    /// A placeholder period is not real coverage and is passed over.
    #[test]
    fn a_placeholder_period_is_passed_over_when_measuring_depth() {
        let context = context();
        let array = bodies(&context, 15, &[5.0, SENTINEL, 5.0]);
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(at(7, 30));
        bean.set_end_date_time(at(9, 0));

        find_minimum_value_in_range(&mut bean, &array);

        assert_eq!(bean.min_value(), 5);
    }

    /// A shift stops at the first gap; a long shift spans it. Java tables cover
    /// each function alone, never the contrast.
    #[test]
    fn a_shift_stops_at_the_first_gap_where_a_long_shift_would_not() {
        let mut context = context();
        context.planner_settings.max_shift_length = 8.0;
        let array = bodies(&context, 16, &[5.0, 5.0, 0.0, 5.0, 5.0, 5.0, 5.0]);
        let params = context.params();

        let mut shift = WorkContentTrackerBean::new(&params);
        shift.set_start_date_time(at(8, 0));
        find_planned_shift_endpoint(&mut shift, &array, &params);

        let mut long_shift = WorkContentTrackerBean::new(&params);
        long_shift.set_start_date_time(at(8, 0));
        find_planned_long_shift_endpoint(&mut long_shift, &array, &params);

        assert_eq!(shift.end_date_time(), at(9, 0));
        assert_eq!(long_shift.end_date_time(), at(11, 30));
    }

    /// Only periods that reach the depth are reduced; a shallower one is left
    /// alone rather than going negative.
    #[test]
    fn a_period_shallower_than_the_layer_is_left_alone() {
        let context = context();
        let mut array = bodies(&context, 15, &[5.0, 1.0, 5.0]);
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(at(7, 30));
        bean.set_end_date_time(at(9, 0));
        bean.set_min_value(5);

        deduct_minimum_value_from_list_items(&mut array, &bean);

        assert_eq!(array[15].value_per_period(), 0.0);
        assert_eq!(array[16].value_per_period(), 1.0);
        assert_eq!(array[17].value_per_period(), 0.0);
    }
}
