//! Converts a configured spread shape into a distribution array.
//!
//! Spread values are stored at five-minute granularity, finer than any planner
//! period. Rather than averaging a block of them, the engine samples the first
//! slot of each block and scales it by the period length — so a period's value
//! is whatever was configured at the moment that period begins.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::domain::spread_standard::SpreadStandardValue;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};

pub trait SpreadValuesConverter {
    fn convert(
        &self,
        spread_value: &SpreadStandardValue,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Vec<DistributionItem>;
}

pub struct SpreadValuesConverterImpl {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl SpreadValuesConverterImpl {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl SpreadValuesConverter for SpreadValuesConverterImpl {
    fn convert(
        &self,
        spread_value: &SpreadStandardValue,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Vec<DistributionItem> {
        let mut items = self.list_creator.create_array(range);
        let period_length = range.period_length_in_minutes();

        // How many five-minute slots make up one planner period. A granularity
        // the engine does not spread at leaves the array empty.
        let Some(slots_per_period) = slots_per_period(period_length) else {
            return items;
        };

        for (period_index, slot) in (0..spread_value.spread_values().len())
            .step_by(slots_per_period)
            .enumerate()
        {
            let Some(item) = items.get_mut(period_index) else {
                break;
            };
            // An unconfigured slot contributes nothing at all, which is not the
            // same as an explicit zero only in that it never overwrites.
            let Some(Some(value)) = spread_value.spread_values().get(slot) else {
                continue;
            };

            item.add_to_period_value((*value * period_length) as f64);
        }

        items
    }
}

/// The number of five-minute slots in one planner period.
fn slots_per_period(period_length: i32) -> Option<usize> {
    match period_length {
        30 => Some(6),
        15 => Some(3),
        10 => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::environment::EnvironmentId;
    use date_range_rs::DateTimeRange;
    use joda_rs::LocalDateTime;
    use rstest::rstest;

    /// A day of five-minute slots: nothing until 07:00, then a hump that tails
    /// off by midday, then nothing again.
    fn spread_values(present: bool) -> SpreadStandardValue {
        let quiet = |count: usize| {
            std::iter::repeat_n(if present { Some(0) } else { None }, count)
        };
        let busy = |count: usize, value: i32| std::iter::repeat_n(Some(value), count);

        let values = quiet(84)
            .chain(busy(6, 3))
            .chain(busy(12, 4))
            .chain(busy(18, 5))
            .chain(busy(18, 6))
            .chain(busy(6, 5))
            .chain(quiet(144))
            .collect();

        SpreadStandardValue::new(EnvironmentId::new(), values)
    }

    fn range(period_length: i32) -> DateTimeRangeWithPeriodLength {
        DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(
                LocalDateTime::new(2013, 9, 26, 7, 0, 0),
                LocalDateTime::new(2013, 9, 26, 12, 0, 0),
            ),
            period_length,
        )
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    #[rstest]
    // Ten-minute periods: the 07:00 hump starts at index 42.
    #[case(10, vec![(41, 0.0), (42, 30.0), (43, 30.0), (44, 30.0), (45, 40.0), (46, 40.0), (47, 40.0), (48, 40.0), (60, 60.0), (66, 60.0), (69, 50.0), (72, 0.0)])]
    // Fifteen-minute periods: the same shape, scaled, starting at index 28.
    #[case(15, vec![(27, 0.0), (28, 45.0), (29, 45.0), (30, 60.0), (31, 60.0), (32, 60.0), (33, 60.0), (34, 75.0), (35, 75.0), (36, 75.0), (37, 75.0)])]
    // Half-hour periods: starting at index 14.
    #[case(30, vec![(13, 0.0), (14, 90.0), (15, 120.0), (16, 120.0), (17, 150.0), (18, 150.0), (19, 150.0), (20, 180.0), (21, 180.0), (22, 180.0), (23, 150.0), (24, 0.0)])]
    fn samples_the_first_slot_of_each_period_and_scales_it(
        #[case] period_length: i32,
        #[case] expected: Vec<(usize, f64)>,
    ) {
        let items = SpreadValuesConverterImpl::new().convert(&spread_values(true), &range(period_length));

        for (index, value) in expected {
            assert_eq!(
                items[index].value_per_period(),
                value,
                "period {index} at {period_length} minutes"
            );
        }
    }

    #[rstest]
    // The same work however finely it is sliced.
    #[case(10)]
    #[case(15)]
    #[case(30)]
    fn the_total_is_the_same_at_every_granularity(#[case] period_length: i32) {
        let items = SpreadValuesConverterImpl::new().convert(&spread_values(true), &range(period_length));

        assert_eq!(total_of(&items), 1470.0);
    }

    #[rstest]
    #[case(10)]
    #[case(15)]
    #[case(30)]
    fn unconfigured_slots_behave_the_same_as_explicit_zeros(#[case] period_length: i32) {
        let converter = SpreadValuesConverterImpl::new();

        let zeroed = converter.convert(&spread_values(true), &range(period_length));
        let unset = converter.convert(&spread_values(false), &range(period_length));

        assert_eq!(zeroed, unset);
    }

    #[test]
    fn a_granularity_the_engine_does_not_spread_at_stays_empty() {
        let items = SpreadValuesConverterImpl::new().convert(&spread_values(true), &range(60));

        assert_eq!(total_of(&items), 0.0);
    }

    #[test]
    fn the_periods_keep_their_times() {
        let items = SpreadValuesConverterImpl::new().convert(&spread_values(true), &range(10));

        assert_eq!(
            items[42].date_time(),
            LocalDateTime::new(2013, 9, 26, 7, 0, 0)
        );
        assert_eq!(
            items[41].date_time(),
            LocalDateTime::new(2013, 9, 26, 6, 50, 0)
        );
    }
}
