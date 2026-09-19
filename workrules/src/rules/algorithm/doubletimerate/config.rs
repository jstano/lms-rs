//! Port of `com.unifocus.watson.common.labor.rules.algorithm.doubletimerate.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/doubletimerate/`.
//!
//! `DoubleTimeRateRuleConfig` extends `SingleDistributionTypeRuleConfig` in
//! the Java hierarchy, but — like `regularrate`'s `SELECTED_HOURS_DISTRIBUTION_TYPES`
//! finding — nothing in this family's `execute` reads that parameter, so it
//! is left out here the same way divergence 34 already left it out of
//! `hoursdistribution`.
//!
//! # `CommissionBasedDTRateRuleConfig.EARNING_TYPES_PROP` is declared but never read
//!
//! The config declares, defaults and validates `earningTypes` (the earning
//! types that count toward the commission total) as its own parameter
//! distinct from the inherited `premiumTypes` (which earning types the rate
//! applies to). But `CommissionBasedDTRateRuleImpl` computes both its
//! shift-path `earningTypeIds` *and* its earning-path gate from
//! `ruleConfig.getEarningTypeIdsList(params)` — the inherited method, which
//! only ever reads `PREMIUM_TYPES`. `EARNING_TYPES_PROP` is never passed to
//! anything that reads it. Reproduced faithfully: [`CommissionBasedDTRateRuleConfig`]
//! carries the parameter for `default_values`/`validate` parity, but the
//! algorithm (in `commission_based_dt_rate.rs`) sources both earning-type
//! lists from `PREMIUM_TYPES`, exactly as the Java does.

use crate::common::json_ids::ids_for_key;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `DoubleTimeRateRuleConfig.DOUBLETIME_FACTOR_PROP`.
pub const DOUBLETIME_FACTOR_PROP: &str = "doubleTimeFactor";
/// `DoubleTimeRateRuleConfig.PREMIUM_TYPES`.
pub const PREMIUM_TYPES: &str = "premiumTypes";

/// The earning-type ids a rule's `premiumTypes` parameter selects.
/// `DoubleTimeRateRuleConfig.getEarningTypeIdsList(Map)`.
pub fn earning_type_ids(params: &RuleParams) -> Vec<i32> {
    ids_for_key(PREMIUM_TYPES, params)
}

/// The one parameter every plain `DoubleTimeRateRuleConfig` subclass starts
/// from.
pub fn double_time_rate_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(DOUBLETIME_FACTOR_PROP, "1.0");
    params.set(PREMIUM_TYPES, "[]");
    params
}

/// The shared validation every concrete config runs: `doubleTimeFactor` must
/// not be negative.
pub fn double_time_rate_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if params.double_at(DOUBLETIME_FACTOR_PROP) < 0.0 {
        results.push("Double Time Factor must be greater than or equal to zero.".to_string());
    }
    results
}

/// `JobDTRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct JobDTRateRuleConfig;

impl RuleConfig for JobDTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::JobDrr
    }

    fn default_values(&self) -> RuleParams {
        double_time_rate_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        double_time_rate_validate(params)
    }
}

/// `HomeJobDTRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeJobDTRateRuleConfig;

impl RuleConfig for HomeJobDTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeJobDrr
    }

    fn default_values(&self) -> RuleParams {
        double_time_rate_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        double_time_rate_validate(params)
    }
}

/// `HomeDeptDTRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeDeptDTRateRuleConfig;

impl RuleConfig for HomeDeptDTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeDeptDrr
    }

    fn default_values(&self) -> RuleParams {
        double_time_rate_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        double_time_rate_validate(params)
    }
}

/// `FLSADTRateRuleConfig.APPLY_MIN_WAGE_PER_SHIFT`.
pub const APPLY_MIN_WAGE_PER_SHIFT: &str = "applyMinWagePerShift";

/// `FLSADTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FLSADTRateRuleConfig;

impl RuleConfig for FLSADTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FlsaDrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = double_time_rate_default_values();
        params.set(APPLY_MIN_WAGE_PER_SHIFT, "true");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        double_time_rate_validate(params)
    }
}

/// `CommissionBasedDTRateRuleConfig`'s own parameter keys.
pub const EARNING_TYPES_PROP: &str = "earningTypes";
pub const MIN_COMM_RATE: &str = "minCommRate";

/// `CommissionBasedDTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommissionBasedDTRateRuleConfig;

impl RuleConfig for CommissionBasedDTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ComBasedDrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = double_time_rate_default_values();
        params.set(EARNING_TYPES_PROP, "[]");
        params.set(MIN_COMM_RATE, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = double_time_rate_validate(params);
        if params.double_at(MIN_COMM_RATE) < 0.0 {
            results
                .push("Minimum Commission Rate must be greater than or equal to zero.".to_string());
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_dt_rate_defaults_to_a_factor_of_one() {
        let defaults = JobDTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(DOUBLETIME_FACTOR_PROP), 1.0);
        assert!(earning_type_ids(&defaults).is_empty());
    }

    #[test]
    fn a_negative_factor_is_invalid() {
        let mut params = JobDTRateRuleConfig.default_values();
        params.set(DOUBLETIME_FACTOR_PROP, "-1.0");
        assert_eq!(JobDTRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn flsa_dt_rate_defaults_to_applying_minimum_wage_per_shift() {
        let defaults = FLSADTRateRuleConfig.default_values();
        assert!(defaults.bool_at(APPLY_MIN_WAGE_PER_SHIFT));
    }

    #[test]
    fn commission_based_defaults_to_zero_minimum_commission() {
        let defaults = CommissionBasedDTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(MIN_COMM_RATE), 0.0);
    }

    #[test]
    fn commission_based_rejects_a_negative_minimum_commission() {
        let mut params = CommissionBasedDTRateRuleConfig.default_values();
        params.set(MIN_COMM_RATE, "-5.0");
        assert_eq!(CommissionBasedDTRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(JobDTRateRuleConfig.rule_class(), RuleClass::JobDrr);
        assert_eq!(HomeJobDTRateRuleConfig.rule_class(), RuleClass::HomeJobDrr);
        assert_eq!(
            HomeDeptDTRateRuleConfig.rule_class(),
            RuleClass::HomeDeptDrr
        );
        assert_eq!(FLSADTRateRuleConfig.rule_class(), RuleClass::FlsaDrr);
        assert_eq!(
            CommissionBasedDTRateRuleConfig.rule_class(),
            RuleClass::ComBasedDrr
        );
    }
}
