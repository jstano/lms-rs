//! `RuleType::RegularRate` — eight concrete rules behind `RegularRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/regularrate/`.
//!
//! The first of the four rate families — see "Scoping — the rate families" in
//! `PARITY_AUDIT.md`. One `RuleType`, several selectable rule classes, the same
//! shape as `punchrounding`, not `hoursdistribution`'s one-`RuleType`-per-rule
//! shape.
//!
//! # `AnnualSalaryOverHoursRegRateRuleImpl` is not ported
//!
//! It backs an hourly rate out of `PayRateCalculator.getPeriodSalary(Employee,
//! LocalDate)` — a calculator service, not a DAO query, and not scoped. Every
//! other dependency the scoping section flagged for this family turned out to
//! already exist (see the findings below), but this one does not, so this rule
//! is deferred with the family the way `ShiftDifferenceOTRuleImpl` was deferred
//! from `hoursdistribution`: it has a catalogue entry (`AsohwRrr`) but no
//! algorithm here yet.
//!
//! # What the scoping section got wrong about the missing surface
//!
//! "Property week/pay-period accessors + `PayPeriodType`" turned out to need no
//! new entity work at all:
//!
//! * `TimeCard::current_pay_period`/`pay_period_type` already stand in for
//!   `Property.getPayPeriod()`/`getPayPeriodType()` (divergence 24) —
//!   `AnnualSalaryOverHoursRegRateRuleImpl`'s `employee.getPayGroup().currentPayPeriod()`
//!   reaches the same value.
//! * `Property.getCurrentWeek().getDateRangeContainingDate(date)` —
//!   `CombinationJobsRegRateRuleImpl`'s only property dependency — is exactly
//!   `WeeklyDateRange::with_end_date(property.period_end_date(id)).range_containing_date(date)`,
//!   which is already how [`RegularHoursByWorkWeekRule`](crate::rules::algorithm::regularhoursdistribution::reg_hours_by_work_week::RegularHoursByWorkWeekRule)
//!   reaches the property, through [`PropertyPort`](crate::rules::ports::PropertyPort).
//!
//! `EmployeeShift.getEmployeeJobStatus()` was also flagged as possibly cached;
//! it is not — it is exactly `TimeCard::employee_job_status_for_shift`, already
//! ported. `EmployeeShift.getWorkedHours()` was flagged as possibly an alias of
//! `net_hours`; it is not, and is already its own field
//! ([`EmployeeShift::worked_hours`](crate::entity::employee_shift::EmployeeShift::worked_hours)).
//!
//! What genuinely needed adding: `EmployeeShift.reg_rate`, `AssignmentPayRate`
//! plus `Assignment::effective_hourly_pay_rate`, and the `ShiftCategory` entity
//! plus `EmployeeShift.shift_category_id`.

pub mod combination_jobs_reg_rate;
pub mod config;
pub mod factor_job_reg_rate;
pub mod home_dept_reg_rate;
pub mod home_job_reg_rate;
pub mod job_reg_rate;
pub mod shift_category_min_wage_reg_rate;
pub mod shift_category_reg_rate;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A regular-rate rule. `RegularRateRuleImpl extends RateRuleImpl`.
///
/// Both overloads stay on the trait rather than collapsing to one: they are
/// two genuinely different write targets (a distribution's `baseRate` versus
/// an earning's `rate`), and in the Java the earning overload is usually a
/// near-duplicate of the shift one rather than a thin wrapper around it.
pub trait RegularRateRule {
    /// Price a shift's hours distribution. `execute(EmployeeShift,
    /// HoursDistribution, TimeCard, RuleItem)`.
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    );

    /// Price a standalone earning. `execute(EmployeeEarning, TimeCard,
    /// RuleItem)`.
    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    );
}

/// Write a distribution's regular rate and the rule item that set it.
/// `BaseRegularRateRuleImpl.setRegularRates`.
pub fn set_regular_rates(distribution: &mut HoursDistribution, rate: f64, rule_item: &RuleItem) {
    distribution.set_base_rate(rate);
    distribution.set_rate_rule_item_id(Some(rule_item.id()));
}

/// Write an earning's rate. `earning.setRate(rate); earning.setDollars(0.0);
/// earning.calcAndSetTotalDollars();`
///
/// The explicit `calcAndSetTotalDollars()` Java calls last is redundant:
/// `setDollars` already recomputes the total, and every concrete rule in this
/// family follows this same three-line shape. Kept as one helper so a rule's
/// earning overload is one call, not three.
pub fn set_earning_rate(earning: &mut EmployeeEarning, rate: f64) {
    earning.set_rate(rate);
    earning.set_dollars(0.0);
}
