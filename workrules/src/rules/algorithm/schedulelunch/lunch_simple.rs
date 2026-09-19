//! Port of `LunchSimpleRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelunch/LunchSimpleRuleImpl.java`.
//!
//! The simplest of the three: if the shift is valid, was scheduled longer
//! than `minHrsWorked`, and has at least `hrsAdjustment` net hours to give
//! up, subtract the break from the shift's hours. Unlike its two siblings,
//! it never touches a punch — the break is purely a deduction, with no
//! corresponding change to when the shift is recorded as starting or
//! ending.
//!
//! Ported cases: `LunchSimpleRuleImplTest.groovy` (one `where:` table, seven
//! rows).

use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelunch::config::{
    HRS_ADJUSTMENT_PROP, LunchSimpleRuleConfig, MIN_HOURS_PROP,
};
use crate::rules::algorithm::schedulelunch::{ScheduleLunchRule, has_valid_shift};
use crate::rules::rule_config::RuleConfig;

/// `LunchSimpleRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchSimpleRule;

impl ScheduleLunchRule for LunchSimpleRule {
    fn execute(&self, shift: &mut EmployeeShift, _dataset: &dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&LunchSimpleRuleConfig.default_values());
        let min_hours = params.double_at(MIN_HOURS_PROP);
        let hrs_adjustment = params.double_at(HRS_ADJUSTMENT_PROP);

        let should_run = has_valid_shift(shift)
            && shift
                .planned_shift()
                .is_some_and(|planned| planned.duration() > min_hours)
            && shift.net_hours() >= hrs_adjustment;

        if should_run {
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
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn punch(id: i32, punch_type: PunchType, time: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Manual, time)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(2, 1, "Lunch simple", RuleClass::LunchSimple, params)
    }

    fn shift_with(
        start: Option<LocalDateTime>,
        end: Option<LocalDateTime>,
        punches: Vec<EmployeeShiftPunch>,
        planned_duration: f64,
        net_hours: f64,
    ) -> EmployeeShift {
        let shift_date = LocalDate::of(2014, 5, 6);
        let planned = end.map(|end_time| PlannedShift::new(end_time, planned_duration));
        EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, punches)
            .with_times(start, end)
            .with_planned_shift(planned)
            .with_worked_hours(net_hours)
            .with_net_hours(net_hours)
    }

    mod java_parity_tests {
        use super::*;

        fn both_punches() -> Vec<EmployeeShiftPunch> {
            vec![
                punch(1, PunchType::In, LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
                punch(2, PunchType::Out, LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
            ]
        }

        fn one_punch() -> Vec<EmployeeShiftPunch> {
            vec![punch(
                1,
                PunchType::In,
                LocalDateTime::of(2014, 5, 6, 0, 0, 0),
            )]
        }

        fn params(adj: &str) -> RuleParams {
            rule_params! { MIN_HOURS_PROP => "7.0", HRS_ADJUSTMENT_PROP => adj }
        }

        /// `LunchSimpleRuleImplTest`, row: "shift doesn't have both times"
        /// (three variants: missing end, missing start, missing both).
        #[test]
        fn no_adjustment_when_the_shift_is_missing_a_time() {
            let dataset = TimeCardData::new();
            let start = Some(LocalDateTime::of(2014, 5, 6, 0, 0, 0));
            let end = Some(LocalDateTime::of(2014, 5, 6, 8, 0, 0));

            for (s, e) in [(start, None), (None, end), (None, None)] {
                let mut shift = shift_with(s, e, both_punches(), 8.0, 0.0);
                LunchSimpleRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));
                assert_eq!(shift.adj_hours(), 0.0, "start={s:?} end={e:?}");
            }
        }

        /// `LunchSimpleRuleImplTest`, row: "shift doesn't only have an in
        /// and out punch".
        #[test]
        fn no_adjustment_with_only_one_punch() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with(
                Some(LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
                one_punch(),
                8.0,
                8.0,
            );

            LunchSimpleRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchSimpleRuleImplTest`, row: "shift doesn't meet min required
        /// hours".
        #[test]
        fn no_adjustment_when_the_shift_is_shorter_than_the_minimum() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with(
                Some(LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 7, 0, 0)),
                both_punches(),
                7.0,
                7.0,
            );

            LunchSimpleRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchSimpleRuleImplTest`, row: "adjustment longer than shift".
        #[test]
        fn no_adjustment_when_the_break_is_longer_than_the_shifts_net_hours() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with(
                Some(LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
                both_punches(),
                8.0,
                8.0,
            );

            LunchSimpleRule.execute(&mut shift, &dataset, &rule_item(params("9.0")));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchSimpleRuleImplTest`, row: "shift length greater than min
        /// required hours".
        #[test]
        fn an_adjustment_is_applied_when_every_condition_is_met() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with(
                Some(LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
                both_punches(),
                8.0,
                8.0,
            );

            LunchSimpleRule.execute(&mut shift, &dataset, &rule_item(params("1.5")));

            assert_eq!(shift.adj_hours(), -1.5);
        }
    }
}
