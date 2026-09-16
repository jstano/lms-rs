//! The parts of `com.unifocus.tbx.core.DTUtil` and `DateUtil` that the rules
//! engine actually calls.
//!
//! Ground truth: `~/workspace/unifocus/legacy-tbx-10.0/legacy-tbx-core/src/main/
//! java/com/unifocus/tbx/core/{DTUtil,DateUtil}.java`.
//!
//! `DateUtil` is 677 lines, but more than half of it is `Locale`/`NumberFormat`
//! display formatting that no rule touches. Only the pure date arithmetic is
//! ported here; the formatting half stays behind with the rest of the UI layer.
//!
//! Java passes `null` dates freely and the `safe*` helpers exist to absorb
//! that. Those take `Option` here; everything else takes values, because the
//! call sites that reach them cannot be null.

use crate::common::rounding::RoundingOption;
use joda_rs::{LocalDate, LocalDateTime};

const MILLIS_PER_SECOND: f64 = 1000.0;
const MILLIS_PER_MINUTE: f64 = 60_000.0;
const MILLIS_PER_HOUR: f64 = 3_600_000.0;
const SECONDS_PER_DAY: i64 = joda_rs::constants::SECONDS_PER_DAY;

fn epoch_millis(date_time: LocalDateTime) -> i128 {
    date_time.epoch_nanoseconds() / 1_000_000
}

fn epoch_day(date: LocalDate) -> i64 {
    date.at_start_of_day()
        .epoch_seconds()
        .div_euclid(SECONDS_PER_DAY)
}

/// Round a time to a multiple of `threshold_minutes` within its own day.
///
/// `DTUtil.roundDateTimeToThresholdMinutes`. The rounding is applied to the
/// second-of-day, then the difference is added back to the original instant —
/// so rounding 23:56 up to a 15-minute boundary lands on 00:00 the *next* day,
/// which the punch-rounding rule's own test table asserts.
pub fn round_date_time_to_threshold_minutes(
    time: LocalDateTime,
    threshold_minutes: i32,
    option: RoundingOption,
) -> LocalDateTime {
    let threshold_seconds = threshold_minutes * 60;

    let total_seconds = time.hour() * 3600 + time.minute() * 60 + time.second();
    let rounded_seconds = option.round(total_seconds, threshold_seconds);

    time.plus_seconds(i64::from(rounded_seconds - total_seconds))
}

/// Whole hours between two times, truncated toward zero. `DTUtil.durationInHours`.
pub fn duration_in_hours(start: LocalDateTime, end: LocalDateTime) -> i32 {
    let diff = (epoch_millis(end) - epoch_millis(start)) as f64;
    (diff / MILLIS_PER_HOUR) as i32
}

/// Whole minutes between two times, truncated toward zero. `DTUtil.durationInMinutes`.
pub fn duration_in_minutes(start: LocalDateTime, end: LocalDateTime) -> i32 {
    let diff = (epoch_millis(end) - epoch_millis(start)) as f64;
    (diff / MILLIS_PER_MINUTE) as i32
}

/// Whole seconds between two times, truncated toward zero. `DTUtil.durationInSeconds`.
pub fn duration_in_seconds(start: LocalDateTime, end: LocalDateTime) -> i32 {
    let diff = (epoch_millis(end) - epoch_millis(start)) as f64;
    (diff / MILLIS_PER_SECOND) as i32
}

/// Hours between two times as a fraction. `DTUtil.durationInFractionalHours`.
pub fn duration_in_fractional_hours(start: LocalDateTime, end: LocalDateTime) -> f64 {
    let diff = (epoch_millis(end) - epoch_millis(start)) as f64;
    diff / MILLIS_PER_HOUR
}

/// Is `date_time` within `[start, end]`? `DTUtil.isBetween` — bounds included.
pub fn is_between(date_time: LocalDateTime, start: LocalDateTime, end: LocalDateTime) -> bool {
    date_time.is_on_or_after(start) && date_time.is_on_or_before(end)
}

/// Is `date_time` within `(start, end)`? `DTUtil.isBetweenExclusive`.
pub fn is_between_exclusive(
    date_time: LocalDateTime,
    start: LocalDateTime,
    end: LocalDateTime,
) -> bool {
    date_time.is_after(start) && date_time.is_before(end)
}

