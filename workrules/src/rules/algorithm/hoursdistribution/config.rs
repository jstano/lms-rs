//! Port of
//! `com.unifocus.watson.common.labor.rules.algorithm.hoursdistribution.*RuleConfig`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/hoursdistribution/`.
//!
//! Twenty-four Java classes for 19 catalogue entries. The ones that are not
//! rules are `HoursDistributionRuleConfig` (the abstract base every other
//! extends), `HoursDistributionRuleWithPayMappingsConfig` (an intermediate base
//! for the rules that map earning types onto distribution buckets),
//! `HoursDistributionConfigConstants`, `ContractHrsRuleConfig` and
//! `ShiftDifferenceOTRuleConfig`.
//!
//! As in `punchrounding`, only the runtime half comes across — keys, defaults
//! and the constraints `validateProperties` expresses. The l2fprod `Property`
//! construction is configuration UI.
//!
//! # The base contributes nothing
//!
//! `HoursDistributionRuleConfig.getDefaultValues()` returns an empty map and
//! `validateProperties` an empty result, unlike `punchrounding`'s base which
//! carries the eight manual/clock gates. So each config here starts from
//! nothing and every parameter a rule reads is its own.
//!
//! Configs arrive with their rules; this holds the ones ported so far.

use crate::rule_params;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// Which holiday types pay double time. `HolidayDTHrsRuleConfig.HOLIDAY_TYPES_PROP`.
///
/// A JSON array of `HolidayType` ids — see
/// [`json_ids`](crate::common::json_ids).
pub const HOLIDAY_TYPES_PROP: &str = "holidayTypes";

/// Hours before weekly overtime starts. `WEEKLY_LIMIT_PROP`, spelled
/// `weeklyOtLimit`, and shared by several of the weekly rules.
pub const WEEKLY_LIMIT_PROP: &str = "weeklyOtLimit";

/// `RegHrsOnlyRuleConfig` — no parameters at all, for a rule that does nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegHrsOnlyRuleConfig;

impl RuleConfig for RegHrsOnlyRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::RegOnlyHdr
    }

    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// `HolidayDTHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HolidayDTHrsRuleConfig;

impl RuleConfig for HolidayDTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::HolidayDtHdr
    }

    /// `holidayTypes` defaults to the empty array, so an unconfigured rule
    /// item pays double time on nothing.
    fn default_values(&self) -> RuleParams {
        rule_params! { HOLIDAY_TYPES_PROP => "[]" }
    }
}

/// Whether the FLSA 7(i) commission exemption is checked.
/// `WeeklyOTHrsRuleConfig.CHECK_7I`.
pub const CHECK_7I: &str = "check7I";

/// Hourly earning types already paid as overtime, as a JSON id array.
/// `WeeklyOTHrsRuleConfig.OT_HOURS_EARNINGS_PROP`.
pub const OT_HOURS_EARNINGS_PROP: &str = "otHoursEarnings";

/// Whether only hours in the home job's department count.
/// `PerMonthOTHrsRuleConfig.HOME_DEPT_ONLY`.
pub const HOME_DEPT_ONLY: &str = "homeDeptOnly";
/// Earning types whose hours count toward the monthly contract, as a JSON id
/// array. `PerMonthOTHrsRuleConfig.EARNING_TYPES`.
pub const EARNING_TYPES: &str = "earningTypes";

/// The twelve month parameter keys, in `com.unifocus.tbx.core.Month`'s own
/// order and spelling — the enum constant names, not the full month names.
pub const MONTH_KEYS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The parameter key holding a date's monthly contract hours.
/// `Month.fromMonthIndex(date.getMonthOfYear()).name()`.
pub fn month_key(date: joda_rs::LocalDate) -> &'static str {
    MONTH_KEYS[(date.month_value() - 1) as usize]
}

/// `PerMonthOTHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PerMonthOTHrsRuleConfig;

