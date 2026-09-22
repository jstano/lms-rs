//! Port of the `com.unifocus.tbx.core.TDouble` overloads `scheduler` calls so far:
//! `TDouble.round(double)` (no decimal-places argument), used by `PeakPlannedShiftSorter`/
//! `ModifiedPeakPlannedShiftSorter` to bucket projected/available hours to whole numbers before
//! comparing, and `TDouble.roundHours(double)` (`= round(double, 2)`), used by
//! `FlatProjectedHoursReducer` to round hours per day.
//!
//! Per the `tdouble-rounding-parity-trap` memory, `TDouble` has three mutually inconsistent
//! rounding modes and `planner`'s `round_to` matches none of them; `workrules::common::numbers`
//! already ported all of them from the real `TDouble.java` (not under `taps`, only test stubs of
//! it are) with the exact rounding-mode breakdown. `scheduler`/`workrules` have zero cross-crate
//! dependency (`PLAN_SCHEDULER.md`'s decision), so [`round_to`]/[`round_hours`]/[`java_signum`]
//! here are independently re-ported from `workrules::common::numbers`'s implementation (same
//! algorithm, same tests transcribed) rather than shared — only `round_hours` (`round(v, 2)`) is
//! needed so far, not the full `TDouble` surface `workrules` carries (`round_currency`,
//! `round_percent`, etc.).
//!
//! [`tdouble_round`] is `TDouble.round(double)`, the zero-argument overload: "hand-rolled
//! half-up away from zero, not `Math.rint`" — i.e. `Math.floor(v + 0.5)` for `v >= 0`,
//! `Math.ceil(v - 0.5)` for `v < 0`. **Do not** conflate it with [`round_hours`]/[`round_to`] —
//! the two-decimal-place overload adds a half-to-even pass plus an epsilon nudge this one does
//! not do.

/// `TDouble.round(double)`.
pub fn tdouble_round(value: f64) -> i32 {
    if value >= 0.0 {
        (value + 0.5).floor() as i32
    } else {
        (value - 0.5).ceil() as i32
    }
}

const HOURS_ROUND_PRECISION: usize = 2;
const ROUND_MULTIPLIERS: [f64; 3] = [1.0, 10.0, 100.0];

/// Java's `Math.signum`, which (unlike Rust's `f64::signum`) returns `0.0`/`-0.0`/`NaN`
/// unchanged instead of `1.0`/`-1.0` for a zero or NaN input.
fn java_signum(value: f64) -> f64 {
    if value.is_nan() || value == 0.0 {
        value
    } else if value > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `TDouble.round(double, int)`, narrowed to the precisions `scheduler` needs (0-2 places) —
/// half to even, plus the epsilon nudge `TDouble`'s own comment explains: `1.175` stores as
/// `1.1749999999…`, so a plain ties-to-even rounds it down to `1.17` when the intent was `1.18`.
fn round_to(value: f64, decimals: usize) -> f64 {
    if decimals == 0 {
        return value.round_ties_even();
    }

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

/// `TDouble.roundHours(double)` (`= round(double, 2)`).
pub fn round_hours(value: f64) -> f64 {
    round_to(value, HOURS_ROUND_PRECISION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_half_rounds_up() {
        assert_eq!(tdouble_round(2.5), 3);
    }

    #[test]
    fn negative_half_rounds_away_from_zero() {
        assert_eq!(tdouble_round(-2.5), -3);
    }

    #[test]
    fn ordinary_values_round_to_the_nearest_whole_number() {
        assert_eq!(tdouble_round(2.4), 2);
        assert_eq!(tdouble_round(2.6), 3);
        assert_eq!(tdouble_round(-2.4), -2);
        assert_eq!(tdouble_round(-2.6), -3);
    }

    #[test]
    fn zero_rounds_to_zero() {
        assert_eq!(tdouble_round(0.0), 0);
    }

    // The following four `round_hours` cases are transcribed from
    // `workrules::common::numbers`'s `round_to`/`round_hours` tests, which independently ported
    // the same `TDouble.round(double, int)` overload from the real `TDouble.java` — see this
    // module's doc for why the two crates each carry their own copy.

    #[test]
    fn round_hours_rounds_to_hundredths() {
        assert_eq!(round_hours(10.714_285_7), 10.71);
    }

    #[test]
    fn tdoubles_documented_example_still_lands_where_java_says() {
        // TDouble's comment cites 1.175 storing as 1.1749999999. In f64 it actually stores just
        // *above* 1.175, so ties-to-even reaches 1.18 on its own. Asserted anyway: the
        // documented answer is what matters.
        assert_eq!(round_hours(1.175), 1.18);
    }

    #[test]
    fn the_nudge_is_signed_so_negatives_round_the_same_way() {
        assert_eq!(round_hours(-1.175), -1.18);
        assert_eq!(round_hours(2.675), 2.68);
        assert_eq!(round_hours(-2.675), -2.68);
    }

    #[test]
    fn negative_zero_is_normalised_to_positive_zero() {
        let rounded = round_hours(-8.56354e-16);
        assert_eq!(rounded, 0.0);
        assert!(rounded.is_sign_positive());
    }
}
