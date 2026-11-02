//! `SplitShiftPayTypes`, the premiums `SplitShiftDifferentialRateRuleImpl` can add together.
//!
//! **Source not located.** Java imports
//! `com.unifocus.watson.common.ta.splitshiftpaytypes.SplitShiftPayTypes`, which
//! is not in the `taps` checkout. The four constants are reconstructed from the
//! rule (which puts exactly these four keys in its premium map) and its spec
//! (which configures them by `name()`): `PAY_FIRST_SHIFT_HOURS`,
//! `PAY_SECOND_SHIFT_HOURS`, `PAY_TIME_BETWEEN_SHIFTS` and `FIXED_AMOUNT`.
//! Any further constant could never be paid by the rule (`getOrDefault(.., 0.0)`),
//! so only config parsing could differ — divergence 90 again.

/// One component of a split-shift premium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitShiftPayTypes {
    /// `PAY_FIRST_SHIFT_HOURS`.
    PayFirstShiftHours,
    /// `PAY_SECOND_SHIFT_HOURS`.
    PaySecondShiftHours,
    /// `PAY_TIME_BETWEEN_SHIFTS`.
    PayTimeBetweenShifts,
    /// `FIXED_AMOUNT`.
    FixedAmount,
}

impl SplitShiftPayTypes {
    /// The constant's `name()`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "PAY_FIRST_SHIFT_HOURS" => Some(Self::PayFirstShiftHours),
            "PAY_SECOND_SHIFT_HOURS" => Some(Self::PaySecondShiftHours),
            "PAY_TIME_BETWEEN_SHIFTS" => Some(Self::PayTimeBetweenShifts),
            "FIXED_AMOUNT" => Some(Self::FixedAmount),
            _ => None,
        }
    }
}

/// Parse a rule parameter such as `["PAY_FIRST_SHIFT_HOURS","FIXED_AMOUNT"]`,
/// **keeping order and duplicates** (a type listed twice is paid twice). Names
/// may be quoted or bare; an unknown name is dropped (Java's `valueOf` throws);
/// a value that is not a bracketed list yields nothing.
pub fn parse_split_shift_pay_types(value: &str) -> Vec<SplitShiftPayTypes> {
    let Some(inner) = value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|name| name.trim().trim_matches(|c| c == '"' || c == '\''))
        .filter_map(SplitShiftPayTypes::from_name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_duplicates_are_kept_and_unknown_names_dropped() {
        assert_eq!(
            parse_split_shift_pay_types(
                "[\"FIXED_AMOUNT\", PAY_FIRST_SHIFT_HOURS,\"FIXED_AMOUNT\",\"NOPE\"]"
            ),
            [
                SplitShiftPayTypes::FixedAmount,
                SplitShiftPayTypes::PayFirstShiftHours,
                SplitShiftPayTypes::FixedAmount
            ]
        );
        assert!(parse_split_shift_pay_types("[]").is_empty());
        assert!(parse_split_shift_pay_types("garbage").is_empty());
    }
}
