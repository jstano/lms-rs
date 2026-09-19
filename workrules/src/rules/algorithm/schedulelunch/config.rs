//! Port of `com.unifocus.watson.common.labor.rules.algorithm.schedulelunch.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelunch/`.
//!
//! `LunchAdjustEndTimeRuleConfig` and `LunchSimpleRuleConfig` are byte-for-byte
//! identical apart from their rule class — same two parameters, same
//! defaults, same validation — so they share one set of helpers here rather
//! than duplicating the logic twice.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `LunchAdjustEndTimeRuleConfig.MIN_HOURS_PROP` / `LunchSimpleRuleConfig.MIN_HOURS_PROP`.
pub const MIN_HOURS_PROP: &str = "minHrsWorked";
/// `LunchAdjustEndTimeRuleConfig.HRS_ADJUSTMENT_PROP` / `LunchSimpleRuleConfig.HRS_ADJUSTMENT_PROP`.
pub const HRS_ADJUSTMENT_PROP: &str = "hrsAdjustment";

/// The shared default values both configs carry.
fn lunch_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(MIN_HOURS_PROP, "4.0");
    params.set(HRS_ADJUSTMENT_PROP, "0.5");
    params
}

/// The shared validation both configs run: `minHrsWorked` must be positive.
fn lunch_validate(params: &RuleParams) -> ValidationResults {
    let mut results = ValidationResults::new();
    if params.double_at(MIN_HOURS_PROP) <= 0.0 {
        results.push("Minimum Hours Scheduled must be greater than zero.".to_string());
    }
    results
}

/// `LunchAdjustEndTimeRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchAdjustEndTimeRuleConfig;

impl RuleConfig for LunchAdjustEndTimeRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::LunchAdjustEndTime
    }

    fn default_values(&self) -> RuleParams {
        lunch_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        lunch_validate(params)
    }
}

/// `LunchSimpleRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchSimpleRuleConfig;

impl RuleConfig for LunchSimpleRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::LunchSimple
    }

    fn default_values(&self) -> RuleParams {
        lunch_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        lunch_validate(params)
    }
}

/// `LunchStartTimeAndLengthRuleConfig`'s own parameter keys.
pub const MINIMUM_SHIFT_LENGTH: &str = "minimumShiftLength";
pub const MAXIMUM_SHIFT_LENGTH: &str = "maximumShiftLength";
pub const EARLIEST_START_TIME: &str = "earliestStartTime";
pub const LATEST_START_TIME: &str = "latestStartTime";
pub const BREAK_LENGTH: &str = "breakLength";
pub const ADJUST_END_TIME: &str = "adjustEndTime";

const UPPER_VALIDATION_LIMIT: f64 = 24.0;

/// `LunchStartTimeAndLengthRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchStartTimeAndLengthRuleConfig;

impl RuleConfig for LunchStartTimeAndLengthRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::LunchStartTimeAndLength
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(MINIMUM_SHIFT_LENGTH, "0.01");
        params.set(MAXIMUM_SHIFT_LENGTH, "8.00");
        params.set(EARLIEST_START_TIME, "00:00:00");
        params.set(LATEST_START_TIME, "23:59:00");
        params.set(BREAK_LENGTH, "1.0");
        params.set(ADJUST_END_TIME, "true");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let min_length = params.double_at(MINIMUM_SHIFT_LENGTH);
        let max_length = params.double_at(MAXIMUM_SHIFT_LENGTH);
        let break_length = params.double_at(BREAK_LENGTH);

        if min_length < 0.0 || min_length > max_length {
            results.push(
                "Minimum Shift Length must be greater than 0 and less than or equal to the maximum shift length"
                    .to_string(),
            );
        }
        if max_length <= 0.0 || max_length >= UPPER_VALIDATION_LIMIT {
            results.push(format!(
                "Maximum Shift Length must be greater than 0 and less than {UPPER_VALIDATION_LIMIT}"
            ));
        }
        if break_length < 0.0 || break_length >= max_length {
            results.push(
                "Break Adjustment must be greater than 0 and less than the maximum shift length"
                    .to_string(),
            );
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lunch_configs_share_the_same_defaults() {
        assert_eq!(
            LunchAdjustEndTimeRuleConfig.default_values(),
            LunchSimpleRuleConfig.default_values()
        );
        assert_eq!(
            LunchSimpleRuleConfig
                .default_values()
                .double_at(MIN_HOURS_PROP),
            4.0
        );
    }

    #[test]
    fn a_non_positive_min_hours_is_invalid() {
        let mut params = LunchSimpleRuleConfig.default_values();
        params.set(MIN_HOURS_PROP, "0.0");
        assert_eq!(LunchSimpleRuleConfig.validate(&params).len(), 1);
        assert_eq!(LunchAdjustEndTimeRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn start_time_and_length_defaults_are_valid() {
        let defaults = LunchStartTimeAndLengthRuleConfig.default_values();
        assert!(
            LunchStartTimeAndLengthRuleConfig
                .validate(&defaults)
                .is_empty()
        );
    }

    #[test]
    fn a_break_length_at_or_past_the_max_shift_length_is_invalid() {
        let mut params = LunchStartTimeAndLengthRuleConfig.default_values();
        params.set(BREAK_LENGTH, "8.0");
        assert_eq!(LunchStartTimeAndLengthRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(
            LunchAdjustEndTimeRuleConfig.rule_class(),
            RuleClass::LunchAdjustEndTime
        );
        assert_eq!(LunchSimpleRuleConfig.rule_class(), RuleClass::LunchSimple);
        assert_eq!(
            LunchStartTimeAndLengthRuleConfig.rule_class(),
            RuleClass::LunchStartTimeAndLength
        );
    }
}
