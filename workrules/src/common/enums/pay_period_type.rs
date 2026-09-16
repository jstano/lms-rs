//! Port of `com.unifocus.watson.common.enums.PayPeriodType`.
//!
//! Marked `@Deprecated` in Java but still referenced from the rules tree, so it
//! comes across as-is.

use crate::coded_enum;

coded_enum! {
    /// How often a pay period closes. `PayPeriodType`.
    PayPeriodType {
        Weekly => "WK",
        BiWeekly => "BW",
        SemiMonthly => "SM",
        Monthly => "MN",
    }
}

impl PayPeriodType {
    /// How many periods fall in a year. `getPeriodsPerYear()`.
    pub fn periods_per_year(&self) -> i32 {
        match self {
            Self::Weekly => 52,
            Self::BiWeekly => 26,
            Self::SemiMonthly => 24,
            Self::Monthly => 12,
        }
    }

    /// Does this period align to whole weeks? `isWeekBased()`.
    ///
    /// Semi-monthly and monthly periods cut across weeks, so rules that
    /// accumulate weekly totals have to treat them differently.
    pub fn is_week_based(&self) -> bool {
        matches!(self, Self::Weekly | Self::BiWeekly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in PayPeriodType::VALUES {
            assert_eq!(PayPeriodType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn periods_per_year() {
        assert_eq!(PayPeriodType::Weekly.periods_per_year(), 52);
        assert_eq!(PayPeriodType::BiWeekly.periods_per_year(), 26);
        assert_eq!(PayPeriodType::SemiMonthly.periods_per_year(), 24);
        assert_eq!(PayPeriodType::Monthly.periods_per_year(), 12);
    }

    #[test]
    fn only_weekly_and_bi_weekly_align_to_weeks() {
        assert!(PayPeriodType::Weekly.is_week_based());
        assert!(PayPeriodType::BiWeekly.is_week_based());
        assert!(!PayPeriodType::SemiMonthly.is_week_based());
        assert!(!PayPeriodType::Monthly.is_week_based());
    }
}
