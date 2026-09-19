//! Port of `com.unifocus.watson.common.labor.rules.algorithm.regularrate.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/regularrate/`.
//!
//! # `SELECTED_HOURS_DISTRIBUTION_TYPES` is declared but never read
//!
//! `RegularRateRuleConfig` — the base every config here extends except
//! `HomeJobRegRateRuleConfig` and the `ShiftCategory*` pair — declares one
//! parameter, `selectedHoursDistributionTypes`, and requires it non-empty. No
//! concrete rule's `execute` reads it: grep across every `*RuleImpl.java` in
//! the family finds no reference. It comes across anyway, in
//! [`regular_rate_default_values`]/[`regular_rate_validate`], because
//! `RuleConfig`'s contract is "what `fixMap`/`validateProperties` would do",
//! not "what `execute` happens to use" — the same reasoning `hoursdistribution`
//! applied to its own unread parameters.
//!
//! # Two configs quietly opt out of the shared base
//!
//! `HomeJobRegRateRuleConfig` extends `AbstractRuleConfig` directly, not
//! `RegularRateRuleConfig` — no parameters, no constraints, despite sitting
//! next to seven siblings that all take `SELECTED_HOURS_DISTRIBUTION_TYPES`.
//!
//! `ShiftCategoryRegRateRuleBaseConfig` extends `RegularRateRuleConfig` in the
//! Java class hierarchy but **overrides `getDefaultValues`/`getProperties`
//! without calling `super`** — so despite the `extends`, neither
//! `ShiftCategoryRegRateRuleConfig` nor `ShiftCategoryMinWageRegRateRuleConfig`
//! actually carries `selectedHoursDistributionTypes`. Reading the class
//! hierarchy would have gotten this wrong; reading the method bodies did not.

use crate::common::json_ids::ids_for_key;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `RegularRateRuleConfig.SELECTED_HOURS_DISTRIBUTION_TYPES`.
pub const SELECTED_HOURS_DISTRIBUTION_TYPES: &str = "selectedHoursDistributionTypes";

/// The one parameter every plain `RegularRateRuleConfig` subclass starts from.
/// `RegularRateRuleConfig.getDefaultValues()`.
pub fn regular_rate_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(SELECTED_HOURS_DISTRIBUTION_TYPES, "[]");
    params
}

/// `RegularRateRuleConfig.validateProperties()` — at least one hours
/// distribution type must be selected.
pub fn regular_rate_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if ids_for_key(SELECTED_HOURS_DISTRIBUTION_TYPES, params).is_empty() {
        results.push(
            "Selected Hours Distribution Types must have at least one hours distribution type configured."
                .to_string(),
        );
    }
    results
}

/// `JobRegRateRuleConfig` — no parameters of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct JobRegRateRuleConfig;

impl RuleConfig for JobRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::JobRrr
    }

    fn default_values(&self) -> RuleParams {
        regular_rate_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        regular_rate_validate(params)
    }
}

/// `HomeJobRegRateRuleConfig` — extends `AbstractRuleConfig` directly, not
/// `RegularRateRuleConfig`: no parameters, no constraints at all.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeJobRegRateRuleConfig;

impl RuleConfig for HomeJobRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeJobRrr
    }

    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// `FactorJobRegRateRuleConfig.WAGE_FACTOR_PROP`.
pub const WAGE_FACTOR_PROP: &str = "wageFactor";

/// `FactorJobRegRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FactorJobRegRateRuleConfig;

impl RuleConfig for FactorJobRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::FactorJobRrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = regular_rate_default_values();
        params.set(WAGE_FACTOR_PROP, "1.0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = regular_rate_validate(params);
        if params.double_at(WAGE_FACTOR_PROP) < 0.0 {
            results.push("Wage Factor must be greater than or equal to zero.".to_string());
        }
        results
    }
}

/// `HomeDeptRegRateRuleConfig.PAY_GREATER_RATE`.
pub const PAY_GREATER_RATE: &str = "payGreaterRate";

/// `HomeDeptRegRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeDeptRegRateRuleConfig;

impl RuleConfig for HomeDeptRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HomeDeptRrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = regular_rate_default_values();
        params.set(PAY_GREATER_RATE, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        regular_rate_validate(params)
    }
}

/// `ShiftCategoryRegRateRuleBaseConfig.SHIFT_CATEGORIES` — shared by both
/// shift-category configs, neither of which carries the plain
/// `RegularRateRuleConfig` parameters (see the module doc).
pub const SHIFT_CATEGORIES: &str = "shiftCategories";

