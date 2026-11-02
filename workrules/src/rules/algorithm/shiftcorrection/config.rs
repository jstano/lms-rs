//! Port of `com.unifocus.watson.common.labor.rules.algorithm.shiftcorrection.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftcorrection/`.
//!
//! Only `SplitToScheduleCorrectionRuleConfig` extends `PriorityRuleConfig`
//! (default priority 5, parameter `"priority"`); the other two rank at the
//! default without storing one. That matters because `ShiftCorrectionRunner`
//! — unlike nearly every other runner — sorts its rule items by priority.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{
    RuleConfig, ValidationResults, priority_default_values, priority_of,
};

/// `OutPunchCorrectionRuleConfig.DAILY_HRS_PROP`.
pub const DAILY_HRS_PROP: &str = "dailyHours";

/// `OutPunchCorrectionRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OutPunchCorrectionRuleConfig;

impl RuleConfig for OutPunchCorrectionRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::OutPunchScr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(DAILY_HRS_PROP, "8.00");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.double_at(DAILY_HRS_PROP) <= 0.0 {
            results.push("Daily Hours must be greater than zero.".to_string());
        }
        results
    }
}

/// `ShiftCategoryCorrectionRuleConfig`'s parameter keys.
pub const GRACE_PRE_START_PROP: &str = "gracePreSchedStart";
pub const GRACE_POST_START_PROP: &str = "gracePostSchedStart";
pub const MATCH_JOB: &str = "matchJob";

/// `ShiftCategoryCorrectionRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftCategoryCorrectionRuleConfig;

impl RuleConfig for ShiftCategoryCorrectionRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ShiftCategoryScr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(GRACE_PRE_START_PROP, "60");
        params.set(GRACE_POST_START_PROP, "60");
        params.set(MATCH_JOB, "true");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.int_at(GRACE_PRE_START_PROP) < 0 {
            results.push(
                "Grace Pre Schedule Start must be greater than or equal to zero.".to_string(),
            );
        }
        if params.int_at(GRACE_POST_START_PROP) < 0 {
            results.push(
                "Grace Post Schedule Start must be greater than or equal to zero.".to_string(),
            );
        }
        results
    }
}

/// `SplitToScheduleCorrectionRuleConfig.INCLUDE_MANAGER_EDITS_PROP`.
pub const INCLUDE_MANAGER_EDITS_PROP: &str = "includeManagerEdits";

/// `SplitToScheduleCorrectionRuleConfig` — extends `PriorityRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitToScheduleCorrectionRuleConfig;

impl RuleConfig for SplitToScheduleCorrectionRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::SplitToScheduleScr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = priority_default_values();
        params.set(INCLUDE_MANAGER_EDITS_PROP, "false");
        params
    }

    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }

    fn priority(&self, params: &RuleParams) -> Option<i32> {
        Some(priority_of(self, params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;
    use crate::rules::rule_config::{DEFAULT_PRIORITY, PRIORITY};

    #[test]
    fn out_punch_defaults() {
        let d = OutPunchCorrectionRuleConfig.default_values();
        assert_eq!(d.get(DAILY_HRS_PROP), Some("8.00"));
        assert!(OutPunchCorrectionRuleConfig.validate(&d).is_empty());
    }

    #[test]
    fn out_punch_rejects_non_positive_hours() {
        let p = rule_params! { DAILY_HRS_PROP => "0" };
        assert_eq!(OutPunchCorrectionRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn shift_category_defaults() {
        let d = ShiftCategoryCorrectionRuleConfig.default_values();
        assert_eq!(d.get(GRACE_PRE_START_PROP), Some("60"));
        assert_eq!(d.get(GRACE_POST_START_PROP), Some("60"));
        assert_eq!(d.get(MATCH_JOB), Some("true"));
        assert!(ShiftCategoryCorrectionRuleConfig.validate(&d).is_empty());
    }

    #[test]
    fn shift_category_rejects_negative_graces() {
        let p = rule_params! { GRACE_PRE_START_PROP => "-1", GRACE_POST_START_PROP => "-5" };
        assert_eq!(ShiftCategoryCorrectionRuleConfig.validate(&p).len(), 2);
    }

    /// `SplitToScheduleCorrectionRuleConfigTest.testGetDefaultValues`.
    #[test]
    fn split_to_schedule_defaults() {
        let d = SplitToScheduleCorrectionRuleConfig.default_values();
        assert_eq!(d.get(INCLUDE_MANAGER_EDITS_PROP), Some("false"));
        assert_eq!(d.get(PRIORITY), Some("5"));
        assert_eq!(d.len(), 2);
    }

    /// `SplitToScheduleCorrectionRuleConfigTest.testValidateProperties`.
    #[test]
    fn split_to_schedule_defaults_are_valid() {
        let d = SplitToScheduleCorrectionRuleConfig.default_values();
        assert!(SplitToScheduleCorrectionRuleConfig.validate(&d).is_empty());
    }

    #[test]
    fn only_split_to_schedule_is_priority_configurable() {
        let p = rule_params! { PRIORITY => "7" };
        assert_eq!(SplitToScheduleCorrectionRuleConfig.priority(&p), Some(7));
        assert_eq!(OutPunchCorrectionRuleConfig.priority(&p), None);
        assert_eq!(ShiftCategoryCorrectionRuleConfig.priority(&p), None);
        assert_eq!(
            SplitToScheduleCorrectionRuleConfig.priority(&RuleParams::new()),
            Some(DEFAULT_PRIORITY)
        );
    }
}
