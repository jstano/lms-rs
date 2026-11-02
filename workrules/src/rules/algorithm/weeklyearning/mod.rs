//! `RuleType::WeeklyDifferential` — eight rules behind `WeeklyEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/weeklyearning/`,
//! runner at `.../server/labor/calcshift/rules/WeeklyDifferentialRuleRunner.java`.
//!
//! Sixth of the seven families prioritized for scheduling.
//!
//! # One call per work week
//!
//! `execute(timeCard, workWeek, ruleItem, ruleDescription)` is called once for
//! each work week the calculation spans, and the rule writes that week's
//! earnings onto the card — some on particular days, some on the week's end
//! date. Unlike `dailyearning` the **runner passes the rule its description**
//! (`"Rule: <rule set> - <rule name>"`) instead of the rule asking for it; four
//! rules use that argument and `DailyWeeklyGuaranteedHours` ignores it and asks
//! `RuleUtils` itself.
//!
//! # Two different "net hours"
//!
//! The base class has its own deprecated `getNetHours(shift, useActual)`, used
//! by `WeeklyHrsFixed` and `DOWHrsFixed`; `DOWWeeklyHrsRateFactor` and
//! `HoursThresholdFixedWeekly` use the newer `ShiftUtil.getNetHours`. They
//! disagree: see [`net_hours_legacy`].
//!
//! Ported cases: `DailySpreadRuleImplTest`, `DailyWeeklyGuaranteedHoursRuleImplTest`,
//! `DOWWeeklyHrsRateFactorRuleImplTest`, `HoursThresholdFixedWeeklyRuleImplTest`
//! and `WeeklyDifferentialRuleRunnerTest`. `NoWeeklyEarning`, `WeeklyHrsFixed`,
//! `DOWHrsFixed` and `SalariedMinWage` have no Groovy spec; their behaviour
//! tests are written from the Java.

pub mod config;
pub mod daily_spread;
pub mod daily_weekly_guaranteed_hours;
pub mod dow_hrs_fixed;
pub mod dow_weekly_hrs_rate_factor;
pub mod hours_threshold_fixed_weekly;
pub mod no_weekly_earning;
pub mod salaried_min_wage;
pub mod weekly_hrs_fixed;

use crate::common::dates::duration_in_fractional_hours;
use crate::common::numbers::round_to;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use date_range_rs::DateRange;

/// A weekly-differential rule. `WeeklyEarningRuleImpl`.
pub trait WeeklyEarningRule {
    /// `execute(TimeCard, DateRange, RuleItem, String)`.
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        rule_description: &str,
    );
}

/// `WeeklyEarningRuleImpl.getNetHours(shift, useActual)`, the deprecated
/// base-class version.
///
/// `use_actual = false` is the stored net hours **unrounded** (`ShiftUtil`'s
/// version rounds to two places). `true` pairs the time-ordered punches
/// 0&1, 2&3, … and sums each pair's adjusted-time span, rounding the running
/// total to two places after each pair, then adds `adjHours` — so it counts
/// every in/out pair, where `ShiftUtil` measures first-to-last and subtracts
/// breaks. It yields zero hours (just `adjHours`) when the shift currently has
/// errors or fewer than two punches, and does **not** exempt schedules.
pub fn net_hours_legacy(shift: &EmployeeShift, use_actual: bool) -> f64 {
    if !use_actual {
        return shift.net_hours();
    }

    let mut worked = 0.0;
    let order = shift.punch_order_for_breaks();
    if shift.derived_errors().is_empty() && order.len() >= 2 {
        let mut i = 0;
        while i + 1 < order.len() {
            if let (Some(from), Some(to)) = (
                shift.punch(order[i]).adj_time(),
                shift.punch(order[i + 1]).adj_time(),
            ) {
                worked = round_to(worked + duration_in_fractional_hours(from, to), 2);
            }
            i += 2;
        }
    }
    round_to(worked + shift.adj_hours(), 2)
}

/// An unpersisted RULE earning dated and paid on `date`, with no shift unless
/// one is given — the part every weekly rule builds identically.
pub fn rule_earning(
    employee_id: i32,
    job_id: i32,
    earning_type_id: i32,
    date: joda_rs::LocalDate,
    rule_item: &RuleItem,
    shift_id: Option<i32>,
) -> EmployeeEarning {
    let mut earning = EmployeeEarning::new(
        0,
        employee_id,
        job_id,
        earning_type_id,
        date,
        0.0,
        0.0,
        crate::common::enums::earning_source::EarningSource::Rule,
    )
    .from_rule(rule_item.id(), shift_id);
    earning.set_pay_date(Some(date));
    earning
}

use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort, PeriodSalaryPort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use daily_spread::DailySpreadRule;
use daily_weekly_guaranteed_hours::DailyWeeklyGuaranteedHoursRule;
use dow_hrs_fixed::DOWHrsFixedRule;
use dow_weekly_hrs_rate_factor::DOWWeeklyHrsRateFactorRule;
use hours_threshold_fixed_weekly::HoursThresholdFixedWeeklyRule;
use no_weekly_earning::NoWeeklyEarningRule;
use salaried_min_wage::SalariedMinWageRule;
use weekly_hrs_fixed::WeeklyHrsFixedRule;

