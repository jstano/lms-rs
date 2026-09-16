//! Port of `com.unifocus.watson.common.enums.EmployeePayType`.
//!
//! How an employee is paid. The two derived predicates —
//! [`EmployeePayType::is_annual_rate_based`] and
//! [`EmployeePayType::is_hourly_rate_based`] — decide which rate field on
//! `EmployeeJobStatus` a rule should read, so they matter more than the
//! individual variant tests.

use crate::coded_enum;

coded_enum! {
    /// An employee's pay basis. `EmployeePayType`.
    EmployeePayType {
        Hourly => "H",
        Piece => "P",
        SalariedExempt => "S",
        SalariedNonExempt => "N",
        Contract => "C",
    }
}

impl EmployeePayType {
    /// `isHourly()`.
    pub fn is_hourly(&self) -> bool {
        matches!(self, Self::Hourly)
    }

    /// `isSalariedExempt()`.
    pub fn is_salaried_exempt(&self) -> bool {
        matches!(self, Self::SalariedExempt)
    }

    /// `isSalariedNonExempt()`.
    pub fn is_salaried_non_exempt(&self) -> bool {
        matches!(self, Self::SalariedNonExempt)
    }

    /// `isNotSalariedExempt()`.
    ///
    /// Java spells this out as its own method rather than negating at the call
    /// site; overtime rules read it directly.
    pub fn is_not_salaried_exempt(&self) -> bool {
        !self.is_salaried_exempt()
    }

    /// `isPieceRate()`.
    pub fn is_piece_rate(&self) -> bool {
        matches!(self, Self::Piece)
    }

    /// `isContract()`.
    pub fn is_contract(&self) -> bool {
        matches!(self, Self::Contract)
    }

    /// Is pay derived from an annual rate? `isAnnualRateBased()`.
    pub fn is_annual_rate_based(&self) -> bool {
        self.is_salaried_exempt() || self.is_salaried_non_exempt()
    }

    /// Is pay derived from an hourly rate? `isHourlyRateBased()`.
    ///
    /// Note contract employees count as hourly-rate based, and that piece-rate
    /// employees are in neither group.
    pub fn is_hourly_rate_based(&self) -> bool {
        self.is_hourly() || self.is_contract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in EmployeePayType::VALUES {
            assert_eq!(EmployeePayType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn annual_rate_covers_both_salaried_kinds() {
        assert!(EmployeePayType::SalariedExempt.is_annual_rate_based());
        assert!(EmployeePayType::SalariedNonExempt.is_annual_rate_based());
        assert!(!EmployeePayType::Hourly.is_annual_rate_based());
    }

    #[test]
    fn hourly_rate_covers_contract_too() {
        assert!(EmployeePayType::Hourly.is_hourly_rate_based());
        assert!(EmployeePayType::Contract.is_hourly_rate_based());
        assert!(!EmployeePayType::SalariedExempt.is_hourly_rate_based());
    }

    #[test]
    fn piece_rate_is_in_neither_rate_group() {
        assert!(!EmployeePayType::Piece.is_annual_rate_based());
        assert!(!EmployeePayType::Piece.is_hourly_rate_based());
    }

    #[test]
    fn not_salaried_exempt_includes_salaried_non_exempt() {
        assert!(EmployeePayType::SalariedNonExempt.is_not_salaried_exempt());
        assert!(!EmployeePayType::SalariedExempt.is_not_salaried_exempt());
    }
}
