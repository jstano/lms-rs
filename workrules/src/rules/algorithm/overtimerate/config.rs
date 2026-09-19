//! Port of `com.unifocus.watson.common.labor.rules.algorithm.overtimerate.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/overtimerate/`.
//!
//! # `overtimeFactor` has no shared default
//!
//! Unlike `doubletimerate`, `OvertimeRateRuleConfig.getDefaultValues()` sets
//! only `premiumTypes` — the factor is left for each subclass to default on
//! its own. Every concrete config here defaults it to `0.5` except
//! `WeightedOTRateRuleConfig`, which defaults to `1.5` (and, see below, skips
//! the shared validation too). [`overtime_rate_default_values`] therefore
//! takes no factor argument; callers set `OVERTIME_FACTOR_PROP` themselves.
//!
//! # `WeightedOTRateRuleConfig` does not call the shared validation
//!
//! Every other config's `validateProperties` calls (directly or via a
//! subclass override that still calls `super`) the base's "factor >= 0"
//! check. `WeightedOTRateRuleConfig.validateProperties` builds a fresh
//! `ValidationResults` and checks "factor >= 1.0" instead — reproduced as
//! [`weighted_ot_rate_validate`], which does not call [`overtime_rate_validate`]
//! at all.

use crate::common::json_ids::ids_for_key;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `OvertimeRateRuleConfig.OVERTIME_FACTOR_PROP`.
pub const OVERTIME_FACTOR_PROP: &str = "overtimeFactor";
/// `OvertimeRateRuleConfig.PREMIUM_TYPES`.
pub const PREMIUM_TYPES: &str = "premiumTypes";

/// The earning-type ids a rule's `premiumTypes` parameter selects.
/// `OvertimeRateRuleConfig.getPremiumEarningTypeIdsList(Map)`.
pub fn earning_type_ids(params: &RuleParams) -> Vec<i32> {
    ids_for_key(PREMIUM_TYPES, params)
}

/// `OvertimeRateRuleConfig.getDefaultValues()` — `premiumTypes` only; the
/// factor is not defaulted here (see the module doc).
pub fn overtime_rate_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(PREMIUM_TYPES, "[]");
    params
}

/// The shared validation most concrete configs run: `overtimeFactor` must not
/// be negative. `OvertimeRateRuleConfig.validateProperties()`.
pub fn overtime_rate_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if params.double_at(OVERTIME_FACTOR_PROP) < 0.0 {
        results.push("Overtime Factor must be greater than or equal to zero.".to_string());
    }
    results
}

/// `JobOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct JobOTRateRuleConfig;

impl RuleConfig for JobOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::JobOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        overtime_rate_validate(params)
    }
}

/// `HomeJobOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeJobOTRateRuleConfig;

impl RuleConfig for HomeJobOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeJobOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        overtime_rate_validate(params)
    }
}

/// `HomeDeptOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeDeptOTRateRuleConfig;

impl RuleConfig for HomeDeptOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeDeptOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        overtime_rate_validate(params)
    }
}

/// `FLSAOTRateRuleConfig`'s own parameter keys.
pub const APPLY_MIN_WAGE_PER_SHIFT: &str = "applyMinWagePerShift";
pub const ADJUST_RATE_FOR_SHORTFALL: &str = "adjustRateForShortfall";
pub const USE_PAY_PERIOD: &str = "usePayPeriod";

/// `FLSAOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FLSAOTRateRuleConfig;

impl RuleConfig for FLSAOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FlsaOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params.set(APPLY_MIN_WAGE_PER_SHIFT, "true");
        params.set(ADJUST_RATE_FOR_SHORTFALL, "false");
        params.set(USE_PAY_PERIOD, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        overtime_rate_validate(params)
    }
}

/// `FLSAWeightedOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FLSAWeightedOTRateRuleConfig;

impl RuleConfig for FLSAWeightedOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FlsaWeightedOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        overtime_rate_validate(params)
    }
}

/// `GuaranteedWageOTRateRuleConfig.WEEKLY_HOURS`.
pub const WEEKLY_HOURS: &str = "weeklyHours";

/// `GuaranteedWageOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GuaranteedWageOTRateRuleConfig;

impl RuleConfig for GuaranteedWageOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::GuaranteedWageOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(WEEKLY_HOURS, "40.0");
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = overtime_rate_validate(params);
        if params.double_at(WEEKLY_HOURS) <= 0.0 {
            results.push("Weekly Hours must be greater than zero.".to_string());
        }
        results
    }
}

/// `WeightedOTRateRuleConfig.MIN_WAGE_BEFORE_CALC`.
pub const MIN_WAGE_BEFORE_CALC: &str = "minWageBeforeCalc";

