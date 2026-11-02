//! `RuleType::ClosePayPeriod` — two rules that run when a pay period is closed.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/closepayperiod/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/closepayperiod/`,
//! runner `com/unifocus/rms/timekeeping/domain/paygroup/closing/ClosePayPeriodRuleRunner.java`.
//!
//! The only family whose rules are **reversible**: [`ClosePayPeriodRule`] has
//! `execute` (close) and `undo` (re-open), so closing a period twice, or
//! re-closing after a correction, undoes the earlier output before writing new.
//!
//! The runner lives in Java's `rms.timekeeping` package rather than `calcshift`,
//! and loads the time cards itself; here it is handed them already loaded.

pub mod config;
pub mod prepay_contract;
pub mod runner;

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{CurrentPropertyPort, EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use date_range_rs::DateRange;
use prepay_contract::PrepayContractCPRule;

/// `ClosePayPeriodRuleImpl`.
pub trait ClosePayPeriodRule {
    /// `execute(payPeriod, dataSet, ruleItem)`. `rule_set_name` is
    /// `ruleItem.getRuleSet().getName()`, for the note a rule writes.
    fn execute(
        &self,
        pay_period: &DateRange,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    );

    /// `undo(payPeriod, dataSet, ruleItem)`.
    fn undo(&self, pay_period: &DateRange, time_card: &mut dyn TimeCard, rule_item: &RuleItem);
}

/// `NoCPRuleImpl`: does nothing, either way.
pub struct NoCPRule;

impl ClosePayPeriodRule for NoCPRule {
    fn execute(&self, _p: &DateRange, _t: &mut dyn TimeCard, _r: &RuleItem, _n: &str) {}
    fn undo(&self, _p: &DateRange, _t: &mut dyn TimeCard, _r: &RuleItem) {}
}

/// Dispatch for the two close-pay-period rules. `RuleImplFactory`, narrowed to this family.
pub struct ClosePayPeriodRegistry<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    C: CurrentPropertyPort,
> {
    no_cp: NoCPRule,
    prepay_contract: PrepayContractCPRule<T, E, M, C>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, C: CurrentPropertyPort>
    ClosePayPeriodRegistry<T, E, M, C>
{
    /// Build both rules over the ports.
    pub fn new(earning_types: T, earnings: E, min_wage: M, current_property: C) -> Self {
        Self {
            no_cp: NoCPRule,
            prepay_contract: PrepayContractCPRule::new(
                earning_types,
                earnings,
                min_wage,
                current_property,
            ),
        }
    }
}

impl<T, E, M, C> RuleRegistry<dyn ClosePayPeriodRule> for ClosePayPeriodRegistry<T, E, M, C>
where
    T: EarningTypePort + 'static,
    E: EmployeeEarningPort + 'static,
    M: MinWagePort + 'static,
    C: CurrentPropertyPort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ClosePayPeriod
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ClosePayPeriodRule + 'static)> {
        match rule_class {
            RuleClass::NoCpr => Some(&self.no_cp),
            RuleClass::PrepayContractCpr => Some(&self.prepay_contract),
            _ => None,
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::common::enums::uom::UOM;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings, earning_type,
    };

    struct Property;
    impl CurrentPropertyPort for Property {
        fn pay_period(&self) -> DateRange {
            DateRange::new(
                joda_rs::LocalDate::of(2014, 3, 1),
                joda_rs::LocalDate::of(2014, 3, 31),
            )
        }
    }

    /// Both catalogue entries dispatch, none is missing.
    #[test]
    fn every_close_pay_period_rule_class_dispatches() {
        let registry = ClosePayPeriodRegistry::new(
            FixedEarningType(earning_type(UOM::Hours)),
            SavedEarnings::default(),
            FixedMinWage(0.0),
            Property,
        );
        assert_eq!(registry.dispatchable().len(), 2);
        assert!(registry.missing().is_empty());
    }
}
