//! Turns work minutes per period into a head count per period.
//!
//! Minutes divide into the period length to give a fractional number of people,
//! which then has to become a whole one. How generously the fraction rounds up
//! is configured separately either side of one person, so that the first body
//! can be earned on a small fraction while later ones need a fuller period.

use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;

/// The precision fractions are compared against their threshold at.
const THRESHOLD_DECIMALS: u32 = 4;

pub trait MinutesToBodiesConverter {
    fn convert(
        &self,
        params: &GeneratorParameters,
        minutes_per_period: &[DistributionItem],
    ) -> Vec<DistributionItem>;
}

pub struct MinutesToBodiesConverterImpl {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl MinutesToBodiesConverterImpl {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl MinutesToBodiesConverter for MinutesToBodiesConverterImpl {
    fn convert(
        &self,
        params: &GeneratorParameters,
        minutes_per_period: &[DistributionItem],
    ) -> Vec<DistributionItem> {
        let mut bodies = self.list_creator.create_array_for(params);
        let period_length = params.period_length() as f64;

        for (index, minutes) in minutes_per_period.iter().enumerate() {
            // The canonical array spans the whole two days, so it is normally
            // the longer of the two; anything past its end has nowhere to go.
            let Some(body) = bodies.get_mut(index) else {
                break;
            };

            let people = minutes.value_per_period() / period_length;

            body.add_to_period_value(round_bodies(params, people) as f64);
        }

        bodies
    }
}

/// How many whole people `value` fractional people rounds to.
fn round_bodies(params: &GeneratorParameters, value: f64) -> i32 {
    let settings = params.planner_settings();

    if value == 0.0 {
        return 0;
    }

    if value < 1.0 {
        return round_below_one(value, settings.rounding_threshold_below_one);
    }

    round_above_one(value, settings.rounding_threshold_above_one)
}

/// Below one person, any fraction reaching the threshold earns the first body.
fn round_below_one(value: f64, threshold: f64) -> i32 {
    i32::from(reaches_threshold(value, threshold))
}

/// At or above one person, only the fraction past the whole part is weighed.
fn round_above_one(value: f64, threshold: f64) -> i32 {
    let whole = numbers::truncate(value);
    let fraction = value - whole as f64;

    whole + i32::from(reaches_threshold(fraction, threshold))
}

/// Whether `value` clears `threshold`.
///
/// A threshold of exactly zero compares strictly, so a value that rounds to
/// nothing does not earn a body; every other threshold is inclusive, so a value
/// landing exactly on it does. Replicated from Java, where the two comparisons
/// are written out separately.
fn reaches_threshold(value: f64, threshold: f64) -> bool {
    let rounded = numbers::round_to(value, THRESHOLD_DECIMALS);

    if threshold == 0.0 {
        rounded > threshold
    } else {
        rounded >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};
    use rstest::rstest;

    fn context(below_one: f64, above_one: f64) -> Context {
        let mut context = Context::new(
            LocalDate::new(2013, 8, 26),
            LocalTime::new(7, 30, 0),
            LocalTime::new(12, 0, 0),
            30,
        );
        context.planner_settings.rounding_threshold_below_one = below_one;
        context.planner_settings.rounding_threshold_above_one = above_one;
        context
    }

    /// Lays `minutes` into the canonical array starting at `start_index`.
    fn minutes_from(
        context: &Context,
        start_index: usize,
        minutes: &[f64],
    ) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in minutes.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value);
        }

