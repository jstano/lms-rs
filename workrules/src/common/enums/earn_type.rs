//! Port of `com.unifocus.watson.common.enums.EarnType`.

use crate::coded_enum;

coded_enum! {
    /// What kind of thing an earning type represents. `EarnType`.
    EarnType {
        Earning => "E",
        /// Recorded but not paid.
        Memo => "M",
        /// Adds to a balance rather than to pay.
        Accrual => "A",
        Regular => "R",
        Premium => "P",
    }
}

impl EarnType {
    /// `isEarning()`.
    pub fn is_earning(&self) -> bool {
        matches!(self, Self::Earning)
    }

    /// `isMemo()`.
    pub fn is_memo(&self) -> bool {
        matches!(self, Self::Memo)
    }

    /// `isAccrual()`.
    pub fn is_accrual(&self) -> bool {
        matches!(self, Self::Accrual)
    }

    /// `isRegular()`.
    pub fn is_regular(&self) -> bool {
        matches!(self, Self::Regular)
    }

    /// `isPremium()`.
    pub fn is_premium(&self) -> bool {
        matches!(self, Self::Premium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in EarnType::VALUES {
            assert_eq!(EarnType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn each_predicate_matches_exactly_its_own_variant() {
        assert!(EarnType::Earning.is_earning());
        assert!(EarnType::Memo.is_memo());
        assert!(EarnType::Accrual.is_accrual());
        assert!(EarnType::Regular.is_regular());
        assert!(EarnType::Premium.is_premium());
        assert!(!EarnType::Regular.is_earning());
    }
}
