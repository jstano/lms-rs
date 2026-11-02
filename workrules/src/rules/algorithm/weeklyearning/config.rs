//! Port of `com.unifocus.watson.common.labor.rules.algorithm.weeklyearning.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/weeklyearning/`.
//!
//! None of the eight extends `PriorityRuleConfig`, and `WeeklyDifferentialRuleRunner`
//! does not sort, so rule items run in rule-set order.
//!
//! # Display names are placeholders
//!
//! Java prefixes every validation message with `getName()`, a localized
//! resource string this crate does not carry. The names below ("Daily Spread",
//! …) stand in for them; the *conditions* are exact, the message text is not.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

pub use crate::rules::algorithm::dailyearning::config::{EARNING, MIN_DAILY_HRS, RATE_FACTOR};

/// `DefaultPropertyFactory` / per-config keys, gathered.
pub const ACTUAL_PUNCH_PROP: &str = "actualPunchTime";
pub const HOURS: &str = "hours";
pub const MIN_HRS_SPREAD: &str = "minHours";
pub const USE_MIN_WAGE: &str = "useMinWage";
pub const MAX_DAILY_HRS: &str = "maxDailyHrs";
pub const MIN_WEEKLY_HRS: &str = "minWeeklyHrs";
pub const MAX_WEEKLY_HRS: &str = "maxWeeklyHrs";
pub const ADDITIONAL_HOURS: &str = "additionalHours";
/// `DOWHrsFixedRuleConfig` / `WeeklyHrsFixedRuleConfig` /
/// `HoursThresholdFixedWeeklyRuleConfig`.
pub const DOLLARS_PROP: &str = "dollars";
pub const MIN_HRS_PROP: &str = "minHours";
pub const MAX_HRS_PROP: &str = "maxHours";
pub const DOW_PROP: &str = "dow";
pub const MAX_OCCURRENCES: &str = "maxOccurrences";
/// `DOWWeeklyHrsRateFactorRuleConfig`.
pub const FACTOR_PROP: &str = "factor";
pub const DOW_EARNING_PROP: &str = "earningType";
/// `SalariedMinWageRuleConfig`.
pub const MIN_WAGE_PROP: &str = "minWage";
pub const HOURLY_EARNINGS: &str = "hourlyEarnings";
pub const DOLLARS_EARNINGS: &str = "dollarsEarnings";

/// `AbstractRuleConfig.SMALLEST_VALID_DOUBLE`.
const SMALLEST_VALID_DOUBLE: f64 = 0.01;

fn earning_selected(p: &RuleParams, key: &str, r: &mut ValidationResults, name: &str) {
    if p.int_at(key) <= 0 {
        r.push(format!("{name}: Earning must be selected."));
    }
}

fn at_least_a_cent(p: &RuleParams, key: &str, label: &str, r: &mut ValidationResults, name: &str) {
    if p.double_at(key) < SMALLEST_VALID_DOUBLE {
        r.push(format!(
            "{name}: {label} must be greater than or equal to {SMALLEST_VALID_DOUBLE}"
        ));
    }
}

/// `NoWeeklyEarningRuleConfig` — no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWeeklyEarningRuleConfig;

impl RuleConfig for NoWeeklyEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoWdf
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }
}

/// `WeeklyHrsFixedRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyHrsFixedRuleConfig;

fn validate_fixed_hours(p: &RuleParams, name: &str) -> ValidationResults {
    let mut r = ValidationResults::new();
    let (dollars, min, max) = (
        p.double_at(DOLLARS_PROP),
        p.double_at(MIN_HRS_PROP),
        p.double_at(MAX_HRS_PROP),
    );
    if min <= 0.0 {
        r.push(format!("{name}: Min Hours must be greater than zero"));
    }
    if max <= min {
        r.push(format!("{name}: Max Hours must be greater than Min Hours"));
    }
    if dollars <= 0.0 {
        r.push(format!("{name}: Dollars must be greater than zero"));
    }
    earning_selected(p, EARNING, &mut r, name);
    r
}

impl RuleConfig for WeeklyHrsFixedRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WklyHrsFixedWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DOLLARS_PROP, "10.0");
        p.set(EARNING, "0");
        p.set(MIN_HRS_PROP, "20");
        p.set(MAX_HRS_PROP, "40");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        validate_fixed_hours(params, "Weekly Hours Fixed")
    }
}

/// `DOWHrsFixedRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DOWHrsFixedRuleConfig;