impl RuleConfig for PerMonthOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PerMonthOtHdr
    }

    /// Every month defaults to 160 hours.
    fn default_values(&self) -> RuleParams {
        let mut params = rule_params! {
            HOME_DEPT_ONLY => "false",
            EARNING_TYPES => "[]",
        };
        for key in MONTH_KEYS {
            params.set(key, "160.0");
        }
        params
    }

    /// Each month's contract must be above zero.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        MONTH_KEYS
            .into_iter()
            .filter(|key| params.double_at(key) <= 0.0)
            .map(|key| format!("{key}: res_gtZeroRuleValidation"))
            .collect()
    }
}

/// Days worked in the week at which overtime starts.
/// `WORKED_DAY_OT_LIMIT_PROP`.
pub const WORKED_DAY_OT_LIMIT_PROP: &str = "workedDayOtLimit";
/// Days worked in the week at which double time starts.
/// `WORKED_DAY_DT_LIMIT_PROP`.
pub const WORKED_DAY_DT_LIMIT_PROP: &str = "workedDayDtLimit";

/// `DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig;

impl RuleConfig for DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::Dw6ot7dtNcsHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            DAILY_OT_LIMIT_PROP => "8.0",
            WEEKLY_LIMIT_PROP => "40.0",
            WORKED_DAY_OT_LIMIT_PROP => "6",
            WORKED_DAY_DT_LIMIT_PROP => "7",
        }
    }
}

/// How many weeks the rolling window spans.
/// `RollingXWeeksOTHrsRuleConfig.WEEKS`.
pub const WEEKS: &str = "weeks";

/// `RollingXWeeksOTHrsRuleConfig`.
///
/// Its limit parameter is spelled `weeklyOtLimit` — the same key three other
/// rules use — even though the limit is over `weeks` weeks rather than one.
/// `ROLLING_OT_LIMIT` is only the Java constant's name.
#[derive(Debug, Clone, Copy, Default)]
pub struct RollingXWeeksOTHrsRuleConfig;

impl RuleConfig for RollingXWeeksOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::RollingXWeeksOtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            WEEKLY_LIMIT_PROP => "132.0",
            WEEKS => "3",
        }
    }
}

/// Whether double time is paid at all. `PAY_DT_PROP`.
pub const PAY_DT_PROP: &str = "payDT";
/// Hours in a day before overtime starts. `DAILY_OT_LIMIT_PROP`.
pub const DAILY_OT_LIMIT_PROP: &str = "dailyOtLimit";
/// Hours in a day before double time starts. `DAILY_DT_LIMIT_PROP`.
pub const DAILY_DT_LIMIT_PROP: &str = "dailyDtLimit";
/// Consecutive days worked before the daily limits stop applying.
/// `CONSEC_DAY_LIMIT`.
pub const CONSEC_DAY_LIMIT: &str = "consecDayLimit";
/// Whether the consecutive-day count is confined to this work week.
/// `CONSEC_DAYS_IN_WEEK`.
pub const CONSEC_DAYS_IN_WEEK: &str = "consecDaysInWeek";
/// The consecutive-day count's wrap point, less one. `MAX_CONSEC_DAYS_PD`.
pub const MAX_CONSEC_DAYS_PD: &str = "maxConsecDaysPd";
/// `BOTH_CONSECUTIVE_AND_WEEKLY_OT` — see the finding on
/// [`WeeklyAccumulator`](super::weekly_accumulator).
pub const BOTH_CONSECUTIVE_AND_WEEKLY_OT: &str = "bothConsecutiveAndWeeklyOt";
/// Whether every overtime hour is upgraded to double time afterwards.
/// `CONVERT_ALL_OT_TO_DT`.
pub const CONVERT_ALL_OT_TO_DT: &str = "convertAllOtToDT";
/// Which weekly-overtime formula to use.
/// `PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT`.
pub const PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT: &str = "premiumHoursCountTowardsWeeklyOT";

