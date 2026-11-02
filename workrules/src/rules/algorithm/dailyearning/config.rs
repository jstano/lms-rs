//! Port of `com.unifocus.watson.common.labor.rules.algorithm.dailyearning.*RuleConfig`
//! — the four configs slice 1 needs.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/dailyearning/`.
//!
//! # `MinDailyWorkedHours` and `MinDailyHours` disagree about `rateFactor`
//!
//! Their defaults overlap but are not the same. `MinDailyHoursEarningRuleConfig`
//! defaults `rateFactor` to `"0"` and adds `amount = "1"`;
//! `MinDailyWorkedHoursEarningRuleConfig` defaults `rateFactor` to `"1"` and has
//! no `amount`. `MinDailyWorkedHoursEarningRuleImpl` backfills from the
//! **former** — see that rule's module doc.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `DefaultPropertyFactory.EARNING`.
pub const EARNING: &str = "earning";
/// `DefaultPropertyFactory.AMOUNT`.
pub const AMOUNT: &str = "amount";
/// `DefaultPropertyFactory.SCHEDULE_LABEL`.
pub const SCHEDULE_LABEL: &str = "scheduleLabel";
/// `DefaultPropertyFactory.VALIDATE_SCHEDULE_LABEL`.
pub const VALIDATE_SCHEDULE_LABEL: &str = "validateScheduleLabel";
/// `DefaultPropertyFactory.MIN_QUALIFYING_HOURS`.
pub const MIN_QUALIFYING_HOURS: &str = "minQualifyingHours";
pub use crate::rules::algorithm::earningrate::config::RATE_FACTOR;

const MAX_LABEL_CHARACTERS: usize = 10;

fn validate_earning(params: &RuleParams, results: &mut ValidationResults, name: &str) {
    if params.int_at(EARNING) < 1 {
        results.push(format!("{name}: An earning must be selected"));
    }
}

fn validate_amount(params: &RuleParams, results: &mut ValidationResults, name: &str) {
    if params.double_at(AMOUNT) <= 0.0 {
        results.push(format!("{name}: Amount must be greater than 0"));
    }
}

fn validate_factor(params: &RuleParams, results: &mut ValidationResults, name: &str) {
    if params.double_at(RATE_FACTOR) < 0.0 {
        results.push(format!("{name}: Factor must be greater than or equal to 0"));
    }
}

fn validate_min_qualifying(params: &RuleParams, results: &mut ValidationResults, name: &str) {
    if params.double_at(MIN_QUALIFYING_HOURS) < 0.0 {
        results.push(format!(
            "{name}: Min qualifying hours must be greater than or equal to 0"
        ));
    }
}

/// The label is required only when `validateScheduleLabel` is on. Length is
/// measured on the **trimmed** text, as `MinDaily*`'s validators do.
fn validate_optional_label(params: &RuleParams, results: &mut ValidationResults, name: &str) {
    if params.bool_at(VALIDATE_SCHEDULE_LABEL) {
        let label = params.get(SCHEDULE_LABEL).unwrap_or("").trim();
        if label.is_empty() || label.chars().count() > MAX_LABEL_CHARACTERS {
            results.push(format!(
                "{name}: A schedule label must be non empty and 10 characters or less if validate schedule labels is checked"
            ));
        }
    }
}

/// `NoDailyEarningRuleConfig` — no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDailyEarningRuleConfig;

impl RuleConfig for NoDailyEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoDdf
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }
}

/// `MinDailyHoursEarningRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinDailyHoursEarningRuleConfig;

const MIN_DAILY_HOURS_NAME: &str = "Min Daily Hours Earning";

impl RuleConfig for MinDailyHoursEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinDailyHrsDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(AMOUNT, "1");
        p.set(RATE_FACTOR, "0");
        p.set(MIN_QUALIFYING_HOURS, "6");
        p.set(VALIDATE_SCHEDULE_LABEL, "false");
        p.set(SCHEDULE_LABEL, "");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        validate_earning(params, &mut r, MIN_DAILY_HOURS_NAME);
        validate_amount(params, &mut r, MIN_DAILY_HOURS_NAME);
        validate_factor(params, &mut r, MIN_DAILY_HOURS_NAME);
        validate_min_qualifying(params, &mut r, MIN_DAILY_HOURS_NAME);
        validate_optional_label(params, &mut r, MIN_DAILY_HOURS_NAME);
        r
    }
}

/// `MinDailyWorkedHoursEarningRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinDailyWorkedHoursEarningRuleConfig;

const MIN_DAILY_WORKED_NAME: &str = "Min Daily Worked Hours Earning";

