//! Numeric helpers ported from the Java `Numbers` utility class.
//!
//! The Java engine funnels every duration through these so that repeated
//! arithmetic stays stable. They are kept here as free functions so the
//! generators can share one definition of "rounded".

/// Round a value expressed in hours to hundredths of an hour.
pub fn round_hours(hours: f64) -> f64 {
    round_to(hours, 2)
}

/// Round a raw (not yet unit-converted) value to ten-thousandths.
///
/// Used for the per-standard work-minute results, which are accumulated into
/// an integer total afterwards, and for every per-period value the flowed
/// distribution engine adds up. The extra precision over [`round_hours`] is
/// load-bearing: the distribution converters carry a known rounding drift that
/// only shows up at the fourth decimal.
pub fn round_raw_hours(value: f64) -> f64 {
    round_to(value, 4)
}

/// Round a percentage-like value to ten-thousandths.
///
/// Every `DistributionItem` addition funnels through this, so it is what keeps
/// repeated accumulation across a period array stable.
pub fn round_percent(value: f64) -> f64 {
    round_to(value, 4)
}

/// Round to the nearest whole number.
pub fn round(value: f64) -> i32 {
    value.round() as i32
}

/// Drop the fractional part, rounding toward zero.
pub fn truncate(value: f64) -> i32 {
    value.trunc() as i32
}

/// Round to a given number of decimal places.
pub fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);

    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(8.0, 8.0)]
    #[case(7.499, 7.5)]
    #[case(7.494, 7.49)]
    #[case(0.333333, 0.33)]
    #[case(1.006, 1.01)]
    fn round_hours_rounds_to_hundredths(#[case] input: f64, #[case] expected: f64) {
        assert_eq!(round_hours(input), expected);
    }

    #[rstest]
    #[case(0.4, 0)]
    #[case(0.5, 1)]
    #[case(1.49, 1)]
    #[case(1.5, 2)]
    #[case(120.0, 120)]
    fn round_rounds_to_nearest_whole(#[case] input: f64, #[case] expected: i32) {
        assert_eq!(round(input), expected);
    }

    #[rstest]
    #[case(0.9, 0)]
    #[case(1.0, 1)]
    #[case(2.99, 2)]
    #[case(-1.5, -1)]
    fn truncate_drops_the_fraction(#[case] input: f64, #[case] expected: i32) {
        assert_eq!(truncate(input), expected);
    }

    #[rstest]
    #[case(123.456, 123.456)]
    #[case(123.45678, 123.4568)]
    #[case(0.00004, 0.0)]
    // Pins the precision the flowed distribution converters depend on: the Java
    // suite asserts `roundRawHours(total) == 149.0008`, which a two-decimal
    // round could never produce.
    #[case(149.00081, 149.0008)]
    fn round_raw_hours_rounds_to_ten_thousandths(#[case] input: f64, #[case] expected: f64) {
        assert_eq!(round_raw_hours(input), expected);
    }

    #[rstest]
    #[case(0.5, 0.5)]
    #[case(0.123456, 0.1235)]
    #[case(30.0, 30.0)]
    fn round_percent_rounds_to_ten_thousandths(#[case] input: f64, #[case] expected: f64) {
        assert_eq!(round_percent(input), expected);
    }

    #[rstest]
    #[case(1.23456, 0, 1.0)]
    #[case(1.23456, 2, 1.23)]
    #[case(1.23456, 4, 1.2346)]
    fn round_to_rounds_to_the_requested_decimals(
        #[case] input: f64,
        #[case] decimals: u32,
        #[case] expected: f64,
    ) {
        assert_eq!(round_to(input, decimals), expected);
    }
}
