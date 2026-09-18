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
use crate::rules::types::earning_type_pay_set::EarningTypePaySet;

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

/// Which earning types a rule treats as regular, and what it pays their
/// premium hours against.
/// `HoursDistributionRuleWithPayMappingsConfig.EARNING_TYPE_PAY_SET`.
///
/// An [`EarningTypePaySet`] serialized as JSON. The intermediate base seeds it
/// with an **empty** pay set carrying only the concrete config's
/// `getPremiumLevels()`, so a rule that filters earnings through it touches
/// none of them until a property configures one.
///
/// [`EarningTypePaySet`]: crate::rules::types::earning_type_pay_set::EarningTypePaySet
pub const EARNING_TYPE_PAY_SET: &str = "earningTypePaySet";

/// `CaliforniaOTHrsRuleConfig`.
///
/// The first ported config to extend
/// `HoursDistributionRuleWithPayMappingsConfig`, whose one contribution is
/// [`EARNING_TYPE_PAY_SET`] defaulted from `getPremiumLevels()` — `2` here,
/// overtime and double time.
///
/// # Its consecutive-day limits are not parameters
///
/// `CaliforniaExtendedOTHrsRuleConfig` above exposes `consecDayLimit` and
/// `maxConsecDaysPd`; this one does not. `CaliforniaOTHrsRuleImpl` hardcodes
/// them as `final` fields — 7 and 36500 — so there is nothing to configure and
/// no key to declare.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaliforniaOTHrsRuleConfig;

impl RuleConfig for CaliforniaOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::CaliforniaHdr
    }

    /// Java's order, the inherited pay set first. No key is put twice here,
    /// unlike the extended config above.
    fn default_values(&self) -> RuleParams {
        rule_params! {
            EARNING_TYPE_PAY_SET => EarningTypePaySet::new().with_premium_levels(2).to_json_string(),
            PAY_DT_PROP => "true",
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT => "true",
        }
    }

    /// `validateProperties` — the three limits must nest.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        let daily_ot = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt = params.double_at(DAILY_DT_LIMIT_PROP);
        let weekly = params.double_at(WEEKLY_LIMIT_PROP);

        if daily_ot >= daily_dt {
            results.push(
                "The Daily OT Limit property must be less than the Daily DT Limit property"
                    .to_string(),
            );
        }
        if daily_ot <= 0.0 {
            results.push("The Daily OT Limit property must be greater than zero".to_string());
        }
        if weekly <= daily_dt {
            results.push(
                "The Weekly OT Limit property must be greater than the Daily DT Limit property"
                    .to_string(),
            );
        }

        results
    }
}

/// Hours across the consecutive-day run before overtime starts.
/// `DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig.CONSEC_DAY_HRS_LIMIT`.
///
/// The only **hours**-based consecutive-day limit in the family; every other
/// rule counts days alone. `DlyWklyOffConsecOTMinBreak`'s spec sets this key
/// and that rule never reads it — see the finding on its last case.
pub const CONSEC_DAY_HRS_LIMIT: &str = "consecDayHrsLimit";
/// Whether a minimum-break violation pays the **whole** shift rather than the
/// shortfall. `MIN_TIME_BETWEEN_PAY_FULL_SHIFT`.
pub const MIN_TIME_BETWEEN_PAY_FULL_SHIFT: &str = "minTimeBetweenPayFullShift";

/// `DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig`.
///
/// # Its `minTimeBetweenShifts` default is 10, not 7
///
/// `DlyWklyOffConsecOTMinBreakRuleConfig` defaults the same key to `"7.0"`.
/// That three-hour difference is what makes the case the two specs share come
/// out differently under each rule; see the finding under "Rules".
#[derive(Debug, Clone, Copy, Default)]
pub struct DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig;

