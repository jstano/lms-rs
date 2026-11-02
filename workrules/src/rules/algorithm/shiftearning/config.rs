//! Port of `com.unifocus.watson.common.labor.rules.algorithm.shiftearning.*RuleConfig`
//! — the nine configs slice 1 needs.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftearning/`.
//!
//! As in `dailyearning`/`weeklyearning`, the display names in validation
//! messages are placeholders for localized strings; the conditions are exact.
//!
//! # `TimeBetweenShiftsRuleConfig`'s keys are capitalized phrases
//!
//! `"Hours Between Shifts"`, `"Dollars"` and `"Earning"` — unlike every
//! camelCase key beside them, and reproduced exactly (cf. `MinBreakRuleConfig`).
//! Likewise `ShiftStartTimeFlatRuleConfig`'s `"Start Time"` and `"End Time"`,
//! and `MinimumHoursWorkedShiftDifferentialRuleConfig`'s `"MinimumWage"`.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

pub use crate::rules::algorithm::dailyearning::config::{EARNING, MIN_DAILY_HRS, RATE_FACTOR};

/// Keys shared by several of the configs.
pub const ACTUAL_PUNCH_PROP: &str = "actualPunchTime";
pub const DOLLARS_PROP: &str = "dollars";
pub const MIN_HRS_PROP: &str = "minHours";
pub const MAX_HRS_PROP: &str = "maxHours";
pub const DOW_PROP: &str = "dow";
pub const RATE_PROP: &str = "rate";
pub const FACTOR_PROP: &str = "factor";
pub const HOME_AS_BASE_PROP: &str = "homeAsBase";
pub const MAX_OCCURRENCES: &str = "maxOccurrences";
/// `TimeBetweenShiftsRuleConfig`.
pub const HOURS_BETWEEN_SHIFTS_PROP: &str = "Hours Between Shifts";
pub const TIME_BETWEEN_DOLLARS_PROP: &str = "Dollars";
pub const TIME_BETWEEN_EARNING_PROP: &str = "Earning";
/// `ShiftStartTimeFlatRuleConfig`.
pub const DIFF_RATE_PROP: &str = "diffRate";
pub const START_TIME_PROP: &str = "Start Time";
pub const END_TIME_PROP: &str = "End Time";
/// Slice 2: the break and window rules' keys.
pub const MIN_SHIFT_LENGTH: &str = "minShiftLength";
pub const MIN_BREAK_LENGTH: &str = "minBreakLength";
pub const HOURS_PER_PENALTY: &str = "hoursPerPenalty";
pub const CHECK_MIN_BREAK_PROP: &str = "checkMinBreak";
pub const BREAK_LIMIT_PROP: &str = "breakLimit";
pub const FIRST_BREAK_LIMIT: &str = "firstBreakLimit";
pub const FIRST_MIN_SHIFT_LENGTH: &str = "firstMinShiftLength";
pub const SEC_BREAK_LIMIT: &str = "secBreakLimit";
pub const SEC_MIN_SHIFT_LENGTH: &str = "secMinShiftLength";
pub const MAX_DAILY_PENALTY: &str = "maxDailyPenalty";
pub const EARLIEST_START_TIME_PROP: &str = "earliestStartTime";
pub const LATEST_START_TIME_PROP: &str = "latestStartTime";
pub const EARLIEST_END_TIME_PROP: &str = "earliestEndTime";
pub const LATEST_END_TIME_PROP: &str = "latestEndTime";
pub const EARNING_AMOUNT_PROP: &str = "earningAmount";
pub const PAY_ONLY_ONCE_PROP: &str = "payOnlyOnce";
pub const USE_ACTUAL_TIME_PROP: &str = "useActualTime";
/// Slice 3: the time-of-day and spread rules' keys.
pub const MIN_HRS_BETWEEN: &str = "minHrsBetween";
pub const MIN_HOURS_PAID: &str = "minAmount";
pub const MAX_HOURS_PAID: &str = "maxAmount";
pub const PCT_WAGE_PROP: &str = "pctWage";
pub const MIN_PCT_HRS_BETWEEN: &str = "pctHrsBetween";
pub const MAX_PCT_HRS_BETWEEN: &str = "maxPctHrsBetween";
pub const PAY_NET_HRS: &str = "payNetHrs";
pub const EARLY_START_TIME_PROP: &str = "earliestStartTime";
pub const LATE_START_TIME_PROP: &str = "latestStartTime";
pub const ROUNDING_OPTION: &str = "roundingOption";
pub const ROUND_TO: &str = "roundTo";
pub const MAX_HRS_PAID: &str = "maxHrsPaid";
pub const HOURS: &str = "hours";
pub const MIN_HRS_SPREAD: &str = "minHours";
pub const USE_MIN_WAGE: &str = "useMinWage";
/// `DefaultPropertyFactory.MINIMUM_WAGE_PROP`.
pub const MINIMUM_WAGE_PROP: &str = "MinimumWage";

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

