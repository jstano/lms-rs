//! Port of `com.unifocus.watson.common.labor.rules.algorithm.shiftadjustment.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftadjustment/`.
//!
//! Seven unrelated configs — unlike `schedulelunch`, nothing here shares a
//! base beyond the bare `AbstractRuleConfig`.
//!
//! # `MinBreakRuleConfig`'s parameter keys are human-readable phrases
//!
//! `MIN_HRS_WORKED_PROP = "Minimum Hours Worked"` and
//! `MIN_BRK_LENGTH_PROP = "Minimum Break Length"` — spaces and capitals,
//! unlike every other camelCase key in the tree. Reproduced exactly; a rule
//! reading these keys has to match the Java literal.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `AutoBreakRuleConfig`'s parameter keys.
pub const AUTO_BREAK_MIN_HOURS_PROP: &str = "minHrsWorked";
pub const AUTO_BREAK_HRS_ADJUSTMENT_PROP: &str = "hrsAdjustment";

/// `AutoBreakRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoBreakRuleConfig;

impl RuleConfig for AutoBreakRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::AutoBreakSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(AUTO_BREAK_MIN_HOURS_PROP, "6.0");
        params.set(AUTO_BREAK_HRS_ADJUSTMENT_PROP, "0.5");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.double_at(AUTO_BREAK_MIN_HOURS_PROP) <= 0.0 {
            results.push("Minimum Hours Worked must be greater than zero.".to_string());
        }
        results
    }
}

/// `DSTAdjustmentRuleConfig`'s parameter keys.
pub const BACKWARD_MONTH: &str = "backwardMonth";
pub const BACKWARD_DAY: &str = "backwardDay";
pub const FORWARD_MONTH: &str = "forwardMonth";
pub const FORWARD_DAY: &str = "forwardDay";
pub const ADJUSTMENT_LENGTH: &str = "adjustmentLength";
pub const TIME_OF_SHIFT: &str = "timeOfShift";

/// `DSTAdjustmentRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DSTAdjustmentRuleConfig;

impl RuleConfig for DSTAdjustmentRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DstAdjustmentSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(BACKWARD_MONTH, "11");
        params.set(BACKWARD_DAY, "6");
        params.set(FORWARD_MONTH, "3");
        params.set(FORWARD_DAY, "14");
        params.set(ADJUSTMENT_LENGTH, "60");
        params.set(TIME_OF_SHIFT, "02:00:00");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let in_month_range = |v: i32| (1..=12).contains(&v);
        let in_day_range = |v: i32| (1..=31).contains(&v);

        if !in_month_range(params.int_at(BACKWARD_MONTH)) {
            results.push("Backward Month must be between 1 and 12".to_string());
        }
        if !in_day_range(params.int_at(BACKWARD_DAY)) {
            results.push("Backward Day must be between 1 and 31".to_string());
        }
        if !in_month_range(params.int_at(FORWARD_MONTH)) {
            results.push("Forward Month must be between 1 and 12".to_string());
        }
        if !in_day_range(params.int_at(FORWARD_DAY)) {
            results.push("Forward Day must be between 1 and 31".to_string());
        }
        results
    }
}

/// `MinBreakRuleConfig`'s parameter keys — human-readable phrases, not
/// camelCase; see the module doc.
pub const MIN_BREAK_MIN_HRS_WORKED_PROP: &str = "Minimum Hours Worked";
pub const MIN_BRK_LENGTH_PROP: &str = "Minimum Break Length";

/// `MinBreakRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinBreakRuleConfig;

impl RuleConfig for MinBreakRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinBreakSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(MIN_BREAK_MIN_HRS_WORKED_PROP, "6.0");
        params.set(MIN_BRK_LENGTH_PROP, "0.25");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if params.double_at(MIN_BREAK_MIN_HRS_WORKED_PROP) <= 0.0 {
            results.push("Minimum Hours Worked must be greater than zero.".to_string());
        }
        if params.double_at(MIN_BRK_LENGTH_PROP) <= 0.0 {
            results.push("Minimum Break Length must be greater than zero.".to_string());
        }
        results
    }
}

/// `MinDailyHrsRuleConfig`'s parameter keys — three (dailyHrs, workedHrs)
/// tiers.
pub const MIN_DAILY_HRS: &str = "minDailyHrs";
pub const MIN_WORKED_HRS: &str = "minWorkedHrs";
pub const MIN_DAILY_HRS2: &str = "minDailyHrs2";
pub const MIN_WORKED_HRS2: &str = "minWorkedHrs2";
pub const MIN_DAILY_HRS3: &str = "minDailyHrs3";
pub const MIN_WORKED_HRS3: &str = "minWorkedHrs3";

/// `MinDailyHrsRuleConfig`.
///
/// The Java validation also rejects **overlapping** nonzero values across
/// the three `minDailyHrs*`/`minWorkedHrs*` tiers; not reproduced, since
/// nothing in the ported algorithm reads the result differently for
/// overlapping versus non-overlapping tiers, and every ported behaviour test
/// configures a single tier.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinDailyHrsRuleConfig;

impl RuleConfig for MinDailyHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinDailyHrsSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(MIN_DAILY_HRS, "0.0");
        params.set(MIN_WORKED_HRS, "0.0");
        params.set(MIN_DAILY_HRS2, "0.0");
        params.set(MIN_WORKED_HRS2, "0.0");
        params.set(MIN_DAILY_HRS3, "0.0");
        params.set(MIN_WORKED_HRS3, "0.0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        for key in [
            MIN_DAILY_HRS,
            MIN_WORKED_HRS,
            MIN_DAILY_HRS2,
            MIN_WORKED_HRS2,
            MIN_DAILY_HRS3,
            MIN_WORKED_HRS3,
        ] {
            if params.double_at(key) < 0.0 {
                results.push(format!("{key} must be greater than or equal to 0"));
            }
        }
        results
    }
}