impl RuleConfig for DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DwcotMinBrSpanMnHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            CONSEC_DAY_HRS_LIMIT => "40.0",
            CONSEC_DAY_OT_LIMIT => "6",
            MAX_CONSEC_DAYS_PD => "2",
            MIN_TIME_BETWEEN_SHIFTS => "10.0",
            DAILY_SPLIT_SHIFT_LIMIT => "3.0",
            THIS_WEEK_ONLY => "false",
            EARNING_TYPES => "[]",
            MIN_TIME_BETWEEN_PAY_FULL_SHIFT => "false",
            BOTH_CONSECUTIVE_AND_WEEKLY_OT => "false",
            PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT => "true",
        }
    }
}

/// Consecutive days worked at which **double time** starts.
/// `DailyWeekly6thOT7thDTHrsRuleConfig.CONSEC_DAY_DT_LIMIT`.
pub const CONSEC_DAY_DT_LIMIT: &str = "consecDayDtLimit";
/// Whether daily overtime is paid at all. `PAY_DAILY_OT`.
pub const PAY_DAILY_OT: &str = "payDailyOT";
/// Whether the sixth consecutive day also pays double time past the daily
/// **overtime** limit. `PAY_6TH_DAY_DT`, spelled `paySixthDayDT`.
pub const PAY_6TH_DAY_DT: &str = "paySixthDayDT";
/// Whether earnings of the configured types count toward the consecutive-day
/// counter. `INCLUDE_EARNING_TYPE_PAY_MAP`.
pub const INCLUDE_EARNING_TYPE_PAY_MAP: &str = "includeEarningTypePayMap";
/// Whether the sixth-day test counts **days worked in the week** rather than
/// consecutive days. `OVERRIDE_CONSEC_DAY_OT`.
pub const OVERRIDE_CONSEC_DAY_OT: &str = "overrideConsecDayOt";

/// `DailyWeekly6thOT7thDTHrsRuleConfig`.
///
/// Fourteen parameters — the most of any config in the family.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailyWeekly6thOT7thDTHrsRuleConfig;

impl RuleConfig for DailyWeekly6thOT7thDTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::Dw6ot7dtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            EARNING_TYPE_PAY_SET => EarningTypePaySet::new().with_premium_levels(2).to_json_string(),
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            PAY_DAILY_OT => "true",
            PAY_DAILY_DT => "true",
            PAY_6TH_DAY_DT => "true",
            CONSEC_DAY_OT_LIMIT => "6",
            CONSEC_DAY_DT_LIMIT => "7",
            CONSEC_DAYS_IN_WEEK => "true",
            MAX_CONSEC_DAYS_PD => "999",
            INCLUDE_EARNING_TYPE_PAY_MAP => "false",
            OVERRIDE_CONSEC_DAY_OT => "false",
            BOTH_CONSECUTIVE_AND_WEEKLY_OT => "false",
            PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT => "true",
        }
    }
}

/// Consecutive days worked at which overtime starts.
/// `DlyWklyOffConsecOTMinBreakRuleConfig.CONSEC_DAY_OT_LIMIT`, and
/// `DailyWeekly6thOT7thDTHrsRuleConfig`'s key of the same name and spelling.
pub const CONSEC_DAY_OT_LIMIT: &str = "consecDayOtLimit";
/// The minimum rest between two shifts; work inside it is overtime.
/// `MIN_TIME_BETWEEN_SHIFTS`.
pub const MIN_TIME_BETWEEN_SHIFTS: &str = "minTimeBetweenShifts";
/// The most time between two shifts on one date for them still to count as a
/// **split shift** rather than a rest violation. `DAILY_SPLIT_SHIFT_LIMIT`,
/// spelled `dailyShiftSplitLimit` — note the constant and the key disagree
/// about word order.
pub const DAILY_SPLIT_SHIFT_LIMIT: &str = "dailyShiftSplitLimit";
/// Whether the consecutive-day count is confined to this work week.
/// `THIS_WEEK_ONLY` — a fourth spelling of the idea
/// [`CONSEC_DAYS_IN_WEEK`] carries elsewhere, and a different key.
pub const THIS_WEEK_ONLY: &str = "thisWeekOnly";
/// Whether a day with no schedule pays every hour as overtime.
/// `OT_ON_UNSCHED_DAY`.
pub const OT_ON_UNSCHED_DAY: &str = "otOnUnschedDay";

