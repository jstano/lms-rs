//! Holds each period's work to what the operation can physically serve.
//!
//! A curve can ask for more work in a period than there are seats to serve it
//! in. This clamps each period to its capacity and carries the excess forward,
//! filling later periods that have room to spare. Work still left over when the
//! shift ends is simply lost — the operation had nowhere to put it.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::RetentionCapacityProvider;

/// Clamp every period of the shift to its serving capacity.
pub fn apply_capacity(
    distribution: &[DistributionItem],
    business_driver_id: BusinessDriverId,
    work_minutes: f64,
    params: &GeneratorParameters,
    range: &DateTimeRangeWithPeriodLength,
    retention_capacity: &dyn RetentionCapacityProvider,
) -> Vec<DistributionItem> {
    let mut capped = distribution.to_vec();

    let (Some(starting_index), Some(end_index)) = (range.start_index(), range.end_index()) else {
        return capped;
    };

    let mut carryover_minutes = 0.0;

    for index in starting_index..end_index {
        let Some(item) = capped.get_mut(index as usize) else {
            break;
        };
        let date_time = item.date_time();

        let Some(utilization) = retention_capacity.utilization_for(business_driver_id, date_time)
        else {
            continue;
        };

        // How many minutes of work one period's worth of served guests
        // represents.
        let net_capacity = utilization.capacity * utilization.utilization;
        let driver_value = params
            .planner_model()
            .business_driver_value(business_driver_id, date_time.to_local_date());
        let minutes_per_guest = work_minutes / driver_value as f64;
        let capacity_per_period = numbers::round_raw_hours(net_capacity * minutes_per_guest);

        if capacity_per_period > 0.0 {
            carryover_minutes = apply_capacity_to_item(item, carryover_minutes, capacity_per_period);
        }
    }

    capped
}

/// Clamp one period, returning the minutes still looking for somewhere to go.
fn apply_capacity_to_item(
    item: &mut DistributionItem,
    mut carryover_minutes: f64,
    capacity_per_period: f64,
) -> f64 {
    let value = item.value_per_period();
    let extra_minutes = if value <= capacity_per_period {
        0.0
    } else {
        value - capacity_per_period
    };

    // A period with room to spare takes back as much of the carried overflow
    // as it can hold.
    if extra_minutes == 0.0 && carryover_minutes > 0.0 {
        let headroom = capacity_per_period - value;
        let remainder = carryover_minutes.min(headroom);

        item.add_to_period_value(remainder);

        return carryover_minutes - numbers::round_raw_hours(remainder);
    }

    carryover_minutes += numbers::round_raw_hours(extra_minutes);

    // Less than a minute left over is not worth carrying, and the period is
    // left as it is — including its excess.
    if carryover_minutes < 1.0 {
        return carryover_minutes;
    }

    // Java recomputes the carryover here as `(value + carryover) - value`,
    // which is the carryover it already held. Kept so the two read alike; it
    // has no effect either way.
    let new_total = value + carryover_minutes;
    if new_total > capacity_per_period {
        carryover_minutes = new_total - value;
    }

    // Unrounded, unlike the add above.
    item.subtract_from_period_value(extra_minutes);

    numbers::round_raw_hours(carryover_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use crate::workcontent::generators::advanced::providers::RetentionCapacityUtilization;
    use date_range_rs::{DateRange, DateTimeRange};
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};
    use rstest::rstest;
    use std::collections::HashMap;

    /// The Java fixture's capacity: a hundred seats, three-quarters used.
    struct Seating;

    impl RetentionCapacityProvider for Seating {
        fn utilization_for(
            &self,
            _business_driver_id: BusinessDriverId,
            _date_time: LocalDateTime,
        ) -> Option<RetentionCapacityUtilization> {
            Some(RetentionCapacityUtilization {
                retention_hours: 0.75,
                capacity: 100.0,
                utilization: 0.75,
            })
        }
    }

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    /// A context whose driver records `driver_value` covers on the shift date.
    fn context(business_driver_id: BusinessDriverId, driver_value: i32) -> Context {
        let mut context = Context::new(
            date(),
            LocalTime::new(5, 0, 0),
            LocalTime::new(11, 0, 0),
            30,
        );
        context.planner_model = PlannerModel::new(
            DateRange::new(date(), date()),
            PlannerMode::Standard,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                business_driver_id,
                BusinessDriverValues::new(
                    business_driver_id,
                    HashMap::from([(date(), driver_value)]),
                ),
            )]),
        );
        context
    }

    /// The pre-capacity curve, laid into a two-day half-hour array from 05:00.
    fn curve() -> Vec<DistributionItem> {
        let mut array: Vec<_> = (0..96)
            .map(|index| {
                DistributionItem::new(
                    index,
                    date().at_start_of_day().plus_minutes(index as i64 * 30),
                    0.0,
                )
            })
            .collect();

        let demand = [
            91.82, 94.36, 98.29, 111.46, 104.87, 124.51, 111.46, 115.38, 100.95, 110.07, 91.82,
        ];
        for (offset, value) in demand.iter().enumerate() {
            array[10 + offset].set_value_per_period(*value);
        }

        array
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        numbers::round_raw_hours(items.iter().map(|item| item.value_per_period()).sum())
    }

    #[rstest]
    // The shift runs to 11:00, so there is a spare period at the end for the
    // accumulated overflow to settle into.
    #[case((11, 0), 83.9136, 1154.99)]
    // Cut the shift half an hour short and that last period never runs, so the
    // overflow it would have absorbed is lost.
    #[case((10, 30), 0.0, 1071.0764)]
    fn work_over_capacity_carries_forward_into_periods_with_room(
        #[case] shift_end: (i32, i32),
        #[case] value_for_last_period: f64,
        #[case] expected_total: f64,
    ) {
        let business_driver_id = BusinessDriverId::new();
        let context = context(business_driver_id, 881);
        let params = context.params();
        let range = DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(
                date().at_time(LocalTime::new(5, 0, 0)),
                date().at_time(LocalTime::new(shift_end.0, shift_end.1, 0)),
            ),
            30,
        );

        let capped = apply_capacity(
            &curve(),
            business_driver_id,
            1155.0,
            &params,
            &range,
            &Seating,
        );

        assert_eq!(capped.len(), 96);
        // The first three periods are already inside capacity and keep their
        // own demand; every period after is held at the ceiling.
        assert_eq!(capped[10].value_per_period(), 91.82);
        assert_eq!(capped[11].value_per_period(), 94.36);
        assert_eq!(capped[12].value_per_period(), 98.29);
        for (index, item) in capped.iter().enumerate().take(21).skip(13) {
            assert_eq!(item.value_per_period(), 98.3258, "period {index}");
        }
        assert_eq!(capped[21].value_per_period(), value_for_last_period);
        assert_eq!(total_of(&capped), expected_total);
    }

    #[test]
    fn with_no_capacity_data_the_curve_is_left_as_it_is() {
        struct NoCapacity;
        impl RetentionCapacityProvider for NoCapacity {
            fn utilization_for(
                &self,
                _business_driver_id: BusinessDriverId,
                _date_time: LocalDateTime,
            ) -> Option<RetentionCapacityUtilization> {
                None
            }
        }

        let business_driver_id = BusinessDriverId::new();
        let context = context(business_driver_id, 881);
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let original = curve();

        let capped = apply_capacity(
            &original,
            business_driver_id,
            1155.0,
            &params,
            &range,
            &NoCapacity,
        );

        assert_eq!(total_of(&capped), total_of(&original));
    }
}
