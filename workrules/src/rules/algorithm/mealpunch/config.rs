//! Port of `com.unifocus.watson.common.labor.rules.algorithm.mealpunch.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/mealpunch/`.
//!
//! `NoMealPunchRuleConfig` in Java reports its rule class as `NO_IPVR` — a
//! copy-paste from the in-punch-validation config; the catalogue's `NO_MPR`
//! entry points at it by name. Here it reports `NoMpr` (divergence 127).

use crate::common::enums::meal_punch_permission::MealPunchPermission;
use crate::rules::algorithm::dailyearning::config::EARNING;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

pub const DOLLARS: &str = "dollars";
pub const SCHEDULED_ONLY: &str = "scheduledOnly";
pub const GRACE_PRE_SCHED: &str = "gracePreSchedule";
pub const GRACE_POST_SCHED: &str = "gracePostSchedule";
pub const PUNCH_PERMISSION: &str = "punchPermission";

/// `NoMealPunchRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMealPunchRuleConfig;

impl RuleConfig for NoMealPunchRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoMpr
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// `FixedMealEarningRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedMealEarningRuleConfig;

impl RuleConfig for FixedMealEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FixedEarningMpr
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(DOLLARS, "5.00");
        p.set(SCHEDULED_ONLY, "true");
        p.set(GRACE_PRE_SCHED, "30");
        p.set(GRACE_POST_SCHED, "30");
        p.set(PUNCH_PERMISSION, MealPunchPermission::Disallow.code());
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let name = "Fixed Meal Earning";
        let mut r = ValidationResults::new();
        if p.double_at(DOLLARS) == 0.0 {
            r.push(format!("{name}: Dollars must not be 0."));
        }
        if p.int_at(EARNING) <= 0 {
            r.push(format!("{name}: Earning must be selected."));
        }
        for (key, label) in [
            (GRACE_PRE_SCHED, "Pre-Schedule Grace Period"),
            (GRACE_POST_SCHED, "Post-Schedule Grace Period"),
        ] {
            if p.int_at(key) < 0 {
                r.push(format!(
                    "{name}: {label} must be greater than or equal to 0"
                ));
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    // ---- java_parity_tests: FixedMealEarningRuleConfigTest.groovy ----

    #[test]
    fn defaults_contain_appropriate_values() {
        let d = FixedMealEarningRuleConfig.default_values();
        assert_eq!(d.get("dollars"), Some("5.00"));
        assert_eq!(d.get("earning"), Some("0"));
        assert_eq!(d.get("scheduledOnly"), Some("true"));
        assert_eq!(d.get("gracePreSchedule"), Some("30"));
        assert_eq!(d.get("gracePostSchedule"), Some("30"));
        assert_eq!(
            d.get("punchPermission"),
            Some(MealPunchPermission::Disallow.code())
        );
    }

    #[test]
    fn validation_is_correct() {
        let props = |dollars: &str, earning: &str, pre: &str, post: &str| {
            FixedMealEarningRuleConfig.validate(&rule_params! {
                DOLLARS => dollars, EARNING => earning, GRACE_PRE_SCHED => pre, GRACE_POST_SCHED => post
            })
        };
        // (has errors, result)
        assert!(
            props("5.00", "1", "30", "30").is_empty(),
            "dollars greater than zero"
        );
        assert!(
            props("-5.00", "1", "30", "30").is_empty(),
            "dollars less than zero"
        );
        assert!(
            !props("0", "1", "0", "0").is_empty(),
            "dollars equal to zero"
        );
        assert!(
            !props("1", "0", "0", "0").is_empty(),
            "earning not selected"
        );
        assert!(
            !props("1", "1", "-1", "0").is_empty(),
            "grace pre sched less than 0"
        );
        assert!(
            !props("1", "1", "0", "-1").is_empty(),
            "grace post sched less than 0"
        );
    }

    #[test]
    fn the_no_meal_punch_config_has_no_parameters() {
        assert!(NoMealPunchRuleConfig.default_values().is_empty());
        assert_eq!(NoMealPunchRuleConfig.rule_class(), RuleClass::NoMpr);
    }
}