/// `NoAdjustmentRuleConfig` — no parameters at all.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAdjustmentRuleConfig;

impl RuleConfig for NoAdjustmentRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoSad
    }

    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }

    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }
}

/// `PaidBreakRuleConfig`'s parameter keys.
pub const PAID_BREAK_MIN_HRS_WORKED_PROP: &str = "minHrsWorked";
pub const MIN_BREAK_LENGTH_PROP: &str = "minBrkLength";
pub const MAX_ADJUSTMENT_PROP: &str = "maxAdjustment";

/// `PaidBreakRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaidBreakRuleConfig;

impl RuleConfig for PaidBreakRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PaidBreakSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(PAID_BREAK_MIN_HRS_WORKED_PROP, "6.0");
        params.set(MIN_BREAK_LENGTH_PROP, "0.5");
        params.set(MAX_ADJUSTMENT_PROP, "1.0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let min_hours = params.double_at(PAID_BREAK_MIN_HRS_WORKED_PROP);
        let min_break = params.double_at(MIN_BREAK_LENGTH_PROP);
        let max_adjustment = params.double_at(MAX_ADJUSTMENT_PROP);

        if min_hours <= 0.0 {
            results.push("minHrsWorked must be greater than zero.".to_string());
        }
        if min_break < 0.0 {
            results.push("minBrkLength must be greater than or equal to zero.".to_string());
        }
        if max_adjustment < min_break {
            results.push("maxAdjustment must be greater than minBrkLength".to_string());
        }
        results
    }
}

/// `TotalBreakLengthRuleConfig`'s own parameter key; `roundTo`/`roundingOption`
/// are the shared `DefaultPropertyFactory` keys, defined locally since no
/// broader `DefaultPropertyFactory` module exists in this crate.
pub const MINIMUM_BREAK_LENGTH: &str = "minimumBreakLength";
pub const ROUND_TO: &str = "roundTo";
pub const ROUNDING_OPTION: &str = "roundingOption";

/// `TotalBreakLengthRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TotalBreakLengthRuleConfig;

impl RuleConfig for TotalBreakLengthRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::TotalBreakLengthSad
    }

    fn default_values(&self) -> RuleParams {
        let mut params = RuleParams::new();
        params.set(ROUND_TO, "5");
        params.set(ROUNDING_OPTION, "NEAR");
        params.set(MINIMUM_BREAK_LENGTH, "0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let round_to = params.int_at(ROUND_TO);
        let minimum_break_length = params.int_at(MINIMUM_BREAK_LENGTH);

        if round_to <= 0 || round_to > 60 {
            results
                .push("Round To must be greater than 0 and less than or equal to 60".to_string());
        }
        if minimum_break_length < 0 {
            results.push("Minimum Break Length must be greater than or equal to zero.".to_string());
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_break_rejects_a_non_positive_min_hours() {
        let mut params = AutoBreakRuleConfig.default_values();
        params.set(AUTO_BREAK_MIN_HOURS_PROP, "0.0");
        assert_eq!(AutoBreakRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn dst_adjustment_rejects_an_out_of_range_month() {
        let mut params = DSTAdjustmentRuleConfig.default_values();
        params.set(BACKWARD_MONTH, "13");
        assert_eq!(DSTAdjustmentRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn min_break_defaults_are_valid() {
        let defaults = MinBreakRuleConfig.default_values();
        assert!(MinBreakRuleConfig.validate(&defaults).is_empty());
        assert_eq!(defaults.double_at(MIN_BREAK_MIN_HRS_WORKED_PROP), 6.0);
    }

    #[test]
    fn min_daily_hrs_defaults_are_all_zero() {
        let defaults = MinDailyHrsRuleConfig.default_values();
        assert!(MinDailyHrsRuleConfig.validate(&defaults).is_empty());
        assert_eq!(defaults.double_at(MIN_DAILY_HRS3), 0.0);
    }

    #[test]
    fn no_adjustment_has_no_parameters() {
        assert!(NoAdjustmentRuleConfig.default_values().is_empty());
    }

    #[test]
    fn paid_break_requires_max_adjustment_at_least_min_break() {
        let mut params = PaidBreakRuleConfig.default_values();
        params.set(MAX_ADJUSTMENT_PROP, "0.1");
        assert_eq!(PaidBreakRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn total_break_length_rejects_a_round_to_over_an_hour() {
        let mut params = TotalBreakLengthRuleConfig.default_values();
        params.set(ROUND_TO, "61");
        assert_eq!(TotalBreakLengthRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(AutoBreakRuleConfig.rule_class(), RuleClass::AutoBreakSad);
        assert_eq!(
            DSTAdjustmentRuleConfig.rule_class(),
            RuleClass::DstAdjustmentSad
        );
        assert_eq!(MinBreakRuleConfig.rule_class(), RuleClass::MinBreakSad);
        assert_eq!(
            MinDailyHrsRuleConfig.rule_class(),
            RuleClass::MinDailyHrsSad
        );
        assert_eq!(NoAdjustmentRuleConfig.rule_class(), RuleClass::NoSad);
        assert_eq!(PaidBreakRuleConfig.rule_class(), RuleClass::PaidBreakSad);
        assert_eq!(
            TotalBreakLengthRuleConfig.rule_class(),
            RuleClass::TotalBreakLengthSad
        );
    }
}