/// The earlier of two times, tolerating a missing one. `DTUtil.earliest`.
pub fn earliest(
    time1: Option<LocalDateTime>,
    time2: Option<LocalDateTime>,
) -> Option<LocalDateTime> {
    match (time1, time2) {
        (Some(a), Some(b)) => Some(if a.is_on_or_before(b) { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// The later of two times, tolerating a missing one. `DTUtil.latest`.
pub fn latest(time1: Option<LocalDateTime>, time2: Option<LocalDateTime>) -> Option<LocalDateTime> {
    match (time1, time2) {
        (Some(a), Some(b)) => Some(if a.is_on_or_after(b) { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// Translate a day number from ISO (Mon=1, Sun=7) to the engine's own
/// convention (Sun=1, Sat=7). `DateUtil.translateDOWFromISO`.
///
/// The single most-used `DateUtil` method in the rules engine. The two
/// conventions coexist because Joda counts from Monday while the database and
/// the rule parameters count from Sunday — mixing them up shifts a whole rule
/// by one day, silently.
pub fn translate_dow_from_iso(day_of_week: i32) -> i32 {
    if day_of_week == 7 { 1 } else { day_of_week + 1 }
}

/// Translate a day number from Sun=1, Sat=7 to ISO Mon=1, Sun=7.
/// `DateUtil.translateDOWToISO`.
pub fn translate_dow_to_iso(day_of_week: i32) -> i32 {
    if day_of_week == 1 { 7 } else { day_of_week - 1 }
}

/// Whole days from `start` to `end`, negative if `end` precedes `start`.
/// `DateUtil.daysBetween`.
pub fn days_between(start: LocalDate, end: LocalDate) -> i32 {
    (epoch_day(end) - epoch_day(start)) as i32
}

/// Whole months from `start` to `end`. `DateUtil.monthsBetween`.
///
/// Joda defines this as the largest `n` with `start.plusMonths(n) <= end`,
/// which is not the same as differencing the year and month fields: Joda clamps
/// day-of-month when adding, so 31 Jan to 28 Feb is one whole month, not zero.
/// The estimate-then-correct shape below reproduces that; a field subtraction
/// would not.
pub fn months_between(start: LocalDate, end: LocalDate) -> i32 {
    let mut months = (end.year() - start.year()) * 12 + (end.month_value() - start.month_value());

    // Walk back while we have overshot.
    while months > 0 && start.plus_months(months).is_after(end) {
        months -= 1;
    }
    while months < 0 && start.plus_months(months).is_before(end) {
        months += 1;
    }
    // Walk forward while another whole month still fits.
    while start.plus_months(months + 1).is_on_or_before(end) {
        months += 1;
    }
    while months < 0 && start.plus_months(months).is_after(end) {
        months -= 1;
    }

    months
}

/// The first day of the quarter containing `date`. `DateUtil.getStartOfQuarter`.
pub fn start_of_quarter(date: LocalDate) -> LocalDate {
    let into_quarter = (date.month_value() - 1) % 3;
    let start = date.minus_months(into_quarter);
    start.minus_days(i64::from(start.day_of_month() - 1))
}

/// The later of two dates, tolerating a missing one. `DateUtil.safeMaxDate`.
///
/// Used by `AbstractRunner.calculateAdjustedDate` to clamp a job status start
/// date into the work week being processed.
pub fn safe_max_date(date1: Option<LocalDate>, date2: Option<LocalDate>) -> Option<LocalDate> {
    match (date1, date2) {
        (Some(a), Some(b)) => Some(if a.is_before(b) { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// The earlier of two dates, tolerating a missing one. `DateUtil.safeMinDate`.
pub fn safe_min_date(date1: Option<LocalDate>, date2: Option<LocalDate>) -> Option<LocalDate> {
    match (date1, date2) {
        (Some(a), Some(b)) => Some(if a.is_after(b) { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// The later of two times, tolerating a missing one. `DateUtil.safeMaxDateTime`.
pub fn safe_max_date_time(
    time1: Option<LocalDateTime>,
    time2: Option<LocalDateTime>,
) -> Option<LocalDateTime> {
    match (time1, time2) {
        (Some(a), Some(b)) => Some(if a.is_before(b) { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// The earlier of two times, tolerating a missing one. `DateUtil.safeMinDateTime`.
pub fn safe_min_date_time(
    time1: Option<LocalDateTime>,
    time2: Option<LocalDateTime>,
) -> Option<LocalDateTime> {
    match (time1, time2) {
        (Some(a), Some(b)) => Some(if a.is_after(b) { b } else { a }),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn dt(y: i32, m: i32, d: i32, h: i32, min: i32, s: i32) -> LocalDateTime {
        LocalDateTime::of(y, m, d, h, min, s)
    }

    fn date(y: i32, m: i32, d: i32) -> LocalDate {
        LocalDate::of(y, m, d)
    }

    #[test]
    fn rounding_to_a_threshold_moves_the_time_to_the_boundary() {
        let rounded = round_date_time_to_threshold_minutes(
            dt(2010, 1, 2, 7, 40, 0),
            15,
            RoundingOption::Nearest,
        );
        assert_eq!(rounded, dt(2010, 1, 2, 7, 45, 0));
    }

    #[test]
    fn rounding_up_can_cross_into_the_next_day() {
        // Straight from MinuteRoundingRuleImplTest.groovy's "round to midnight"
        // row: the seconds-of-day arithmetic overflows a day and the date moves.
        let rounded = round_date_time_to_threshold_minutes(
            dt(2013, 4, 1, 23, 56, 0),
            15,
            RoundingOption::Nearest,
        );
        assert_eq!(rounded, dt(2013, 4, 2, 0, 0, 0));
    }

    #[test]
    fn a_threshold_of_one_minute_leaves_a_whole_minute_alone() {
        let time = dt(2010, 1, 2, 7, 47, 0);
        assert_eq!(
            round_date_time_to_threshold_minutes(time, 1, RoundingOption::Nearest),
            time
        );
    }

    #[test]
    fn rounding_with_none_leaves_the_time_untouched() {
        let time = dt(2010, 1, 2, 7, 40, 0);
        assert_eq!(
            round_date_time_to_threshold_minutes(time, 15, RoundingOption::None),
            time
        );
    }

    #[test]
    fn durations_truncate_rather_than_round() {
        let start = dt(2010, 1, 2, 7, 0, 0);
        let end = dt(2010, 1, 2, 9, 59, 59);

        assert_eq!(duration_in_hours(start, end), 2);
        assert_eq!(duration_in_minutes(start, end), 179);
        assert_eq!(duration_in_seconds(start, end), 10799);
    }

    #[test]
    fn a_fractional_duration_keeps_the_remainder() {
        let start = dt(2010, 1, 2, 7, 0, 0);
        let end = dt(2010, 1, 2, 10, 30, 0);

        assert_eq!(duration_in_fractional_hours(start, end), 3.5);
    }

    #[test]
    fn a_backwards_duration_is_negative() {
        let start = dt(2010, 1, 2, 10, 0, 0);
        let end = dt(2010, 1, 2, 7, 30, 0);

        assert_eq!(duration_in_fractional_hours(start, end), -2.5);
        assert_eq!(duration_in_hours(start, end), -2);
    }

    #[test]
    fn is_between_includes_its_bounds_and_the_exclusive_form_does_not() {
        let start = dt(2010, 1, 2, 7, 0, 0);
        let end = dt(2010, 1, 2, 9, 0, 0);

        assert!(is_between(start, start, end));
        assert!(is_between(end, start, end));
        assert!(is_between(dt(2010, 1, 2, 8, 0, 0), start, end));
        assert!(!is_between(dt(2010, 1, 2, 6, 59, 59), start, end));

        assert!(!is_between_exclusive(start, start, end));
        assert!(!is_between_exclusive(end, start, end));
        assert!(is_between_exclusive(dt(2010, 1, 2, 8, 0, 0), start, end));
    }

    #[test]
    fn earliest_and_latest_fall_back_to_whichever_side_is_present() {
        let a = dt(2010, 1, 2, 7, 0, 0);
        let b = dt(2010, 1, 2, 9, 0, 0);

        assert_eq!(earliest(Some(a), Some(b)), Some(a));
        assert_eq!(latest(Some(a), Some(b)), Some(b));
        assert_eq!(earliest(None, Some(b)), Some(b));
        assert_eq!(latest(Some(a), None), Some(a));
        assert_eq!(earliest(None, None), None);
    }

    #[rstest]
    // ISO Mon=1..Sun=7  ->  Sun=1..Sat=7
    #[case(1, 2)]
    #[case(2, 3)]
    #[case(3, 4)]
    #[case(4, 5)]
    #[case(5, 6)]
    #[case(6, 7)]
    #[case(7, 1)]
    fn day_of_week_translates_out_of_iso(#[case] iso: i32, #[case] expected: i32) {
        assert_eq!(translate_dow_from_iso(iso), expected);
        assert_eq!(translate_dow_to_iso(expected), iso);
    }

    #[test]
    fn the_day_of_week_translations_are_inverses_across_the_whole_week() {
        for iso in 1..=7 {
            assert_eq!(translate_dow_to_iso(translate_dow_from_iso(iso)), iso);
        }
    }

    #[rstest]
    #[case(date(2013, 7, 1), date(2013, 7, 24), 23)]
    #[case(date(2013, 7, 24), date(2013, 7, 24), 0)]
    #[case(date(2013, 7, 24), date(2013, 7, 1), -23)]
    #[case(date(2012, 2, 28), date(2012, 3, 1), 2)] // leap year
    fn days_between_counts_whole_days(
        #[case] start: LocalDate,
        #[case] end: LocalDate,
        #[case] expected: i32,
    ) {
        assert_eq!(days_between(start, end), expected);
    }

    #[rstest]
    #[case(date(2013, 1, 1), date(2013, 7, 1), 6)]
    #[case(date(2013, 1, 15), date(2013, 2, 14), 0)]
    #[case(date(2013, 1, 15), date(2013, 2, 15), 1)]
    #[case(date(2013, 7, 1), date(2013, 1, 1), -6)]
    #[case(date(2013, 1, 1), date(2013, 1, 31), 0)]
    fn months_between_counts_whole_months(
        #[case] start: LocalDate,
        #[case] end: LocalDate,
        #[case] expected: i32,
    ) {
        assert_eq!(months_between(start, end), expected);
    }

    #[test]
    fn a_month_end_start_date_still_yields_a_whole_month() {
        // Joda clamps day-of-month when adding, so 31 Jan + 1 month is 28 Feb
        // and the month counts. A year/month field subtraction would say 0.
        assert_eq!(months_between(date(2013, 1, 31), date(2013, 2, 28)), 1);
        assert_eq!(months_between(date(2013, 1, 31), date(2013, 2, 27)), 0);
    }

    #[rstest]
    #[case(date(2013, 1, 15), date(2013, 1, 1))]
    #[case(date(2013, 2, 28), date(2013, 1, 1))]
    #[case(date(2013, 3, 31), date(2013, 1, 1))]
    #[case(date(2013, 4, 1), date(2013, 4, 1))]
    #[case(date(2013, 6, 30), date(2013, 4, 1))]
    #[case(date(2013, 7, 24), date(2013, 7, 1))]
    #[case(date(2013, 12, 31), date(2013, 10, 1))]
    fn start_of_quarter_lands_on_the_quarter_boundary(
        #[case] date_in_quarter: LocalDate,
        #[case] expected: LocalDate,
    ) {
        assert_eq!(start_of_quarter(date_in_quarter), expected);
    }

    #[test]
    fn safe_max_and_min_absorb_a_missing_side() {
        let early = date(2013, 1, 1);
        let late = date(2013, 7, 1);

        assert_eq!(safe_max_date(Some(early), Some(late)), Some(late));
        assert_eq!(safe_min_date(Some(early), Some(late)), Some(early));
        assert_eq!(safe_max_date(None, Some(late)), Some(late));
        assert_eq!(safe_min_date(Some(early), None), Some(early));
        assert_eq!(safe_max_date(None, None), None);
    }

    #[test]
    fn safe_max_and_min_date_time_behave_the_same_way() {
        let early = dt(2013, 1, 1, 7, 0, 0);
        let late = dt(2013, 1, 1, 9, 0, 0);

        assert_eq!(safe_max_date_time(Some(early), Some(late)), Some(late));
        assert_eq!(safe_min_date_time(Some(early), Some(late)), Some(early));
        assert_eq!(safe_max_date_time(Some(early), None), Some(early));
        assert_eq!(safe_min_date_time(None, None), None);
    }
}
