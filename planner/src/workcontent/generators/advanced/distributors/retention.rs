//! Smears demand forward over the time a guest stays.
//!
//! A curve says when guests *arrive*, but the work they create lasts as long as
//! they are there. So each period's arrivals are added again to every period
//! they are still being served in — an additive smear, not a redistribution:
//! the array's total deliberately grows.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::providers::RetentionCapacityProvider;
use joda_rs::constants::{SECONDS_PER_HOUR, SECONDS_PER_MINUTE};

/// Spread each period's demand across the periods it is still being served in.
pub fn apply_retention(
    curve: &[DistributionItem],
    business_driver_id: BusinessDriverId,
    period_length: i32,
    retention_capacity: &dyn RetentionCapacityProvider,
) -> Vec<DistributionItem> {
    let mut smeared = DistributionItemListCreatorImpl::new().clone_and_reset_array(curve);

    for (period_index, item) in curve.iter().enumerate() {
        if item.value_per_period() <= 0.0 {
            continue;
        }

        let additional_periods = retention_capacity
            .utilization_for(business_driver_id, item.date_time())
            .map(|utilization| additional_periods_to_retain(utilization.retention_hours, period_length))
            .unwrap_or(0);

        let last_index = (period_index + additional_periods).min(curve.len().saturating_sub(1));

        for target in &mut smeared[period_index..=last_index] {
            target.add_to_period_value(numbers::round_percent(item.value_per_period()));
        }
    }

    smeared
}

