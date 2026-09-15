//! Re-times a distribution pattern onto the planner's own period length.
//!
//! A pattern is configured at whatever granularity suited whoever drew the
//! curve — hourly, quarter-hourly — which rarely matches the granularity the
//! plan is being built at. Converting between the two is a matter of splitting
//! periods up, adding them together, or, where neither divides the other, both.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::domain::distribution_pattern::DistributionPattern;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::error::GenerationError;

/// The granularity a pattern of an unrecognised length is assumed to be at.
const DEFAULT_PATTERN_PERIOD_LENGTH: i32 = 5;

pub trait PatternToDistributionConverter {
    fn convert(
        &self,
        pattern: &DistributionPattern,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Result<Vec<DistributionItem>, GenerationError>;
}

pub struct PatternToDistributionConverterImpl {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl PatternToDistributionConverterImpl {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl PatternToDistributionConverter for PatternToDistributionConverterImpl {
    fn convert(
        &self,
        pattern: &DistributionPattern,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Result<Vec<DistributionItem>, GenerationError> {
        let mut items = self.list_creator.create_array(range);
        let pattern_period_length = pattern
            .period_length_in_minutes()
            .unwrap_or(DEFAULT_PATTERN_PERIOD_LENGTH);
        let planner_period_length = range.period_length_in_minutes();

        if planner_period_length <= 0 {
            return Ok(items);
        }

        let values: Vec<f64> = pattern
            .periods()
            .iter()
            .map(|period| period.pattern_value())
            .collect();

        // Ten and fifteen minutes divide neither way, so they have to meet at
        // the half hour; everything else is a clean split or a clean sum.
        if is_ten_fifteen_pairing(planner_period_length, pattern_period_length) {
            let lcd =
                calculate_least_common_denominator(planner_period_length, pattern_period_length)?;

            regroup_through(
                &mut items,
                &values,
                (lcd / pattern_period_length) as usize,
                (lcd / planner_period_length) as usize,
            );
        } else if pattern_period_length > planner_period_length {
            let sub_periods = (pattern_period_length / planner_period_length) as usize;

            regroup_through(&mut items, &values, 1, sub_periods);
        } else {
            let pattern_periods = (planner_period_length / pattern_period_length) as usize;

            regroup_through(&mut items, &values, pattern_periods, 1);
        }

        Ok(items)
    }
}

/// Whether the two granularities are the one pair that needs a common multiple.
fn is_ten_fifteen_pairing(planner_period_length: i32, pattern_period_length: i32) -> bool {
    matches!(
        (planner_period_length, pattern_period_length),
        (10, 15) | (15, 10)
    )
}

/// Regroups the pattern by taking `pattern_periods` at a time and spreading
/// each group evenly over `sub_periods` of the output.
///
/// Splitting evenly rarely divides exactly, so each share is rounded and
/// whatever is then missing is dropped into the group's **last** sub-period.
/// That leaves a group's total very slightly off when the rounded shares
/// overshoot instead of falling short — no top-up is applied in that direction.
/// It is a known, accepted drift in the original engine, reproduced here so the
/// two agree to the fourth decimal.
fn regroup_through(
    items: &mut [DistributionItem],
    values: &[f64],
    pattern_periods: usize,
    sub_periods: usize,
) {
    if pattern_periods == 0 || sub_periods == 0 {
        return;
    }

    let mut item_index = 0;

    for group in values.chunks(pattern_periods) {
        let group_total: f64 = group.iter().sum();
        let share = numbers::round_percent(group_total / sub_periods as f64);

        let group_start = item_index;
        let mut spread = 0.0;

        for _ in 0..sub_periods {
            let Some(item) = items.get_mut(item_index) else {
                return;
            };

            item.add_to_period_value(share);
            spread += share;
            item_index += 1;
        }

        if spread < group_total && item_index > group_start {
            items[item_index - 1].add_to_period_value(group_total - spread);
        }
    }
}

/// The smallest number of minutes both period lengths divide into.
///
/// Returns an error rather than panicking if none is found, which cannot happen
/// for the granularities the planner supports but keeps the function total.
pub fn calculate_least_common_denominator(a: i32, b: i32) -> Result<i32, GenerationError> {
    if a <= 0 || b <= 0 {
        return Err(GenerationError::LowestCommonDenominatorUndefined { a, b });
    }

    let (max, min) = if a > b { (a, b) } else { (b, a) };

    (1..=min)
        .find(|multiplier| (max * multiplier) % min == 0)
        .map(|multiplier| multiplier * max)
        .ok_or(GenerationError::LowestCommonDenominatorUndefined { a, b })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::distribution_pattern::DistributionPatternPeriod;
    use crate::workcontent::domain::location::LocationId;
    use date_range_rs::DateTimeRange;
    use joda_rs::LocalDateTime;
    use rstest::rstest;

    /// A pattern whose only work sits between 05:00 and 11:00, padded with
    /// empty periods either side to fill the day.
    fn pattern(leading_empty: usize, work: &[f64], trailing_empty: usize) -> DistributionPattern {
        let values = std::iter::repeat_n(0.0, leading_empty)
            .chain(work.iter().copied())
            .chain(std::iter::repeat_n(0.0, trailing_empty));

        DistributionPattern::new(
            LocationId::new(),
            "Test".to_string(),
            100.0,
            1000,
            values
                .enumerate()
                .map(|(index, value)| DistributionPatternPeriod::new(index as i32, value, None))
                .collect(),
        )
    }

    /// Hourly: 24 periods, work from 05:00 to 11:00.
    fn hourly_pattern() -> DistributionPattern {
        pattern(5, &[13.0, 25.0, 35.0, 37.0, 29.0, 10.0], 13)
    }

    /// Half-hourly: 48 periods, the same window at twice the resolution.
    fn half_hourly_pattern() -> DistributionPattern {
        pattern(
            10,
            &[
                13.0, 14.0, 25.0, 26.0, 35.0, 36.0, 37.0, 38.0, 29.0, 30.0, 10.0, 11.0,
            ],
            26,
        )
    }

    fn range(period_length: i32) -> DateTimeRangeWithPeriodLength {
        DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(
                LocalDateTime::new(2013, 9, 26, 5, 0, 0),
                LocalDateTime::new(2013, 9, 26, 11, 0, 0),
            ),
            period_length,
        )
    }