/// `ShiftCategoryRegRateRuleBaseConfig.getDefaultValues()`.
pub fn shift_category_base_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(SHIFT_CATEGORIES, "[]");
    params
}

/// `ShiftCategoryRegRateRuleBaseConfig.validateProperties()`.
pub fn shift_category_base_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if ids_for_key(SHIFT_CATEGORIES, params).is_empty() {
        results
            .push("Shift Categories must have at least one shift category selected.".to_string());
    }
    results
}

/// `ShiftCategoryMinWageRegRateRuleConfig` — the base's parameters, unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftCategoryMinWageRegRateRuleConfig;

impl RuleConfig for ShiftCategoryMinWageRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ScmwRrr
    }

    fn default_values(&self) -> RuleParams {
        shift_category_base_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        shift_category_base_validate(params)
    }
}

/// `ShiftCategoryRegRateRuleConfig`'s own parameter keys.
pub const ENFORCE_MINIMUM_WAGE_PROP: &str = "enforceMinimumWage";
pub const JOB_RATE_ADDITION_PROP: &str = "jobRateAddition";
pub const JOB_RATE_MULTIPLIER_PROP: &str = "jobRateMultiplier";
pub const USE_FIXED_RATE_PROP: &str = "useFixedRate";
pub const ONLY_ADJUST_SHIFTS_WITH_CATEGORY_PROP: &str = "onlyAdjustShiftsWithCategory";

/// `DefaultPropertyFactory.FIXED_RATE` — shared with other families' configs
/// in Java; declared again here since nothing else in this crate needs it yet.
pub const FIXED_RATE: &str = "fixedRate";

/// `ShiftCategoryRegRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftCategoryRegRateRuleConfig;

impl RuleConfig for ShiftCategoryRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ShiftCatRrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = shift_category_base_default_values();
        params.set(ENFORCE_MINIMUM_WAGE_PROP, "false");
        params.set(JOB_RATE_ADDITION_PROP, "0.0");
        params.set(JOB_RATE_MULTIPLIER_PROP, "0.0");
        params.set(USE_FIXED_RATE_PROP, "true");
        params.set(FIXED_RATE, "10.0");
        params.set(ONLY_ADJUST_SHIFTS_WITH_CATEGORY_PROP, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = shift_category_base_validate(params);

        if params.bool_at(USE_FIXED_RATE_PROP) {
            if params.double_at(FIXED_RATE) < 0.0 {
                results.push("Fixed Rate must be greater than or equal to zero.".to_string());
            }
        } else if params.double_at(FIXED_RATE) > 0.0 {
            results.push("Fixed Rate must be 0 if Use Fixed Rate is not selected.".to_string());
        }

        if params.double_at(JOB_RATE_MULTIPLIER_PROP) < 0.0 {
            results.push("Job Rate Multiplier must be greater than or equal to zero.".to_string());
        }

        results
    }
}

/// `CombinationJobsRegRateRuleConfig`'s parameter keys.
pub const HOURS_THRESHOLD: &str = "hoursThreshold";
pub const DAYS_THRESHOLD: &str = "daysThreshold";
pub const HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB: &str =
    "higherClassificationMustBeSeparateJob";

/// `CombinationJobsRegRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CombinationJobsRegRateRuleConfig;