        array
    }

    #[rstest]
    // The worked example from the acceptance criteria: minutes per half hour
    // become a head count per half hour, totalling 21 bodies.
    #[case(vec![4.0, 36.0, 32.0, 90.0, 120.0, 100.0, 90.0, 60.0, 30.0], vec![1.0, 2.0, 1.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0])]
    // The same, but the opening four minutes drop to nothing.
    #[case(vec![0.0, 36.0, 32.0, 90.0, 120.0, 100.0, 90.0, 60.0, 30.0], vec![0.0, 2.0, 1.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0])]
    fn converts_an_array_of_minutes_into_an_array_of_bodies(
        #[case] minutes: Vec<f64>,
        #[case] expected_bodies: Vec<f64>,
    ) {
        let context = context(0.1, 0.2);
        let params = context.params();
        let start_index = 15;
        let minutes_per_period = minutes_from(&context, start_index, &minutes);

        let bodies = MinutesToBodiesConverterImpl::new().convert(&params, &minutes_per_period);

        assert_eq!(bodies.len(), minutes_per_period.len());
        // The period before the work starts, and the one after it ends, are empty.
        assert_eq!(bodies[start_index - 1].value_per_period(), 0.0);
        assert_eq!(bodies[start_index + minutes.len()].value_per_period(), 0.0);

        for (offset, expected) in expected_bodies.iter().enumerate() {
            assert_eq!(
                bodies[start_index + offset].value_per_period(),
                *expected,
                "period {offset}"
            );
        }
    }

    #[test]
    fn the_converted_periods_keep_their_times() {
        let context = context(0.1, 0.2);
        let params = context.params();
        let minutes_per_period = minutes_from(&context, 15, &[4.0]);

        let bodies = MinutesToBodiesConverterImpl::new().convert(&params, &minutes_per_period);

        assert_eq!(
            bodies[15].date_time(),
            LocalDateTime::new(2013, 8, 26, 7, 30, 0)
        );
        assert_eq!(
            bodies[16].date_time(),
            LocalDateTime::new(2013, 8, 26, 8, 0, 0)
        );
    }

    #[rstest]
    // Nothing is always nobody, whatever the thresholds.
    #[case(0.0, 0.1, 0.2, 0)]
    // Below one body, clearing the threshold earns the first one.
    #[case(0.13, 0.1, 0.2, 1)]
    #[case(0.4, 0.1, 0.2, 1)]
    // Above one, only the fraction past the whole part counts, and it must
    // reach the above-one threshold of 0.2.
    #[case(1.0, 0.1, 0.2, 1)]
    #[case(1.01, 0.1, 0.2, 1)]
    #[case(1.05, 0.1, 0.2, 1)]
    #[case(1.1, 0.1, 0.2, 1)]
    #[case(1.2, 0.1, 0.2, 2)]
    #[case(1.5, 0.1, 0.2, 2)]
    #[case(1.9, 0.1, 0.2, 2)]
    // A stricter above-one threshold holds the fraction back until half.
    #[case(1.4, 0.1, 0.5, 1)]
    #[case(1.5, 0.1, 0.5, 2)]
    #[case(1.9, 0.1, 0.5, 2)]
    // A below-one threshold of zero rounds up on any positive fraction.
    #[case(0.13, 0.0, 0.2, 1)]
    #[case(1.07, 0.0, 0.2, 1)]
    #[case(3.33, 0.0, 0.2, 4)]
    #[case(4.01, 0.0, 0.2, 4)]
    // An above-one threshold of zero does the same past the whole part, but
    // an exact whole still stays put.
    #[case(0.0, 0.0, 0.0, 0)]
    #[case(0.9, 0.0, 0.0, 1)]
    #[case(1.0, 0.0, 0.0, 1)]
    #[case(1.01, 0.0, 0.0, 2)]
    #[case(4.0, 0.0, 0.0, 4)]
    #[case(4.01, 0.0, 0.0, 5)]
    // A tiny non-zero threshold is inclusive, so a fraction landing exactly on
    // it rounds up.
    #[case(5.01, 0.0, 0.01, 6)]
    fn fractional_people_round_to_whole_ones(
        #[case] value: f64,
        #[case] below_one: f64,
        #[case] above_one: f64,
        #[case] expected: i32,
    ) {
        let context = context(below_one, above_one);

        assert_eq!(round_bodies(&context.params(), value), expected);
    }
}
