//! Port of `com.unifocus.watson.common.enums.PunchType`.
//!
//! The most behaviour-carrying enum in the set: six classification predicates
//! that rules branch on. Java warns in a comment that this enum is duplicated
//! into the UFTC time clock and must stay backwards compatible, which is why
//! the codes are not derived from the variant names.

use crate::coded_enum;

coded_enum! {
    /// What a punch records. `PunchType`.
    PunchType {
        In => "IN",
        Break => "BREAK",
        Back => "BACK",
        Out => "OUT",
        Tips => "TIPS",
        Gross => "GRS",
        Pieces => "PCS",
        ChargeTips => "GTPS",
        Memo1 => "MEMO1",
        Memo2 => "MEMO2",
        Memo3 => "MEMO3",
        Memo4 => "MEMO4",
        OnSite => "ON_SITE",
        OffSite => "OFF_SITE",
        Meal => "MEAL",
    }
}

impl PunchType {
    /// Does this punch record a time? `isTimePunchType()`.
    pub fn is_time_punch_type(&self) -> bool {
        matches!(self, Self::In | Self::Break | Self::Back | Self::Out)
    }

    /// Does this punch record on/off site? `isAccessModePunchType()`.
    pub fn is_access_mode_punch_type(&self) -> bool {
        matches!(self, Self::OnSite | Self::OffSite)
    }

    /// Is this the meal punch? `isMealPunchType()`.
    pub fn is_meal_punch_type(&self) -> bool {
        matches!(self, Self::Meal)
    }

    /// Does this punch record an earning? `isEarningPunchType()`.
    ///
    /// Java defines this by exclusion rather than by listing variants, so a new
    /// punch type is an earning punch until someone says otherwise. Kept that
    /// way deliberately — listing the variants here would silently change
    /// behaviour the next time the enum grows.
    pub fn is_earning_punch_type(&self) -> bool {
        !self.is_time_punch_type()
            && !self.is_access_mode_punch_type()
            && !self.is_meal_punch_type()
    }

    /// Is this a tip punch? `isTipPunchType()` — both cash and charged tips.
    pub fn is_tip_punch_type(&self) -> bool {
        matches!(self, Self::Tips | Self::ChargeTips)
    }

    /// Is this a gross sales punch? `isGrossSalesPunchType()`.
    pub fn is_gross_sales_punch_type(&self) -> bool {
        matches!(self, Self::Gross)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn every_code_round_trips() {
        for value in PunchType::VALUES {
            assert_eq!(PunchType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn the_codes_are_not_the_variant_names() {
        // They are shared with the time clock and cannot be regenerated.
        assert_eq!(PunchType::Gross.code(), "GRS");
        assert_eq!(PunchType::Pieces.code(), "PCS");
        assert_eq!(PunchType::ChargeTips.code(), "GTPS");
    }

    #[rstest]
    #[case(PunchType::In)]
    #[case(PunchType::Break)]
    #[case(PunchType::Back)]
    #[case(PunchType::Out)]
    fn the_four_time_punches(#[case] punch_type: PunchType) {
        assert!(punch_type.is_time_punch_type());
        assert!(!punch_type.is_earning_punch_type());
    }

    #[rstest]
    #[case(PunchType::OnSite)]
    #[case(PunchType::OffSite)]
    fn the_two_access_mode_punches(#[case] punch_type: PunchType) {
        assert!(punch_type.is_access_mode_punch_type());
        assert!(!punch_type.is_earning_punch_type());
    }

    #[test]
    fn meal_is_its_own_category_and_not_an_earning() {
        assert!(PunchType::Meal.is_meal_punch_type());
        assert!(!PunchType::Meal.is_earning_punch_type());
        assert!(!PunchType::Meal.is_time_punch_type());
    }

    #[rstest]
    #[case(PunchType::Tips)]
    #[case(PunchType::Gross)]
    #[case(PunchType::Pieces)]
    #[case(PunchType::ChargeTips)]
    #[case(PunchType::Memo1)]
    #[case(PunchType::Memo4)]
    fn everything_else_is_an_earning_punch(#[case] punch_type: PunchType) {
        assert!(punch_type.is_earning_punch_type());
    }

    #[test]
    fn the_categories_partition_the_enum() {
        for punch_type in PunchType::VALUES {
            let categories = [
                punch_type.is_time_punch_type(),
                punch_type.is_access_mode_punch_type(),
                punch_type.is_meal_punch_type(),
                punch_type.is_earning_punch_type(),
            ];
            assert_eq!(
                categories.iter().filter(|x| **x).count(),
                1,
                "{punch_type:?} should fall in exactly one category"
            );
        }
    }

    #[test]
    fn tips_covers_both_cash_and_charged() {
        assert!(PunchType::Tips.is_tip_punch_type());
        assert!(PunchType::ChargeTips.is_tip_punch_type());
        assert!(!PunchType::Gross.is_tip_punch_type());
    }

    #[test]
    fn gross_sales_is_only_gross() {
        assert!(PunchType::Gross.is_gross_sales_punch_type());
        assert!(!PunchType::Pieces.is_gross_sales_punch_type());
    }
}