/// How many periods *beyond* the arrival period the demand lingers in.
///
/// A retention shorter than one period lingers nowhere, so the count floors at
/// zero rather than going negative.
fn additional_periods_to_retain(retention_hours: f64, period_length: i32) -> usize {
    let retention_seconds = retention_hours * SECONDS_PER_HOUR as f64;
    let period_seconds = (SECONDS_PER_MINUTE * period_length as i64) as f64;

    ((retention_seconds / period_seconds).ceil() as i64 - 1).max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::providers::RetentionCapacityUtilization;
    use joda_rs::{LocalDate, LocalDateTime};
    use rstest::rstest;

    /// The same retention everywhere.
    struct FixedRetention(f64);

    impl RetentionCapacityProvider for FixedRetention {
        fn utilization_for(
            &self,
            _business_driver_id: BusinessDriverId,
            _date_time: LocalDateTime,
        ) -> Option<RetentionCapacityUtilization> {
            Some(RetentionCapacityUtilization {
                retention_hours: self.0,
                capacity: 0.0,
                utilization: 0.0,
            })
        }
    }

    /// No retention data at all.
    struct NoRetention;

    impl RetentionCapacityProvider for NoRetention {
        fn utilization_for(
            &self,
            _business_driver_id: BusinessDriverId,
            _date_time: LocalDateTime,
        ) -> Option<RetentionCapacityUtilization> {
            None
        }
    }

    /// The Java fixture's arrival curve, laid into a two-day array from
    /// `start_index` at the given granularity.
    fn curve(period_length: i32, start_index: usize, percents: &[f64]) -> Vec<DistributionItem> {
        let periods = (2 * 24 * 60 / period_length) as usize;
        let mut array: Vec<_> = (0..periods)
            .map(|index| {
                DistributionItem::new(
                    index as i32,
                    LocalDate::new(2013, 7, 24)
                        .at_start_of_day()
                        .plus_minutes((index as i64) * period_length as i64),
                    0.0,
                )
            })
            .collect();

        for (offset, percent) in percents.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*percent);
        }

        array
    }

    /// The 18:30-22:00 arrival curve at quarter-hour granularity, indexes 74-87.
    fn quarter_hour_curve() -> Vec<DistributionItem> {
        curve(
            15,
            74,
            &[
                4.7, 3.96, 6.17, 4.99, 11.45, 9.54, 8.81, 7.49, 13.51, 8.37, 7.05, 4.99, 7.49,
                1.47,
            ],
        )
    }

    /// The same curve at half-hour granularity, indexes 37-43.
    fn half_hour_curve() -> Vec<DistributionItem> {
        curve(30, 37, &[8.66, 11.16, 20.99, 16.3, 21.88, 12.04, 8.96])
    }

    fn assert_from(items: &[DistributionItem], start_index: usize, expected: &[f64]) {
        for (offset, value) in expected.iter().enumerate() {
            assert_eq!(
                items[start_index + offset].value_per_period(),
                *value,
                "period {}",
                start_index + offset
            );
        }
        // Everything past the smeared run is untouched.
        for (index, item) in items.iter().enumerate().skip(start_index + expected.len()) {
            assert_eq!(item.value_per_period(), 0.0, "period {index}");
        }
    }

    #[rstest]
    // Quarter-hour periods: a retention under one period lingers nowhere, and
    // each further quarter-hour of it adds one more period of overlap.
    #[case(0.0, &[4.7, 3.96, 6.17, 4.99, 11.45, 9.54, 8.81, 7.49, 13.51, 8.37, 7.05, 4.99, 7.49, 1.47])]
    #[case(0.25, &[4.7, 3.96, 6.17, 4.99, 11.45, 9.54, 8.81, 7.49, 13.51, 8.37, 7.05, 4.99, 7.49, 1.47])]
    #[case(0.5, &[4.7, 8.66, 10.13, 11.16, 16.44, 20.99, 18.35, 16.3, 21.0, 21.88, 15.42, 12.04, 12.48, 8.96, 1.47])]
    #[case(0.75, &[4.7, 8.66, 14.83, 15.12, 22.61, 25.98, 29.8, 25.84, 29.81, 29.37, 28.93, 20.41, 19.53, 13.95, 8.96, 1.47])]
    #[case(1.0, &[4.7, 8.66, 14.83, 19.82, 26.57, 32.15, 34.79, 37.29, 39.35, 38.18, 36.42, 33.92, 27.9, 21.0, 13.95, 8.96, 1.47])]
    #[case(1.25, &[4.7, 8.66, 14.83, 19.82, 31.27, 36.11, 40.96, 42.28, 50.8, 47.72, 45.23, 41.41, 41.41, 29.37, 21.0, 13.95, 8.96, 1.47])]
    #[case(1.5, &[4.7, 8.66, 14.83, 19.82, 31.27, 40.81, 44.92, 48.45, 55.79, 59.17, 54.77, 50.22, 48.9, 42.88, 29.37, 21.0, 13.95, 8.96, 1.47])]
    #[case(1.75, &[4.7, 8.66, 14.83, 19.82, 31.27, 40.81, 49.62, 52.41, 61.96, 64.16, 66.22, 59.76, 57.71, 50.37, 42.88, 29.37, 21.0, 13.95, 8.96, 1.47])]
    #[case(2.0, &[4.7, 8.66, 14.83, 19.82, 31.27, 40.81, 49.62, 57.11, 65.92, 70.33, 71.21, 71.21, 67.25, 59.18, 50.37, 42.88, 29.37, 21.0, 13.95, 8.96, 1.47])]
    fn retention_smears_the_quarter_hour_curve(
        #[case] retention_hours: f64,
        #[case] expected: &[f64],
    ) {
        let smeared = apply_retention(
            &quarter_hour_curve(),
            BusinessDriverId::new(),
            15,
            &FixedRetention(retention_hours),
        );

        assert_from(&smeared, 74, expected);
    }

    #[rstest]
    // Half-hour periods: it takes a full half-hour of retention to reach the
    // next period, so the rows come in pairs.
    #[case(0.0, &[8.66, 11.16, 20.99, 16.3, 21.88, 12.04, 8.96])]
    #[case(0.25, &[8.66, 11.16, 20.99, 16.3, 21.88, 12.04, 8.96])]
    #[case(0.5, &[8.66, 11.16, 20.99, 16.3, 21.88, 12.04, 8.96])]
    #[case(0.75, &[8.66, 19.82, 32.15, 37.29, 38.18, 33.92, 21.0, 8.96])]
    #[case(1.0, &[8.66, 19.82, 32.15, 37.29, 38.18, 33.92, 21.0, 8.96])]
    #[case(1.25, &[8.66, 19.82, 40.81, 48.45, 59.17, 50.22, 42.88, 21.0, 8.96])]
    #[case(1.5, &[8.66, 19.82, 40.81, 48.45, 59.17, 50.22, 42.88, 21.0, 8.96])]
    #[case(1.75, &[8.66, 19.82, 40.81, 57.11, 70.33, 71.21, 59.18, 42.88, 21.0, 8.96])]
    #[case(2.0, &[8.66, 19.82, 40.81, 57.11, 70.33, 71.21, 59.18, 42.88, 21.0, 8.96])]
    fn retention_smears_the_half_hour_curve(
        #[case] retention_hours: f64,
        #[case] expected: &[f64],
    ) {
        let smeared = apply_retention(
            &half_hour_curve(),
            BusinessDriverId::new(),
            30,
            &FixedRetention(retention_hours),
        );

        assert_from(&smeared, 37, expected);
    }

    #[test]
    fn with_no_retention_data_the_curve_is_left_as_it_is() {
        let original = quarter_hour_curve();

        let smeared = apply_retention(&original, BusinessDriverId::new(), 15, &NoRetention);

        for (index, item) in original.iter().enumerate() {
            assert_eq!(smeared[index].value_per_period(), item.value_per_period());
        }
    }

    #[test]
    fn a_smear_running_past_the_end_of_the_day_stops_at_it() {
        // Demand in the very last period with two hours of retention has
        // nowhere to spread to.
        let period_length = 30;
        let mut array = curve(period_length, 0, &[]);
        let last = array.len() - 1;
        array[last].set_value_per_period(10.0);

        let smeared = apply_retention(
            &array,
            BusinessDriverId::new(),
            period_length,
            &FixedRetention(2.0),
        );

        assert_eq!(smeared[last].value_per_period(), 10.0);
    }

    #[rstest]
    // Retention shorter than a period reaches no further than its own.
    #[case(0.0, 15, 0)]
    #[case(0.25, 15, 0)]
    #[case(0.5, 15, 1)]
    #[case(2.0, 15, 7)]
    #[case(0.5, 30, 0)]
    #[case(0.75, 30, 1)]
    #[case(2.0, 30, 3)]
    fn the_retention_window_is_measured_in_whole_periods(
        #[case] retention_hours: f64,
        #[case] period_length: i32,
        #[case] expected: usize,
    ) {
        assert_eq!(
            additional_periods_to_retain(retention_hours, period_length),
            expected
        );
    }
}
