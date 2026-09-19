//! Port of `com.unifocus.watson.common.labor.rules.algorithm.earningrate.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/earningrate/`.
//!
//! `EarningRateRuleConfig` is the family's root (`selectedEarnings`, a list of
//! earning-type ids); `EarningRateWithFactorRuleConfig` extends it with the
//! shared `rateFactor` parameter every rule but `EarningFixedRateRuleConfig`
//! and the two accrual configs carries.
//!
//! # `selectedEarnings` is read by exactly one rule
//!
//! Every concrete config declares, defaults and validates `selectedEarnings`
//! through this shared base, but only `EarningFactorRateRuleImpl` ever reads
//! it (as the earning-type allow-list its rate factor gates on — see
//! `earning_factor_rate.rs`). The other rules in the family carry the
//! parameter without consulting it, the same dead-configuration shape
//! `regularrate`'s `SELECTED_HOURS_DISTRIBUTION_TYPES` and
//! `doubletimerate`/`overtimerate`'s `EARNING_TYPES_PROP` findings already
//! made twice over in this crate (divergences 34, 58, 62). Reproduced as-is:
//! every config validates a non-empty `selectedEarnings`, whether or not its
//! rule reads the parameter.

use crate::common::json_ids::ids_for_key;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `EarningRateRuleConfig.SELECTED_EARNINGS`.
pub const SELECTED_EARNINGS: &str = "selectedEarnings";
/// `DefaultPropertyFactory.RATE_FACTOR`.
pub const RATE_FACTOR: &str = "rateFactor";
/// `DefaultPropertyFactory.USE_MIN_WAGE`.
pub const USE_MIN_WAGE: &str = "useMinWage";

/// The earning-type ids a rule's `selectedEarnings` parameter selects.
pub fn selected_earning_ids(params: &RuleParams) -> Vec<i32> {
    ids_for_key(SELECTED_EARNINGS, params)
}

/// `EarningRateRuleConfig.getDefaultValues()`.
pub fn earning_rate_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(SELECTED_EARNINGS, "[]");
    params
}

/// `EarningRateRuleConfig.validateProperties()`: at least one earning type
/// must be selected.
pub fn earning_rate_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if selected_earning_ids(params).is_empty() {
        results.push("Selected Earnings must have at least one earning configured.".to_string());
    }
    results
}

/// `EarningRateWithFactorRuleConfig.getDefaultValues()`.
pub fn earning_rate_with_factor_default_values() -> RuleParams {
    let mut params = earning_rate_default_values();
    params.set(RATE_FACTOR, "1.0");
    params
}

/// `EarningRateWithFactorRuleConfig.validateProperties()`: the base check
/// plus `rateFactor >= 0`.
pub fn earning_rate_with_factor_validate(params: &RuleParams) -> ValidationResults {
    let mut results = earning_rate_validate(params);
    if params.double_at(RATE_FACTOR) < 0.0 {
        results.push("Rate Factor must be greater than or equal to zero.".to_string());
    }
    results
}

/// `EarningFixedRateRuleConfig.RATE`.
pub const RATE: &str = "rate";

/// `EarningFixedRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarningFixedRateRuleConfig;

impl RuleConfig for EarningFixedRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FixedRateErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_default_values();
        params.set(RATE, "0.0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_validate(params)
    }
}

/// `EarningFactorRateRuleConfig.HOME_AS_BASE_PROP`.
pub const HOME_AS_BASE_PROP: &str = "homeAsBase";

/// `EarningFactorRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarningFactorRateRuleConfig;

impl RuleConfig for EarningFactorRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FactorErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_with_factor_default_values();
        params.set(HOME_AS_BASE_PROP, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_with_factor_validate(params)
    }
}

/// `HomeJobRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeJobRateRuleConfig;

impl RuleConfig for HomeJobRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeJobErr
    }

    fn default_values(&self) -> RuleParams {
        earning_rate_with_factor_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_with_factor_validate(params)
    }
}

/// `HomeDeptRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeDeptRateRuleConfig;

impl RuleConfig for HomeDeptRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeDeptErr
    }

    fn default_values(&self) -> RuleParams {
        earning_rate_with_factor_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_with_factor_validate(params)
    }
}

/// `ContractDailyRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContractDailyRateRuleConfig;

impl RuleConfig for ContractDailyRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ContractDailyErr
    }

    fn default_values(&self) -> RuleParams {
        earning_rate_with_factor_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_with_factor_validate(params)
    }
}

/// `EarningOverrideJobRateRuleConfig.OVERRIDE_RATE`.
pub const OVERRIDE_RATE: &str = "overrideRate";

