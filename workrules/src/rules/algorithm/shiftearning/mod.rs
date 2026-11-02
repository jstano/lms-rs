//! `RuleType::ShiftDifferential` — 23 rules behind `ShiftEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftearning/`,
//! runner at `.../server/labor/calcshift/rules/ShiftDifferentialRunner.java`.
//!
//! The last of the seven families prioritized for scheduling, scoped in
//! `PARITY_AUDIT.md` ("Scoping — `shiftearning`") and ported in five slices.
//! **Slices 1-3** are here: the shared base, nine simple rules, the four
//! break and window rules, and the three time-of-day rules.
//!
//! # Per shift
//!
//! `execute(EmployeeShift, TimeCard, RuleItem)` runs once for each shift. The
//! rule reads the shift and writes earnings onto the card, so it takes the shift
//! by shared reference and the card mutably; the runner (slice 5) hands it a
//! clone of the card's shift, as `ShiftCorrectionRunner` does.
//!
//! # The base class's pricing helpers
//!
//! `ShiftEarningRuleImpl` has `getEarningRate` (a job's rate on a date, floored
//! at minimum wage if the earning type says so), `getEarningRateMinimumWage`
//! and `getEarningRateHomeJob`. They are
//! [`earning_rate_for_job`](crate::rules::algorithm::dailyearning::earning_rate_for_job),
//! [`MinWagePort::min_wage`](crate::rules::ports::MinWagePort::min_wage) and
//! [`earning_rate_home_job`](crate::rules::algorithm::dailyearning::earning_rate_home_job),
//! shared with `dailyearning`.

pub mod break_period_penalty;
pub mod break_period_penalty_hours;
pub mod config;
pub mod daily_spread_shift_differential;
pub mod dow_flat;
pub mod dow_rate_factor;
pub mod hours_threshold_fixed;
pub mod hours_threshold_fixed_with_max_cap;
pub mod minimum_hours_worked_shift;
pub mod no_earning;
pub mod paid_break_fixed_hrs;
pub mod shift_start_time_flat;
pub mod start_end_between_premium;
pub mod time_between_shifts;
pub mod time_of_day_differential_rate;
pub mod time_of_day_pct_differential_rate;
pub mod utility;
pub mod weekly_job_hrs_fixed;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A shift-differential rule. `ShiftEarningRuleImpl`.
pub trait ShiftEarningRule {
    /// `execute(EmployeeShift, TimeCard, RuleItem)`.
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem);
}

/// The gate most of the family opens with:
/// `shift.getErrors().isEmpty() && dataset.isOpenForEditingFor(shift)`.
pub fn shift_is_eligible(shift: &EmployeeShift, time_card: &dyn TimeCard) -> bool {
    !shift.has_errors() && time_card.is_open_for_editing_for_shift(shift)
}

/// The earning every rule here starts from: a RULE earning on the shift's
/// employee and job, dated and paid on the shift's date, tagged with the rule
/// item (id **and** class) and, when `attach_shift`, with the shift itself.
pub fn new_shift_earning(
    shift: &EmployeeShift,
    earning_type_id: i32,
    rule_item: &RuleItem,
    attach_shift: bool,
) -> EmployeeEarning {
    let date = shift.shift_date();
    let mut earning = EmployeeEarning::new(
        0,
        shift.employee_id(),
        shift.job_id(),
        earning_type_id,
        date,
        0.0,
        0.0,
        crate::common::enums::earning_source::EarningSource::Rule,
    )
    .from_rule_item(rule_item, attach_shift.then(|| shift.id()));
    earning.set_pay_date(Some(date));
    earning
}
