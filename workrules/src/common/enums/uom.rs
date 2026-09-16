//! Port of `com.unifocus.watson.common.enums.UOM`.
//!
//! The unit an earning type is measured in. Rules branch on this constantly to
//! decide whether a value is hours to distribute or dollars to pay.

use crate::coded_enum;

coded_enum! {
    /// Unit of measure. `UOM`.
    UOM {
        Hours => "H",
        Dollars => "D",
        Units => "U",
        /// Carries both an hours figure and a dollar figure.
        HoursDollars => "C",
        Days => "Y",
        /// Hours at a rate, where the rate rather than the total is stored.
        HoursRate => "R",
    }
}

impl UOM {
    /// Does this unit carry money? `isCostBased()`.
    pub fn is_cost_based(&self) -> bool {
        matches!(self, Self::HoursDollars | Self::Dollars)
    }

    /// The codes of every cost-based unit. `getCostBasedUomCodes()`.
    pub fn cost_based_codes() -> Vec<&'static str> {
        Self::VALUES
            .iter()
            .filter(|uom| uom.is_cost_based())
            .map(|uom| uom.code())
            .collect()
    }

    /// `isHours()`.
    pub fn is_hours(&self) -> bool {
        matches!(self, Self::Hours)
    }

    /// `isDollars()`.
    pub fn is_dollars(&self) -> bool {
        matches!(self, Self::Dollars)
    }

    /// `isUnits()`.
    pub fn is_units(&self) -> bool {
        matches!(self, Self::Units)
    }

    /// `isHoursDollars()`.
    pub fn is_hours_dollars(&self) -> bool {
        matches!(self, Self::HoursDollars)
    }

    /// `isDays()`.
    pub fn is_days(&self) -> bool {
        matches!(self, Self::Days)
    }

    /// `isHoursRate()`.
    pub fn is_hours_rate(&self) -> bool {
        matches!(self, Self::HoursRate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in UOM::VALUES {
            assert_eq!(UOM::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn the_codes_do_not_follow_the_names() {
        // Days is Y and HoursDollars is C; H and D were already taken.
        assert_eq!(UOM::Days.code(), "Y");
        assert_eq!(UOM::HoursDollars.code(), "C");
    }

    #[test]
    fn cost_based_is_dollars_and_hours_dollars() {
        assert!(UOM::Dollars.is_cost_based());
        assert!(UOM::HoursDollars.is_cost_based());
        assert!(!UOM::Hours.is_cost_based());
        assert!(!UOM::Units.is_cost_based());
        assert!(!UOM::Days.is_cost_based());
        assert!(!UOM::HoursRate.is_cost_based());
    }

    #[test]
    fn cost_based_codes_are_listed_in_declaration_order() {
        assert_eq!(UOM::cost_based_codes(), vec!["D", "C"]);
    }

    #[test]
    fn each_predicate_matches_exactly_its_own_variant() {
        assert!(UOM::Hours.is_hours());
        assert!(UOM::Dollars.is_dollars());
        assert!(UOM::Units.is_units());
        assert!(UOM::HoursDollars.is_hours_dollars());
        assert!(UOM::Days.is_days());
        assert!(UOM::HoursRate.is_hours_rate());
        assert!(!UOM::Hours.is_hours_dollars());
    }
}
