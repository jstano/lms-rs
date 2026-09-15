//! Builds the canonical period array every distribution stage works against.
//!
//! The array is always two days long and always starts at midnight of its
//! first day, regardless of when the shift it describes actually begins. That
//! is what lets a shift running past midnight keep counting into the second
//! day instead of wrapping, and it means every array in the pipeline shares one
//! index space — which the aggregator later relies on.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use joda_rs::constants::{HOURS_PER_DAY, MINUTES_PER_HOUR};

const ARRAY_LENGTH_IN_DAYS: i32 = 2;
const MINUTES_PER_DAY: i32 = (MINUTES_PER_HOUR * HOURS_PER_DAY) as i32;

pub trait DistributionItemListCreator {
    /// A zero-filled two-day array at the range's granularity.
    ///
    /// Empty when the period length is not positive.
    fn create_array(&self, range: &DateTimeRangeWithPeriodLength) -> Vec<DistributionItem>;

    /// The canonical zero-filled array for what is being planned.
    fn create_array_for(&self, params: &GeneratorParameters) -> Vec<DistributionItem>;

    /// A copy of `items` with the same periods and times but every value zeroed.
    fn clone_and_reset_array(&self, items: &[DistributionItem]) -> Vec<DistributionItem>;
}

pub struct DistributionItemListCreatorImpl;

impl DistributionItemListCreatorImpl {
    pub fn new() -> Self {
        Self
    }
}

impl DistributionItemListCreator for DistributionItemListCreatorImpl {
    fn create_array(&self, range: &DateTimeRangeWithPeriodLength) -> Vec<DistributionItem> {
        let period_length = range.period_length_in_minutes();

        if period_length <= 0 {
            return Vec::new();
        }

        let period_count = (MINUTES_PER_DAY * ARRAY_LENGTH_IN_DAYS) / period_length;
        // Anchored at midnight of the start date, not at the start time.
        let mut period_date_time = range.range().start().to_local_date().at_start_of_day();

        (0..period_count)
            .map(|period_index| {
                let item = DistributionItem::new(period_index, period_date_time, 0.0);
                period_date_time = period_date_time.plus_minutes(period_length as i64);
                item
            })
            .collect()
    }

    fn create_array_for(&self, params: &GeneratorParameters) -> Vec<DistributionItem> {
        self.create_array(&params.distribution_array_range())
    }

    fn clone_and_reset_array(&self, items: &[DistributionItem]) -> Vec<DistributionItem> {
        items
            .iter()
            .map(|item| DistributionItem::new(item.period(), item.date_time(), 0.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use date_range_rs::DateTimeRange;
    use joda_rs::LocalDateTime;
    use rstest::rstest;

    fn range_with(
        start: LocalDateTime,
        end: LocalDateTime,
        period_length: i32,
    ) -> DateTimeRangeWithPeriodLength {
        DateTimeRangeWithPeriodLength::of(DateTimeRange::of(start, end), period_length)
    }

    #[test]
    fn a_period_length_of_zero_yields_no_periods() {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 16, 0, 0),
            0,
        );

        assert!(
            DistributionItemListCreatorImpl::new()
                .create_array(&range)
                .is_empty()
        );
    }

    #[rstest]
    // A daytime shift: the array still spans two full days from midnight, and
    // the shift's own start/end land at these indexes within it.
    #[case(10, 288, 48, 96)]
    #[case(15, 192, 32, 64)]
    #[case(30, 96, 16, 32)]
    fn covers_two_days_with_the_shift_at_the_expected_indexes(
        #[case] period_length: i32,
        #[case] expected_size: usize,
        #[case] start_index: usize,
        #[case] end_index: usize,
    ) {
        let start = LocalDateTime::new(2013, 7, 24, 8, 0, 0);
        let end = LocalDateTime::new(2013, 7, 24, 16, 0, 0);
        let range = range_with(start, end, period_length);

        let array = DistributionItemListCreatorImpl::new().create_array(&range);

        assert_eq!(array.len(), expected_size);
        assert_eq!(array[start_index].date_time(), start);
        assert_eq!(array[end_index].date_time(), end);
    }

    #[rstest]
    // An overnight shift: the end time lands in the array's second day rather
    // than wrapping back to the start.
    #[case(10, 288, 138, 186)]
    #[case(15, 192, 92, 124)]
    #[case(30, 96, 46, 62)]
    fn an_overnight_shift_runs_into_the_second_day_of_the_array(
        #[case] period_length: i32,
        #[case] expected_size: usize,
        #[case] start_index: usize,
        #[case] end_index: usize,
    ) {
        let start = LocalDateTime::new(2013, 7, 24, 23, 0, 0);
        let end = LocalDateTime::new(2013, 7, 25, 7, 0, 0);
        let range = range_with(start, end, period_length);

        let array = DistributionItemListCreatorImpl::new().create_array(&range);

        assert_eq!(array.len(), expected_size);
        assert_eq!(array[start_index].date_time(), start);
        assert_eq!(array[end_index].date_time(), end);
    }

    #[test]
    fn an_overnight_shift_crossing_a_year_boundary_is_indexed_the_same_way() {
        let start = LocalDateTime::new(2013, 12, 31, 23, 0, 0);
        let end = LocalDateTime::new(2014, 1, 1, 7, 0, 0);
        let range = range_with(start, end, 30);

        let array = DistributionItemListCreatorImpl::new().create_array(&range);

        assert_eq!(array.len(), 96);
        assert_eq!(array[46].date_time(), start);
        assert_eq!(array[62].date_time(), end);
    }

    #[test]
    fn every_period_starts_zeroed() {
        let range = range_with(
            LocalDateTime::new(2013, 7, 24, 8, 0, 0),
            LocalDateTime::new(2013, 7, 24, 16, 0, 0),
            30,
        );

        let array = DistributionItemListCreatorImpl::new().create_array(&range);

        assert!(array.iter().all(|item| item.value_per_period() == 0.0));
        assert!(
            array
                .iter()
                .enumerate()
                .all(|(index, item)| item.period() == index as i32)
        );
    }

    #[rstest]
    #[case(10, 288)]
    #[case(15, 192)]
    #[case(30, 96)]
    fn the_array_for_a_generation_covers_two_days_from_the_shift_date(
        #[case] period_length: u32,
        #[case] expected_size: usize,
    ) {
        use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
        use joda_rs::{LocalDate, LocalTime};

        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        );
        let params = context.params();

        let array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        assert_eq!(array.len(), expected_size);
        assert_eq!(array[0].date_time(), LocalDateTime::new(2013, 7, 24, 0, 0, 0));
    }

    #[test]
    fn cloning_keeps_the_shape_but_drops_the_values() {
        let date_time = LocalDateTime::new(2013, 10, 18, 12, 12, 0);
        let original = vec![DistributionItem::new(50, date_time, 10.0)];

        let cloned = DistributionItemListCreatorImpl::new().clone_and_reset_array(&original);

        assert_eq!(cloned.len(), 1);
        assert_eq!(cloned[0].period(), 50);
        assert_eq!(cloned[0].date_time(), date_time);
        assert_eq!(cloned[0].value_per_period(), 0.0);

        // The original is left untouched.
        assert_eq!(original[0].value_per_period(), 10.0);
    }
}