/// `EarningOverrideJobRateRuleConfig`.
///
/// # Its validation does not match its own parent's
///
/// `EarningOverrideJobRateRuleConfig.validateProperties` builds a fresh
/// `ValidationResults` rather than calling `super.validateProperties` — so,
/// uniquely in this family, it does **not** require `selectedEarnings` to be
/// non-empty. And where `EarningRateWithFactorRuleConfig` requires
/// `rateFactor >= 0`, this config runs `validateDoubleGreaterThanZero`
/// instead (`AbstractRuleConfig`'s `SMALLEST_VALID_DOUBLE`, `0.01`) — a
/// stricter, `> 0` bound, reproduced here as `rateFactor < 0.01`. The same
/// stricter check also gates `overrideRate`, but only when `useMinWage` is
/// false.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarningOverrideJobRateRuleConfig;

impl RuleConfig for EarningOverrideJobRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::EarnOverrideJobErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_with_factor_default_values();
        params.set(USE_MIN_WAGE, "true");
        params.set(OVERRIDE_RATE, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.double_at(RATE_FACTOR) < 0.01 {
            results.push("Rate Factor must be greater than or equal to 0.01".to_string());
        }
        if !params.bool_at(USE_MIN_WAGE) && params.double_at(OVERRIDE_RATE) < 0.01 {
            results.push("Override Rate must be greater than or equal to 0.01".to_string());
        }
        results
    }
}

/// `FLSAEarningRateRuleConfig.APPLY_MIN_WAGE_PER_SHIFT`.
pub const APPLY_MIN_WAGE_PER_SHIFT: &str = "applyMinWagePerShift";
/// `FLSAEarningRateRuleConfig.USE_PAY_PERIOD`.
pub const USE_PAY_PERIOD: &str = "usePayPeriod";

/// `FLSAEarningRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FLSAEarningRateRuleConfig;

impl RuleConfig for FLSAEarningRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FlsaErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_with_factor_default_values();
        params.set(APPLY_MIN_WAGE_PER_SHIFT, "true");
        params.set(USE_PAY_PERIOD, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        earning_rate_with_factor_validate(params)
    }
}

/// `AvgDayXWeeksRateRuleConfig.LAST_X_WEEKS`.
pub const LAST_X_WEEKS: &str = "lastXWeeks";

/// `AvgDayXWeeksRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AvgDayXWeeksRateRuleConfig;

impl RuleConfig for AvgDayXWeeksRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::AvgXWeeksWorkedErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_with_factor_default_values();
        params.set(LAST_X_WEEKS, "12");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = earning_rate_with_factor_validate(params);
        if params.int_at(LAST_X_WEEKS) <= 0 {
            results.push("Last X Weeks must be greater than zero.".to_string());
        }
        results
    }
}

/// `PriorBalancesRateRuleConfig.HOURS_ACCRUAL`.
pub const PRIOR_BALANCES_HOURS_ACCRUAL: &str = "hoursAccrual";
/// `PriorBalancesRateRuleConfig.COSTS_ACCRUAL`.
pub const COSTS_ACCRUAL: &str = "costsAccrual";

/// `PriorBalancesRateRuleConfig`.
///
/// Extends the bare `EarningRateRuleConfig`, not the with-factor variant —
/// this rule has no `rateFactor` parameter at all. Its validation does not
/// call the parent's either (own fresh `ValidationResults`, no
/// `selectedEarnings` check), the same shape as
/// [`EarningOverrideJobRateRuleConfig`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PriorBalancesRateRuleConfig;

impl RuleConfig for PriorBalancesRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PriorBalancesErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_default_values();
        params.set(PRIOR_BALANCES_HOURS_ACCRUAL, "0");
        params.set(COSTS_ACCRUAL, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.int_at(PRIOR_BALANCES_HOURS_ACCRUAL) <= 0 {
            results.push("Hours Accrual must be selected.".to_string());
        }
        if params.int_at(COSTS_ACCRUAL) <= 0 {
            results.push("Costs Accrual must be selected.".to_string());
        }
        results
    }
}

/// `CalculatedAccrualRateRuleConfig.COST_ACCRUAL`.
pub const COST_ACCRUAL: &str = "costAccrual";
/// `CalculatedAccrualRateRuleConfig.HOURS_ACCRUAL`.
pub const CALC_ACCRUAL_HOURS_ACCRUAL: &str = "hoursAccrual";

/// `CalculatedAccrualRateRuleConfig`.
///
/// Also extends the bare `EarningRateRuleConfig` — no `rateFactor`. Unlike
/// [`PriorBalancesRateRuleConfig`], its validation *does* call the parent's
/// (`selectedEarnings` must be non-empty) before adding its own two
/// accrual-id checks.
#[derive(Debug, Clone, Copy, Default)]
pub struct CalculatedAccrualRateRuleConfig;

