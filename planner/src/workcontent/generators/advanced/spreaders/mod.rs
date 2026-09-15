//! The shapes work can take across the periods of a shift.
//!
//! A spreader is handed a total number of work minutes and an array to lay them
//! into. Which periods it fills, and in what order, is the whole of its
//! behavior — the arithmetic is deliberately simple so the resulting curve is
//! predictable to whoever configured it.

pub mod beginning;
pub mod breaks;
pub mod ending;
pub mod even;
pub mod middle;
pub mod varying;

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;

pub trait WorkSpreader {
    /// Lay `total_work_minutes` into `target` across the periods of `range`.
    fn populate_array_with_work_minutes(
        &self,
        total_work_minutes: f64,
        params: &GeneratorParameters,
        range: &DateTimeRangeWithPeriodLength,
        target: &mut [DistributionItem],
    );

    /// Lay work in around work that is already there.
    ///
    /// Only the break spreader has anything to say here; Java gives the
    /// interface a do-nothing default and every other spreader takes it.
    fn populate_against_existing_work(
        &self,
        _total_work_minutes: f64,
        _params: &GeneratorParameters,
        _range: &DateTimeRangeWithPeriodLength,
        _target: &mut [DistributionItem],
        _existing_work: &[DistributionItem],
    ) {
    }
}

/// The first and last period a spreader may write to.
///
/// The range's end index is the period the shift *ends* in, which it does not
/// occupy, so the last usable period is one before it. When the settings cap
/// shifts at the maximum length, the window is shortened to fit.
///
/// `None` when the range has no granularity to index by.
pub fn spread_bounds(
    params: &GeneratorParameters,
    range: &DateTimeRangeWithPeriodLength,
) -> Option<(i32, i32)> {
    let starting_index = range.start_index()?;
    let ending_index = range.end_index()? - 1;

    let ending_index = if params.planner_settings().limit_shift_to_max_shift {
        adjust_ending_index_for_limit_max_shift(params, starting_index, ending_index)
    } else {
        ending_index
    };

    Some((starting_index, ending_index))
}

/// The spread window as a half-open range, `[start, end)`.
///
/// The even and varying spreaders index differently from the rest: they keep
/// the range's own end index rather than stepping back from it, and they guard
/// the max-shift clamp behind an explicit length check before adding one back.
/// Java flags both as a HACK and notes the inconsistency; it is replicated here
/// because the clamped window is a period wider than the other spreaders would
/// produce, and the tables depend on that.
pub fn exclusive_spread_bounds(
    params: &GeneratorParameters,
    range: &DateTimeRangeWithPeriodLength,
) -> Option<(i32, i32)> {
    let starting_index = range.start_index()?;
    let ending_index = range.end_index()?;

    let ending_index = if params.planner_settings().limit_shift_to_max_shift
        && ending_index - starting_index > params.max_shift_length_in_periods()
    {
        adjust_ending_index_for_limit_max_shift(params, starting_index, ending_index) + 1
    } else {
        ending_index
    };

    Some((starting_index, ending_index))
}

/// Shorten a window that runs longer than the maximum shift.
pub fn adjust_ending_index_for_limit_max_shift(
    params: &GeneratorParameters,
    starting_index: i32,
    ending_index: i32,
) -> i32 {
    let max_shift_in_periods = params.max_shift_length_in_periods();

    if ending_index - starting_index >= max_shift_in_periods {
        starting_index + max_shift_in_periods - 1
    } else {
        ending_index
    }
}

/// How much of the remaining work goes into one period: a full period's worth,
/// or whatever is left if that is less.
pub fn minutes_for_next_period(remaining_work_minutes: f64, period_length: i32) -> f64 {
    let period_length = period_length as f64;

    if remaining_work_minutes > period_length {
        period_length
    } else {
        remaining_work_minutes
    }
}

/// Whether there is still work worth placing.
///
/// Rounded before the comparison so that a residue far below a whole minute
/// does not keep the loop running.
pub fn has_work_left(remaining_work_minutes: f64) -> bool {
    crate::workcontent::common::numbers::round_raw_hours(remaining_work_minutes) > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    fn context(limit_shift_to_max_shift: bool) -> Context {
        let mut context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        );
        context.planner_settings.limit_shift_to_max_shift = limit_shift_to_max_shift;
        context
    }

    #[test]
    fn the_window_stops_one_period_before_the_shift_ends() {
        let context = context(false);
        let params = context.params();
        let range = params.shift_date_range().expect("has times");

        // 08:00 is period 16 and 16:00 is period 32, so the last period the
        // shift occupies is 31.
        assert_eq!(spread_bounds(&params, &range), Some((16, 31)));
    }

    #[test]
    fn capping_shifts_at_the_maximum_shortens_the_window() {
        let context = context(true);
        let params = context.params();
        let range = params.shift_date_range().expect("has times");

        // The default maximum is 8 hours — 16 half-hour periods — so the
        // window runs 16 through 31 and is already exactly at the cap.
        assert_eq!(spread_bounds(&params, &range), Some((16, 31)));
    }

    #[rstest]
    // A window shorter than the maximum is left alone.
    #[case(16, 20, 20)]
    // One exactly at the maximum is clamped to it, which is a no-op here.
    #[case(16, 32, 31)]
    // A longer one is cut back to the maximum.
    #[case(16, 47, 31)]
    fn a_window_longer_than_the_maximum_is_cut_back(
        #[case] starting_index: i32,
        #[case] ending_index: i32,
        #[case] expected: i32,
    ) {
        let context = context(true);

        assert_eq!(
            adjust_ending_index_for_limit_max_shift(
                &context.params(),
                starting_index,
                ending_index
            ),
            expected
        );
    }

    #[rstest]
    #[case(100.0, 30, 30.0)]
    #[case(30.0, 30, 30.0)]
    #[case(10.0, 30, 10.0)]
    fn a_period_takes_a_full_period_or_what_is_left(
        #[case] remaining: f64,
        #[case] period_length: i32,
        #[case] expected: f64,
    ) {
        assert_eq!(minutes_for_next_period(remaining, period_length), expected);
    }

    #[test]
    fn a_residue_below_rounding_is_not_worth_placing() {
        assert!(has_work_left(0.001));
        assert!(!has_work_left(0.000_01));
        assert!(!has_work_left(0.0));
    }
}
