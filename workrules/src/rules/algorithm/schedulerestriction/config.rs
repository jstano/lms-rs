//! Port of `com.unifocus.watson.common.labor.rules.algorithm.schedulerestriction.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulerestriction/`.
//!
//! `ScheduleRestrictionRuleConfig` is the shared base: one parameter,
//! `strictMode`, defaulting to `false`. `NoRestrictionRuleConfig` is the one
//! config in the family that does **not** extend it — it extends
//! `AbstractRuleConfig` directly and carries no parameters at all, matching
//! `NoRestrictionRuleImpl.isStrict` always answering `false` rather than
//! reading a `strictMode` it never declares.

use crate::common::json_ids::ids_for_key;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// `ScheduleRestrictionRuleConfig.STRICT_MODE`.
pub const STRICT_MODE: &str = "strictMode";

/// The one parameter every plain `ScheduleRestrictionRuleConfig` subclass
/// starts from.
pub fn schedule_restriction_default_values() -> RuleParams {
    let mut params = RuleParams::new();
    params.set(STRICT_MODE, "false");
    params
}

/// `NoRestrictionRuleConfig` — extends `AbstractRuleConfig` directly, no
/// `strictMode` and no parameters at all.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRestrictionRuleConfig;

impl RuleConfig for NoRestrictionRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoSrr
    }

    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }

    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }
}

/// `EarliestStartLatestEndTimeRuleConfig`'s own parameter keys.
pub const DOW_MULTIPLE_PROP: &str = "dows";
pub const EARLIEST_START_TIME_PROP: &str = "earliestStartTime";
pub const LATEST_END_TIME_PROP: &str = "latestEndTime";

/// `EarliestStartLatestEndTimeRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarliestStartLatestEndTimeRuleConfig;

impl RuleConfig for EarliestStartLatestEndTimeRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::EarliestStartLatestEndSrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = schedule_restriction_default_values();
        params.set(EARLIEST_START_TIME_PROP, "00:00:00");
        params.set(LATEST_END_TIME_PROP, "00:00:00");
        params.set(DOW_MULTIPLE_PROP, "[]");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        if ids_for_key(DOW_MULTIPLE_PROP, params).is_empty() {
            results.push("Must include at least one day of week".to_string());
        }
        results
    }
}

/// `MaxDaysWorkedPerWeekRuleConfig`'s own parameter keys.
pub const USE_CALENDAR_WEEK_PROP: &str = "useCalendarWeek";
pub const DOW_PROP: &str = "dow";
pub const MAX_DAYS_PER_WEEK_PROP: &str = "maxDaysPerWeek";
pub const MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP: &str = "maxConsecutiveDaysPerWeek";

/// `MaxDaysWorkedPerWeekRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaxDaysWorkedPerWeekRuleConfig;

impl RuleConfig for MaxDaysWorkedPerWeekRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MaxDaysWorkedPerWeek
    }

    fn default_values(&self) -> RuleParams {
        let mut params = schedule_restriction_default_values();
        params.set(USE_CALENDAR_WEEK_PROP, "true");
        params.set(DOW_PROP, "1");
        params.set(MAX_DAYS_PER_WEEK_PROP, "7");
        params.set(MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP, "7");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let valid_day = |value: i32| (1..=7).contains(&value);

        if params.bool_at(USE_CALENDAR_WEEK_PROP) && !valid_day(params.int_at(DOW_PROP)) {
            results.push("Invalid Week Start Day".to_string());
        }
        if !valid_day(params.int_at(MAX_DAYS_PER_WEEK_PROP)) {
            results.push("Invalid Max Days Per Week".to_string());
        }
        if !valid_day(params.int_at(MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP)) {
            results.push("Invalid Max Consecutive Days Per Week".to_string());
        }
        results
    }
}

/// `MaxHoursOnDayRuleConfig.MAX_HOURS_PROP`.
pub const MAX_HOURS_ON_DAY_PROP: &str = "maxHours";
/// `MaxHoursOnDayRuleConfig.DOW_MULTIPLE_PROP` — same key as
/// [`DOW_MULTIPLE_PROP`], a separate constant since the two configs are
/// otherwise unrelated.
pub const MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP: &str = "dows";

/// `MaxHoursOnDayRuleConfig`. `HOURS_PER_DAY` is `24.0` —
/// `com.unifocus.framework.datetime.DateTimeConstants.HOURS_PER_DAY`, not
/// ported elsewhere in this crate, so the literal is inlined here.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaxHoursOnDayRuleConfig;

