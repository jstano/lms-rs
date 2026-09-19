//! Port of `LunchAdjustEndTimeRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelunch/LunchAdjustEndTimeRuleImpl.java`.
//!
//! Same eligibility test as its sibling `LunchSimpleRuleImpl` — same config
//! shape too, see `config.rs` — but this one also pushes the shift's OUT
//! punch out by the break length before deducting it, so the break shows up
//! as extra time at the end of the shift rather than a pure reduction.
//!
//! Ported cases: `LunchAdjustEndTimeRuleImplTest.groovy` (one `where:` table,
//! eight rows).

use crate::common::enums::punch_type::PunchType;
use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelunch::config::{
    HRS_ADJUSTMENT_PROP, LunchAdjustEndTimeRuleConfig, MIN_HOURS_PROP,
};
use crate::rules::algorithm::schedulelunch::{ScheduleLunchRule, has_valid_shift};
use crate::rules::rule_config::RuleConfig;

/// `LunchAdjustEndTimeRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchAdjustEndTimeRule;

impl ScheduleLunchRule for LunchAdjustEndTimeRule {
    fn execute(&self, shift: &mut EmployeeShift, _dataset: &dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&LunchAdjustEndTimeRuleConfig.default_values());
        let min_hours = params.double_at(MIN_HOURS_PROP);
        let hrs_adjustment = params.double_at(HRS_ADJUSTMENT_PROP);
        let seconds_adjustment = (hrs_adjustment * 3600.0) as i64;

        let should_run = has_valid_shift(shift)
            && shift
                .planned_shift()
                .is_some_and(|planned| planned.duration() > min_hours)
            && shift.net_hours() >= hrs_adjustment;

        if should_run {
            let new_end_time = shift
                .end_date_time()
                .map(|end| end.plus_seconds(seconds_adjustment));
            let out_index = shift
                .punch_index_of_type(PunchType::Out)
                .expect("has_valid_shift guarantees an OUT punch");
            shift.punch_cursor(out_index).set_all_times(new_end_time);

            shift.apply_adjustment(hrs_adjustment, ShiftAdjustType::Break);
            shift.mark_schedule_lunch_adjustment();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn punch(id: i32, punch_type: PunchType, time: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Manual, time)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            2,
            1,
            "Lunch adjust end time",
            RuleClass::LunchAdjustEndTime,
            params,
        )
    }

    fn both_punches(start: LocalDateTime, end: LocalDateTime) -> Vec<EmployeeShiftPunch> {
        vec![
            punch(1, PunchType::In, start),
            punch(2, PunchType::Out, end),
        ]
    }

    fn shift_with(
        start: Option<LocalDateTime>,
        end: Option<LocalDateTime>,
        punches: Vec<EmployeeShiftPunch>,
        has_planned_shift: bool,
        net_hours: f64,
    ) -> EmployeeShift {
        let planned = (has_planned_shift && end.is_some())
            .then(|| crate::entity::planned_shift::PlannedShift::new(end.unwrap(), net_hours));
        EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Schedule,
            punches,
        )
        .with_times(start, end)
        .with_planned_shift(planned)
        .with_worked_hours(net_hours)
        .with_net_hours(net_hours)
    }

    mod java_parity_tests {
        use super::*;

        fn params(adj: &str) -> RuleParams {
            rule_params! { MIN_HOURS_PROP => "7.0", HRS_ADJUSTMENT_PROP => adj }
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "shift doesn't have both times"
        /// (three variants).
        #[test]
        fn no_adjustment_when_the_shift_is_missing_a_time() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);

            for (s, e) in [(Some(start), None), (None, Some(end)), (None, None)] {
                let punches = both_punches(start, end);
                let mut shift = shift_with(s, e, punches, true, 0.0);
                LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));
                assert_eq!(shift.adj_hours(), 0.0, "start={s:?} end={e:?}");
            }
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "shift doesn't only have an in
        /// and out punch".
        #[test]
        fn no_adjustment_with_only_one_punch() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(
                Some(start),
                Some(end),
                vec![punch(1, PunchType::In, start)],
                true,
                8.0,
            );

            LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "shift doesn't have a
        /// plannedShift".
        #[test]
        fn no_adjustment_without_a_planned_shift() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift =
                shift_with(Some(start), Some(end), both_punches(start, end), false, 8.0);

            LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "shift doesn't meet min
        /// required hours".
        #[test]
        fn no_adjustment_when_the_shift_is_shorter_than_the_minimum() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 7, 0, 0);
            let mut shift = shift_with(Some(start), Some(end), both_punches(start, end), true, 7.0);

            LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "adjustment longer than shift".
        #[test]
        fn no_adjustment_when_the_break_is_longer_than_net_hours() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end), both_punches(start, end), true, 8.0);

            LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("9.0")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchAdjustEndTimeRuleImplTest`, "shift length greater than min
        /// required hours" — the punch moves and the adjustment applies.
        #[test]
        fn the_out_punch_moves_and_the_adjustment_applies() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end), both_punches(start, end), true, 8.0);

            LunchAdjustEndTimeRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), -1.5);
            let out_index = shift.punch_index_of_type(PunchType::Out).unwrap();
            let expected = LocalDateTime::of(2014, 5, 6, 9, 30, 0);
            assert_eq!(shift.punch(out_index).punch_time(), Some(expected));
            assert_eq!(shift.punch(out_index).adj_time(), Some(expected));
            assert_eq!(shift.punch(out_index).rounded_time(), Some(expected));
        }
    }
}
