//! `RuleType::ScheduleLunch` — three concrete rules behind `ScheduleLunchRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelunch/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelunch/`.
//!
//! Second of the seven families prioritized for scheduling. Every rule here
//! inserts an unpaid lunch break into a shift that was scheduled without
//! one: subtract the break length from the shift's hours, and optionally
//! push the shift's end time out to cover it.
//!
//! # `execute` collapses to one `RuleItem` argument
//!
//! Java's abstract method is `execute(EmployeeShift, TimeCard, RuleItem,
//! Map<String, String>)` — a `RuleItem` **and** a separately-passed params
//! map, even though every concrete rule reads `ruleItem.getParams()` for
//! nothing but the map handed to it. The Rust trait takes just `&RuleItem`,
//! the same shape every rate family already settled on (divergence 9);
//! `rule_item.params().fixed(&Config.default_values())` reaches the same
//! values.
//!
//! # `addAdjustmentToShift` writes through the existing scalar stand-in
//!
//! `ScheduleLunchRuleImpl.addAdjustmentToShift` builds an
//! `EmployeeShiftAdjustment` (audit fields: `reason`, `changedByUser`,
//! `changedOnDT`, the owning `RuleItem`) and appends it to the shift's
//! adjustment list, which then recomputes `adjHours`/`netHours`. That
//! entity is not ported — `EmployeeShift` already stands in with a plain
//! `adj_hours` scalar (see that module's own doc). This family is the first
//! to *write* through the stand-in rather than only read it:
//! [`EmployeeShift::apply_adjustment`](crate::entity::employee_shift::EmployeeShift::apply_adjustment)
//! reproduces `calcAdjustments`' arithmetic for a `BREAK`-type entry —
//! `adjHours -= adjustment`, `netHours = workedHours + adjHours` — with no
//! audit trail behind it. See divergence 72; `shiftadjustment` extends the
//! same method to the `WORKED` type.
//!
//! # `PlannedShift` and punch mutation are new surface
//!
//! `hasValidShift` needs `EmployeeShift.getPlannedShift()`, and two of the
//! three rules move the shift's own OUT punch outright
//! (`EmployeeShiftPunch.setAllTimes`) rather than adjusting a rounding.
//! Neither existed before this family: see the new
//! [`PlannedShift`](crate::entity::planned_shift::PlannedShift) entity,
//! [`EmployeeShift::punch_index_of_type`](crate::entity::employee_shift::EmployeeShift::punch_index_of_type)
//! and [`PunchCursor::set_all_times`](crate::entity::employee_shift::PunchCursor::set_all_times).
//!
//! # `LunchStartTimeAndLengthRuleImpl` reaches into `shiftearning`
//!
//! Its start-time window check calls
//! `shiftearning.utility.ShiftTimeWindowUtility.getTimeRangesIncludingOverlaps`
//! — ported early, ahead of the rest of that family; see
//! [`shiftearning`](crate::rules::algorithm::shiftearning)'s module doc.
//!
//! Ported cases: `LunchAdjustEndTimeRuleImplTest.groovy`,
//! `LunchSimpleRuleImplTest.groovy`, `LunchStartTimeAndLengthRuleImplTest.groovy`
//! — full spec coverage for all three rules.

pub mod config;
pub mod lunch_adjust_end_time;
pub mod lunch_simple;
pub mod lunch_start_time_and_length;

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A schedule-lunch rule. `ScheduleLunchRuleImpl`.
pub trait ScheduleLunchRule {
    /// `execute(EmployeeShift, TimeCard, RuleItem, Map<String, String>)`.
    fn execute(&self, shift: &mut EmployeeShift, dataset: &dyn TimeCard, rule_item: &RuleItem);
}

/// Whether a shift is even eligible for a lunch rule to touch.
/// `ScheduleLunchRuleImpl.hasValidShift`.
///
/// `shiftHasInAndOutPunch` requires **exactly** two punches, both an IN and
/// an OUT — a shift with a break already punched (four punches) is not
/// "invalid", it is simply out of scope for these rules.
pub fn has_valid_shift(shift: &EmployeeShift) -> bool {
    shift.has_both_times()
        && shift.punch_count() == 2
        && shift.punch_index_of_type(PunchType::In).is_some()
        && shift.punch_index_of_type(PunchType::Out).is_some()
        && shift.planned_shift().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::planned_shift::PlannedShift;
    use joda_rs::{LocalDate, LocalTime};

    fn punch(id: i32, punch_type: PunchType, hour: i32) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(
            id,
            punch_type,
            PunchSource::Manual,
            LocalDate::of(2014, 5, 6).at_time(LocalTime::of(hour, 0, 0)),
        )
    }

    fn valid_shift() -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Schedule,
            vec![punch(1, PunchType::In, 0), punch(2, PunchType::Out, 8)],
        )
        .with_times(
            Some(LocalDate::of(2014, 5, 6).at_time(LocalTime::of(0, 0, 0))),
            Some(LocalDate::of(2014, 5, 6).at_time(LocalTime::of(8, 0, 0))),
        )
        .with_planned_shift(Some(PlannedShift::new(
            LocalDate::of(2014, 5, 6).at_time(LocalTime::of(0, 0, 0)),
            8.0,
        )))
    }

    #[test]
    fn a_shift_with_two_punches_both_times_and_a_planned_shift_is_valid() {
        assert!(has_valid_shift(&valid_shift()));
    }

    #[test]
    fn a_shift_missing_its_planned_shift_is_not_valid() {
        let shift = valid_shift().with_planned_shift(None);
        assert!(!has_valid_shift(&shift));
    }

    #[test]
    fn a_shift_with_only_one_punch_is_not_valid() {
        let shift = EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Schedule,
            vec![punch(1, PunchType::In, 0)],
        )
        .with_times(
            Some(LocalDate::of(2014, 5, 6).at_time(LocalTime::of(0, 0, 0))),
            Some(LocalDate::of(2014, 5, 6).at_time(LocalTime::of(8, 0, 0))),
        )
        .with_planned_shift(Some(PlannedShift::new(
            LocalDate::of(2014, 5, 6).at_time(LocalTime::of(0, 0, 0)),
            8.0,
        )));

        assert!(!has_valid_shift(&shift));
    }

    #[test]
    fn a_shift_missing_either_time_is_not_valid() {
        let shift = valid_shift().with_times(
            None,
            Some(LocalDate::of(2014, 5, 6).at_time(LocalTime::of(8, 0, 0))),
        );
        assert!(!has_valid_shift(&shift));
    }
}