impl RuleConfig for DOWHrsFixedRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DowHrsFixedWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DOLLARS_PROP, "10.0");
        p.set(EARNING, "0");
        p.set(MIN_HRS_PROP, "4");
        p.set(MAX_HRS_PROP, "24");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(DOW_PROP, "[]");
        p
    }
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        validate_fixed_hours(params, "Day of Week Hours Fixed")
    }
}

/// `DOWWeeklyHrsRateFactorRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DOWWeeklyHrsRateFactorRuleConfig;

impl RuleConfig for DOWWeeklyHrsRateFactorRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DowWeeklyHrsFactorWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(FACTOR_PROP, "0.5");
        p.set(DOW_EARNING_PROP, "0");
        p.set(DOW_PROP, "1");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Day of Week Weekly Hours Rate Factor";
        let mut r = ValidationResults::new();
        if p.double_at(FACTOR_PROP) <= 0.0 {
            r.push(format!("{name}: Factor must be greater than zero"));
        }
        let dow = p.int_at(DOW_PROP);
        if !(1..=7).contains(&dow) {
            r.push(format!("{name}: Day of Week must be selected"));
        }
        earning_selected(p, DOW_EARNING_PROP, &mut r, name);
        r
    }
}

/// `DailySpreadRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailySpreadRuleConfig;

impl RuleConfig for DailySpreadRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DailySpreadWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(HOURS, "1.0");
        p.set(EARNING, "0");
        p.set(MIN_HRS_SPREAD, "10");
        p.set(USE_MIN_WAGE, "true");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Daily Spread";
        let mut r = ValidationResults::new();
        at_least_a_cent(p, HOURS, "Hours", &mut r, name);
        at_least_a_cent(p, MIN_HRS_SPREAD, "Minimum Spread Hours", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `SalariedMinWageRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SalariedMinWageRuleConfig;

impl RuleConfig for SalariedMinWageRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::SalaryMinWageWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(MIN_WAGE_PROP, "6.08");
        p.set(EARNING, "0");
        p.set(HOURLY_EARNINGS, "[]");
        p.set(DOLLARS_EARNINGS, "[]");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Salaried Minimum Wage";
        let mut r = ValidationResults::new();
        if p.double_at(MIN_WAGE_PROP) <= 0.0 {
            r.push(format!("{name}: Minimum Wage must be greater than zero"));
        }
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `HoursThresholdFixedWeeklyRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HoursThresholdFixedWeeklyRuleConfig;

impl RuleConfig for HoursThresholdFixedWeeklyRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HrsThreshFixedWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DOLLARS_PROP, "10.0");
        p.set(EARNING, "0");
        p.set(MIN_HRS_PROP, "6.0");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(MAX_OCCURRENCES, "5");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Hours Threshold Fixed Weekly";
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, name);
        at_least_a_cent(p, DOLLARS_PROP, "Dollars", &mut r, name);
        at_least_a_cent(p, MIN_HRS_PROP, "Min Hours", &mut r, name);
        if p.int_at(MAX_OCCURRENCES) <= 0 {
            r.push(format!("{name}: Max Occurrences must be greater than zero"));
        }
        r
    }
}

/// `DailyWeeklyGuaranteedHoursRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailyWeeklyGuaranteedHoursRuleConfig;

