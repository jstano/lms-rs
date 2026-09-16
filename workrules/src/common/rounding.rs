//! Port of `com.unifocus.tbx.core.rounding`.
//!
//! Ground truth: `~/workspace/unifocus/legacy-tbx-10.0/legacy-tbx-core/src/main/
//! java/com/unifocus/tbx/core/rounding/`.
//!
//! In Java this is an enum that reflectively instantiates one of four
//! `MultipleRounder` strategy classes, each a single-method class rounding an
//! integer to a multiple. Here it collapses to one enum and a `match` — the
//! reflection carried no behaviour, and neither does the resource key the Java
//! `toString()` looks up.
//!
//! Punch rounding runs through this, so it is on the critical path for the
//! first family ported.

/// How to round a value to a multiple. `RoundingOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingOption {
    /// `MultipleNearestRounder` — to the closest multiple.
    Nearest,
    /// `MultipleUpRounder` — toward positive infinity.
    Up,
    /// `MultipleDownRounder` — toward negative infinity.
    Down,
    /// `MultipleNoneRounder` — leaves the value alone.
    None,
}

impl RoundingOption {
    /// The persisted code. `RoundingOption.getCode()`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Nearest => "NEAR",
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::None => "NONE",
        }
    }

    /// Look up by persisted code. `RoundingOption.fromCode(String)`.
    ///
    /// Java throws `IllegalArgumentException` on an unknown code; here the
    /// caller decides, since these codes arrive from rule parameters and the
    /// database rather than from the program.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "NEAR" => Some(Self::Nearest),
            "UP" => Some(Self::Up),
            "DOWN" => Some(Self::Down),
            "NONE" => Some(Self::None),
            _ => None,
        }
    }

    /// Round `target` to the nearest multiple of `multiple` under this option.
    ///
    /// `MultipleRounder.round(int, int)`, with the strategy class chosen by
    /// [`RoundingOption::getInstance`] in Java folded into the match.
    ///
    /// Java computes in `double` and casts back, so this does the same rather
    /// than using integer arithmetic — the two differ once the quotient stops
    /// being exactly representable, and matching Java is the point.
    pub fn round(&self, target: i32, multiple: i32) -> i32 {
        if matches!(self, Self::None) {
            return target;
        }

        let quotient = f64::from(target) / f64::from(multiple);

        let scaled = match self {
            // Java's Math.round is floor(x + 0.5) — half toward *positive
            // infinity*, not half away from zero. The two agree for the
            // non-negative values punch rounding produces, but the engine is
            // matching Java, not approximating it.
            Self::Nearest => (quotient + 0.5).floor(),
            Self::Up => quotient.ceil(),
            Self::Down => quotient.floor(),
            Self::None => unreachable!("handled above"),
        };

        (scaled as i32) * multiple
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(RoundingOption::Nearest, "NEAR")]
    #[case(RoundingOption::Up, "UP")]
    #[case(RoundingOption::Down, "DOWN")]
    #[case(RoundingOption::None, "NONE")]
    fn codes_round_trip(#[case] option: RoundingOption, #[case] code: &str) {
        assert_eq!(option.code(), code);
        assert_eq!(RoundingOption::from_code(code), Some(option));
    }

    #[test]
    fn an_unknown_code_is_not_a_rounding_option() {
        assert_eq!(RoundingOption::from_code("SIDEWAYS"), None);
    }

    #[rstest]
    // 900 seconds is the 15-minute threshold punch rounding uses by default.
    #[case(0, 0)]
    #[case(449, 0)]
    #[case(450, 900)]
    #[case(451, 900)]
    #[case(900, 900)]
    #[case(1349, 900)]
    #[case(1350, 1800)]
    fn nearest_rounds_at_the_half_way_point(#[case] target: i32, #[case] expected: i32) {
        assert_eq!(RoundingOption::Nearest.round(target, 900), expected);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(1, 900)]
    #[case(899, 900)]
    #[case(900, 900)]
    #[case(901, 1800)]
    fn up_rounds_toward_positive_infinity(#[case] target: i32, #[case] expected: i32) {
        assert_eq!(RoundingOption::Up.round(target, 900), expected);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(899, 0)]
    #[case(900, 900)]
    #[case(1799, 900)]
    fn down_rounds_toward_negative_infinity(#[case] target: i32, #[case] expected: i32) {
        assert_eq!(RoundingOption::Down.round(target, 900), expected);
    }

    #[rstest]
    #[case(0)]
    #[case(451)]
    #[case(86399)]
    fn none_leaves_the_value_alone(#[case] target: i32) {
        assert_eq!(RoundingOption::None.round(target, 900), target);
    }

    #[test]
    fn nearest_rounds_a_negative_half_toward_positive_infinity() {
        // Java's Math.round(-0.5) is 0, not -1. Punch rounding never reaches
        // here, but the difference is the reason floor(x + 0.5) is spelled out
        // rather than using Rust's f64::round.
        assert_eq!(RoundingOption::Nearest.round(-450, 900), 0);
        assert_eq!((-0.5f64).round(), -1.0);
    }

    #[test]
    fn a_one_minute_threshold_is_a_no_op_on_whole_minutes() {
        // MinuteRoundingRuleConfig allows a round-to of 1, which the rule's own
        // test table asserts leaves 07:47:00 untouched.
        assert_eq!(RoundingOption::Nearest.round(28020, 60), 28020);
    }
}