    fn convert(
        pattern: &DistributionPattern,
        period_length: i32,
    ) -> Vec<DistributionItem> {
        PatternToDistributionConverterImpl::new()
            .convert(pattern, &range(period_length))
            .expect("the supported granularities all convert")
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        numbers::round_raw_hours(items.iter().map(|item| item.value_per_period()).sum())
    }

    #[test]
    fn an_hourly_pattern_splits_evenly_into_half_hours() {
        let items = convert(&hourly_pattern(), 30);

        assert_eq!(items.len(), 96);
        assert_eq!(items[10].date_time(), LocalDateTime::new(2013, 9, 26, 5, 0, 0));
        // Each hour's value is halved across its two periods.
        for (index, expected) in [
            (10, 6.5),
            (11, 6.5),
            (12, 12.5),
            (13, 12.5),
            (14, 17.5),
            (15, 17.5),
            (16, 18.5),
            (17, 18.5),
            (18, 14.5),
            (19, 14.5),
            (20, 5.0),
            (21, 5.0),
            (22, 0.0),
        ] {
            assert_eq!(items[index].value_per_period(), expected, "period {index}");
        }
        assert_eq!(total_of(&items), 149.0);
    }

    #[test]
    fn an_hourly_pattern_splits_evenly_into_quarter_hours() {
        let items = convert(&hourly_pattern(), 15);

        assert_eq!(items.len(), 192);
        assert_eq!(items[20].date_time(), LocalDateTime::new(2013, 9, 26, 5, 0, 0));
        for (index, expected) in [
            (20, 3.25),
            (21, 3.25),
            (22, 3.25),
            (23, 3.25),
            (24, 6.25),
            (25, 6.25),
            (26, 6.25),
            (27, 6.25),
        ] {
            assert_eq!(items[index].value_per_period(), expected, "period {index}");
        }
        assert_eq!(total_of(&items), 149.0);
    }

    #[test]
    fn a_split_that_does_not_divide_evenly_drifts_by_a_fraction() {
        // Thirteen over six periods is 2.1667 once rounded, which multiplied
        // back out overshoots thirteen slightly. The engine only tops a group
        // up when the shares fall short, never trims when they overshoot, so
        // the total lands a fraction above the pattern's own.
        let items = convert(&hourly_pattern(), 10);

        assert_eq!(items.len(), 288);
        assert_eq!(items[30].value_per_period(), 2.1667);
        assert_eq!(total_of(&items), 149.0008);
    }