/// `DlyWklyOffConsecOTMinBreakRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DlyWklyOffConsecOTMinBreakRuleConfig;

impl RuleConfig for DlyWklyOffConsecOTMinBreakRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::DwocotMinBrHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            CONSEC_DAY_OT_LIMIT => "6",
            MAX_CONSEC_DAYS_PD => "999",
            MIN_TIME_BETWEEN_SHIFTS => "7.0",
            DAILY_SPLIT_SHIFT_LIMIT => "3.0",
            THIS_WEEK_ONLY => "false",
            OT_ON_UNSCHED_DAY => "true",
            EARNING_TYPES => "[]",
            BOTH_CONSECUTIVE_AND_WEEKLY_OT => "false",
        }
    }
}

/// Hours in a day before overtime starts —
/// `HoursDistributionConfigConstants.DAILY_OT_LIMIT_PROP`, spelled
/// **`dailyOTLimit`**.
///
/// # Not [`DAILY_OT_LIMIT_PROP`]
///
/// Five rules in this family read `dailyOtLimit`; `MinHrsForFullTimeOT` reads
/// `dailyOTLimit`, from a different constants class, and the two differ only in
/// the case of one letter. The same is true of [`WEEKLY_OT_LIMIT_PROP_CAPS`]
/// against [`WEEKLY_LIMIT_PROP`]. A property that copies a parameter from one
/// rule to the other gets the default and no warning.
pub const DAILY_OT_LIMIT_PROP_CAPS: &str = "dailyOTLimit";
/// Hours in a week before overtime starts. `WEEKLY_OT_LIMIT_PROP`, spelled
/// **`weeklyOTLimit`** — see the note on [`DAILY_OT_LIMIT_PROP_CAPS`].
pub const WEEKLY_OT_LIMIT_PROP_CAPS: &str = "weeklyOTLimit";
/// Whether consecutive-day overtime is switched on at all.
/// `ENABLE_CONSEC_DAY_OT_PROP`.
pub const ENABLE_CONSEC_DAY_OT_PROP: &str = "enableConsecDayOT";
/// The consecutive-day counter's wrap point, less one. `MAX_CONSEC_DAYS_PAID_PROP`.
pub const MAX_CONSEC_DAYS_PAID_PROP: &str = "maxConsecDaysPaid";
/// Weekly hours at or under which an employee counts as part time, and is paid
/// double time instead of overtime. `PART_TIME_ELIGIBILITY_LIMIT_PROP`.
pub const PART_TIME_ELIGIBILITY_LIMIT_PROP: &str = "partTimeEligibilityLimit";
/// The most double time a part-time employee may be paid in a week.
/// `MAX_DT_PAID_PROP`.
pub const MAX_DT_PAID_PROP: &str = "maxDTPaid";

/// `MinHrsForFullTimeOTRuleConfig`.
///
/// The rule that pays **double time to part-timers and overtime to everyone
/// else**, so its parameters come in two halves: the ordinary daily, weekly and
/// consecutive-day limits, and the two that define part time.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinHrsForFullTimeOTRuleConfig;

impl RuleConfig for MinHrsForFullTimeOTRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinHrsFullTimeOtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            EARNING_TYPE_PAY_SET => EarningTypePaySet::new().with_premium_levels(2).to_json_string(),
            ENABLE_CONSEC_DAY_OT_PROP => "true",
            CONSEC_DAYS_IN_WEEK => "true",
            CONSEC_DAY_LIMIT => "7",
            MAX_CONSEC_DAYS_PAID_PROP => "999",
            DAILY_OT_LIMIT_PROP_CAPS => "8.0",
            WEEKLY_OT_LIMIT_PROP_CAPS => "40.0",
            PART_TIME_ELIGIBILITY_LIMIT_PROP => "28.0",
            MAX_DT_PAID_PROP => "17.5",
            BOTH_CONSECUTIVE_AND_WEEKLY_OT => "false",
        }
    }
}