impl RuleConfig for MaxHoursOnDayRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MaxHoursOnDaySrr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = schedule_restriction_default_values();
        params.set(MAX_HOURS_ON_DAY_PROP, "8.0");
        params.set(MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP, "[]");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let max_hours = params.double_at(MAX_HOURS_ON_DAY_PROP);
        if max_hours < 0.01 {
            results.push("Max Hours must be greater than or equal to 0.01".to_string());
        }
        if max_hours > 24.0 {
            results.push("max hours must be less than 24.0".to_string());
        }
        if ids_for_key(MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP, params).is_empty() {
            results.push("Must include at least one day of week".to_string());
        }
        results
    }
}

/// `MaxHoursPerWeekRuleConfig.MAX_HOURS_PROP`.
pub const MAX_HOURS_PER_WEEK_PROP: &str = "maxHours";

/// `MaxHoursPerWeekRuleConfig`. `HOURS_PER_WEEK` is `168.0` (24 * 7) —
/// same unported constant as [`MaxHoursOnDayRuleConfig`]'s `HOURS_PER_DAY`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaxHoursPerWeekRuleConfig;

impl RuleConfig for MaxHoursPerWeekRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MaxHoursPerWeekSsr
    }

    fn default_values(&self) -> RuleParams {
        let mut params = schedule_restriction_default_values();
        params.set(MAX_HOURS_PER_WEEK_PROP, "40.0");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        let max_hours = params.double_at(MAX_HOURS_PER_WEEK_PROP);
        if max_hours < 0.01 {
            results.push("Max Hours must be greater than or equal to 0.01".to_string());
        }
        if max_hours > 168.0 {
            results.push("max hours must be less than 168.0".to_string());
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_restriction_defaults_to_not_strict() {
        assert!(!schedule_restriction_default_values().bool_at(STRICT_MODE));
    }

    #[test]
    fn no_restriction_has_no_parameters() {
        assert!(NoRestrictionRuleConfig.default_values().is_empty());
        assert!(
            NoRestrictionRuleConfig
                .validate(&RuleParams::new())
                .is_empty()
        );
    }

    #[test]
    fn earliest_start_latest_end_requires_a_day_of_week() {
        let defaults = EarliestStartLatestEndTimeRuleConfig.default_values();
        assert_eq!(
            EarliestStartLatestEndTimeRuleConfig
                .validate(&defaults)
                .len(),
            1
        );

        let mut params = defaults;
        params.set(DOW_MULTIPLE_PROP, "[1]");
        assert!(
            EarliestStartLatestEndTimeRuleConfig
                .validate(&params)
                .is_empty()
        );
    }

    #[test]
    fn max_days_worked_per_week_validates_each_day_field() {
        let defaults = MaxDaysWorkedPerWeekRuleConfig.default_values();
        assert!(
            MaxDaysWorkedPerWeekRuleConfig
                .validate(&defaults)
                .is_empty()
        );

        let mut params = defaults;
        params.set(MAX_DAYS_PER_WEEK_PROP, "8");
        assert_eq!(MaxDaysWorkedPerWeekRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn max_hours_on_day_rejects_more_than_a_full_day() {
        let mut params = MaxHoursOnDayRuleConfig.default_values();
        params.set(MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP, "[1]");
        assert!(MaxHoursOnDayRuleConfig.validate(&params).is_empty());

        params.set(MAX_HOURS_ON_DAY_PROP, "25.0");
        assert_eq!(MaxHoursOnDayRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn max_hours_per_week_rejects_more_than_a_full_week() {
        let defaults = MaxHoursPerWeekRuleConfig.default_values();
        assert!(MaxHoursPerWeekRuleConfig.validate(&defaults).is_empty());

        let mut params = defaults;
        params.set(MAX_HOURS_PER_WEEK_PROP, "200.0");
        assert_eq!(MaxHoursPerWeekRuleConfig.validate(&params).len(), 1);
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(NoRestrictionRuleConfig.rule_class(), RuleClass::NoSrr);
        assert_eq!(
            EarliestStartLatestEndTimeRuleConfig.rule_class(),
            RuleClass::EarliestStartLatestEndSrr
        );
        assert_eq!(
            MaxDaysWorkedPerWeekRuleConfig.rule_class(),
            RuleClass::MaxDaysWorkedPerWeek
        );
        assert_eq!(
            MaxHoursOnDayRuleConfig.rule_class(),
            RuleClass::MaxHoursOnDaySrr
        );
        assert_eq!(
            MaxHoursPerWeekRuleConfig.rule_class(),
            RuleClass::MaxHoursPerWeekSsr
        );
    }
}
