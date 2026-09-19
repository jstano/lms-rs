//! `RuleType::EarningRate` — eleven concrete rules behind `EarningRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/earningrate/`.
//!
//! The fourth and last of the four rate families — see "Scoping — the rate
//! families" in `PARITY_AUDIT.md`. Unlike `regularrate`/`overtimerate`/
//! `doubletimerate`, `EarningRateRuleImpl` is a **separate root**: it extends
//! the bare `RuleImpl`, not `RateRuleImpl`, and its `execute` takes raw
//! parameters rather than a `RuleItem` — `void execute(TimeCard,
//! EmployeeEarning, Map<String, String>)`. It never touches a shift or a
//! distribution. Read literally this family prices *earnings*, not worked
//! time, and three of its rules (`CalculatedAccrualRateRuleImpl`,
//! `PriorBalancesRateRuleImpl`, and — despite its name — `EarningFixedRateRuleImpl`
//! only incidentally) have nothing to do with hourly wages at all: they price
//! accrual/benefit payouts.
//!
//! `AvgWageEarningRateRule` is a one-implementor marker sub-interface
//! (`FLSAEarningRateRuleImpl`) with no behaviour of its own; it is not ported
//! as a separate Rust trait, the way `ContractHrsRule` folded away in
//! `hoursdistribution`.
//!
//! # What the scoping section got wrong about the missing surface
//!
//! It predicted a `PayRateCalculator`-equivalent port would likely be needed
//! again here, the way `regularrate`'s `AnnualSalaryOverHoursRegRateRuleImpl`
//! needed it (divergence 57). **None of this family's eleven rules call
//! anything like `getPeriodSalary`** — the prediction does not hold for
//! `earningrate`; no `PayRateCalculator` port was added.
//!
//! It also predicted `EmployeeShift.getEmployeeJobStatus()`-style caching
//! surprises and new `Assignment`/`EmployeeJobStatus` surface across the
//! board; in fact everything `regularrate` already proved out
//! (`AssignmentPort`, `MinWagePort`, `Assignment::effective_hourly_pay_rate`,
//! `Employee::home_employee_job_status`/`employee_job_status`) covers every
//! rule but the three named below. What genuinely needed adding, confirmed by
//! reading each rule rather than guessing from the interface list:
//!
//! * `EmployeeJobStatus.contract_days` — a plain `f64`, needed by
//!   `ContractDailyRateRuleImpl` alongside the already-ported `contract_hours`.
//! * `AccrualTransactionPort` (`AccrualTransactionDAO`'s two methods) plus the
//!   new [`AccrualTransaction`](crate::entity::accrual_transaction::AccrualTransaction)
//!   entity, needed by `CalculatedAccrualRateRuleImpl` and
//!   `PriorBalancesRateRuleImpl`.
//! * `HolidayDataPort` (`HolidayDataDAO.getWeekStartOfNthPastWorkedWeek` plus
//!   the raw `EligibleHours` SQL aggregate), needed only by
//!   `AvgDayXWeeksRateRuleImpl`.
//! * `EmployeeEarningPort::banked_rate_for_rule` (`EmployeeEarningDAO.getBankedRateForRule`),
//!   needed only by `CalculatedAccrualRateRuleImpl`'s last-bucket fallback.
//!
//! # The stateful pair
//!
//! `CalculatedAccrualRateRuleImpl` and `PriorBalancesRateRuleImpl` both cache
//! state in a field across calls within one calculation run (Spring
//! `@Scope("prototype")`/`"request")` semantics — a fresh instance per
//! `RuleItem`, living for the run). No rule ported anywhere in the crate
//! before this family has needed that, so [`EarningRateRule::execute`] takes
//! `&mut self` — the one difference from every other rate family's `&self`
//! trait shape — even though nine of the eleven rules never touch their own
//! `&mut`. A per-instance cache is ordinary Rust state on the struct, not
//! interior mutability, because each `RuleItem` already owns one rule
//! instance for the run.
//!
//! # `execute` takes raw params, not a `RuleItem`
//!
//! Every other rate family's rule reads `rule_item.params().fixed(&defaults)`.
//! This family's Java interface has no `RuleItem` parameter at all, so the
//! Rust trait takes `&RuleParams` directly; callers are expected to have
//! already resolved the rule item's params before calling in, and each rule
//! still calls `.fixed(&Config.default_values())` on what it is handed —
//! matching every other family's uniform-`.fixed()` shape (divergence 9)
//! regardless of whether the Java rule remembered to call `fixMap` itself.
//! Most of this family's rules did not call `fixMap` explicitly (only
//! `EarningFixedRateRuleImpl`, `EarningFactorRateRuleImpl` and
//! `EarningOverrideJobRateRuleImpl` do); that inconsistency is a Java quirk
//! this port does not reproduce, the same choice already made for every
//! other family.

pub mod avg_day_x_weeks_rate;
pub mod calculated_accrual_rate;
pub mod config;
pub mod contract_daily_rate;
pub mod earning_factor_rate;
pub mod earning_fixed_rate;
pub mod earning_override_job_rate;
pub mod flsa_earning_rate;
pub mod home_dept_rate;
pub mod home_job_rate;
pub mod prior_balances_rate;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::params::RuleParams;

/// An earning-rate rule. `EarningRateRuleImpl`.
///
/// `&mut self` rather than `&self` — see the module doc's "the stateful
/// pair" for why the whole family shares the signature even though only two
/// of its eleven rules use the mutability.
pub trait EarningRateRule {
    /// Price a standalone earning. `execute(TimeCard, EmployeeEarning,
    /// Map<String, String>)`.
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    );
}

/// Write an earning's rate and recompute its total. `earning.setRate(rate);
/// earning.calcAndSetTotalDollars();`
///
/// The regular/overtime/double-time families' `set_earning_rate` also zeroes
/// `dollars` first (`earning.setDollars(0.0)`); most of this family's rules do
/// not call `setDollars` at all, so that reset is not folded in here — a
/// caller that needs it writes `earning.set_dollars(0.0)` itself, the way
/// [`FlsaEarningRateRule`](flsa_earning_rate::FlsaEarningRateRule) does.
pub fn set_earning_rate(earning: &mut EmployeeEarning, rate: f64) {
    earning.set_rate(rate);
}