/// The daily overtime limit for a special job on a special day.
/// `CaliforniaExtSpecialJobOTHrsRuleConfig.SPEC_DAILY_OT_LIMIT_PROP`.
pub const SPEC_DAILY_OT_LIMIT_PROP: &str = "specDailyOtLimit";
/// The daily double-time limit for a special job on a special day.
pub const SPEC_DAILY_DT_LIMIT_PROP: &str = "specDailyDtLimit";
/// The weekly limit for a special job on a special day.
pub const SPEC_WEEKLY_LIMIT_PROP: &str = "specWeeklyOtLimit";
/// Which days of the week are special, as a JSON array of **Sunday-based** day
/// numbers. `SPEC_DOW_PROP`.
///
/// Read through [`ids_from_json`](crate::common::json_ids::ids_from_json), not
/// the swallowing parser, and compared against
/// [`translate_dow_from_iso`](crate::common::dates::translate_dow_from_iso) of
/// the date's ISO day.
pub const SPEC_DOW_PROP: &str = "specDaysOfWeek";
/// Which jobs are special, as a JSON array of `Assignment` ids. `SPEC_JOBS_PROP`.
pub const SPEC_JOBS_PROP: &str = "specJobs";

/// `CaliforniaExtSpecialJobOTHrsRuleConfig`.
///
/// Twelve parameters — the six ordinary limits and gates, plus a parallel set
/// of three limits for a "special job on a special day" and the two lists that
/// say which those are.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaliforniaExtSpecialJobOTHrsRuleConfig;

impl RuleConfig for CaliforniaExtSpecialJobOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::CaExtSpJobHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            PAY_DT_PROP => "true",
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            CONSEC_DAY_LIMIT => "7",
            CONSEC_DAYS_IN_WEEK => "true",
            SPEC_DAILY_OT_LIMIT_PROP => "8.0",
            SPEC_DAILY_DT_LIMIT_PROP => "12.0",
            SPEC_WEEKLY_LIMIT_PROP => "40.0",
            SPEC_DOW_PROP => "[]",
            SPEC_JOBS_PROP => "[]",
            MAX_CONSEC_DAYS_PD => "999",
        }
    }

    /// `validateProperties` — the same three limit constraints as the other
    /// California configs, twice over, plus the two consecutive-day bounds.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        let mut limits = |ot_key: &str, dt_key: &str, weekly_key: &str, label: &str| {
            let daily_ot = params.double_at(ot_key);
            let daily_dt = params.double_at(dt_key);

            if daily_ot >= daily_dt {
                results.push(format!(
                    "The {label} Daily DT Limit must be greater than the {label} Daily OT Limit"
                ));
            }
            if daily_ot <= 0.0 {
                results.push(format!(
                    "The {label} Daily OT Limit must be greater than zero"
                ));
            }
            if params.double_at(weekly_key) <= daily_dt {
                results.push(format!(
                    "The {label} Weekly OT Limit must be greater than the {label} Daily DT Limit"
                ));
            }
        };

        limits(
            DAILY_OT_LIMIT_PROP,
            DAILY_DT_LIMIT_PROP,
            WEEKLY_LIMIT_PROP,
            "default",
        );
        limits(
            SPEC_DAILY_OT_LIMIT_PROP,
            SPEC_DAILY_DT_LIMIT_PROP,
            SPEC_WEEKLY_LIMIT_PROP,
            "special",
        );

        if params.int_at(CONSEC_DAY_LIMIT) < 0 {
            results.push("The Consecutive Day Limit cannot be less than zero".to_string());
        }
        if params.int_at(MAX_CONSEC_DAYS_PD) <= 0 {
            results.push("The Max Consecutive Days must be greater than zero".to_string());
        }

        results
    }
}

