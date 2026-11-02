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

/// `DefaultPropertyFactory.MIN_DAILY_HRS`.
pub const MIN_DAILY_HRS: &str = "minDailyHrs";
/// `DefaultPropertyFactory.EARNING_FACTOR`.
pub const EARNING_FACTOR: &str = "earningFactor";
/// `DefaultPropertyFactory.HOURS`.
pub const HOURS: &str = "hours";
/// `DefaultPropertyFactory.DOW_MULTIPLE`.
pub const DOW_MULTIPLE: &str = "dows";
/// `DefaultPropertyFactory.STANDARD_WORK_DAY`.
pub const STANDARD_WORK_DAY: &str = "standardWorkDay";
/// `DefaultPropertyFactory.ANNIVERSARY_DATE_TYPE`.
pub const ANNIVERSARY_DATE_TYPE: &str = "anniversaryDateTypes";
/// `DaysWorkedDailyDifferentialRuleConfig`'s keys.
pub const WEEKLY_DAYS_WORKED_THRESHOLD: &str = "weeklyDaysWorkedThreshold";
pub const DAILY_HOURS_THRESHOLD: &str = "dailyHoursThreshold";
pub const FIRST_DAILY_HOURS_FIXED_COST: &str = "firstDailyHoursFixedCost";
pub const SECOND_DAILY_HOURS_FIXED_COST: &str = "secondDailyHoursFixedCost";

/// `AbstractRuleConfig.SMALLEST_VALID_DOUBLE`, the floor of
/// `validateDoubleGreaterThanZero` (so "greater than zero" means "at least a
/// cent").
const SMALLEST_VALID_DOUBLE: f64 = 0.01;

/// `MinimumHoursWorkedDifferentialRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinimumHoursWorkedDifferentialRuleConfig;

const MIN_HOURS_WORKED_NAME: &str = "Minimum Hours Worked Differential";

impl RuleConfig for MinimumHoursWorkedDifferentialRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinHoursWorkedDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(RATE_FACTOR, "1");
        p.set(MIN_DAILY_HRS, "3");
        p.set(MIN_QUALIFYING_HOURS, "0");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        let name = MIN_HOURS_WORKED_NAME;
        // Unlike the min-hours configs this one accepts earning 0.
        if params.int_at(EARNING) < 0 {
            r.push(format!("{name}: An earning must be selected"));
        }
        if params.double_at(RATE_FACTOR) < 0.0 {
            r.push(format!("{name}: Factor must be greater than 0"));
        }
        let min_daily = params.double_at(MIN_DAILY_HRS);
        if min_daily < 0.0 {
            r.push(format!("{name}: Min daily hours must be greater than 0"));
        }
        let min_qualifying = params.double_at(MIN_QUALIFYING_HOURS);
        if min_qualifying < 0.0 || min_qualifying >= min_daily {
            r.push(format!(
                "{name}: Min qualifying hours must be greater than or equal to 0 and less than min daily hours"
            ));
        }
        r
    }
}

/// `DaysWorkedDailyDifferentialRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DaysWorkedDailyDifferentialRuleConfig;

const DAYS_WORKED_NAME: &str = "Days Worked Daily Differential";

impl RuleConfig for DaysWorkedDailyDifferentialRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DaysWorkedDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(WEEKLY_DAYS_WORKED_THRESHOLD, "5");
        p.set(EARNING, "0");
        p.set(DAILY_HOURS_THRESHOLD, "6.0");
        p.set(FIRST_DAILY_HOURS_FIXED_COST, "1.5");
        p.set(SECOND_DAILY_HOURS_FIXED_COST, "3.0");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        let name = DAYS_WORKED_NAME;
        let days = params.double_at(WEEKLY_DAYS_WORKED_THRESHOLD);
        if !(1.0..=8.0).contains(&days) {
            r.push(format!(
                "{name}: Weekly Days Worked Threshold must be in the range [1, 8]"
            ));
        }
        if params.int_at(EARNING) <= 0 {
            r.push(format!("{name}: Earning must be selected"));
        }
        let hours = params.double_at(DAILY_HOURS_THRESHOLD);
        if hours <= 0.0 || hours > 24.0 {
            r.push(format!(
                "{name}: Daily Hours Threshold must be in the range (0, 24]"
            ));
        }
        for (key, label) in [
            (FIRST_DAILY_HOURS_FIXED_COST, "First Daily Hours Fixed Cost"),
            (
                SECOND_DAILY_HOURS_FIXED_COST,
                "Second Daily Hours Fixed Cost",
            ),
        ] {
            if params.double_at(key) < SMALLEST_VALID_DOUBLE {
                r.push(format!(
                    "{name}: {label} must be greater than or equal to {SMALLEST_VALID_DOUBLE}"
                ));
            }
        }
        r
    }
}