/// Dispatch for the eight weekly-differential rules. `RuleImplFactory`,
/// narrowed to this family.
///
/// Every rule owns its ports, so those several rules share must be `Clone` —
/// in practice a cheap handle onto the same store.
pub struct WeeklyEarningRegistry<T, E, M, S>
where
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    S: PeriodSalaryPort,
{
    no_weekly: NoWeeklyEarningRule,
    weekly_hrs_fixed: WeeklyHrsFixedRule<E>,
    dow_weekly_hrs_rate_factor: DOWWeeklyHrsRateFactorRule<T, E>,
    dow_hrs_fixed: DOWHrsFixedRule<E>,
    daily_spread: DailySpreadRule<T, E, M>,
    salaried_min_wage: SalariedMinWageRule<E, S>,
    hours_threshold_fixed: HoursThresholdFixedWeeklyRule<E>,
    daily_weekly_guaranteed: DailyWeeklyGuaranteedHoursRule<E>,
}

impl<T, E, M, S> WeeklyEarningRegistry<T, E, M, S>
where
    T: EarningTypePort + Clone,
    E: EmployeeEarningPort + Clone,
    M: MinWagePort,
    S: PeriodSalaryPort,
{
    /// Build every rule over the shared ports.
    pub fn new(earning_types: T, earnings: E, min_wage: M, period_salary: S) -> Self {
        Self {
            no_weekly: NoWeeklyEarningRule,
            weekly_hrs_fixed: WeeklyHrsFixedRule::new(earnings.clone()),
            dow_weekly_hrs_rate_factor: DOWWeeklyHrsRateFactorRule::new(
                earning_types.clone(),
                earnings.clone(),
            ),
            dow_hrs_fixed: DOWHrsFixedRule::new(earnings.clone()),
            daily_spread: DailySpreadRule::new(earning_types, earnings.clone(), min_wage),
            salaried_min_wage: SalariedMinWageRule::new(earnings.clone(), period_salary),
            hours_threshold_fixed: HoursThresholdFixedWeeklyRule::new(earnings.clone()),
            daily_weekly_guaranteed: DailyWeeklyGuaranteedHoursRule::new(earnings),
        }
    }
}

impl<T, E, M, S> RuleRegistry<dyn WeeklyEarningRule> for WeeklyEarningRegistry<T, E, M, S>
where
    T: EarningTypePort + 'static,
    E: EmployeeEarningPort + 'static,
    M: MinWagePort + 'static,
    S: PeriodSalaryPort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::WeeklyDifferential
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn WeeklyEarningRule + 'static)> {
        match rule_class {
            RuleClass::NoWdf => Some(&self.no_weekly),
            RuleClass::WklyHrsFixedWdf => Some(&self.weekly_hrs_fixed),
            RuleClass::DowWeeklyHrsFactorWdf => Some(&self.dow_weekly_hrs_rate_factor),
            RuleClass::DowHrsFixedWdf => Some(&self.dow_hrs_fixed),
            RuleClass::DailySpreadWdf => Some(&self.daily_spread),
            RuleClass::SalaryMinWageWdf => Some(&self.salaried_min_wage),
            RuleClass::HrsThreshFixedWdf => Some(&self.hours_threshold_fixed),
            RuleClass::DailyWeeklyGuaranteedHrsWdf => Some(&self.daily_weekly_guaranteed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::common::enums::uom::UOM;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use joda_rs::LocalDate;

    #[derive(Clone)]
    struct Types;
    impl EarningTypePort for Types {
        fn find_by_id(&self, _id: i32) -> Option<crate::entity::earning_type::EarningType> {
            Some(earning_type(UOM::Hours))
        }
    }
    struct Salary;
    impl PeriodSalaryPort for Salary {
        fn period_salary(&self, _e: i32, _d: LocalDate) -> f64 {
            0.0
        }
    }

    /// All eight catalogue entries dispatch, none is missing.
    #[test]
    fn every_weekly_differential_rule_class_dispatches() {
        let registry =
            WeeklyEarningRegistry::new(Types, SavedEarnings::default(), FixedMinWage(1.0), Salary);
        assert_eq!(registry.dispatchable().len(), 8);
        assert!(registry.missing().is_empty());
    }

    #[test]
    fn the_legacy_net_hours_differ_from_shift_utils() {
        use crate::common::enums::punch_source::PunchSource;
        use crate::common::enums::punch_type::PunchType;
        use crate::common::enums::shift_type::ShiftType;
        use crate::entity::employee_shift::EmployeeShift;
        use crate::entity::employee_shift_punch::EmployeeShiftPunch;
        let at = |h, m| LocalDate::of(2014, 5, 6).at_time(joda_rs::LocalTime::of(h, m, 0));
        let p = |id, t, h, m| EmployeeShiftPunch::new(id, t, PunchSource::Clock, at(h, m));
        // 8:00-12:00, 12:30-17:00: the legacy sum of in/out pairs is 4 + 4.5.
        // ShiftUtil measures first-to-last (9h) less the 30 minute break.
        let shift = EmployeeShift::new(
            1,
            1,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![
                p(1, PunchType::In, 8, 0),
                p(2, PunchType::Break, 12, 0),
                p(3, PunchType::Back, 12, 30),
                p(4, PunchType::Out, 17, 0),
            ],
        )
        .with_net_hours(7.123);
        assert_eq!(net_hours_legacy(&shift, true), 8.5);
        assert_eq!(shift.net_hours_using(true), 8.5);
        assert_eq!(
            net_hours_legacy(&shift, false),
            7.123,
            "unrounded, unlike ShiftUtil"
        );
        assert_eq!(shift.net_hours_using(false), 7.12);
    }
}