impl RuleConfig for CalculatedAccrualRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::CalcAccrualErr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = earning_rate_default_values();
        params.set(COST_ACCRUAL, "0");
        params.set(CALC_ACCRUAL_HOURS_ACCRUAL, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = earning_rate_validate(params);
        if params.int_at(COST_ACCRUAL) <= 0 {
            results.push("Cost Accrual must be selected.".to_string());
        }
        if params.int_at(CALC_ACCRUAL_HOURS_ACCRUAL) <= 0 {
            results.push("Hours Accrual must be selected.".to_string());
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earning_fixed_rate_defaults_to_zero() {
        let defaults = EarningFixedRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(RATE), 0.0);
        assert!(selected_earning_ids(&defaults).is_empty());
    }

    #[test]
    fn an_empty_selected_earnings_list_is_invalid() {
        let defaults = EarningFixedRateRuleConfig.default_values();
        assert_eq!(EarningFixedRateRuleConfig.validate(&defaults).len(), 1);
    }

    #[test]
    fn earning_factor_rate_defaults_to_not_using_the_home_job() {
        let defaults = EarningFactorRateRuleConfig.default_values();
        assert!(!defaults.bool_at(HOME_AS_BASE_PROP));
        assert_eq!(defaults.double_at(RATE_FACTOR), 1.0);
    }

    #[test]
    fn a_negative_rate_factor_is_invalid() {
        let mut params = HomeJobRateRuleConfig.default_values();
        params.set(SELECTED_EARNINGS, "[1]");
        params.set(RATE_FACTOR, "-1.0");
        assert_eq!(HomeJobRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn earning_override_job_rate_does_not_require_selected_earnings() {
        let defaults = EarningOverrideJobRateRuleConfig.default_values();
        // selectedEarnings is "[]" by default, but this config never checks it.
        assert!(
            EarningOverrideJobRateRuleConfig
                .validate(&defaults)
                .is_empty()
        );
    }

    #[test]
    fn earning_override_job_rate_requires_an_override_rate_when_not_using_min_wage() {
        let mut params = EarningOverrideJobRateRuleConfig.default_values();
        params.set(USE_MIN_WAGE, "false");
        assert_eq!(EarningOverrideJobRateRuleConfig.validate(&params).len(), 1);

        params.set(OVERRIDE_RATE, "5.0");
        assert!(
            EarningOverrideJobRateRuleConfig
                .validate(&params)
                .is_empty()
        );
    }

    #[test]
    fn avg_day_x_weeks_rejects_a_non_positive_week_count() {
        let mut params = AvgDayXWeeksRateRuleConfig.default_values();
        params.set(SELECTED_EARNINGS, "[1]");
        params.set(LAST_X_WEEKS, "0");
        assert_eq!(AvgDayXWeeksRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn prior_balances_requires_both_accruals_selected() {
        let defaults = PriorBalancesRateRuleConfig.default_values();
        assert_eq!(PriorBalancesRateRuleConfig.validate(&defaults).len(), 2);

        let mut params = defaults;
        params.set(PRIOR_BALANCES_HOURS_ACCRUAL, "11");
        params.set(COSTS_ACCRUAL, "22");
        assert!(PriorBalancesRateRuleConfig.validate(&params).is_empty());
    }

    #[test]
    fn calculated_accrual_requires_selected_earnings_and_both_accruals() {
        let defaults = CalculatedAccrualRateRuleConfig.default_values();
        assert_eq!(CalculatedAccrualRateRuleConfig.validate(&defaults).len(), 3);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(
            EarningFixedRateRuleConfig.rule_class(),
            RuleClass::FixedRateErr
        );
        assert_eq!(
            EarningFactorRateRuleConfig.rule_class(),
            RuleClass::FactorErr
        );
        assert_eq!(HomeJobRateRuleConfig.rule_class(), RuleClass::HomeJobErr);
        assert_eq!(HomeDeptRateRuleConfig.rule_class(), RuleClass::HomeDeptErr);
        assert_eq!(
            ContractDailyRateRuleConfig.rule_class(),
            RuleClass::ContractDailyErr
        );
        assert_eq!(
            EarningOverrideJobRateRuleConfig.rule_class(),
            RuleClass::EarnOverrideJobErr
        );
        assert_eq!(FLSAEarningRateRuleConfig.rule_class(), RuleClass::FlsaErr);
        assert_eq!(
            AvgDayXWeeksRateRuleConfig.rule_class(),
            RuleClass::AvgXWeeksWorkedErr
        );
        assert_eq!(
            PriorBalancesRateRuleConfig.rule_class(),
            RuleClass::PriorBalancesErr
        );
        assert_eq!(
            CalculatedAccrualRateRuleConfig.rule_class(),
            RuleClass::CalcAccrualErr
        );
    }
}
