//! Port of `com.unifocus.tbx.core.TDouble`.
//!
//! Ground truth: `~/workspace/unifocus/legacy-tbx-10.0/legacy-tbx-core/src/main/
//! java/com/unifocus/tbx/core/TDouble.java`. That package is *not* under `taps`,
//! which carries only test stubs of it.
//!
//! # Three rounding modes, and why they must stay separate
//!
//! `TDouble` does not have one rounding rule. It has three, and they disagree
//! with each other:
//!
//! | Java | Here | Semantics |
//! |---|---|---|
//! | `round(double)` | [`round`] | half-up **away from zero** |
//! | `round(double, 0)` | [`round_to`] with `0` | `Math.rint` — half to **even** |
//! | `round(double, n>0)` | [`round_to`] | half to even **plus an epsilon nudge** |
//!
//! So a call site that used `round(v)` and one that used `round(v, 0)` produce
//! different answers for `2.5` (3 vs 2). Resolving each of the ~137 `TDouble`
//! call sites in the rules engine to the overload it actually invoked is part
//! of porting that rule — there is no single function to map them all onto.
//!
//! The epsilon in the `n > 0` case is deliberate, and `TDouble`'s own comment
//! explains it: `1.175` is stored as `1.1749999999…`, so a plain rint rounds it
//! down to `1.17` when the intent was `1.18`. Nudging by `signum(v) * 0.01 / m`
//! before rounding recovers the intended value.
//!
//! `planner/src/workcontent/common/numbers.rs::round_to` is
//! `(value * factor).round() / factor` — half away from zero, no epsilon. It
//! therefore disagrees with `TDouble::round(v, n)` on exactly the boundaries
//! that decide money. Do not reuse it here; that is the drift its own doc
//! comments describe.

/// Java's `roundMultipliers`. Its length caps the supported precision at 6.
const ROUND_MULTIPLIERS: [f64; 7] = [1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0];

pub const CURRENCY_ROUND_PRECISION: usize = 4;
pub const HOURS_ROUND_PRECISION: usize = 2;
const DISPLAY_CURRENCY_ROUND_PRECISION: usize = 2;
const RAW_HOURS_ROUND_PRECISION: usize = 4;
const ACCRUAL_ROUND_PRECISION: usize = 6;
const ROUND_PERCENT_PRECISION: usize = 4;

