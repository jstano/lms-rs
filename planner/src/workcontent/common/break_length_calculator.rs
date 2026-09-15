//! Paid-break arithmetic, ported from the Java `BreakLengthCalculator`.
//!
//! Two questions are answered here:
//!
//! * given a block of scheduled time, how much of it is paid break?
//! * given a block of scheduled time, how much of it is productive?
//!
//! They are not simple complements of one another: the break calculation asks
//! how many break entitlements a block of that length triggers, while the
//! productive calculation walks the block down one break at a time, checking
//! that the block is long enough to contain both the work and the break.
//!
//! Java threads every length through a `Duration` whose resolution is whole
//! minutes, so fractional-hour inputs are floored to the minute first; see
//! [`as_fractional_hours`].

use crate::workcontent::domain::meal_break::MealBreak;
use crate::workcontent::domain::non_meal_break::NonMealBreak;
use crate::workcontent::common::numbers;

/// Paid break hours incurred by a block of `hours` scheduled time.
pub fn calculate_break_in_fractional_hours(
    hours: f64,
    meal_break: Option<&MealBreak>,
    non_meal_break: Option<&NonMealBreak>,
) -> f64 {
    let (meal_break, non_meal_break) = match (meal_break, non_meal_break) {
        (Some(meal), Some(non_meal)) => (meal, non_meal),
        _ => return 0.0,
    };

    if meal_break.is_no_break() && non_meal_break.is_no_break() {
        return 0.0;
    }

    let shift_length = as_fractional_hours(hours);

    if meal_break.is_no_break() {
        return break_given_only_non_meal_break(shift_length, non_meal_break);
    }

    if non_meal_break.is_no_break() {
        return break_given_only_meal_break(shift_length, meal_break);
    }

    break_given_both_break_types(shift_length, meal_break, non_meal_break)
}

/// Productive hours contained in a block of `hours` scheduled time.
pub fn calculate_productive_time_from_total_duration(
    hours: f64,
    meal_break: Option<&MealBreak>,
    non_meal_break: Option<&NonMealBreak>,
) -> f64 {
    let total_duration = as_fractional_hours(hours);

    let (meal_break, non_meal_break) = match (meal_break, non_meal_break) {
        (Some(meal), Some(non_meal)) => (meal, non_meal),
        _ => return total_duration,
    };

    if meal_break.is_no_break() && non_meal_break.is_no_break() {
        return total_duration;
    }

    if non_meal_break.is_no_break() {
        return productive_time_given_meal_break_only(total_duration, meal_break);
    }

    if meal_break.is_no_break() {
        return productive_time_given_non_meal_break_only(total_duration, non_meal_break);
    }

    productive_time_given_both_break_types(total_duration, meal_break, non_meal_break)
}

/// Floor a fractional-hour value to whole minutes.
///
/// Java converts hours to a `Duration` of whole seconds and reads it back with
/// `Duration::toMinutes`, which truncates. Doing the same here keeps the two
/// implementations bit-for-bit comparable on inputs that are not a whole
/// number of minutes.
fn as_fractional_hours(hours: f64) -> f64 {
    let seconds = (hours * 3600.0) as i64;

    (seconds / 60) as f64 / 60.0
}

fn break_given_both_break_types(
    shift_length: f64,
    meal_break: &MealBreak,
    non_meal_break: &NonMealBreak,
) -> f64 {
    if shift_length < non_meal_break.break_every {
        return 0.0;
    }

    if shift_length < meal_break.break_after {
        return non_meal_break.break_length;
    }

    if shift_length < first_multiple_of_non_meal_break_after_meal_break(meal_break, non_meal_break) {
        return non_meal_break.break_length + meal_break.break_length;
    }

    non_meal_break.break_length * 2.0 + meal_break.break_length
}

fn break_given_only_meal_break(shift_length: f64, meal_break: &MealBreak) -> f64 {
    if shift_length < meal_break.break_after {
        0.0
    } else {
        meal_break.break_length
    }
}