impl RuleConfig for CombinationJobsRegRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::CombinationJobRrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = regular_rate_default_values();
        params.set(HOURS_THRESHOLD, "4");
        params.set(DAYS_THRESHOLD, "2");
        params.set(HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB, "false");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = regular_rate_validate(params);
        if params.int_at(HOURS_THRESHOLD) <= 0 {
            results.push("Daily Hours Threshold must be greater than zero.".to_string());
        }
        if params.int_at(DAYS_THRESHOLD) <= 0 {
            results.push("Weekly Rate Days Threshold must be greater than zero.".to_string());
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_reg_rate_defaults_to_no_selected_types() {
        let defaults = JobRegRateRuleConfig.default_values();
        assert_eq!(
            ids_for_key(SELECTED_HOURS_DISTRIBUTION_TYPES, &defaults),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn job_reg_rate_requires_a_selected_type() {
        assert_eq!(
            JobRegRateRuleConfig
                .validate(&JobRegRateRuleConfig.default_values())
                .len(),
            1
        );

        let mut params = RuleParams::new();
        params.set(SELECTED_HOURS_DISTRIBUTION_TYPES, "[1]");
        assert!(JobRegRateRuleConfig.validate(&params).is_empty());
    }

    #[test]
    fn home_job_reg_rate_has_no_parameters_or_constraints() {
        assert!(HomeJobRegRateRuleConfig.default_values().is_empty());
        assert!(
            HomeJobRegRateRuleConfig
                .validate(&RuleParams::new())
                .is_empty()
        );
    }

    #[test]
    fn factor_job_defaults_to_a_factor_of_one() {
        let defaults = FactorJobRegRateRuleConfig.default_values();
        assert_eq!(defaults.double_at(WAGE_FACTOR_PROP), 1.0);
    }

    #[test]
    fn factor_job_rejects_a_negative_factor() {
        let mut params = FactorJobRegRateRuleConfig.default_values();
        params.set(SELECTED_HOURS_DISTRIBUTION_TYPES, "[1]");
        params.set(WAGE_FACTOR_PROP, "-1.0");
        assert_eq!(FactorJobRegRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn home_dept_defaults_to_not_paying_the_greater_rate() {
        let defaults = HomeDeptRegRateRuleConfig.default_values();
        assert!(!defaults.bool_at(PAY_GREATER_RATE));
    }

    #[test]
    fn shift_category_configs_do_not_carry_the_plain_regular_rate_parameters() {
        // ShiftCategoryRegRateRuleBaseConfig overrides getDefaultValues without
        // calling super, so neither shift-category config sees this key.
        assert!(
            !ShiftCategoryMinWageRegRateRuleConfig
                .default_values()
                .contains(SELECTED_HOURS_DISTRIBUTION_TYPES)
        );
        assert!(
            !ShiftCategoryRegRateRuleConfig
                .default_values()
                .contains(SELECTED_HOURS_DISTRIBUTION_TYPES)
        );
    }

    #[test]
    fn shift_category_min_wage_requires_a_selected_category() {
        assert_eq!(
            ShiftCategoryMinWageRegRateRuleConfig
                .validate(&ShiftCategoryMinWageRegRateRuleConfig.default_values())
                .len(),
            1
        );
    }

    #[test]
    fn shift_category_reg_rate_defaults_to_a_fixed_rate_of_ten() {
        let defaults = ShiftCategoryRegRateRuleConfig.default_values();
        assert!(defaults.bool_at(USE_FIXED_RATE_PROP));
        assert_eq!(defaults.double_at(FIXED_RATE), 10.0);
    }

    #[test]
    fn shift_category_reg_rate_rejects_a_nonzero_fixed_rate_when_not_in_use() {
        let mut params = ShiftCategoryRegRateRuleConfig.default_values();
        params.set(SHIFT_CATEGORIES, "[1]");
        params.set(USE_FIXED_RATE_PROP, "false");
        params.set(FIXED_RATE, "5.0");

        assert_eq!(ShiftCategoryRegRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn shift_category_reg_rate_rejects_a_negative_multiplier() {
        let mut params = ShiftCategoryRegRateRuleConfig.default_values();
        params.set(SHIFT_CATEGORIES, "[1]");
        params.set(JOB_RATE_MULTIPLIER_PROP, "-0.5");

        assert_eq!(ShiftCategoryRegRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn combination_jobs_defaults_to_four_hours_and_two_days() {
        let defaults = CombinationJobsRegRateRuleConfig.default_values();
        assert_eq!(defaults.int_at(HOURS_THRESHOLD), 4);
        assert_eq!(defaults.int_at(DAYS_THRESHOLD), 2);
        assert!(!defaults.bool_at(HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB));
    }

    #[test]
    fn combination_jobs_rejects_a_zero_threshold() {
        let mut params = CombinationJobsRegRateRuleConfig.default_values();
        params.set(SELECTED_HOURS_DISTRIBUTION_TYPES, "[1]");
        params.set(HOURS_THRESHOLD, "0");

        assert_eq!(CombinationJobsRegRateRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(JobRegRateRuleConfig.rule_class(), RuleClass::JobRrr);
        assert_eq!(HomeJobRegRateRuleConfig.rule_class(), RuleClass::HomeJobRrr);
        assert_eq!(
            FactorJobRegRateRuleConfig.rule_class(),
            RuleClass::FactorJobRrr
        );
        assert_eq!(
            HomeDeptRegRateRuleConfig.rule_class(),
            RuleClass::HomeDeptRrr
        );
        assert_eq!(
            ShiftCategoryMinWageRegRateRuleConfig.rule_class(),
            RuleClass::ScmwRrr
        );
        assert_eq!(
            ShiftCategoryRegRateRuleConfig.rule_class(),
            RuleClass::ShiftCatRrr
        );
        assert_eq!(
            CombinationJobsRegRateRuleConfig.rule_class(),
            RuleClass::CombinationJobRrr
        );
    }
}