impl RuleConfig for MinDailyWorkedHoursEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinDailyWrkdHrsDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(RATE_FACTOR, "1");
        p.set(MIN_QUALIFYING_HOURS, "6");
        p.set(VALIDATE_SCHEDULE_LABEL, "false");
        p.set(SCHEDULE_LABEL, "");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        validate_earning(params, &mut r, MIN_DAILY_WORKED_NAME);
        validate_factor(params, &mut r, MIN_DAILY_WORKED_NAME);
        validate_min_qualifying(params, &mut r, MIN_DAILY_WORKED_NAME);
        validate_optional_label(params, &mut r, MIN_DAILY_WORKED_NAME);
        r
    }
}

/// `NoDailyHoursWithLabelEarningRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDailyHoursWithLabelEarningRuleConfig;

const NO_DAILY_HOURS_NAME: &str = "No Daily Hours With Label Earning";

impl RuleConfig for NoDailyHoursWithLabelEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoDailyHrsDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(AMOUNT, "1");
        p.set(SCHEDULE_LABEL, "");
        p.set(RATE_FACTOR, "0");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        validate_earning(params, &mut r, NO_DAILY_HOURS_NAME);
        validate_amount(params, &mut r, NO_DAILY_HOURS_NAME);
        // Unlike the min-hours configs the label is always required, and its
        // length is measured on the untrimmed text.
        let label = params.get(SCHEDULE_LABEL).unwrap_or("");
        if label.trim().is_empty() || label.chars().count() > MAX_LABEL_CHARACTERS {
            r.push(format!(
                "{NO_DAILY_HOURS_NAME}: A schedule label must be non empty and 10 characters or less"
            ));
        }
        if params.double_at(RATE_FACTOR) < 0.0 {
            r.push(format!(
                "{NO_DAILY_HOURS_NAME}: Rate factor must be greater than or equal to 0"
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
    fn the_two_min_daily_configs_disagree_about_rate_factor() {
        let hours = MinDailyHoursEarningRuleConfig.default_values();
        let worked = MinDailyWorkedHoursEarningRuleConfig.default_values();
        assert_eq!(hours.get(RATE_FACTOR), Some("0"));
        assert_eq!(worked.get(RATE_FACTOR), Some("1"));
        assert_eq!(hours.get(AMOUNT), Some("1"));
        assert_eq!(worked.get(AMOUNT), None);
    }

    #[test]
    fn defaults_fail_validation_only_for_the_unselected_earning() {
        // earning "0" is the "none selected" sentinel.
        assert_eq!(
            MinDailyHoursEarningRuleConfig
                .validate(&MinDailyHoursEarningRuleConfig.default_values())
                .len(),
            1
        );
        assert_eq!(
            MinDailyWorkedHoursEarningRuleConfig
                .validate(&MinDailyWorkedHoursEarningRuleConfig.default_values())
                .len(),
            1
        );
    }

    #[test]
    fn a_label_is_required_only_when_validation_is_on() {
        let mut p = MinDailyHoursEarningRuleConfig.default_values();
        p.set(EARNING, "3");
        assert!(MinDailyHoursEarningRuleConfig.validate(&p).is_empty());
        p.set(VALIDATE_SCHEDULE_LABEL, "true");
        assert_eq!(MinDailyHoursEarningRuleConfig.validate(&p).len(), 1);
        p.set(SCHEDULE_LABEL, "ABCDEFGHIJK");
        assert_eq!(MinDailyHoursEarningRuleConfig.validate(&p).len(), 1);
        p.set(SCHEDULE_LABEL, "ABCDEFGHIJ");
        assert!(MinDailyHoursEarningRuleConfig.validate(&p).is_empty());
    }

    #[test]
    fn no_daily_hours_always_requires_a_label() {
        let p = rule_params! { EARNING => "3", AMOUNT => "1", RATE_FACTOR => "0", SCHEDULE_LABEL => " " };
        assert_eq!(NoDailyHoursWithLabelEarningRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn negative_factors_and_non_positive_amounts_are_rejected() {
        let p = rule_params! { EARNING => "3", AMOUNT => "0", RATE_FACTOR => "-1", MIN_QUALIFYING_HOURS => "-1" };
        assert_eq!(MinDailyHoursEarningRuleConfig.validate(&p).len(), 3);
    }

    #[test]
    fn no_daily_earning_has_no_parameters() {
        assert!(NoDailyEarningRuleConfig.default_values().is_empty());
        assert!(
            NoDailyEarningRuleConfig
                .validate(&RuleParams::new())
                .is_empty()
        );
    }
}
