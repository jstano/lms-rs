//! `RuleType::ReconcileEmployee` — two rules that repair a salaried employee's job
//! statuses during employee reconciliation.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/reconcileemployee/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/reconcileemployee/`.
//!
//! Unlike every rule family before it these **mutate the employee**: they rewrite
//! job statuses (salary distribution, annual rate, derived hourly rate), so the
//! trait takes `&mut Employee`. There is no time card and no earning. Persisting
//! the result is the reconcile process's business (`empmaint/reconcile`), which
//! is also what orders and drives the rules; nothing in `calcshift/rules` is a
//! reconcile runner, so there is none to port.
//!
//! `ManipulateAnnualRate` reads "now" (`LocalDate.now()`); the port takes `today`.

pub mod config;
pub mod manipulate_annual_rate;
pub mod missing_salary_dist;

use crate::entity::employee::Employee;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::params::RuleParams;
use crate::rules::ports::SalaryDistPort;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use joda_rs::LocalDate;
use manipulate_annual_rate::ManipulateAnnualRateRule;
use missing_salary_dist::MissingSalaryDistRule;

/// `ReconcileEmployeeRuleImpl`.
pub trait ReconcileEmployeeRule {
    /// `execute(employee, params)`; `today` stands in for `LocalDate.now()`.
    fn execute(&self, employee: &mut Employee, params: &RuleParams, today: LocalDate);
}

/// Dispatch for the two reconcile rules. `RuleImplFactory`, narrowed to this family.
pub struct ReconcileEmployeeRegistry<S: SalaryDistPort> {
    missing_salary_dist: MissingSalaryDistRule<S>,
    manipulate_annual_rate: ManipulateAnnualRateRule,
}

impl<S: SalaryDistPort> ReconcileEmployeeRegistry<S> {
    /// Build both rules over the salary-distribution lookup.
    pub fn new(salary_dists: S) -> Self {
        Self {
            missing_salary_dist: MissingSalaryDistRule::new(salary_dists),
            manipulate_annual_rate: ManipulateAnnualRateRule,
        }
    }
}

impl<S: SalaryDistPort + 'static> RuleRegistry<dyn ReconcileEmployeeRule>
    for ReconcileEmployeeRegistry<S>
{
    fn rule_type(&self) -> RuleType {
        RuleType::ReconcileEmployee
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ReconcileEmployeeRule + 'static)> {
        match rule_class {
            RuleClass::MissSalDistRer => Some(&self.missing_salary_dist),
            RuleClass::AnnualRateManipulationRer => Some(&self.manipulate_annual_rate),
            _ => None,
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::entity::salary_dist::SalaryDist;

    struct NoDists;
    impl SalaryDistPort for NoDists {
        fn find_by_id(&self, _id: i32) -> Option<SalaryDist> {
            None
        }
    }

    /// Both catalogue entries dispatch, none is missing.
    #[test]
    fn every_reconcile_employee_rule_class_dispatches() {
        let registry = ReconcileEmployeeRegistry::new(NoDists);
        assert_eq!(registry.dispatchable().len(), 2);
        assert!(registry.missing().is_empty());
    }
}