/// `Math.signum`, which differs from Rust's `f64::signum` at zero.
///
/// Java returns the argument itself for `0.0` and `-0.0`; Rust returns `1.0`
/// and `-1.0`. The difference feeds straight into [`round_to`]'s nudge term, so
/// it has to be reproduced rather than papered over.
fn java_signum(value: f64) -> f64 {
    if value.is_nan() || value == 0.0 {
        value
    } else if value > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Round to a whole number, half-up **away from zero**.
///
/// `TDouble.round(double)`. Note this is *not* the same rule as
/// [`round_to`]`(value, 0)`, which rounds half to even.
pub fn round(value: f64) -> i32 {
    if value >= 0.0 {
        let truncated = value as i32;
        if value - f64::from(truncated) >= 0.5 {
            truncated + 1
        } else {
            truncated
        }
    } else {
        let positive = -value;
        let truncated = positive as i32;
        let magnitude = if positive - f64::from(truncated) >= 0.5 {
            truncated + 1
        } else {
            truncated
        };
        -magnitude
    }
}

/// Round to a whole number, half-up away from zero, as an `i64`.
///
/// `TDouble.roundLong(double)`.
pub fn round_long(value: f64) -> i64 {
    if value >= 0.0 {
        let truncated = value as i64;
        if value - (truncated as f64) >= 0.5 {
            truncated + 1
        } else {
            truncated
        }
    } else {
        let positive = -value;
        let truncated = positive as i64;
        let magnitude = if positive - (truncated as f64) >= 0.5 {
            truncated + 1
        } else {
            truncated
        };
        -magnitude
    }
}

/// Truncate toward zero. `TDouble.truncate(double)`.
pub fn truncate(value: f64) -> i32 {
    value as i32
}

/// Round to `decimals` places.
///
/// `TDouble.round(double, int)`. At `decimals == 0` this is plain half-to-even.
/// Above that it applies the epsilon nudge described in the module docs, then
/// collapses any result within `10^-decimals` of zero to positive `0.0` — which
/// in practice only normalises `-0.0`, since the result is already a multiple
/// of `10^-decimals`.
///
/// # Panics
///
/// If `decimals` exceeds 6. Java throws
/// `RoundMultipliersArrayIndexOutOfBoundsException` here; every call site in
/// the rules engine passes a constant of 2, 4 or 6, so this is unreachable
/// rather than a condition to thread a `Result` through.
pub fn round_to(value: f64, decimals: usize) -> f64 {
    if decimals == 0 {
        return value.round_ties_even();
    }

    assert!(
        decimals < ROUND_MULTIPLIERS.len(),
        "TDouble supports at most {} decimal places, got {decimals}",
        ROUND_MULTIPLIERS.len() - 1
    );

    let multiplier = ROUND_MULTIPLIERS[decimals];
    let rounded = (value * multiplier + java_signum(value) * 0.01 / multiplier).round_ties_even()
        / multiplier;

    let epsilon = 10f64.powi(-(decimals as i32));

    if rounded <= -epsilon || rounded >= epsilon {
        rounded
    } else {
        0.0
    }
}

/// Round a percentage to 4 places. `TDouble.roundPercent`.
pub fn round_percent(value: f64) -> f64 {
    round_to(value, ROUND_PERCENT_PRECISION)
}

/// Round currency for display, to 2 places. `TDouble.roundDisplayCurrency`.
///
/// Distinct from [`round_currency`], which keeps 4 places for calculation.
pub fn round_display_currency(value: f64) -> f64 {
    round_to(value, DISPLAY_CURRENCY_ROUND_PRECISION)
}

/// Round currency for calculation, to 4 places. `TDouble.roundCurrency`.
pub fn round_currency(value: f64) -> f64 {
    round_to(value, CURRENCY_ROUND_PRECISION)
}

/// Round hours to 2 places. `TDouble.roundHours`.
pub fn round_hours(value: f64) -> f64 {
    round_to(value, HOURS_ROUND_PRECISION)
}

/// Round hours to 4 places, before they are reduced to display precision.
/// `TDouble.roundRawHours`.
pub fn round_raw_hours(value: f64) -> f64 {
    round_to(value, RAW_HOURS_ROUND_PRECISION)
}

/// Round accrual hours to 6 places. `TDouble.roundAccrualHours`.
pub fn round_accrual_hours(value: f64) -> f64 {
    round_to(value, ACCRUAL_ROUND_PRECISION)
}

/// `value1 - value2`. `TDouble.computeVariance`.
pub fn compute_variance(value1: f64, value2: f64) -> f64 {
    value1 - value2
}

/// Percent variance, `(value1 - value2) / value2 * 100`, with the zero cases
/// Java special-cases rather than dividing. `TDouble.computeVariancePercent`.
pub fn compute_variance_percent(value1: f64, value2: f64) -> f64 {
    if value1 != 0.0 && value2 != 0.0 && value1 != value2 {
        (value1 - value2) / value2 * 100.0
    } else if value1 == 0.0 && value2 == 0.0 {
        0.0
    } else if value1 == 0.0 {
        -100.0
    } else if value2 == 0.0 {
        100.0
    } else {
        // value1 == value2, both non-zero.
        0.0
    }
}

/// Is `variance_percent` outside the band?
///
/// `TDouble.isOutsideVariance(double, double, double)`. `from_variance` and
/// `to_variance` are always positive; the band is `[-from_variance,
/// to_variance]`, so it is asymmetric by design.
pub fn is_outside_variance(variance_percent: f64, from_variance: f64, to_variance: f64) -> bool {
    variance_percent < -from_variance || variance_percent > to_variance
}

/// Is the percent variance between two values outside the band?
///
/// `TDouble.isOutsideVariance(double, double, double, double)`.
pub fn is_outside_variance_of(
    value1: f64,
    value2: f64,
    from_variance: f64,
    to_variance: f64,
) -> bool {
    is_outside_variance(
        compute_variance_percent(value1, value2),
        from_variance,
        to_variance,
    )
}

/// Divide, falling back to `default_value` when the divisor is too near zero.
///
/// `TDouble.safeDivide(Double, Double, int, Double)`. `precision` selects the
/// threshold `10^-precision` that the divisor's magnitude must reach.
///
/// # Panics
///
/// If `precision` exceeds 8, matching Java's `IllegalArgumentException`.
pub fn safe_divide(dividend: f64, divisor: f64, precision: usize, default_value: f64) -> f64 {
    const PRECISIONS: [f64; 9] = [
        1.0, 0.1, 0.01, 0.001, 0.0001, 0.00001, 0.000001, 0.0000001, 0.00000001,
    ];

    assert!(
        precision < PRECISIONS.len(),
        "Precision must be between 0 and 8"
    );

    if divisor.abs() >= PRECISIONS[precision] {
        dividend / divisor
    } else {
        default_value
    }
}

/// [`safe_divide`] with Java's defaults: precision 4, falling back to `0.0`.
pub fn safe_divide_default(dividend: f64, divisor: f64) -> f64 {
    safe_divide(dividend, divisor, 4, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0.0, 0)]
    #[case(0.49, 0)]
    #[case(0.5, 1)]
    #[case(1.5, 2)]
    #[case(2.5, 3)]
    #[case(-0.49, 0)]
    #[case(-0.5, -1)]
    #[case(-1.5, -2)]
    #[case(-2.5, -3)]
    fn round_goes_half_up_away_from_zero(#[case] value: f64, #[case] expected: i32) {
        assert_eq!(round(value), expected);
    }

    #[rstest]
    #[case(0.5, 0.0)]
    #[case(1.5, 2.0)]
    #[case(2.5, 2.0)]
    #[case(3.5, 4.0)]
    #[case(-2.5, -2.0)]
    fn round_to_zero_places_goes_half_to_even(#[case] value: f64, #[case] expected: f64) {
        assert_eq!(round_to(value, 0), expected);
    }

    #[test]
    fn the_two_whole_number_roundings_disagree_and_that_is_the_point() {
        // The reason each call site has to be resolved to its Java overload.
        assert_eq!(round(2.5), 3);
        assert_eq!(round_to(2.5, 0), 2.0);
    }

    #[test]
    fn the_epsilon_nudge_rescues_values_that_binary_stores_low() {
        // 1.005 is held as 1.00499999999999989..., so *100 lands below the tie
        // and a plain rint gives 1.00. The nudge recovers the intended 1.01.
        assert_eq!(round_to(1.005, 2), 1.01);
        assert_eq!((1.005f64 * 100.0).round_ties_even() / 100.0, 1.0);
    }

    #[test]
    fn the_nudge_reaches_slightly_below_the_true_half() {
        // The nudge is a fixed 0.01/multiplier, so values a hair under the tie
        // are pulled up too. Mathematically generous, but it is what Java does,
        // and matching it is the point.
        assert_eq!(round_to(1.0049999, 2), 1.01);
    }

    #[test]
    fn tdoubles_documented_example_still_lands_where_java_says() {
        // TDouble's comment cites 1.175 storing as 1.1749999999. In f64 it
        // actually stores just *above* 1.175, so ties-to-even reaches 1.18 on
        // its own. Asserted anyway: the documented answer is what matters.
        assert_eq!(round_to(1.175, 2), 1.18);
    }

    #[rstest]
    #[case(-1.175, -1.18)]
    #[case(2.675, 2.68)]
    #[case(-2.675, -2.68)]
    fn the_nudge_is_signed_so_negatives_round_the_same_way(
        #[case] value: f64,
        #[case] expected: f64,
    ) {
        assert_eq!(round_to(value, 2), expected);
    }

    #[test]
    fn negative_zero_is_normalised_to_positive_zero() {
        // Values like -8.5e-16 round to "-0.00" and then fail a == 0.0 test.
        let rounded = round_to(-8.56354e-16, 2);
        assert_eq!(rounded, 0.0);
        assert!(rounded.is_sign_positive());
    }

    #[test]
    fn java_signum_returns_zero_at_zero_unlike_rust() {
        assert_eq!(java_signum(0.0), 0.0);
        assert_eq!(java_signum(-0.0), -0.0);
        assert!(java_signum(-0.0).is_sign_negative());
        assert_eq!(java_signum(3.2), 1.0);
        assert_eq!(java_signum(-3.2), -1.0);
        // Rust's own signum, for contrast.
        assert_eq!(0.0f64.signum(), 1.0);
    }

    #[test]
    fn zero_rounds_to_zero_at_every_precision() {
        for decimals in 0..=6 {
            assert_eq!(round_to(0.0, decimals), 0.0);
        }
    }

    #[rstest]
    #[case(0.005, 0.01)]
    #[case(0.004, 0.0)]
    #[case(-0.005, -0.01)]
    fn half_cent_boundaries(#[case] value: f64, #[case] expected: f64) {
        assert_eq!(round_to(value, 2), expected);
    }

    #[test]
    fn the_domain_wrappers_use_their_documented_precision() {
        assert_eq!(round_hours(1.23456), 1.23);
        assert_eq!(round_raw_hours(1.23456), 1.2346);
        assert_eq!(round_currency(1.234567), 1.2346);
        assert_eq!(round_display_currency(1.234567), 1.23);
        assert_eq!(round_accrual_hours(1.23456789), 1.234568);
        assert_eq!(round_percent(12.345678), 12.3457);
    }

    #[test]
    fn currency_keeps_four_places_and_display_currency_two() {
        // These are different functions in Java and are not interchangeable.
        assert_ne!(round_currency(1.00005), round_display_currency(1.00005));
    }

    #[test]
    #[should_panic(expected = "at most 6 decimal places")]
    fn a_precision_beyond_the_multiplier_table_panics() {
        round_to(1.0, 7);
    }

    #[test]
    fn truncate_and_round_long_follow_java() {
        assert_eq!(truncate(1.9), 1);
        assert_eq!(truncate(-1.9), -1);
        assert_eq!(round_long(2.5), 3);
        assert_eq!(round_long(-2.5), -3);
    }

    #[rstest]
    #[case(10.0, 8.0, 2.0)]
    #[case(8.0, 10.0, -2.0)]
    fn variance_is_a_plain_difference(
        #[case] value1: f64,
        #[case] value2: f64,
        #[case] expected: f64,
    ) {
        assert_eq!(compute_variance(value1, value2), expected);
    }

    #[rstest]
    #[case(0.0, 0.0, 0.0)]
    #[case(0.0, 5.0, -100.0)]
    #[case(5.0, 0.0, 100.0)]
    #[case(5.0, 5.0, 0.0)]
    #[case(110.0, 100.0, 10.0)]
    #[case(90.0, 100.0, -10.0)]
    fn variance_percent_special_cases_zero_rather_than_dividing(
        #[case] value1: f64,
        #[case] value2: f64,
        #[case] expected: f64,
    ) {
        assert_eq!(compute_variance_percent(value1, value2), expected);
    }

    #[rstest]
    // The worked examples from TDouble's own javadoc.
    #[case(-6.0, 7.0, 5.0, false)]
    #[case(20.0, 10.0, 10.0, true)]
    #[case(-5.0, 6.0, 4.0, false)]
    #[case(5.0, 6.0, 4.0, true)]
    fn the_variance_band_is_asymmetric(
        #[case] variance_percent: f64,
        #[case] from_variance: f64,
        #[case] to_variance: f64,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_outside_variance(variance_percent, from_variance, to_variance),
            expected
        );
    }

    #[rstest]
    // Also from the javadoc.
    #[case(34.0, 32.0, 7.0, 5.0, true)]
    #[case(32.0, 34.0, 7.0, 5.0, false)]
    #[case(21.0, 20.0, 6.0, 4.0, true)]
    #[case(20.0, 21.0, 6.0, 4.0, false)]
    fn is_outside_variance_of_two_values(
        #[case] value1: f64,
        #[case] value2: f64,
        #[case] from_variance: f64,
        #[case] to_variance: f64,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_outside_variance_of(value1, value2, from_variance, to_variance),
            expected
        );
    }

    #[test]
    fn safe_divide_falls_back_when_the_divisor_is_too_small() {
        assert_eq!(safe_divide(10.0, 2.0, 4, -1.0), 5.0);
        // 0.00001 is below the 10^-4 threshold.
        assert_eq!(safe_divide(10.0, 0.00001, 4, -1.0), -1.0);
        // Exactly at the threshold still divides.
        assert_eq!(safe_divide(1.0, 0.0001, 4, -1.0), 10000.0);
        assert_eq!(safe_divide_default(10.0, 0.0), 0.0);
    }

    #[test]
    fn safe_divide_uses_the_magnitude_so_negative_divisors_work() {
        assert_eq!(safe_divide(10.0, -2.0, 4, -1.0), -5.0);
    }

    #[test]
    #[should_panic(expected = "Precision must be between 0 and 8")]
    fn safe_divide_rejects_an_out_of_range_precision() {
        safe_divide(1.0, 1.0, 9, 0.0);
    }
}
