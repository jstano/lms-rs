//! Port of `com.unifocus.watson.common.labor.rules.algorithm.closepayperiod.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/closepayperiod/`.

use crate::rules::algorithm::dailyearning::config::EARNING;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `NoCPRuleConfig`: no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCPRuleConfig;

impl RuleConfig for NoCPRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoCpr
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// `PrepayContractCPRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrepayContractCPRuleConfig;

impl RuleConfig for PrepayContractCPRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PrepayContractCpr
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let mut r = ValidationResults::new();
        if p.int_at(EARNING) <= 0 {
            r.push("Prepay Contract Hours: Earning must be selected.".to_string());
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn the_prepay_earning_must_be_selected() {
        assert_eq!(
            PrepayContractCPRuleConfig
                .validate(&RuleParams::new())
                .len(),
            1
        );
        assert!(
            PrepayContractCPRuleConfig
                .validate(&rule_params! { EARNING => "4" })
                .is_empty()
        );
        assert!(NoCPRuleConfig.default_values().is_empty());
    }
}