impl RuleConfig for DailyWeeklyGuaranteedHoursRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DailyWeeklyGuaranteedHrsWdf
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(MIN_DAILY_HRS, "8");
        p.set(MAX_DAILY_HRS, "8");
        p.set(MIN_WEEKLY_HRS, "40");
        p.set(MAX_WEEKLY_HRS, "40");
        p.set(RATE_FACTOR, "1");
        p.set(ADDITIONAL_HOURS, "[]");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Daily Weekly Guaranteed Hours";
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, name);
        for (key, label) in [
            (MIN_DAILY_HRS, "Min Daily Hours"),
            (MAX_DAILY_HRS, "Max Daily Hours"),
            (MIN_WEEKLY_HRS, "Min Weekly Hours"),
            (MAX_WEEKLY_HRS, "Max Weekly Hours"),
        ] {
            if !(0.0..=999.0).contains(&p.double_at(key)) {
                r.push(format!("{name}: {label} must be in the range [0, 999]"));
            }
        }
        if p.double_at(MAX_DAILY_HRS) < p.double_at(MIN_DAILY_HRS) {
            r.push(format!(
                "{name}: Max Daily Hours must be greater than or equal to Min Daily Hours."
            ));
        }
        if p.double_at(MAX_WEEKLY_HRS) < p.double_at(MIN_WEEKLY_HRS) {
            r.push(format!(
                "{name}: Max Weekly Hours must be greater than or equal to Min Weekly Hours."
            ));
        }
        if p.double_at(RATE_FACTOR) < 0.0 {
            r.push(format!(
                "{name}: Rate Factor must be greater than or equal to 0"
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
    fn defaults() {
        let g = DailyWeeklyGuaranteedHoursRuleConfig.default_values();
        assert_eq!(g.get(MIN_DAILY_HRS), Some("8"));
        assert_eq!(g.get(MAX_WEEKLY_HRS), Some("40"));
        assert_eq!(g.get(ADDITIONAL_HOURS), Some("[]"));
        let s = DailySpreadRuleConfig.default_values();
        assert_eq!(s.get(HOURS), Some("1.0"));
        assert_eq!(s.get(MIN_HRS_SPREAD), Some("10"));
        assert_eq!(s.get(USE_MIN_WAGE), Some("true"));
        let w = WeeklyHrsFixedRuleConfig.default_values();
        assert_eq!(
            (w.get(MIN_HRS_PROP), w.get(MAX_HRS_PROP)),
            (Some("20"), Some("40"))
        );
        let d = DOWHrsFixedRuleConfig.default_values();
        assert_eq!(
            (d.get(MIN_HRS_PROP), d.get(MAX_HRS_PROP), d.get(DOW_PROP)),
            (Some("4"), Some("24"), Some("[]"))
        );
        assert_eq!(
            DOWWeeklyHrsRateFactorRuleConfig
                .default_values()
                .get(FACTOR_PROP),
            Some("0.5")
        );
        assert_eq!(
            HoursThresholdFixedWeeklyRuleConfig
                .default_values()
                .get(MAX_OCCURRENCES),
            Some("5")
        );
        assert_eq!(
            SalariedMinWageRuleConfig
                .default_values()
                .get(MIN_WAGE_PROP),
            Some("6.08")
        );
        assert!(NoWeeklyEarningRuleConfig.default_values().is_empty());
    }

    #[test]
    fn every_default_set_fails_only_for_the_unselected_earning() {
        let cases: Vec<(&dyn RuleConfig, usize)> = vec![
            (&WeeklyHrsFixedRuleConfig, 1),
            (&DOWHrsFixedRuleConfig, 1),
            (&DOWWeeklyHrsRateFactorRuleConfig, 1),
            (&DailySpreadRuleConfig, 1),
            (&SalariedMinWageRuleConfig, 1),
            (&HoursThresholdFixedWeeklyRuleConfig, 1),
            (&DailyWeeklyGuaranteedHoursRuleConfig, 1),
            (&NoWeeklyEarningRuleConfig, 0),
        ];
        for (config, expected) in cases {
            assert_eq!(
                config.validate(&config.default_values()).len(),
                expected,
                "{:?}",
                config.rule_class()
            );
        }
    }

    #[test]
    fn fixed_hours_configs_need_max_above_min() {
        let p = rule_params! { DOLLARS_PROP => "5", EARNING => "1", MIN_HRS_PROP => "10", MAX_HRS_PROP => "10" };
        assert_eq!(WeeklyHrsFixedRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn dow_must_be_a_weekday_number() {
        let p = rule_params! { FACTOR_PROP => "1", DOW_EARNING_PROP => "2", DOW_PROP => "8" };
        assert_eq!(DOWWeeklyHrsRateFactorRuleConfig.validate(&p).len(), 1);
        let p = rule_params! { FACTOR_PROP => "1", DOW_EARNING_PROP => "2", DOW_PROP => "7" };
        assert!(DOWWeeklyHrsRateFactorRuleConfig.validate(&p).is_empty());
    }

    #[test]
    fn guaranteed_hours_ranges_and_ordering() {
        let mut p = DailyWeeklyGuaranteedHoursRuleConfig.default_values();
        p.set(EARNING, "1");
        assert!(DailyWeeklyGuaranteedHoursRuleConfig.validate(&p).is_empty());
        p.set(MAX_DAILY_HRS, "1000");
        assert_eq!(DailyWeeklyGuaranteedHoursRuleConfig.validate(&p).len(), 1);
        p.set(MAX_DAILY_HRS, "4");
        assert_eq!(
            DailyWeeklyGuaranteedHoursRuleConfig.validate(&p).len(),
            1,
            "max < min"
        );
    }

    #[test]
    fn threshold_needs_positive_occurrences() {
        let mut p = HoursThresholdFixedWeeklyRuleConfig.default_values();
        p.set(EARNING, "1");
        assert!(HoursThresholdFixedWeeklyRuleConfig.validate(&p).is_empty());
        p.set(MAX_OCCURRENCES, "0");
        assert_eq!(HoursThresholdFixedWeeklyRuleConfig.validate(&p).len(), 1);
    }
}