/// `DailySalariedAllowanceRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailySalariedAllowanceRuleConfig;

impl RuleConfig for DailySalariedAllowanceRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DlySalAllowDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(AMOUNT, "0");
        p.set(STANDARD_WORK_DAY, "0");
        p.set(DOW_MULTIPLE, "[]");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        if params.int_at(EARNING) <= 0 {
            r.push("Earning must be selected".to_string());
        }
        if params.double_at(AMOUNT) <= 0.0 {
            r.push("Amount must be greater than 0".to_string());
        }
        if params.double_at(STANDARD_WORK_DAY) <= 0.0 {
            r.push("Standard work day must be greater than 0".to_string());
        }
        if crate::common::json_ids::ids_for_key(DOW_MULTIPLE, params).is_empty() {
            r.push("Must include at least one day of week".to_string());
        }
        r
    }
}

/// `AnniversaryEarningRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnniversaryEarningRuleConfig;

const ANNIVERSARY_NAME: &str = "Anniversary Earning";

impl RuleConfig for AnniversaryEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::AnniDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(EARNING_FACTOR, "1");
        p.set(HOURS, "8");
        p.set(ANNIVERSARY_DATE_TYPE, "[]");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        let name = ANNIVERSARY_NAME;
        if params.int_at(EARNING) <= 0 {
            r.push(format!("{name}: Earning must be selected"));
        }
        for (key, label) in [(EARNING_FACTOR, "Earning Factor"), (HOURS, "Hours")] {
            if params.double_at(key) < SMALLEST_VALID_DOUBLE {
                r.push(format!(
                    "{name}: {label} must be greater than or equal to {SMALLEST_VALID_DOUBLE}"
                ));
            }
        }
        let selected = params.get(ANNIVERSARY_DATE_TYPE).unwrap_or("").trim();
        if !(selected.starts_with('[') && selected.ends_with(']')) {
            r.push(format!(
                "{name}: Anniversary Date Types is not correctly formatted"
            ));
        } else if selected[1..selected.len() - 1].trim().is_empty() {
            r.push(format!("{name}: Anniversary Date Types must not be empty"));
        }
        r
    }
}

/// `HolidayDifferentialRuleConfig`'s parameter keys.
pub const HOLIDAY_EARNING_PROP: &str = "holidayEarning";
pub const PRIMARY_FACTOR: &str = "primaryFactor";
pub const SECONDARY_FACTOR: &str = "secondaryFactor";
pub const SECONDARY_FACTOR_THRESHOLD: &str = "secondaryFactorThreshold";
pub const MIN_HRS_PAID: &str = "minHrsPaid";
pub const MAX_HRS_PAID: &str = "maxHrsPaid";
pub const ACTUAL_PUNCH_PROP: &str = "actualPunchTime";
pub const COUNT_ENTIRE_SHIFT_AS_HOLIDAY: &str = "countEntireShiftAsHoliday";
pub use crate::rules::algorithm::hoursdistribution::config::HOLIDAY_TYPES_PROP;

/// `HolidayDifferentialRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HolidayDifferentialRuleConfig;

const HOLIDAY_NAME: &str = "Holiday Differential";

