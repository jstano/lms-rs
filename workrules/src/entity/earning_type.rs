//! Port of `com.unifocus.watson.server.hibernate.entity.EarningType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EarningType.java`.
//!
//! What an earning is: regular pay, overtime, a premium, an accrual, a memo.
//! The Java entity carries 22 boolean flags; rules read four of them directly
//! and reach the rest through `EarningTypeDAO` queries, so only those four come
//! across for now.

use crate::common::enums::earn_type::EarnType;
use crate::common::enums::uom::UOM;

/// A kind of earning. `EarningType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarningType {
    id: i32,
    property_id: i32,
    name: String,
    earn_type: EarnType,
    uom: UOM,
    pay_at_least_min_wage: bool,
    include_in_benefit_calcs: bool,
    include_in_salary_distribution: bool,
    include_in_tips: bool,
}

impl EarningType {
    /// Build an earning type with its flags cleared.
    pub fn new(
        id: i32,
        property_id: i32,
        name: impl Into<String>,
        earn_type: EarnType,
        uom: UOM,
    ) -> Self {
        Self {
            id,
            property_id,
            name: name.into(),
            earn_type,
            uom,
            pay_at_least_min_wage: false,
            include_in_benefit_calcs: false,
            include_in_salary_distribution: false,
            include_in_tips: false,
        }
    }

    /// Set the four flags rules read directly.
    #[must_use]
    pub fn with_flags(
        mut self,
        pay_at_least_min_wage: bool,
        include_in_benefit_calcs: bool,
        include_in_salary_distribution: bool,
        include_in_tips: bool,
    ) -> Self {
        self.pay_at_least_min_wage = pay_at_least_min_wage;
        self.include_in_benefit_calcs = include_in_benefit_calcs;
        self.include_in_salary_distribution = include_in_salary_distribution;
        self.include_in_tips = include_in_tips;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getProperty().getID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getName()`. Earning-type lists are sorted by this, case-insensitively.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getType()`.
    pub fn earn_type(&self) -> EarnType {
        self.earn_type
    }

    /// What this earning is measured in. `getUom()`.
    pub fn uom(&self) -> UOM {
        self.uom
    }

    /// `isPayAtLeastMinWage()` — the most-read flag, 16 call sites.
    pub fn pay_at_least_min_wage(&self) -> bool {
        self.pay_at_least_min_wage
    }

    /// `isIncludeInBenefitCalcs()`.
    pub fn include_in_benefit_calcs(&self) -> bool {
        self.include_in_benefit_calcs
    }

    /// `isIncludeInSalaryDistribution()`.
    pub fn include_in_salary_distribution(&self) -> bool {
        self.include_in_salary_distribution
    }

    /// `isIncludeInTips()`.
    pub fn include_in_tips(&self) -> bool {
        self.include_in_tips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earning_type() -> EarningType {
        EarningType::new(5, 11, "Regular", EarnType::Regular, UOM::Hours)
    }

    #[test]
    fn an_earning_type_carries_its_kind_and_unit() {
        let e = earning_type();
        assert_eq!(e.id(), 5);
        assert_eq!(e.property_id(), 11);
        assert_eq!(e.name(), "Regular");
        assert_eq!(e.earn_type(), EarnType::Regular);
        assert_eq!(e.uom(), UOM::Hours);
    }

    #[test]
    fn the_flags_start_cleared() {
        let e = earning_type();
        assert!(!e.pay_at_least_min_wage());
        assert!(!e.include_in_benefit_calcs());
        assert!(!e.include_in_salary_distribution());
        assert!(!e.include_in_tips());
    }

    #[test]
    fn the_flags_can_be_set() {
        let e = earning_type().with_flags(true, true, false, false);
        assert!(e.pay_at_least_min_wage());
        assert!(e.include_in_benefit_calcs());
        assert!(!e.include_in_salary_distribution());
    }

    #[test]
    fn a_dollars_earning_type_is_cost_based() {
        let e = EarningType::new(6, 11, "Bonus", EarnType::Earning, UOM::Dollars);
        assert!(e.uom().is_cost_based());
    }
}