/// `CaliforniaExtendedOTHrsRuleConfig`.
///
/// # `premiumHoursCountTowardsWeeklyOT` defaults to `true`, not `false`
///
/// `getDefaultValues()` puts that key **twice** — `"false"` on its sixth line
/// and `"true"` on its eleventh. The later `put` wins, so the effective default
/// is `true`, and a reader scanning the list top-down sees the wrong one. The
/// Java spec confirms it: `testWeekly45` only produces its stated numbers under
/// the `min(hours - limit, originalHours)` formula that the flag selects.
/// Pinned by `the_premium_formula_is_the_default_despite_the_earlier_put`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaliforniaExtendedOTHrsRuleConfig;

impl RuleConfig for CaliforniaExtendedOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::CaExtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            PAY_DT_PROP => "true",
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            CONSEC_DAY_LIMIT => "7",
            CONSEC_DAYS_IN_WEEK => "true",
            MAX_CONSEC_DAYS_PD => "999",
            BOTH_CONSECUTIVE_AND_WEEKLY_OT => "false",
            CONVERT_ALL_OT_TO_DT => "false",
            // Put twice in Java; this is the one that wins.
            PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT => "true",
        }
    }
}

/// The shortest shift eligible for scheduled-shift overtime, in hours.
/// `ScheduledShiftOTRuleConfig.MIN_SHIFT_LENGTH_PROP`.
pub const MIN_SHIFT_LENGTH_PROP: &str = "minShiftLength";

/// How many minutes either side of a shift's start a schedule may begin and
/// still match it. `ScheduledShiftOTRuleConfig.THRESHOLD_PROP`.
pub const THRESHOLD_PROP: &str = "threshold";

/// `ScheduledShiftOTRuleConfig`.
///
/// The only config in the family that does **not** build on
/// `HoursDistributionRuleConfig`'s empty maps — it overrides `getProperties`
/// and `getDefaultValues` outright rather than calling `super`. Since the base
/// contributes nothing either way, the result is the same.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScheduledShiftOTRuleConfig;

impl RuleConfig for ScheduledShiftOTRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ScheduledShiftOtHdr
    }

    /// A threshold of zero means only a schedule starting at the same instant
    /// matches.
    fn default_values(&self) -> RuleParams {
        rule_params! {
            MIN_SHIFT_LENGTH_PROP => "8",
            THRESHOLD_PROP => "0",
        }
    }

    /// `validateMinShiftLength` and `validateThreshold` — the minimum must be
    /// above zero, the threshold merely not below it.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        if params.double_at(MIN_SHIFT_LENGTH_PROP) <= 0.0 {
            results.push("The Min Shift Length property must be greater than zero".to_string());
        }
        if params.int_at(THRESHOLD_PROP) < 0 {
            results.push("The Threshold property cannot be less than zero".to_string());
        }

        results
    }
}

/// Hours before overtime starts within the pay period.
/// `PayPeriodOTHrsRuleConfig.PERIOD_OT_LIMIT_PROP`.
pub const PERIOD_OT_LIMIT_PROP: &str = "periodOtLimit";

/// `PayPeriodOTHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PayPeriodOTHrsRuleConfig;

impl RuleConfig for PayPeriodOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PayPeriodOtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! { PERIOD_OT_LIMIT_PROP => "80.0" }
    }

    /// Java checks the property is present before reading it, adding
    /// "Period OT Limit is required" when it is not, and then that it is above
    /// zero. A missing parameter cannot reach here — `fixMap` has already
    /// supplied the default — so only the second check has anything to do.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        if params.double_at(PERIOD_OT_LIMIT_PROP) <= 0.0 {
            results.push(format!(
                "{PERIOD_OT_LIMIT_PROP}: cannot be negative or zero"
            ));
        }

        results
    }
}

/// `WeeklyOTHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyOTHrsRuleConfig;

impl RuleConfig for WeeklyOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WeeklyOtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            WEEKLY_LIMIT_PROP => "40.0",
            CHECK_7I => "false",
            OT_HOURS_EARNINGS_PROP => "[]",
        }
    }

    /// `validateDoubleGreaterThanZero(results, properties.get(WEEKLY_LIMIT_PROP))`.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        if params.double_at(WEEKLY_LIMIT_PROP) <= 0.0 {
            results.push(format!("{WEEKLY_LIMIT_PROP}: res_gtZeroRuleValidation"));
        }

        results
    }
}

