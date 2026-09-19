//! `RuleType::ShiftAdjust` — seven concrete rules behind `ShiftAdjustmentRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftadjustment/`.
//!
//! Third of the seven families prioritized for scheduling. Same
//! adjustment-writing shape as `schedulelunch` — every rule here also ends
//! in a write through [`EmployeeShift::apply_adjustment`](crate::entity::employee_shift::EmployeeShift::apply_adjustment) —
//! but this family is the first to need the **`WORKED`** adjustment type
//! (`DSTAdjustmentRuleImpl`, `MinDailyHrsRuleImpl`) alongside `BREAK`, which
//! is why `apply_adjustment` takes an explicit [`ShiftAdjustType`](crate::common::enums::shift_adjust_type::ShiftAdjustType)
//! rather than being break-only as `schedulelunch` first wrote it.
//!
//! # `execute` collapses to `EmployeeShift` + `TimeCard` + `RuleItem`
//!
//! Java's abstract method carries a `RuleItem` **and** a separately-passed
//! `ruleDescription` string **and** a separately-passed params map — three
//! redundant ways to reach values the `RuleItem` already carries (divergence
//! 9/75's shape, a third time).
//!
//! # A sign convention worth reading twice: stored `adjHours`, not a delta
//!
//! `AutoBreakRuleImpl` stores `-hrsAdjustment` (negative) on a `BREAK`-type
//! adjustment. `calcAdjustments`' `BREAK` branch is `adjHours -=
//! adj.getAdjHours()`, so subtracting a negative number **increases** the
//! shift's total `adjHours` — `AutoBreakRuleImpl`'s net effect is to *add*
//! `hrsAdjustment` to net hours, not remove it. `apply_adjustment`'s `hours`
//! parameter is always the adjustment's own stored value, sign and all —
//! never a magnitude the caller expects to be subtracted or added by
//! convention. See that method's own doc and divergence 76's sibling
//! findings.
//!
//! # `hasNoScheduleLunchAdjustment` narrows to a flag
//!
//! `AutoBreakRuleImpl` skips itself if the shift already carries an
//! adjustment created by a `schedulelunch` rule — checked in Java by walking
//! the adjustments list for one whose `RuleItem.getRuleSet().getRuleType()
//! == SCHEDULE_LUNCH`. Nothing in this crate carries that provenance (no
//! adjustments list at all — see `schedulelunch`'s own doc), so
//! [`EmployeeShift::has_schedule_lunch_adjustment`](crate::entity::employee_shift::EmployeeShift::has_schedule_lunch_adjustment)
//! narrows the question to "has any `schedulelunch` rule touched this
//! shift", set by those rules themselves rather than inferred after the
//! fact. See divergence 76.
//!
//! # `MinDailyHrsRuleImpl` and `DSTAdjustmentRuleImpl` build `WORKED`
//! adjustments manually, not through a shared helper
//!
//! Unlike `AutoBreakRuleImpl`/`MinBreakRuleImpl`/`PaidBreakRuleImpl`/`TotalBreakLengthRuleImpl`,
//! which all end up at `BREAK` (three through `ShiftAdjustmentRuleImpl.createAdjustment`,
//! which hardcodes `BREAK` and rounds with `roundRawHours`; `AutoBreakRuleImpl`
//! inline but still `BREAK`), these two build `EmployeeShiftAdjustment`
//! directly with `adjType = WORKED` and no `roundRawHours` call on the
//! stored value — only `MinDailyHrsRuleImpl` rounds, and with `TDouble.round(_,
//! 2)`, not `roundRawHours`. Each rule's own rounding is reproduced exactly
//! rather than folded into one shared helper.
//!
//! Ported cases: `AutoBreakRuleImplTest.groovy`, `MinBreakRuleImplTest.groovy`,
//! `PaidBreakRuleImplTest.groovy`, `TotalBreakLengthRuleImplTest.groovy` —
//! `DSTAdjustmentRuleImpl`, `MinDailyHrsRuleImpl` and `NoAdjustmentRuleImpl`
//! have no Groovy spec; their behaviour tests are written from the Java.

pub mod auto_break;
pub mod config;
pub mod dst_adjustment;
pub mod min_break;
pub mod min_daily_hrs;
pub mod no_adjustment;
pub mod paid_break;
pub mod total_break_length;

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A shift-adjustment rule. `ShiftAdjustmentRuleImpl`.
pub trait ShiftAdjustmentRule {
    /// `execute(EmployeeShift, TimeCard, RuleItem, String, Map<String, String>)`.
    fn execute(&self, shift: &mut EmployeeShift, dataset: &dyn TimeCard, rule_item: &RuleItem);
}

/// The gate `AutoBreakRuleImpl`/`MinBreakRuleImpl`/`PaidBreakRuleImpl`/`TotalBreakLengthRuleImpl`/
/// `DSTAdjustmentRuleImpl` all open with. `shift.getErrors().isEmpty() &&
/// shift.hasBothTimes()`.
pub fn shift_is_valid(shift: &EmployeeShift) -> bool {
    !shift.has_errors() && shift.has_both_times()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use joda_rs::{LocalDate, LocalDateTime};

    fn shift(errors: Vec<ShiftErrorType>, both_times: bool) -> EmployeeShift {
        let shift = EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_errors(errors);
        if both_times {
            shift.with_times(
                Some(LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 16, 0, 0)),
            )
        } else {
            shift
        }
    }

    #[test]
    fn a_shift_with_no_errors_and_both_times_is_valid() {
        assert!(shift_is_valid(&shift(Vec::new(), true)));
    }

    #[test]
    fn a_shift_with_errors_is_not_valid() {
        assert!(!shift_is_valid(&shift(
            vec![ShiftErrorType::MissingOut],
            true
        )));
    }

    #[test]
    fn a_shift_missing_a_time_is_not_valid() {
        assert!(!shift_is_valid(&shift(Vec::new(), false)));
    }
}