impl RuleConfig for HolidayDifferentialRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HolidayDifDdf
    }

    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(HOLIDAY_EARNING_PROP, "0");
        p.set(PRIMARY_FACTOR, "0.5");
        p.set(SECONDARY_FACTOR, "1.0");
        p.set(SECONDARY_FACTOR_THRESHOLD, "8.0");
        p.set(MIN_HRS_PAID, "6.0");
        p.set(HOLIDAY_TYPES_PROP, "[]");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(MAX_HRS_PAID, "999");
        p.set(COUNT_ENTIRE_SHIFT_AS_HOLIDAY, "false");
        p
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        let name = HOLIDAY_NAME;
        if params.int_at(HOLIDAY_EARNING_PROP) <= 0 {
            r.push(format!("{name}: Holiday Earning must be selected"));
        }
        for (key, label) in [
            (PRIMARY_FACTOR, "Primary Factor"),
            (SECONDARY_FACTOR, "Secondary Factor"),
            (SECONDARY_FACTOR_THRESHOLD, "Secondary Factor Threshold"),
        ] {
            if params.double_at(key) < SMALLEST_VALID_DOUBLE {
                r.push(format!(
                    "{name}: {label} must be greater than or equal to {SMALLEST_VALID_DOUBLE}"
                ));
            }
        }
        if params.double_at(MIN_HRS_PAID) < 0.0 {
            r.push(format!(
                "{name}: Minimum Hours Paid must be greater than or equal to 0"
            ));
        }
        if params.double_at(MAX_HRS_PAID) < params.double_at(MIN_HRS_PAID) {
            r.push(format!(
                "{name}: Maximum Hours Paid must be greater than or equal to Minimum Hours Paid."
            ));
        }
        if params.double_at(MAX_HRS_PAID) < params.double_at(SECONDARY_FACTOR_THRESHOLD) {
            r.push(format!(
                "{name}: Maximum Hours Paid must be greater than or equal to Secondary Factor Threshold."
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

    #[test]
    fn slice_two_defaults() {
        let d = DaysWorkedDailyDifferentialRuleConfig.default_values();
        assert_eq!(d.get(WEEKLY_DAYS_WORKED_THRESHOLD), Some("5"));
        assert_eq!(d.get(DAILY_HOURS_THRESHOLD), Some("6.0"));
        assert_eq!(d.get(FIRST_DAILY_HOURS_FIXED_COST), Some("1.5"));
        assert_eq!(d.get(SECOND_DAILY_HOURS_FIXED_COST), Some("3.0"));
        let m = MinimumHoursWorkedDifferentialRuleConfig.default_values();
        assert_eq!(m.get(MIN_DAILY_HRS), Some("3"));
        assert_eq!(m.get(RATE_FACTOR), Some("1"));
        let a = AnniversaryEarningRuleConfig.default_values();
        assert_eq!(a.get(HOURS), Some("8"));
        assert_eq!(a.get(ANNIVERSARY_DATE_TYPE), Some("[]"));
        let s = DailySalariedAllowanceRuleConfig.default_values();
        assert_eq!(s.get(DOW_MULTIPLE), Some("[]"));
    }

    #[test]
    fn minimum_hours_worked_requires_qualifying_hours_below_the_minimum() {
        let ok = rule_params! { EARNING => "0", RATE_FACTOR => "1", MIN_DAILY_HRS => "6", MIN_QUALIFYING_HOURS => "4" };
        assert!(
            MinimumHoursWorkedDifferentialRuleConfig
                .validate(&ok)
                .is_empty()
        );
        let bad = rule_params! { EARNING => "0", RATE_FACTOR => "1", MIN_DAILY_HRS => "6", MIN_QUALIFYING_HOURS => "6" };
        assert_eq!(
            MinimumHoursWorkedDifferentialRuleConfig
                .validate(&bad)
                .len(),
            1
        );
    }

    #[test]
    fn days_worked_ranges_are_inclusive() {
        let mut p = DaysWorkedDailyDifferentialRuleConfig.default_values();
        p.set(EARNING, "2");
        assert!(
            DaysWorkedDailyDifferentialRuleConfig
                .validate(&p)
                .is_empty()
        );
        p.set(WEEKLY_DAYS_WORKED_THRESHOLD, "8");
        p.set(DAILY_HOURS_THRESHOLD, "24");
        assert!(
            DaysWorkedDailyDifferentialRuleConfig
                .validate(&p)
                .is_empty()
        );
        p.set(WEEKLY_DAYS_WORKED_THRESHOLD, "9");
        p.set(DAILY_HOURS_THRESHOLD, "24.5");
        assert_eq!(DaysWorkedDailyDifferentialRuleConfig.validate(&p).len(), 2);
        p.set(WEEKLY_DAYS_WORKED_THRESHOLD, "0");
        p.set(DAILY_HOURS_THRESHOLD, "0");
        assert_eq!(DaysWorkedDailyDifferentialRuleConfig.validate(&p).len(), 2);
    }

    #[test]
    fn fixed_costs_must_be_at_least_a_cent() {
        let mut p = DaysWorkedDailyDifferentialRuleConfig.default_values();
        p.set(EARNING, "2");
        p.set(FIRST_DAILY_HOURS_FIXED_COST, "0.009");
        assert_eq!(DaysWorkedDailyDifferentialRuleConfig.validate(&p).len(), 1);
        p.set(FIRST_DAILY_HOURS_FIXED_COST, "0.01");
        assert!(
            DaysWorkedDailyDifferentialRuleConfig
                .validate(&p)
                .is_empty()
        );
    }

    #[test]
    fn salaried_allowance_requires_everything() {
        let d = DailySalariedAllowanceRuleConfig.default_values();
        assert_eq!(DailySalariedAllowanceRuleConfig.validate(&d).len(), 4);
        let ok = rule_params! { EARNING => "1", AMOUNT => "5", STANDARD_WORK_DAY => "8", DOW_MULTIPLE => "[2,3]" };
        assert!(DailySalariedAllowanceRuleConfig.validate(&ok).is_empty());
    }

    #[test]
    fn anniversary_requires_a_selection_and_positive_numbers() {
        let d = AnniversaryEarningRuleConfig.default_values();
        assert_eq!(
            AnniversaryEarningRuleConfig.validate(&d).len(),
            2,
            "earning and types"
        );
        let ok = rule_params! { EARNING => "1", EARNING_FACTOR => "1.5", HOURS => "8", ANNIVERSARY_DATE_TYPE => "[BIRTHDAY]" };
        assert!(AnniversaryEarningRuleConfig.validate(&ok).is_empty());
        let malformed = rule_params! { EARNING => "1", EARNING_FACTOR => "1", HOURS => "8", ANNIVERSARY_DATE_TYPE => "BIRTHDAY" };
        assert_eq!(AnniversaryEarningRuleConfig.validate(&malformed).len(), 1);
    }

    #[test]
    fn holiday_differential_defaults() {
        let d = HolidayDifferentialRuleConfig.default_values();
        assert_eq!(d.get(PRIMARY_FACTOR), Some("0.5"));
        assert_eq!(d.get(SECONDARY_FACTOR), Some("1.0"));
        assert_eq!(d.get(SECONDARY_FACTOR_THRESHOLD), Some("8.0"));
        assert_eq!(d.get(MIN_HRS_PAID), Some("6.0"));
        assert_eq!(d.get(ACTUAL_PUNCH_PROP), Some("true"));
        assert_eq!(d.get(MAX_HRS_PAID), Some("999"));
        assert_eq!(d.get(COUNT_ENTIRE_SHIFT_AS_HOLIDAY), Some("false"));
        assert_eq!(d.get(HOLIDAY_TYPES_PROP), Some("[]"));
        assert_eq!(d.len(), 9);
    }

    #[test]
    fn holiday_differential_max_must_cover_min_and_the_secondary_threshold() {
        let mut p = HolidayDifferentialRuleConfig.default_values();
        p.set(HOLIDAY_EARNING_PROP, "3");
        assert!(HolidayDifferentialRuleConfig.validate(&p).is_empty());
        p.set(MAX_HRS_PAID, "5");
        assert_eq!(
            HolidayDifferentialRuleConfig.validate(&p).len(),
            2,
            "below both the minimum (6) and the threshold (8)"
        );
        p.set(MAX_HRS_PAID, "8");
        assert!(HolidayDifferentialRuleConfig.validate(&p).is_empty());
    }

    #[test]
    fn holiday_differential_needs_an_earning_and_positive_factors() {
        let d = HolidayDifferentialRuleConfig.default_values();
        assert_eq!(
            HolidayDifferentialRuleConfig.validate(&d).len(),
            1,
            "earning 0"
        );
        let mut p = d;
        p.set(HOLIDAY_EARNING_PROP, "3");
        p.set(PRIMARY_FACTOR, "0");
        p.set(MIN_HRS_PAID, "-1");
        assert_eq!(HolidayDifferentialRuleConfig.validate(&p).len(), 2);
    }
}