/// Whether double time is paid for hours past the daily double-time limit on
/// an ordinary day. `DailyWeekly7thDTHrsRuleConfig.PAY_DAILY_DT`.
///
/// Spelled `payDailyDT`, and **not** the same key as [`PAY_DT_PROP`]
/// (`payDT`), which the two California rules read. It gates only the ordinary
/// daily limit: the seventh consecutive day pays double time either way.
pub const PAY_DAILY_DT: &str = "payDailyDT";

/// `DailyWeekly7thDTHrsRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailyWeekly7thDTHrsRuleConfig;

impl RuleConfig for DailyWeekly7thDTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::Dw7dtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            EARNING_TYPE_PAY_SET => EarningTypePaySet::new().with_premium_levels(2).to_json_string(),
            DAILY_OT_LIMIT_PROP => "8.0",
            DAILY_DT_LIMIT_PROP => "12.0",
            WEEKLY_LIMIT_PROP => "40.0",
            PAY_DAILY_DT => "true",
        }
    }

    /// The same three constraints `CaliforniaOTHrsRuleConfig` expresses.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        let daily_ot = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt = params.double_at(DAILY_DT_LIMIT_PROP);

        if daily_ot >= daily_dt {
            results.push(
                "The Daily DT Limit property must be greater than the Daily OT Limit property"
                    .to_string(),
            );
        }
        if daily_ot <= 0.0 {
            results.push("The Daily OT Limit property must be greater than zero".to_string());
        }
        if params.double_at(WEEKLY_LIMIT_PROP) <= daily_dt {
            results.push(
                "The Weekly OT Limit property must be greater than the Daily DT Limit property"
                    .to_string(),
            );
        }

        results
    }
}

/// The property's weekly contracted hours, which a
/// [`ContractCalculator`](crate::common::contract::ContractCalculator) scales
/// to one pay period. `ContractOTHrsRuleConfig.WEEKLY_CONTRACT_HOURS`.
pub const WEEKLY_CONTRACT_HOURS: &str = "weeklyContractHours";

/// `ContractOTHrsRuleConfig`.
///
/// It shares three of its four keys with `PerMonthOTHrsRuleConfig`
/// (`homeDeptOnly`, `earningTypes`) and `HolidayDTHrsRuleConfig`
/// (`holidayTypes`) — the same spellings, read the same way.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContractOTHrsRuleConfig;

impl RuleConfig for ContractOTHrsRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ContractOtHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            HOME_DEPT_ONLY => "false",
            EARNING_TYPES => "[]",
            WEEKLY_CONTRACT_HOURS => "39.0",
            HOLIDAY_TYPES_PROP => "[]",
        }
    }

    /// `validateProperties` — one constraint, and it allows zero.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        if params.double_at(WEEKLY_CONTRACT_HOURS) < 0.0 {
            results.push("Weekly Contract Hours: res_gteZeroRuleValidation".to_string());
        }

        results
    }
}

/// Hours worked inside a rolling 24-hour work day before overtime starts.
/// `TwentyFourHourOTRuleConfig.EMPLOYEE_WORK_DAY_LIMIT_PROP`, spelled
/// `twentyFourHourOtLimit`.
pub const EMPLOYEE_WORK_DAY_LIMIT_PROP: &str = "twentyFourHourOtLimit";

/// The hourly rate above which an employee stops earning work-day overtime.
/// `RATE_THRESHOLD_PROP`.
pub const RATE_THRESHOLD_PROP: &str = "rateThreshold";

/// Whether the rate compared against the threshold is the FLSA regular rate
/// rather than the employee's home job rate. `USE_FLSA_REGULAR_RATE_PROP` —
/// note the capitalised spelling, `useFLSARegularRate`.
pub const USE_FLSA_REGULAR_RATE_PROP: &str = "useFLSARegularRate";