/// `WeeklyOTSecJobHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyOTSecJobHrsRuleConfig;

impl RuleConfig for WeeklyOTSecJobHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WeeklyOtSecJobHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! { WEEKLY_LIMIT_PROP => "40.0" }
    }

    /// `validateProperties` — the weekly limit must be above zero.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        if params.double_at(WEEKLY_LIMIT_PROP) <= 0.0 {
            results.push(format!("{WEEKLY_LIMIT_PROP}: res_gtZeroRuleValidation"));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::json_ids::ids_for_key;

    #[test]
    fn the_regular_hours_only_config_has_no_parameters() {
        assert_eq!(RegHrsOnlyRuleConfig.rule_class(), RuleClass::RegOnlyHdr);
        assert!(RegHrsOnlyRuleConfig.default_values().is_empty());
    }

    #[test]
    fn an_unconfigured_holiday_rule_selects_no_holiday_types() {
        let defaults = HolidayDTHrsRuleConfig.default_values();

        assert_eq!(defaults.get(HOLIDAY_TYPES_PROP), Some("[]"));
        assert!(ids_for_key(HOLIDAY_TYPES_PROP, &defaults).is_empty());
    }

    #[test]
    fn a_rule_item_that_sets_nothing_is_fixed_up_to_the_defaults() {
        let params = RuleParams::new().fixed(&HolidayDTHrsRuleConfig.default_values());

        assert_eq!(params.get(HOLIDAY_TYPES_PROP), Some("[]"));
    }

    #[test]
    fn the_secondary_job_weekly_limit_defaults_to_forty() {
        let defaults = WeeklyOTSecJobHrsRuleConfig.default_values();

        assert_eq!(defaults.get(WEEKLY_LIMIT_PROP), Some("40.0"));
        assert!(WeeklyOTSecJobHrsRuleConfig.validate(&defaults).is_empty());
    }

    #[test]
    fn a_weekly_limit_of_zero_or_less_is_rejected() {
        let at_zero = rule_params! { WEEKLY_LIMIT_PROP => "0.0" };
        let negative = rule_params! { WEEKLY_LIMIT_PROP => "-1.0" };

        assert_eq!(WeeklyOTSecJobHrsRuleConfig.validate(&at_zero).len(), 1);
        assert_eq!(WeeklyOTSecJobHrsRuleConfig.validate(&negative).len(), 1);
    }

    #[test]
    fn the_weekly_overtime_config_defaults_to_forty_hours_and_no_seven_i_check() {
        let defaults = WeeklyOTHrsRuleConfig.default_values();

        assert_eq!(defaults.get(WEEKLY_LIMIT_PROP), Some("40.0"));
        assert!(!defaults.bool_at(CHECK_7I));
        assert!(ids_for_key(OT_HOURS_EARNINGS_PROP, &defaults).is_empty());
        assert!(WeeklyOTHrsRuleConfig.validate(&defaults).is_empty());
    }

    #[test]
    fn the_pay_period_limit_defaults_to_eighty_hours() {
        let defaults = PayPeriodOTHrsRuleConfig.default_values();

        assert_eq!(defaults.get(PERIOD_OT_LIMIT_PROP), Some("80.0"));
        assert!(PayPeriodOTHrsRuleConfig.validate(&defaults).is_empty());
        assert_eq!(
            PayPeriodOTHrsRuleConfig
                .validate(&rule_params! { PERIOD_OT_LIMIT_PROP => "0.0" })
                .len(),
            1
        );
    }

    #[test]
    fn the_scheduled_shift_defaults_match_only_an_exact_schedule_start() {
        let defaults = ScheduledShiftOTRuleConfig.default_values();

        assert_eq!(defaults.double_at(MIN_SHIFT_LENGTH_PROP), 8.0);
        assert_eq!(defaults.int_at(THRESHOLD_PROP), 0);
        assert!(ScheduledShiftOTRuleConfig.validate(&defaults).is_empty());
    }

    #[test]
    fn a_zero_minimum_shift_length_is_rejected_but_a_zero_threshold_is_not() {
        let zero_length = rule_params! { MIN_SHIFT_LENGTH_PROP => "0", THRESHOLD_PROP => "0" };
        let negative_threshold =
            rule_params! { MIN_SHIFT_LENGTH_PROP => "8", THRESHOLD_PROP => "-1" };

        assert_eq!(ScheduledShiftOTRuleConfig.validate(&zero_length).len(), 1);
        assert_eq!(
            ScheduledShiftOTRuleConfig
                .validate(&negative_threshold)
                .len(),
            1
        );
    }

    #[test]
    fn the_premium_formula_is_the_default_despite_the_earlier_put() {
        // getDefaultValues puts this key twice, "false" then "true".
        let defaults = CaliforniaExtendedOTHrsRuleConfig.default_values();

        assert!(defaults.bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT));
    }

    #[test]
    fn the_california_defaults_are_eight_twelve_and_forty() {
        let defaults = CaliforniaExtendedOTHrsRuleConfig.default_values();

        assert_eq!(defaults.double_at(DAILY_OT_LIMIT_PROP), 8.0);
        assert_eq!(defaults.double_at(DAILY_DT_LIMIT_PROP), 12.0);
        assert_eq!(defaults.double_at(WEEKLY_LIMIT_PROP), 40.0);
        assert!(defaults.bool_at(PAY_DT_PROP));
        assert!(defaults.bool_at(CONSEC_DAYS_IN_WEEK));
        assert_eq!(defaults.int_at(CONSEC_DAY_LIMIT), 7);
        assert_eq!(defaults.int_at(MAX_CONSEC_DAYS_PD), 999);
        assert!(!defaults.bool_at(CONVERT_ALL_OT_TO_DT));
    }

    #[test]
    fn the_rolling_window_defaults_to_three_weeks_of_one_hundred_and_thirty_two_hours() {
        let defaults = RollingXWeeksOTHrsRuleConfig.default_values();

        assert_eq!(defaults.double_at(WEEKLY_LIMIT_PROP), 132.0);
        assert_eq!(defaults.int_at(WEEKS), 3);
    }

    #[test]
    fn the_worked_day_limits_default_to_the_sixth_and_seventh_day() {
        let defaults = DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig.default_values();

        assert_eq!(defaults.int_at(WORKED_DAY_OT_LIMIT_PROP), 6);
        assert_eq!(defaults.int_at(WORKED_DAY_DT_LIMIT_PROP), 7);
        assert_eq!(defaults.double_at(DAILY_OT_LIMIT_PROP), 8.0);
        assert_eq!(defaults.double_at(WEEKLY_LIMIT_PROP), 40.0);
    }

    #[test]
    fn every_month_defaults_to_one_hundred_and_sixty_hours() {
        let defaults = PerMonthOTHrsRuleConfig.default_values();

        for key in MONTH_KEYS {
            assert_eq!(defaults.double_at(key), 160.0, "{key}");
        }
        assert!(!defaults.bool_at(HOME_DEPT_ONLY));
        assert!(PerMonthOTHrsRuleConfig.validate(&defaults).is_empty());
    }

    #[test]
    fn a_months_key_is_the_java_enum_constants_name() {
        use joda_rs::LocalDate;

        assert_eq!(month_key(LocalDate::of(2016, 1, 15)), "JAN");
        assert_eq!(month_key(LocalDate::of(2016, 12, 31)), "DEC");
    }

    #[test]
    fn neither_config_is_priority_configurable() {
        assert_eq!(RegHrsOnlyRuleConfig.priority(&RuleParams::new()), None);
        assert_eq!(HolidayDTHrsRuleConfig.priority(&RuleParams::new()), None);
    }

    #[test]
    fn the_base_contributes_no_validation_constraints() {
        assert!(
            HolidayDTHrsRuleConfig
                .validate(&HolidayDTHrsRuleConfig.default_values())
                .is_empty()
        );
    }
}
