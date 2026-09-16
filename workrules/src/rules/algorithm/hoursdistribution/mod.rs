//! Hours distribution. `RuleType::HoursDistribution` — 19 rules.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/`.
//!
//! Split a day's worked hours into buckets — regular, overtime, double time —
//! according to whichever overtime law the property is under. The largest
//! family in the engine and the one the rate families price afterwards.
//!
//! The package holds 28 files, of which **19 are catalogue rules**. The other
//! nine are the `HoursDistributionRuleImpl` interface, the `ContractHrsRuleImpl`
//! marker interface that `RuleUtils.getContractHours` reaches for by
//! `instanceof`, `ShiftDifferenceOTRule`, and the six shared helpers this module
//! holds. `RegHrsOnlyRuleImpl.execute` is empty — the default `REG_ONLY_HDR`
//! rule genuinely means "leave regular hours where the runner put them".
//!
//! # The helpers
//!
//! | Module | Java | Used by |
//! |---|---|---|
//! | [`daily_accumulator`] | `DailyAccumulator` | 8 rules |
//! | [`weekly_accumulator`] | `WeeklyAccumulator` | 9 rules |
//! | [`daily_data`] | `DailyData` | [`california_ot_hrs`], its only user |
//! | [`earning_mapper`] | `EarningMapper` | `MinHrsForFullTimeOTRuleImpl` |
//! | [`consecutive_days_calculator`] | `ConsecutiveDaysCalculator` | `MinHrsForFullTimeOTRuleImpl` |
//! | [`prior_days_calculator`] | `PriorDaysCalculator` | 5 rules |
//!
//! Both accumulators round through `TDouble.roundHours` on every `add*`, so
//! they lean on `common/numbers.rs` being exact. The two consecutive-day
//! calculators answer the same question differently and are **not**
//! interchangeable — [`consecutive_days_calculator`] carries the table.
//!
//! # Rules ported so far
//!
//! Seventeen of nineteen: [`reg_hrs_only`], [`holiday_dt_hrs`],
//! [`weekly_ot_sec_job_hrs`], [`weekly_ot_hrs`], [`pay_period_ot_hrs`],
//! [`scheduled_shift_ot`], [`california_extended_ot_hrs`],
//! [`rolling_x_weeks_ot_hrs`], [`daily_weekly_6th_ot_7th_dt_non_consec`],
//! [`per_month_ot_hrs`], [`california_ot_hrs`], [`twenty_four_hour_ot`] and
//! [`contract_ot_hrs`] [`daily_weekly_7th_dt_hrs`] [`california_ext_special_job_ot_hrs`] [`min_hrs_for_full_time_ot`] and [`dly_wkly_off_consec_ot_min_break`]. The rest in size order,
//! smallest first, which is the porting order the audit settled on:
//! `DlyWklyOffConsecOTMinBreak`, `DailyWeekly6thOT7thDTHrs`,
//! `DlyWklyConsecOTMinBreakSpanningMidnight`.
//!
//! [`california_ot_hrs`] is the only one so far that writes **earnings** as
//! well as hours distributions; `CaliforniaExtSpecialJobOTHrs` will be the
//! second, and reads the same
//! [`EarningTypePaySet`](crate::rules::types::earning_type_pay_set) parameter.

pub mod california_ext_special_job_ot_hrs;
pub mod california_extended_ot_hrs;
pub mod california_ot_hrs;
pub mod config;
pub mod consecutive_days_calculator;
pub mod contract_ot_hrs;
pub mod daily_accumulator;
pub mod daily_data;
pub mod daily_weekly_6th_ot_7th_dt_non_consec;
pub mod daily_weekly_7th_dt_hrs;
pub mod dly_wkly_off_consec_ot_min_break;
pub mod earning_mapper;
pub mod holiday_dt_hrs;
pub mod min_hrs_for_full_time_ot;
pub mod pay_period_ot_hrs;
pub mod per_month_ot_hrs;
pub mod prior_days_calculator;
pub mod reg_hrs_only;
pub mod rolling_x_weeks_ot_hrs;
pub mod scheduled_shift_ot;
pub mod twenty_four_hour_ot;
pub mod weekly_accumulator;
pub mod weekly_ot_hrs;
pub mod weekly_ot_sec_job_hrs;

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use date_range_rs::DateRange;

/// An hours distribution rule.
///
/// `HoursDistributionRuleImpl.execute(TimeCard dataset, LegacyDatePeriod workWeek, RuleItem ruleItem)`.
///
/// # The time card arrives `&mut`
///
/// Unlike [`PunchRoundingRule`](super::punchrounding::PunchRoundingRule), where
/// the card is read-only context and four of six rules ignore it, here the card
/// **is** what the rule rewrites: every rule in the family moves hours between
/// the distributions its shifts own. So it is `&mut dyn TimeCard`, and a rule
/// that needs to read while writing goes through the index primitives — see
/// divergence 22 and the [`TimeCard`] module.
///
/// # `workWeek` is a plain `DateRange`
///
/// Java's parameter type is `LegacyDatePeriod`, a deprecated alias for the same
/// behaviour every other date range in the tree has; divergence 21 collapses
/// the three names onto one type.
///
/// # No rule reads the rule item beyond its params and id
///
/// `getParams()` for the configuration and `getID()` to stamp on the
/// distributions it creates, so the reader can tell which rule produced which
/// hours. The whole [`RuleItem`] is passed because Java's signature does.
pub trait HoursDistributionRule {
    /// Redistribute this work week's hours.
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem);
}

/// A rule that also answers how many hours a contract covers.
/// `ContractHrsRuleImpl`.
///
/// A marker interface in Java — `RuleUtils.getContractHours` walks a rule set
/// looking for one with `instanceof` and asks it, so the engine can price a
/// contract without knowing which rule defines it.
///
/// [`per_month_ot_hrs`] is its **only** implementation, in Java as here.
/// [`contract_ot_hrs`] does not implement it despite the name: it keeps its
/// contract to itself and answers nobody about it.
pub trait ContractHrsRule: HoursDistributionRule {
    /// The period the contract covers. `getContractPeriod()`.
    fn contract_period(&self) -> crate::common::enums::pay_period_type::PayPeriodType;

    /// The contracted hours covering `date`. `getContractHours()`.
    fn contract_hours(
        &self,
        employee: &crate::entity::employee::Employee,
        date: joda_rs::LocalDate,
        params: &crate::rules::params::RuleParams,
    ) -> f64;
}
