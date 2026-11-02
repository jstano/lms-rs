//! Port of `ReferenceRangeUtility`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabeladdendum/ReferenceRangeUtility.java`.
//!
//! A *reference range* is a yearly window — "15 April to 12 April next year" —
//! given by two dates whose years are irrelevant. For a period, this finds
//! every occurrence of that window that overlaps it.
//!
//! The start is placed in the period's start year, moved back a year if that
//! lands after the period starts (so the occurrence **already running** when the
//! period begins is included); the end moves by the same number of years. Then
//! the window steps forward a year at a time while it starts on or before the
//! period's end, keeping each that overlaps. A leap-day date lands on
//! 28 February in a common year, as Joda does.

use crate::rules::algorithm::dailyearning::anniversary_earning::with_year_clamped;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// `getReferenceRangesOverlappingPeriod(period, referenceStart, referenceEnd)`.
pub fn reference_ranges_overlapping_period(
    period: &DateRange,
    reference_start: LocalDate,
    reference_end: LocalDate,
) -> Vec<DateRange> {
    let in_period_year = with_year_clamped(reference_start, period.start_date().year());
    let adjusted_start = if in_period_year > period.start_date() {
        in_period_year.minus_years(1)
    } else {
        in_period_year
    };
    let year_adjustment = i64::from(adjusted_start.year() - reference_start.year());
    let adjusted_end = reference_end.plus_years(year_adjustment);

    let mut range = DateRange::new(adjusted_start, adjusted_end);
    let mut found = Vec::new();
    while range.start_date() <= period.end_date() {
        if range.overlaps(period) {
            found.push(range);
        }
        range = DateRange::new(
            range.start_date().plus_years(1),
            range.end_date().plus_years(1),
        );
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: i32, day: i32) -> LocalDate {
        LocalDate::of(y, m, day)
    }

    // ---- java_parity_tests: ReferenceRangeUtilityTest.groovy ----

    #[test]
    fn finds_all_ranges_between_two_reference_days_that_overlap_the_period() {
        let period = DateRange::new(d(2017, 4, 1), d(2017, 4, 30));
        let found = reference_ranges_overlapping_period(&period, d(2017, 4, 15), d(2018, 4, 14));
        assert_eq!(
            found,
            vec![
                DateRange::new(d(2016, 4, 15), d(2017, 4, 14)),
                DateRange::new(d(2017, 4, 15), d(2018, 4, 14)),
            ]
        );
    }

    #[test]
    fn handles_reference_ranges_that_do_not_cross_years() {
        let period = DateRange::new(d(2016, 12, 1), d(2016, 12, 15));
        let found = reference_ranges_overlapping_period(&period, d(2015, 12, 3), d(2015, 12, 10));
        assert_eq!(found, vec![DateRange::new(d(2016, 12, 3), d(2016, 12, 10))]);
    }

    #[test]
    fn includes_ranges_that_start_on_the_period_end_date() {
        let period = DateRange::new(d(2016, 12, 9), d(2017, 1, 1));
        let found = reference_ranges_overlapping_period(&period, d(2015, 1, 1), d(2015, 12, 13));
        assert_eq!(
            found,
            vec![
                DateRange::new(d(2016, 1, 1), d(2016, 12, 13)),
                DateRange::new(d(2017, 1, 1), d(2017, 12, 13)),
            ]
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_leap_day_reference_start_lands_on_the_28th_in_a_common_year() {
        let period = DateRange::new(d(2023, 3, 1), d(2023, 3, 31));
        let found = reference_ranges_overlapping_period(&period, d(2020, 2, 29), d(2020, 5, 1));
        // 2023-02-28 .. 2023-05-01 overlaps March.
        assert_eq!(found, vec![DateRange::new(d(2023, 2, 28), d(2023, 5, 1))]);
    }

    #[test]
    fn no_range_overlapping_the_period_yields_nothing() {
        let period = DateRange::new(d(2017, 7, 1), d(2017, 7, 31));
        let found = reference_ranges_overlapping_period(&period, d(2017, 1, 1), d(2017, 2, 1));
        assert!(found.is_empty());
    }
}