fn break_given_only_non_meal_break(shift_length: f64, non_meal_break: &NonMealBreak) -> f64 {
    if shift_length < non_meal_break.break_every {
        return 0.0;
    }

    let remaining_shift = shift_length - non_meal_break.break_every;
    let number_of_breaks = 1
        + numbers::truncate(
            remaining_shift / (non_meal_break.break_every + non_meal_break.break_length),
        );

    number_of_breaks as f64 * non_meal_break.break_length
}

fn productive_time_given_both_break_types(
    shift_length: f64,
    meal_break: &MealBreak,
    non_meal_break: &NonMealBreak,
) -> f64 {
    let mut productive_time = shift_length;

    if productive_time >= non_meal_break.break_every + non_meal_break.break_length {
        productive_time -= non_meal_break.break_length;
    }

    if productive_time >= meal_break.break_after + meal_break.break_length {
        productive_time -= meal_break.break_length;
    }

    if productive_time
        >= first_multiple_of_non_meal_break_after_meal_break(meal_break, non_meal_break)
            + non_meal_break.break_length
    {
        productive_time -= non_meal_break.break_length;
    }

    productive_time
}

fn productive_time_given_meal_break_only(shift_length: f64, meal_break: &MealBreak) -> f64 {
    if shift_length >= meal_break.break_after + meal_break.break_length {
        shift_length - meal_break.break_length
    } else {
        shift_length
    }
}

fn productive_time_given_non_meal_break_only(
    shift_length: f64,
    non_meal_break: &NonMealBreak,
) -> f64 {
    let total_number_of_breaks = numbers::truncate(
        shift_length / (non_meal_break.break_every + non_meal_break.break_length),
    );

    shift_length - total_number_of_breaks as f64 * non_meal_break.break_length
}