    #[test]
    fn a_group_whose_shares_fall_short_is_topped_up_in_its_last_period() {
        let items = convert(&hourly_pattern(), 10);

        // Thirty-five over six rounds down to 5.8333, so the sixth period of
        // that hour absorbs the missing 0.0002.
        assert_eq!(items[42].value_per_period(), 5.8333);
        assert_eq!(items[47].value_per_period(), 5.8335);
    }

    #[test]
    fn a_half_hourly_pattern_sums_up_into_hours() {
        let items = convert(&half_hourly_pattern(), 60);

        assert_eq!(items.len(), 48);
        // The two half hours of 05:00 add back together.
        assert_eq!(items[5].value_per_period(), 27.0);
        assert_eq!(items[6].value_per_period(), 51.0);
        assert_eq!(items[7].value_per_period(), 71.0);
    }

    #[test]
    fn a_pattern_already_at_the_planners_granularity_is_unchanged() {
        let items = convert(&half_hourly_pattern(), 30);

        assert_eq!(items.len(), 96);
        assert_eq!(items[10].value_per_period(), 13.0);
        assert_eq!(items[11].value_per_period(), 14.0);
        assert_eq!(items[21].value_per_period(), 11.0);
        assert_eq!(total_of(&items), 304.0);
    }

    #[test]
    fn a_ten_minute_pattern_meets_fifteen_minute_periods_at_the_half_hour() {
        // Neither length divides the other, so three ten-minute periods are
        // added together and split back across two fifteen-minute ones.
        let ten_minute = pattern(30, &[10.0; 36], 78);

        let items = convert(&ten_minute, 15);

        assert_eq!(items.len(), 192);
        // Three tens make thirty, split evenly into two fifteens.
        assert_eq!(items[20].value_per_period(), 15.0);
        assert_eq!(items[21].value_per_period(), 15.0);
        assert_eq!(total_of(&items), 360.0);
    }

    #[test]
    fn a_fifteen_minute_pattern_meets_ten_minute_periods_and_drifts() {
        // Two fifteens make thirty, split across three tens at 10.0 each — but
        // an odd group total will not divide cleanly, which is where the known
        // drift shows up.
        let fifteen_minute = pattern(20, &[13.0; 24], 52);

        let items = convert(&fifteen_minute, 10);

        assert_eq!(items.len(), 288);
        // Twenty-six over three rounds to 8.6667, so each of the twelve groups
        // overshoots by 0.0001 and the total lands 0.0012 above the pattern's.
        assert_eq!(items[30].value_per_period(), 8.6667);
        assert_eq!(total_of(&items), 312.0012);
    }

    #[rstest]
    // The full table of granularity pairings the planner supports. The last two
    // rows are the ten/fifteen pairing that needs a genuine common multiple.
    #[case(60, 30, 60, 1, 2)]
    #[case(60, 15, 60, 1, 4)]
    #[case(60, 10, 60, 1, 6)]
    #[case(30, 30, 30, 1, 1)]
    #[case(30, 15, 30, 1, 2)]
    #[case(30, 10, 30, 1, 3)]
    #[case(15, 30, 30, 2, 1)]
    #[case(15, 15, 15, 1, 1)]
    #[case(10, 30, 30, 3, 1)]
    #[case(10, 10, 10, 1, 1)]
    #[case(15, 10, 30, 2, 3)]
    #[case(10, 15, 30, 3, 2)]
    fn the_common_denominator_regroups_both_granularities(
        #[case] pattern_length: i32,
        #[case] planner_length: i32,
        #[case] expected_lcd: i32,
        #[case] expected_pattern_periods: i32,
        #[case] expected_sub_periods: i32,
    ) {
        let lcd = calculate_least_common_denominator(pattern_length, planner_length)
            .expect("supported granularities have a common denominator");

        assert_eq!(lcd, expected_lcd);
        assert_eq!(lcd / pattern_length, expected_pattern_periods);
        assert_eq!(lcd / planner_length, expected_sub_periods);
    }

    #[rstest]
    #[case(0, 15)]
    #[case(15, 0)]
    #[case(-10, 15)]
    fn a_non_positive_period_length_has_no_common_denominator(#[case] a: i32, #[case] b: i32) {
        assert_eq!(
            calculate_least_common_denominator(a, b),
            Err(GenerationError::LowestCommonDenominatorUndefined { a, b })
        );
    }

    #[test]
    fn a_range_with_no_granularity_converts_to_nothing() {
        let items = PatternToDistributionConverterImpl::new()
            .convert(&hourly_pattern(), &range(0))
            .expect("an empty range is not an error");

        assert!(items.is_empty());
    }
}