/// `WeightedOTRateRuleConfig`. Defaults its factor to `1.5`, not `0.5` — this
/// is the only rule in the family priced as a full rate rather than a
/// half-time premium.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeightedOTRateRuleConfig;

impl RuleConfig for WeightedOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WeightedOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "1.5");
        params.set(MIN_WAGE_BEFORE_CALC, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        weighted_ot_rate_validate(params)
    }
}

/// `WeightedOTRateRuleConfig.validateProperties()` — see the module doc: this
/// does not call [`overtime_rate_validate`], and the threshold is `1.0`, not
/// `0.0`.
pub fn weighted_ot_rate_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if params.double_at(OVERTIME_FACTOR_PROP) < 1.0 {
        results.push("Overtime Factor must be greater than or equal to one.".to_string());
    }
    results
}

/// `CommissionBasedOTRateRuleConfig`'s own parameter keys.
pub const EARNING_TYPES_PROP: &str = "earningTypes";
pub const MIN_COMM_RATE: &str = "minCommRate";

/// `CommissionBasedOTRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommissionBasedOTRateRuleConfig;

impl RuleConfig for CommissionBasedOTRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ComBasedOrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = overtime_rate_default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        params.set(EARNING_TYPES_PROP, "[]");
        params.set(MIN_COMM_RATE, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = overtime_rate_validate(params);
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
    fn job_ot_rate_defaults_to_a_half_factor() {
        let defaults = JobOTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(OVERTIME_FACTOR_PROP), 0.5);
        assert!(earning_type_ids(&defaults).is_empty());
    }

    #[test]
    fn a_negative_factor_is_invalid() {
        let mut params = JobOTRateRuleConfig.default_values();
        params.set(OVERTIME_FACTOR_PROP, "-1.0");
        assert_eq!(JobOTRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn flsa_ot_rate_defaults_to_applying_minimum_wage_per_shift_only() {
        let defaults = FLSAOTRateRuleConfig.default_values();
        assert!(defaults.bool_at(APPLY_MIN_WAGE_PER_SHIFT));
        assert!(!defaults.bool_at(ADJUST_RATE_FOR_SHORTFALL));
        assert!(!defaults.bool_at(USE_PAY_PERIOD));
    }

    #[test]
    fn guaranteed_wage_defaults_to_forty_weekly_hours() {
        let defaults = GuaranteedWageOTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(WEEKLY_HOURS), 40.0);
    }

    #[test]
    fn guaranteed_wage_rejects_zero_weekly_hours() {
        let mut params = GuaranteedWageOTRateRuleConfig.default_values();
        params.set(WEEKLY_HOURS, "0");
        assert_eq!(GuaranteedWageOTRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn weighted_defaults_to_a_factor_of_one_point_five() {
        let defaults = WeightedOTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(OVERTIME_FACTOR_PROP), 1.5);
    }

    #[test]
    fn weighted_rejects_a_factor_below_one_even_though_it_would_pass_the_shared_check() {
        let mut params = WeightedOTRateRuleConfig.default_values();
        params.set(OVERTIME_FACTOR_PROP, "0.5");
        assert_eq!(WeightedOTRateRuleConfig.validate(&params).len(), 1);
        // The shared check alone would have accepted 0.5 (it only rejects < 0).
        assert!(overtime_rate_validate(&params).is_empty());
    }

    #[test]
    fn commission_based_defaults_to_zero_minimum_commission() {
        let defaults = CommissionBasedOTRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(MIN_COMM_RATE), 0.0);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(JobOTRateRuleConfig.rule_class(), RuleClass::JobOrr);
        assert_eq!(HomeJobOTRateRuleConfig.rule_class(), RuleClass::HomeJobOrr);
        assert_eq!(
            HomeDeptOTRateRuleConfig.rule_class(),
            RuleClass::HomeDeptOrr
        );
        assert_eq!(FLSAOTRateRuleConfig.rule_class(), RuleClass::FlsaOrr);
        assert_eq!(
            FLSAWeightedOTRateRuleConfig.rule_class(),
            RuleClass::FlsaWeightedOrr
        );
        assert_eq!(
            GuaranteedWageOTRateRuleConfig.rule_class(),
            RuleClass::GuaranteedWageOrr
        );
        assert_eq!(
            WeightedOTRateRuleConfig.rule_class(),
            RuleClass::WeightedOrr
        );
        assert_eq!(
            CommissionBasedOTRateRuleConfig.rule_class(),
            RuleClass::ComBasedOrr
        );
    }
}