/// The first whole multiple of the rest-break interval that falls strictly
/// after the meal break is due — i.e. when the second rest break lands.
fn first_multiple_of_non_meal_break_after_meal_break(
    meal_break: &MealBreak,
    non_meal_break: &NonMealBreak,
) -> f64 {
    let mut multiple = 0.0;

    while multiple <= meal_break.break_after {
        multiple += non_meal_break.break_every;
    }

    multiple
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn meal() -> MealBreak {
        MealBreak::new(5.0, 0.5)
    }

    fn non_meal() -> NonMealBreak {
        NonMealBreak::new(2.0, 0.25)
    }

    #[test]
    fn a_missing_break_rule_yields_no_break_time() {
        assert_eq!(calculate_break_in_fractional_hours(8.0, None, None), 0.0);
        assert_eq!(
            calculate_break_in_fractional_hours(8.0, Some(&meal()), None),
            0.0
        );
        assert_eq!(
            calculate_break_in_fractional_hours(8.0, None, Some(&non_meal())),
            0.0
        );
    }

    #[test]
    fn a_missing_break_rule_leaves_the_whole_block_productive() {
        assert_eq!(
            calculate_productive_time_from_total_duration(8.0, None, None),
            8.0
        );
        assert_eq!(
            calculate_productive_time_from_total_duration(8.0, Some(&meal()), None),
            8.0
        );
    }

    #[test]
    fn two_no_break_rules_yield_no_break_time() {
        let no_meal = MealBreak::with_no_break();
        let no_non_meal = NonMealBreak::with_no_break();

        assert_eq!(
            calculate_break_in_fractional_hours(8.0, Some(&no_meal), Some(&no_non_meal)),
            0.0
        );
        assert_eq!(
            calculate_productive_time_from_total_duration(8.0, Some(&no_meal), Some(&no_non_meal)),
            8.0
        );
    }

    // Meal break only: nothing until `break_after`, then the full break length.
    #[rstest]
    #[case(4.99, 0.0)]
    #[case(5.0, 0.5)]
    #[case(8.0, 0.5)]
    fn break_with_meal_break_only(#[case] hours: f64, #[case] expected: f64) {
        let no_non_meal = NonMealBreak::with_no_break();

        assert_eq!(
            calculate_break_in_fractional_hours(hours, Some(&meal()), Some(&no_non_meal)),
            expected
        );
    }

    // The block must be long enough to hold the work *and* the break before the
    // break comes out of it, so the boundary is `break_after + break_length`.
    #[rstest]
    #[case(5.0, 5.0)]
    #[case(5.49, 5.483333333333333)]
    #[case(5.5, 5.0)]
    #[case(8.0, 7.5)]
    fn productive_time_with_meal_break_only(#[case] hours: f64, #[case] expected: f64) {
        let no_non_meal = NonMealBreak::with_no_break();
        let actual =
            calculate_productive_time_from_total_duration(hours, Some(&meal()), Some(&no_non_meal));

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // Rest break only: one break at `break_every`, then one more for every
    // further `break_every + break_length` of the block.
    #[rstest]
    #[case(1.99, 0.0)]
    #[case(2.0, 0.25)]
    #[case(4.24, 0.25)]
    #[case(4.25, 0.5)]
    #[case(8.0, 0.75)]
    fn break_with_non_meal_break_only(#[case] hours: f64, #[case] expected: f64) {
        let no_meal = MealBreak::with_no_break();
        let actual =
            calculate_break_in_fractional_hours(hours, Some(&no_meal), Some(&non_meal()));

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[rstest]
    #[case(2.0, 2.0)]
    #[case(2.25, 2.0)]
    #[case(4.49, 4.233333333333333)]
    #[case(4.5, 4.0)]
    #[case(8.0, 7.25)]
    fn productive_time_with_non_meal_break_only(#[case] hours: f64, #[case] expected: f64) {
        let no_meal = MealBreak::with_no_break();
        let actual = calculate_productive_time_from_total_duration(
            hours,
            Some(&no_meal),
            Some(&non_meal()),
        );

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // Both rules. With break_every 2.0 and break_after 5.0 the first multiple of
    // the rest interval falling after the meal break is 6.0.
    #[rstest]
    #[case(1.99, 0.0)]
    #[case(2.0, 0.25)]
    #[case(4.99, 0.25)]
    #[case(5.0, 0.75)]
    #[case(5.99, 0.75)]
    #[case(6.0, 1.0)]
    #[case(8.0, 1.0)]
    fn break_with_both_break_types(#[case] hours: f64, #[case] expected: f64) {
        let actual =
            calculate_break_in_fractional_hours(hours, Some(&meal()), Some(&non_meal()));

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[rstest]
    #[case(2.0, 2.0)]
    #[case(2.25, 2.0)]
    #[case(6.0, 5.25)]
    #[case(8.0, 7.0)]
    fn productive_time_with_both_break_types(#[case] hours: f64, #[case] expected: f64) {
        let actual = calculate_productive_time_from_total_duration(
            hours,
            Some(&meal()),
            Some(&non_meal()),
        );

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn first_multiple_after_the_meal_break_is_strictly_greater() {
        // break_after 5.0 with break_every 2.0 -> 2, 4, 6
        assert_eq!(
            first_multiple_of_non_meal_break_after_meal_break(&meal(), &non_meal()),
            6.0
        );

        // An exact multiple still steps past it.
        let meal_at_four = MealBreak::new(4.0, 0.5);
        assert_eq!(
            first_multiple_of_non_meal_break_after_meal_break(&meal_at_four, &non_meal()),
            6.0
        );
    }

    #[test]
    fn fractional_hours_are_floored_to_whole_minutes() {
        // 2 hours and 59 seconds is still 2 hours' worth of minutes.
        assert_eq!(as_fractional_hours(2.0 + 59.0 / 3600.0), 2.0);
        assert_eq!(as_fractional_hours(2.5), 2.5);
    }
}
