//! Port of `com.unifocus.watson.common.labor.rules.algorithm.reconcileemployee.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/reconcileemployee/`.
//! Neither extends `PriorityRuleConfig`; each carries a fixed, non-editable
//! `priority` parameter (`1` for `MissingSalaryDist`, `2` for `ManipulateAnnualRate`)
//! that the employee-reconcile process reads to order them — the fill-in runs first.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{PRIORITY, RuleConfig, ValidationResults};

pub const SALARY_DIST_ID: &str = "salaryDistID";
pub const SECONDARY_SALARY_DIST_ID: &str = "secondarySalaryDistID";

/// `ManipulateAnnualRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ManipulateAnnualRateRuleConfig;

impl RuleConfig for ManipulateAnnualRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::AnnualRateManipulationRer
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(PRIORITY, "2");
        p
    }
}

/// `MissingSalaryDistRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MissingSalaryDistRuleConfig;

impl RuleConfig for MissingSalaryDistRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MissSalDistRer
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(SALARY_DIST_ID, "0");
        p.set(SECONDARY_SALARY_DIST_ID, "0");
        p.set(PRIORITY, "1");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let name = "Fill Missing Salary Distribution";
        let mut r = ValidationResults::new();
        if p.int_at(SALARY_DIST_ID) <= 0 {
            r.push(format!("{name}: Salary Distribution must be selected."));
        }
        if p.int_at(SECONDARY_SALARY_DIST_ID) <= 0 {
            r.push(format!(
                "{name}: Secondary Salary Distribution must be selected."
            ));
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn defaults_carry_the_fixed_priorities() {
        assert_eq!(
            ManipulateAnnualRateRuleConfig
                .default_values()
                .get(PRIORITY),
            Some("2")
        );
        assert_eq!(
            MissingSalaryDistRuleConfig.default_values().get(PRIORITY),
            Some("1")
        );
    }

    #[test]
    fn both_salary_distributions_must_be_selected() {
        assert_eq!(
            MissingSalaryDistRuleConfig
                .validate(&RuleParams::new())
                .len(),
            2
        );
        let ok = rule_params! { SALARY_DIST_ID => "1", SECONDARY_SALARY_DIST_ID => "2" };
        assert!(MissingSalaryDistRuleConfig.validate(&ok).is_empty());
        assert!(
            ManipulateAnnualRateRuleConfig
                .validate(&RuleParams::new())
                .is_empty()
        );
    }
}