/// `TwentyFourHourOTRuleConfig`.
///
/// The threshold defaults to 99,999, which is to say the rate gate is off
/// unless a property turns it on.
#[derive(Debug, Clone, Copy, Default)]
pub struct TwentyFourHourOTRuleConfig;

impl RuleConfig for TwentyFourHourOTRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::TwentyFourHourHdr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            EMPLOYEE_WORK_DAY_LIMIT_PROP => "8.0",
            WEEKLY_LIMIT_PROP => "40.0",
            RATE_THRESHOLD_PROP => "99999.0",
            USE_FLSA_REGULAR_RATE_PROP => "false",
        }
    }

    /// `validateProperties`: the work-day limit above zero, the weekly limit
    /// strictly above the work-day limit, and the rate threshold not negative.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();

        let work_day_limit = params.double_at(EMPLOYEE_WORK_DAY_LIMIT_PROP);

        if work_day_limit <= 0.0 {
            results.push(
                "The Twenty Four Hour OT Limit property must be greater than zero".to_string(),
            );
        }
        if params.double_at(WEEKLY_LIMIT_PROP) <= work_day_limit {
            results.push(
                "The Weekly OT Limit property must be greater than the Twenty Four Hour OT Limit property"
                    .to_string(),
            );
        }
        if params.double_at(RATE_THRESHOLD_PROP) < 0.0 {
            results.push("The Rate Threshold property cannot be less than zero".to_string());
        }

        results
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
    use rstest::rstest;

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
    fn the_plain_california_config_shares_the_same_limits() {
        let defaults = CaliforniaOTHrsRuleConfig.default_values();

        assert_eq!(defaults.double_at(DAILY_OT_LIMIT_PROP), 8.0);
        assert_eq!(defaults.double_at(DAILY_DT_LIMIT_PROP), 12.0);
        assert_eq!(defaults.double_at(WEEKLY_LIMIT_PROP), 40.0);
        assert!(defaults.bool_at(PAY_DT_PROP));
        assert!(defaults.bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT));
    }

    #[test]
    fn it_declares_no_consecutive_day_parameters() {
        // CaliforniaOTHrsRuleImpl hardcodes 7 and 36500 as final fields, so
        // unlike the extended config there is nothing to configure.
        let defaults = CaliforniaOTHrsRuleConfig.default_values();

        assert!(!defaults.contains(CONSEC_DAY_LIMIT));
        assert!(!defaults.contains(MAX_CONSEC_DAYS_PD));
        assert!(!defaults.contains(CONSEC_DAYS_IN_WEEK));
    }

    #[test]
    fn its_pay_set_defaults_to_two_premium_levels_and_no_mappings() {
        // The one thing HoursDistributionRuleWithPayMappingsConfig contributes.
        // An empty mapping set is what silences the rule's earnings half.
        let defaults = CaliforniaOTHrsRuleConfig.default_values();
        let pay_set =
            EarningTypePaySet::from_json_string(defaults.get(EARNING_TYPE_PAY_SET).unwrap());

        assert_eq!(pay_set.premium_levels(), 2);
        assert!(pay_set.configured_earning_type_ids().is_empty());
    }

    #[test]
    fn the_california_limits_must_nest() {
        let valid = CaliforniaOTHrsRuleConfig.default_values();
        assert!(CaliforniaOTHrsRuleConfig.validate(&valid).is_empty());

        let mut daily_ot_above_dt = CaliforniaOTHrsRuleConfig.default_values();
        daily_ot_above_dt.set(DAILY_OT_LIMIT_PROP, "16.0");
        assert_eq!(
            CaliforniaOTHrsRuleConfig.validate(&daily_ot_above_dt).len(),
            1
        );

        let mut daily_ot_at_zero = CaliforniaOTHrsRuleConfig.default_values();
        daily_ot_at_zero.set(DAILY_OT_LIMIT_PROP, "0.0");
        assert_eq!(
            CaliforniaOTHrsRuleConfig.validate(&daily_ot_at_zero).len(),
            1,
            "zero is still below the twelve-hour double-time limit"
        );

        // One parameter can trip more than one constraint: both limits at zero
        // is not above zero *and* not below the double-time limit.
        let mut both_at_zero = CaliforniaOTHrsRuleConfig.default_values();
        both_at_zero.set(DAILY_OT_LIMIT_PROP, "0.0");
        both_at_zero.set(DAILY_DT_LIMIT_PROP, "0.0");
        assert_eq!(CaliforniaOTHrsRuleConfig.validate(&both_at_zero).len(), 2);

        let mut weekly_below_daily_dt = CaliforniaOTHrsRuleConfig.default_values();
        weekly_below_daily_dt.set(WEEKLY_LIMIT_PROP, "12.0");
        assert_eq!(
            CaliforniaOTHrsRuleConfig
                .validate(&weekly_below_daily_dt)
                .len(),
            1
        );
    }

    #[test]
    fn the_contract_config_defaults_to_thirty_nine_weekly_hours() {
        let defaults = ContractOTHrsRuleConfig.default_values();

        assert_eq!(defaults.double_at(WEEKLY_CONTRACT_HOURS), 39.0);
        assert!(!defaults.bool_at(HOME_DEPT_ONLY));
        assert!(ids_for_key(EARNING_TYPES, &defaults).is_empty());
        assert!(ids_for_key(HOLIDAY_TYPES_PROP, &defaults).is_empty());
    }

    #[test]
    fn the_contract_hours_may_be_zero_but_not_negative() {
        let with_hours = |hours: &str| {
            let mut params = ContractOTHrsRuleConfig.default_values();
            params.set(WEEKLY_CONTRACT_HOURS, hours);
            ContractOTHrsRuleConfig.validate(&params)
        };

        assert!(with_hours("39.0").is_empty());
        assert!(
            with_hours("0.0").is_empty(),
            "the test is `< 0`, not `<= 0`"
        );
        assert_eq!(with_hours("-1.0").len(), 1);
    }

    /// `TwentyFourHourOTRuleConfigTest.groovy`'s `testGetDefaultValues`.
    #[test]
    fn test_get_default_values() {
        let defaults = TwentyFourHourOTRuleConfig.default_values();

        assert_eq!(defaults.get(EMPLOYEE_WORK_DAY_LIMIT_PROP), Some("8.0"));
        assert_eq!(defaults.get(WEEKLY_LIMIT_PROP), Some("40.0"));
        assert_eq!(defaults.get(RATE_THRESHOLD_PROP), Some("99999.0"));
        assert_eq!(defaults.get(USE_FLSA_REGULAR_RATE_PROP), Some("false"));
    }

    /// The same spec's `testValidateProperties`, all six rows. Its
    /// `testGetProperties` case is the l2fprod UI half, which this module does
    /// not port; `testRuleClass` and `testName` are covered by the catalogue.
    #[rstest]
    #[case("8", "40", "0", false, "no errors")]
    #[case("0", "40", "0", true, "twenty four hour limit is zero")]
    #[case("-1", "40", "0", true, "twenty four hour limit is less than zero")]
    #[case("8", "8", "0", true, "weekly limit is equal to the 24 hour limit")]
    #[case("8", "7", "0", true, "weekly limit is less than the 24 hour limit")]
    #[case("8", "40", "-1", true, "rate threshold less than zero")]
    fn test_validate_properties(
        #[case] work_day_limit: &str,
        #[case] weekly_limit: &str,
        #[case] rate_threshold: &str,
        #[case] has_errors: bool,
        #[case] scenario: &str,
    ) {
        let params = rule_params! {
            EMPLOYEE_WORK_DAY_LIMIT_PROP => work_day_limit,
            WEEKLY_LIMIT_PROP => weekly_limit,
            RATE_THRESHOLD_PROP => rate_threshold,
        };

        assert_eq!(
            !TwentyFourHourOTRuleConfig.validate(&params).is_empty(),
            has_errors,
            "{scenario}"
        );
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