fn positive(p: &RuleParams, key: &str, label: &str, r: &mut ValidationResults, name: &str) {
    if p.double_at(key) <= 0.0 {
        r.push(format!("{name}: {label} must be greater than zero"));
    }
}

/// `NoEarningRuleConfig`. Its Java class declares four unused key constants and
/// no defaults or validation.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEarningRuleConfig;

impl RuleConfig for NoEarningRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoDif
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }
}

/// `HoursThresholdFixedRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HoursThresholdFixedRuleConfig;

impl RuleConfig for HoursThresholdFixedRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HrsThreshFixedDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DOLLARS_PROP, "10.0");
        p.set(EARNING, "0");
        p.set(MIN_HRS_PROP, "6.0");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Hours Threshold Fixed";
        let mut r = ValidationResults::new();
        positive(p, MIN_HRS_PROP, "Min Hours", &mut r, name);
        positive(p, DOLLARS_PROP, "Dollars", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `DOWFlatRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DOWFlatRuleConfig;

impl RuleConfig for DOWFlatRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DowFlatDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(RATE_PROP, "5.00");
        p.set(EARNING, "0");
        p.set(DOW_PROP, "1");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Day of Week Flat";
        let mut r = ValidationResults::new();
        if p.double_at(RATE_PROP) < 0.0 {
            r.push(format!("{name}: rate must be greater than zero"));
        }
        // Unlike its siblings this accepts 0 as "a day" (`dow < 0 || dow > 7`).
        if !(0..=7).contains(&p.int_at(DOW_PROP)) {
            r.push(format!("{name}: Day of Week must be selected."));
        }
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `DOWRateFactorRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DOWRateFactorRuleConfig;

impl RuleConfig for DOWRateFactorRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DowFactorDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(FACTOR_PROP, "0.5");
        p.set(EARNING, "0");
        p.set(DOW_PROP, "1");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(HOME_AS_BASE_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Day of Week Rate Factor";
        let mut r = ValidationResults::new();
        if p.double_at(FACTOR_PROP) < 0.0 {
            r.push(format!("{name}: factor must be greater than zero"));
        }
        if !(0..=7).contains(&p.int_at(DOW_PROP)) {
            r.push(format!("{name}: Day of Week must be selected."));
        }
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `WeeklyJobHrsFixedRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyJobHrsFixedRuleConfig;

impl RuleConfig for WeeklyJobHrsFixedRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WklyJobHrsFixedDif
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
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Weekly Job Hours Fixed";
        let mut r = ValidationResults::new();
        positive(p, MIN_HRS_PROP, "Min Hours", &mut r, name);
        if p.double_at(MAX_HRS_PROP) <= p.double_at(MIN_HRS_PROP) {
            r.push(format!("{name}: Max Hours must be greater than Min Hours"));
        }
        positive(p, DOLLARS_PROP, "Dollars", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `HoursThresholdFixedWithMaxCapRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HoursThresholdFixedWithMaxCapRuleConfig;

impl RuleConfig for HoursThresholdFixedWithMaxCapRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HrsThreshFixedWithMaxCapDif
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
        let name = "Hours Threshold Fixed With Max Cap";
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

/// `TimeBetweenShiftsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeBetweenShiftsRuleConfig;

impl RuleConfig for TimeBetweenShiftsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::TimeBetweenShiftsDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(HOURS_BETWEEN_SHIFTS_PROP, "8.00");
        p.set(TIME_BETWEEN_DOLLARS_PROP, "1.00");
        p.set(TIME_BETWEEN_EARNING_PROP, "0");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Time Between Shifts";
        let mut r = ValidationResults::new();
        positive(
            p,
            HOURS_BETWEEN_SHIFTS_PROP,
            HOURS_BETWEEN_SHIFTS_PROP,
            &mut r,
            name,
        );
        positive(
            p,
            TIME_BETWEEN_DOLLARS_PROP,
            TIME_BETWEEN_DOLLARS_PROP,
            &mut r,
            name,
        );
        earning_selected(p, TIME_BETWEEN_EARNING_PROP, &mut r, name);
        r
    }
}

