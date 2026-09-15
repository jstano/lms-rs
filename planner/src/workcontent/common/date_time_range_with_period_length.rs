//! A date-time range paired with the granularity it is measured in.
//!
//! The flowed distribution engine works in fixed-length periods, so nearly
//! every array it builds is described by a range plus a period length. This is
//! the owned counterpart to `date_range_rs`'s borrowing [`WithPeriod`] view:
//! the index arithmetic lives there and is delegated to, this type just makes
//! the pair storable in a struct.
//!
//! [`WithPeriod`]: date_range_rs::datetimerange::date_time_range::WithPeriod

use date_range_rs::DateTimeRange;

#[derive(Debug, Clone)]
pub struct DateTimeRangeWithPeriodLength {
    range: DateTimeRange,
    period_length_minutes: i32,
}

impl DateTimeRangeWithPeriodLength {
    pub fn of(range: DateTimeRange, period_length_minutes: i32) -> Self {
        Self {
            range,
            period_length_minutes,
        }
    }

    pub fn range(&self) -> &DateTimeRange {
        &self.range
    }

    pub fn period_length_in_minutes(&self) -> i32 {
        self.period_length_minutes
    }

    /// The index of the period the range starts in, counted from midnight of
    /// the start date.
    ///
    /// `None` when the period length is not positive, which would otherwise
    /// divide by zero.
    pub fn start_index(&self) -> Option<i32> {
        self.with_period().map(|period| period.start_index())
    }

    /// The index of the period the range ends in, counted from midnight of the
    /// *start* date — so a range running past midnight keeps counting rather
    /// than wrapping back to zero.
    pub fn end_index(&self) -> Option<i32> {
        self.with_period().map(|period| period.end_index())
    }

    /// The periods the range covers, as an inclusive index range.
    ///
    /// The end index is the period the range *ends* in, which the range does
    /// not itself occupy, so the last covered period is one before it.
    pub fn index_range(&self) -> Option<std::ops::RangeInclusive<i32>> {
        let (start, end) = (self.start_index()?, self.end_index()?);

        Some(start..=end - 1)
    }

    /// The number of whole periods that fit between the start and the end.
    pub fn number_of_periods(&self) -> Option<i32> {
        self.with_period().map(|period| period.number_of_periods())
    }

    fn with_period(&self) -> Option<date_range_rs::datetimerange::date_time_range::WithPeriod<'_>> {
        (self.period_length_minutes > 0)
            .then(|| self.range.with_period_len(self.period_length_minutes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalDateTime;
    use rstest::rstest;

    fn range_with(
        start: LocalDateTime,
        end: LocalDateTime,
        period_length: i32,
    ) -> DateTimeRangeWithPeriodLength {
        DateTimeRangeWithPeriodLength::of(DateTimeRange::of(start, end), period_length)
    }

    #[rstest]
    // An ordinary daytime shift, at each of the three planner granularities.
    #[case(10, 48, 96)]
    #[case(15, 32, 64)]
    #[case(30, 16, 32)]
    fn indexes_a_daytime_range_from_midnight(
        #[case] period_length: i32,
        #[case] expected_start: i32,
        #[case] expected_end: i32,
    ) {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 16, 0, 0),
            period_length,
        );

        assert_eq!(range.start_index(), Some(expected_start));
        assert_eq!(range.end_index(), Some(expected_end));
    }

    #[rstest]
    // A shift running past midnight keeps counting into the second day.
    #[case(10, 138, 186)]
    #[case(15, 92, 124)]
    #[case(30, 46, 62)]
    fn indexes_an_overnight_range_without_wrapping(
        #[case] period_length: i32,
        #[case] expected_start: i32,
        #[case] expected_end: i32,
    ) {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 23, 0, 0),
            LocalDateTime::new(2013, 7, 25, 7, 0, 0),
            period_length,
        );

        assert_eq!(range.start_index(), Some(expected_start));
        assert_eq!(range.end_index(), Some(expected_end));
    }

    #[test]
    fn indexes_an_overnight_range_across_a_year_boundary() {
        let range = range_with(
            LocalDateTime::new(2013, 12, 31, 23, 0, 0),
            LocalDateTime::new(2014, 1, 1, 7, 0, 0),
            30,
        );

        assert_eq!(range.start_index(), Some(46));
        assert_eq!(range.end_index(), Some(62));
    }

    #[test]
    fn a_non_positive_period_length_has_no_indexes() {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 16, 0, 0),
            0,
        );

        assert_eq!(range.start_index(), None);
        assert_eq!(range.end_index(), None);
        assert_eq!(range.number_of_periods(), None);
    }

    #[test]
    fn the_index_range_stops_before_the_period_the_range_ends_in() {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 20, 0, 0),
            30,
        );

        // 08:00 is index 16 and 20:00 is index 40, so the shift occupies 16
        // through 39 — twenty-four half hours.
        assert_eq!(range.index_range(), Some(16..=39));
        assert_eq!(range.index_range().unwrap().count(), 24);
    }

    #[test]
    fn counts_the_whole_periods_that_fit() {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 16, 0, 0),
            30,
        );

        assert_eq!(range.number_of_periods(), Some(16));
    }
}