/// `MinimumHoursWorkedShiftDifferentialRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinimumHoursWorkedShiftDifferentialRuleConfig;

impl RuleConfig for MinimumHoursWorkedShiftDifferentialRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinimumHoursWorkedShiftDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(MINIMUM_WAGE_PROP, "false");
        p.set(EARNING, "0");
        p.set(RATE_FACTOR, "1");
        p.set(MIN_DAILY_HRS, "3");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Minimum Hours Worked Shift Differential";
        let mut r = ValidationResults::new();
        at_least_a_cent(p, RATE_FACTOR, "Rate Factor", &mut r, name);
        at_least_a_cent(p, MIN_DAILY_HRS, "Min Daily Hours", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `ShiftStartTimeFlatRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftStartTimeFlatRuleConfig;

impl RuleConfig for ShiftStartTimeFlatRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ShiftStartFlatDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DIFF_RATE_PROP, "0.50");
        p.set(EARNING, "0");
        p.set(START_TIME_PROP, "22:00:00");
        p.set(END_TIME_PROP, "06:00:00");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Shift Start Time Flat";
        let mut r = ValidationResults::new();
        positive(p, DIFF_RATE_PROP, "Differential Rate", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

/// `PaidBreakFixedHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaidBreakFixedHrsRuleConfig;

impl RuleConfig for PaidBreakFixedHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PdBreakFixedHrsDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(MIN_SHIFT_LENGTH, "6.0");
        p.set(MIN_BREAK_LENGTH, "0.5");
        p.set(HOURS_PER_PENALTY, "0.5");
        p.set(EARNING, "0");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(CHECK_MIN_BREAK_PROP, "false");
        p.set(BREAK_LIMIT_PROP, "5.0");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Paid Break Fixed Hours";
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, name);
        if p.double_at(HOURS_PER_PENALTY) <= 0.0 {
            r.push("Hours Per Penalty must be greater than 0.".to_string());
        }
        for (key, label) in [
            (MIN_SHIFT_LENGTH, "Min Shift Length"),
            (MIN_BREAK_LENGTH, "Min Break Length"),
            (BREAK_LIMIT_PROP, "Break Limit"),
        ] {
            if p.double_at(key) < 0.0 {
                r.push(format!("{label} must be greater than or equal to 0."));
            }
        }
        r
    }
}

/// `BreakPeriodPenaltyHoursRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BreakPeriodPenaltyHoursRuleConfig;

impl RuleConfig for BreakPeriodPenaltyHoursRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::BreakPerPenaltyHrsDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(MIN_SHIFT_LENGTH, "5.0");
        p.set(MIN_BREAK_LENGTH, "0.5");
        p.set(EARNING, "0");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, "Break Period Penalty Hours");
        r
    }
}

/// `BreakPeriodPenaltyRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BreakPeriodPenaltyRuleConfig;

impl RuleConfig for BreakPeriodPenaltyRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::BreakPeriodPenaltyDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(FIRST_BREAK_LIMIT, "5.0");
        p.set(FIRST_MIN_SHIFT_LENGTH, "5.0");
        p.set(MIN_BREAK_LENGTH, "0.5");
        p.set(SEC_BREAK_LIMIT, "10.0");
        p.set(SEC_MIN_SHIFT_LENGTH, "10.0");
        p.set(MAX_DAILY_PENALTY, "1");
        p.set(HOURS_PER_PENALTY, "1.0");
        p.set(EARNING, "0");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, "Break Period Penalty");
        r
    }
}

/// `StartEndBetweenPremiumRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct StartEndBetweenPremiumRuleConfig;

impl RuleConfig for StartEndBetweenPremiumRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::StartEndBetweenDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(EARNING, "0");
        p.set(EARLIEST_START_TIME_PROP, "00:00:00");
        p.set(LATEST_START_TIME_PROP, "05:30:00");
        p.set(EARLIEST_END_TIME_PROP, "00:00:00");
        p.set(LATEST_END_TIME_PROP, "05:30:00");
        p.set(EARNING_AMOUNT_PROP, "350.00");
        p.set(PAY_ONLY_ONCE_PROP, "true");
        p.set(USE_ACTUAL_TIME_PROP, "true");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Start Or End Between Premium";
        let mut r = ValidationResults::new();
        earning_selected(p, EARNING, &mut r, name);
        if p.double_at(EARNING_AMOUNT_PROP) <= 0.0 {
            r.push(format!("{name}: Earning amount must be greater than 0"));
        }
        r
    }
}

/// `TimeOfDayDifferentialRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeOfDayDifferentialRateRuleConfig;

impl RuleConfig for TimeOfDayDifferentialRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::TimeOfDayDifRateDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DIFF_RATE_PROP, "5.75");
        p.set(EARNING, "0");
        p.set(START_TIME_PROP, "22:00:00");
        p.set(END_TIME_PROP, "06:00:00");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(MIN_HRS_BETWEEN, "0.0");
        p.set(DOW_PROP, "[1,2,3,4,5,6,7]");
        p.set(MIN_HOURS_PAID, "0.0");
        p.set(MAX_HOURS_PAID, "99");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Time Of Day Differential Rate";
        let mut r = ValidationResults::new();
        if p.double_at(DIFF_RATE_PROP) < 0.0 {
            r.push(format!(
                "{name}: Differential Rate must be greater than or equal 0"
            ));
        }
        earning_selected(p, EARNING, &mut r, name);
        if p.double_at(MIN_HRS_BETWEEN) < 0.0 {
            r.push(format!(
                "{name}: Min Hrs Between must be greater than or equal 0"
            ));
        }
        if p.double_at(MIN_HOURS_PAID) < 0.0 {
            r.push(format!(
                "{name}: Min Hours Paid must be greater than or equal 0"
            ));
        }
        if p.double_at(MAX_HOURS_PAID) < p.double_at(MIN_HOURS_PAID) {
            r.push(format!(
                "{name}: Max Hours Paid must be greater than or equal Min Hours Paid"
            ));
        }
        r
    }
}

/// `TimeOfDayPctDifferentialRateRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeOfDayPctDifferentialRateRuleConfig;

impl RuleConfig for TimeOfDayPctDifferentialRateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::TodPctDifRateDif
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(DIFF_RATE_PROP, "0");
        p.set(PCT_WAGE_PROP, "1.00");
        p.set(EARNING, "0");
        p.set(START_TIME_PROP, "22:00:00");
        p.set(END_TIME_PROP, "06:00:00");
        p.set(ACTUAL_PUNCH_PROP, "true");
        p.set(MIN_HRS_BETWEEN, "0.0");
        p.set(MIN_PCT_HRS_BETWEEN, "0.0");
        p.set(MAX_PCT_HRS_BETWEEN, "1.0");
        p.set(PAY_NET_HRS, "false");
        p.set(EARLY_START_TIME_PROP, "00:00:00");
        p.set(LATE_START_TIME_PROP, "23:59:00");
        p.set(DOW_PROP, "[1,2,3,4,5,6,7]");
        p.set(ROUNDING_OPTION, "NONE");
        p.set(ROUND_TO, "5");
        p.set(MAX_HRS_PAID, "99");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let name = "Time Of Day Pct Differential Rate";
        let mut r = ValidationResults::new();
        for (key, label) in [
            (PCT_WAGE_PROP, "Pct of Job Rate"),
            (MIN_PCT_HRS_BETWEEN, "Min. Pct. Hrs. Between"),
            (MAX_PCT_HRS_BETWEEN, "Max. Pct. Hrs. Between"),
        ] {
            if !(0.0..=1.0).contains(&p.double_at(key)) {
                r.push(format!("{name}: {label} must be between 0 and 1"));
            }
        }
        for (key, label) in [
            (DIFF_RATE_PROP, "Differential Rate"),
            (MIN_HRS_BETWEEN, "Min. Hrs. Between"),
        ] {
            if p.double_at(key) < 0.0 {
                r.push(format!("{name}: {label} must be greater than or equal 0"));
            }
        }
        earning_selected(p, EARNING, &mut r, name);
        if p.double_at(MAX_PCT_HRS_BETWEEN) < p.double_at(MIN_PCT_HRS_BETWEEN) {
            r.push(format!("{name}: Max. Pct. Hrs. Between must be greater than or equal to Min. Pct. Hrs. Between"));
        }
        let round_to = p.int_at(ROUND_TO);
        if round_to <= 0 || round_to > 60 {
            r.push(format!(
                "{name}: Round To must be greater than 0 and less than or equal to 60"
            ));
        }
        if p.double_at(MAX_HRS_PAID) <= 0.0 {
            r.push(format!("{name}: Max Hours Paid must be greater than 0"));
        }
        r
    }
}

/// `DailySpreadShiftDifferentialRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailySpreadShiftDifferentialRuleConfig;

impl RuleConfig for DailySpreadShiftDifferentialRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DailySpreadShiftDif
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
        let name = "Daily Spread Shift Differential";
        let mut r = ValidationResults::new();
        at_least_a_cent(p, HOURS, "Hours", &mut r, name);
        at_least_a_cent(p, MIN_HRS_SPREAD, "Minimum Spread Hours", &mut r, name);
        earning_selected(p, EARNING, &mut r, name);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn defaults() {
        let g = |c: &dyn RuleConfig, k: &str| c.default_values().get(k).map(str::to_owned);
        assert_eq!(
            g(&HoursThresholdFixedRuleConfig, MIN_HRS_PROP).as_deref(),
            Some("6.0")
        );
        assert_eq!(g(&DOWFlatRuleConfig, RATE_PROP).as_deref(), Some("5.00"));
        assert_eq!(
            g(&DOWRateFactorRuleConfig, HOME_AS_BASE_PROP).as_deref(),
            Some("true")
        );
        assert_eq!(
            g(&WeeklyJobHrsFixedRuleConfig, MAX_HRS_PROP).as_deref(),
            Some("40")
        );
        assert_eq!(
            g(&HoursThresholdFixedWithMaxCapRuleConfig, MAX_OCCURRENCES).as_deref(),
            Some("5")
        );
        assert_eq!(
            g(&TimeBetweenShiftsRuleConfig, HOURS_BETWEEN_SHIFTS_PROP).as_deref(),
            Some("8.00")
        );
        assert_eq!(
            g(
                &MinimumHoursWorkedShiftDifferentialRuleConfig,
                MINIMUM_WAGE_PROP
            )
            .as_deref(),
            Some("false")
        );
        assert_eq!(
            g(&ShiftStartTimeFlatRuleConfig, START_TIME_PROP).as_deref(),
            Some("22:00:00")
        );
        assert_eq!(
            g(&ShiftStartTimeFlatRuleConfig, END_TIME_PROP).as_deref(),
            Some("06:00:00")
        );
        assert!(NoEarningRuleConfig.default_values().is_empty());
    }

    #[test]
    fn every_default_set_fails_only_for_the_unselected_earning() {
        let cases: Vec<(&dyn RuleConfig, usize)> = vec![
            (&HoursThresholdFixedRuleConfig, 1),
            (&DOWFlatRuleConfig, 1),
            (&DOWRateFactorRuleConfig, 1),
            (&WeeklyJobHrsFixedRuleConfig, 1),
            (&HoursThresholdFixedWithMaxCapRuleConfig, 1),
            (&TimeBetweenShiftsRuleConfig, 1),
            (&MinimumHoursWorkedShiftDifferentialRuleConfig, 1),
            (&ShiftStartTimeFlatRuleConfig, 1),
            (&NoEarningRuleConfig, 0),
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
    fn dow_accepts_zero_through_seven_unlike_the_weekly_configs() {
        for dow in ["0", "7"] {
            let p = rule_params! { RATE_PROP => "1", EARNING => "1", DOW_PROP => dow };
            assert!(DOWFlatRuleConfig.validate(&p).is_empty(), "{dow}");
        }
        let p = rule_params! { RATE_PROP => "1", EARNING => "1", DOW_PROP => "8" };
        assert_eq!(DOWFlatRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn weekly_job_hours_needs_max_above_min() {
        let p = rule_params! { DOLLARS_PROP => "5", EARNING => "1", MIN_HRS_PROP => "10", MAX_HRS_PROP => "10" };
        assert_eq!(WeeklyJobHrsFixedRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn slice_two_defaults_and_validation() {
        let g = |c: &dyn RuleConfig, k: &str| c.default_values().get(k).map(str::to_owned);
        assert_eq!(
            g(&PaidBreakFixedHrsRuleConfig, HOURS_PER_PENALTY).as_deref(),
            Some("0.5")
        );
        assert_eq!(
            g(&PaidBreakFixedHrsRuleConfig, BREAK_LIMIT_PROP).as_deref(),
            Some("5.0")
        );
        assert_eq!(
            g(&BreakPeriodPenaltyHoursRuleConfig, MIN_SHIFT_LENGTH).as_deref(),
            Some("5.0")
        );
        assert_eq!(
            g(&BreakPeriodPenaltyRuleConfig, SEC_BREAK_LIMIT).as_deref(),
            Some("10.0")
        );
        assert_eq!(
            g(&BreakPeriodPenaltyRuleConfig, MAX_DAILY_PENALTY).as_deref(),
            Some("1")
        );
        assert_eq!(
            g(&StartEndBetweenPremiumRuleConfig, EARNING_AMOUNT_PROP).as_deref(),
            Some("350.00")
        );
        assert_eq!(
            g(&StartEndBetweenPremiumRuleConfig, LATEST_END_TIME_PROP).as_deref(),
            Some("05:30:00")
        );
        for c in [
            &PaidBreakFixedHrsRuleConfig as &dyn RuleConfig,
            &BreakPeriodPenaltyHoursRuleConfig,
            &BreakPeriodPenaltyRuleConfig,
            &StartEndBetweenPremiumRuleConfig,
        ] {
            assert_eq!(
                c.validate(&c.default_values()).len(),
                1,
                "{:?}: only the earning",
                c.rule_class()
            );
        }
        let p = rule_params! { EARNING => "1", HOURS_PER_PENALTY => "0", MIN_SHIFT_LENGTH => "-1", MIN_BREAK_LENGTH => "-1", BREAK_LIMIT_PROP => "-1" };
        assert_eq!(PaidBreakFixedHrsRuleConfig.validate(&p).len(), 4);
    }

    #[test]
    fn slice_three_defaults_and_validation() {
        let g = |c: &dyn RuleConfig, k: &str| c.default_values().get(k).map(str::to_owned);
        assert_eq!(
            g(&TimeOfDayDifferentialRateRuleConfig, DIFF_RATE_PROP).as_deref(),
            Some("5.75")
        );
        assert_eq!(
            g(&TimeOfDayDifferentialRateRuleConfig, MAX_HOURS_PAID).as_deref(),
            Some("99")
        );
        assert_eq!(
            g(
                &TimeOfDayPctDifferentialRateRuleConfig,
                LATE_START_TIME_PROP
            )
            .as_deref(),
            Some("23:59:00")
        );
        assert_eq!(
            g(&TimeOfDayPctDifferentialRateRuleConfig, ROUNDING_OPTION).as_deref(),
            Some("NONE")
        );
        assert_eq!(
            g(&TimeOfDayPctDifferentialRateRuleConfig, ROUND_TO).as_deref(),
            Some("5")
        );
        assert_eq!(
            g(&TimeOfDayPctDifferentialRateRuleConfig, PCT_WAGE_PROP).as_deref(),
            Some("1.00")
        );
        assert_eq!(
            g(&DailySpreadShiftDifferentialRuleConfig, MIN_HRS_SPREAD).as_deref(),
            Some("10")
        );
        for c in [
            &TimeOfDayDifferentialRateRuleConfig as &dyn RuleConfig,
            &TimeOfDayPctDifferentialRateRuleConfig,
            &DailySpreadShiftDifferentialRuleConfig,
        ] {
            assert_eq!(
                c.validate(&c.default_values()).len(),
                1,
                "{:?}: only the earning",
                c.rule_class()
            );
        }
    }

    #[test]
    fn pct_config_ranges() {
        let mut p = TimeOfDayPctDifferentialRateRuleConfig.default_values();
        p.set(EARNING, "1");
        assert!(
            TimeOfDayPctDifferentialRateRuleConfig
                .validate(&p)
                .is_empty()
        );
        p.set(PCT_WAGE_PROP, "1.01");
        p.set(ROUND_TO, "61");
        p.set(MAX_HRS_PAID, "0");
        assert_eq!(TimeOfDayPctDifferentialRateRuleConfig.validate(&p).len(), 3);
        p.set(PCT_WAGE_PROP, "1");
        p.set(ROUND_TO, "60");
        p.set(MAX_HRS_PAID, "1");
        p.set(MIN_PCT_HRS_BETWEEN, "0.8");
        p.set(MAX_PCT_HRS_BETWEEN, "0.5");
        assert_eq!(TimeOfDayPctDifferentialRateRuleConfig.validate(&p).len(), 1);
    }

    #[test]
    fn time_of_day_paid_hours_must_be_ordered() {
        let mut p = TimeOfDayDifferentialRateRuleConfig.default_values();
        p.set(EARNING, "1");
        p.set(MIN_HOURS_PAID, "5");
        p.set(MAX_HOURS_PAID, "4");
        assert_eq!(TimeOfDayDifferentialRateRuleConfig.validate(&p).len(), 1);
    }
}
