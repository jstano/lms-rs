//! Port of `LunchStartTimeAndLengthRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelunch/LunchStartTimeAndLengthRuleImpl.java`.
//!
//! Gates on the *scheduled* shift's own start time and length, not the
//! actual shift's: `isShiftStartTimeValid` reads
//! `shift.getPlannedShift().getStartDateTime()`, and `isShiftLengthValid`
//! reads `shift.getPlannedShift().getDuration()`. Only the final net-hours
//! floor (`shift.getNetHours() - breakLength >= 0`) reads the actual shift.
//!
//! The start-time check tolerates a configured window crossing midnight by
//! checking the planned start against three candidate windows — the day
//! before, on, and after the planned shift's own date — via
//! [`shiftearning`'s `ShiftTimeWindowUtility`](crate::rules::algorithm::shiftearning::utility::shift_time_window_utility).
//!
//! Unlike its two siblings, `adjustEndTime` is configurable rather than
//! hardcoded: when set, the shift's own current end time (not the planned
//! shift's) is pushed out by the break length before the punch moves.
//!
//! Ported cases: `LunchStartTimeAndLengthRuleImplTest.groovy` (one `where:`
//! table, eleven rows).

use crate::common::enums::punch_type::PunchType;
use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelunch::config::{
    ADJUST_END_TIME, BREAK_LENGTH, EARLIEST_START_TIME, LATEST_START_TIME,
    LunchStartTimeAndLengthRuleConfig, MAXIMUM_SHIFT_LENGTH, MINIMUM_SHIFT_LENGTH,
};
use crate::rules::algorithm::schedulelunch::{ScheduleLunchRule, has_valid_shift};
use crate::rules::algorithm::shiftearning::utility::shift_time_window_utility::time_ranges_including_overlaps;
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalTime;

/// `LunchStartTimeAndLengthRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LunchStartTimeAndLengthRule;

impl LunchStartTimeAndLengthRule {
    /// `isShiftStartTimeValid`.
    fn is_shift_start_time_valid(&self, shift: &EmployeeShift, params: &RuleParams) -> bool {
        let Some(planned) = shift.planned_shift() else {
            return false;
        };
        let min_start_time =
            LocalTime::parse(params.get(EARLIEST_START_TIME).unwrap_or("00:00:00"));
        let max_start_time = LocalTime::parse(params.get(LATEST_START_TIME).unwrap_or("00:00:00"));
        let start_date_time = planned.start_date_time();

        time_ranges_including_overlaps(
            start_date_time.to_local_date(),
            min_start_time,
            max_start_time,
        )
        .iter()
        .any(|window| window.contains(start_date_time))
    }

    /// `isShiftLengthValid`.
    fn is_shift_length_valid(&self, shift: &EmployeeShift, params: &RuleParams) -> bool {
        let Some(planned) = shift.planned_shift() else {
            return false;
        };
        let min_shift_length = params.double_at(MINIMUM_SHIFT_LENGTH);
        let max_shift_length = params.double_at(MAXIMUM_SHIFT_LENGTH);
        let break_length = params.double_at(BREAK_LENGTH);
        let shift_length = planned.duration();

        shift_length >= min_shift_length
            && shift_length <= max_shift_length
            && (shift.net_hours() - break_length) >= 0.0
    }
}

impl ScheduleLunchRule for LunchStartTimeAndLengthRule {
    fn execute(&self, shift: &mut EmployeeShift, _dataset: &dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&LunchStartTimeAndLengthRuleConfig.default_values());

        let should_run = has_valid_shift(shift)
            && self.is_shift_start_time_valid(shift, &params)
            && self.is_shift_length_valid(shift, &params);

        if !should_run {
            return;
        }

        let break_length = params.double_at(BREAK_LENGTH);

        if params.bool_at(ADJUST_END_TIME) {
            let break_length_minutes = (break_length * 60.0) as i64;
            let new_end_time = shift
                .end_date_time()
                .map(|end| end.plus_minutes(break_length_minutes));
            let out_index = shift
                .punch_index_of_type(PunchType::Out)
                .expect("has_valid_shift guarantees an OUT punch");
            shift.punch_cursor(out_index).set_all_times(new_end_time);
        }

        shift.apply_adjustment(break_length, ShiftAdjustType::Break);
        shift.mark_schedule_lunch_adjustment();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    fn punch(id: i32, punch_type: PunchType, time: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Manual, time)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            2,
            1,
            "Lunch start time and length",
            RuleClass::LunchStartTimeAndLength,
            params,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn props(
        min_length: &str,
        max_length: &str,
        earliest_start: &str,
        latest_start: &str,
        break_length: &str,
        adjust_end_time: &str,
    ) -> RuleParams {
        rule_params! {
            MINIMUM_SHIFT_LENGTH => min_length,
            MAXIMUM_SHIFT_LENGTH => max_length,
            EARLIEST_START_TIME => earliest_start,
            LATEST_START_TIME => latest_start,
            BREAK_LENGTH => break_length,
            ADJUST_END_TIME => adjust_end_time
        }
    }

    fn shift_with(start: Option<LocalDateTime>, end: Option<LocalDateTime>) -> EmployeeShift {
        let duration = match (start, end) {
            (Some(s), Some(e)) => {
                date_range_rs::datetimerange::date_time_range::DateTimeRange::of(s, e)
                    .duration()
                    .fractional_hours()
            }
            _ => 0.0,
        };
        let punches = vec![
            punch(
                1,
                PunchType::In,
                start.unwrap_or(LocalDateTime::of(2014, 5, 6, 0, 0, 0)),
            ),
            punch(
                2,
                PunchType::Out,
                end.unwrap_or(LocalDateTime::of(2014, 5, 6, 8, 0, 0)),
            ),
        ];
        let planned = start.map(|s| PlannedShift::new(s, duration));

        EmployeeShift::new(1, 100, 1, s_date(start), ShiftType::Schedule, punches)
            .with_times(start, end)
            .with_planned_shift(planned)
            .with_worked_hours(duration)
            .with_net_hours(duration)
    }

    fn s_date(start: Option<LocalDateTime>) -> joda_rs::LocalDate {
        start.map_or(joda_rs::LocalDate::of(2014, 5, 6), |s| s.to_local_date())
    }

    mod java_parity_tests {
        use super::*;

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift does not have both
        /// times" (three variants).
        #[test]
        fn no_adjustment_when_the_shift_is_missing_a_time() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let params = props("", "", "00:00:00", "23:59:00", "", "false");

            for (s, e) in [(Some(start), None), (None, Some(end)), (None, None)] {
                let mut shift = shift_with(s, e);
                LunchStartTimeAndLengthRule.execute(
                    &mut shift,
                    &dataset,
                    &rule_item(params.clone()),
                );
                assert_eq!(shift.adj_hours(), 0.0, "start={s:?} end={e:?}");
            }
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift meets rules
        /// requirements".
        #[test]
        fn an_adjustment_and_punch_move_when_requirements_are_met() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("8.0", "10.0", "00:00:00", "23:59:00", "1.0", "false");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), -1.0);
            let out_index = shift.punch_index_of_type(PunchType::Out).unwrap();
            // adjustEndTime is false, so the OUT punch never moves.
            assert_eq!(shift.punch(out_index).punch_time(), Some(end));
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift meets rules
        /// requirements spans midnight".
        #[test]
        fn requirements_met_when_the_shift_spans_midnight() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 22, 0, 0);
            let end = LocalDateTime::of(2014, 5, 7, 3, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("4.0", "10.0", "17:00:00", "01:00:00", "1.0", "false");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), -1.0);
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift before earliest
        /// start time".
        #[test]
        fn no_adjustment_when_the_shift_starts_before_the_earliest_start_time() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("8.0", "10.0", "10:00:00", "23:59:00", "1.0", "false");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift shorter than the
        /// minimun shift time".
        #[test]
        fn no_adjustment_when_the_shift_is_shorter_than_the_minimum() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("10.0", "12.0", "00:00:00", "23:59:00", "1.0", "false");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "shift longer than the
        /// maximum shift time".
        #[test]
        fn no_adjustment_when_the_shift_is_longer_than_the_maximum() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 9, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("0.0", "8.0", "00:00:00", "23:59:00", "1.0", "false");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "Add time to shift end if
        /// match" — `adjustEndTime` true moves the OUT punch.
        #[test]
        fn the_out_punch_moves_when_adjust_end_time_is_set() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 8, 0, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("8.0", "10.0", "00:00:00", "23:59:00", "1.5", "true");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), -1.5);
            let out_index = shift.punch_index_of_type(PunchType::Out).unwrap();
            let expected = LocalDateTime::of(2014, 5, 6, 9, 30, 0);
            assert_eq!(shift.punch(out_index).punch_time(), Some(expected));
            assert_eq!(shift.punch(out_index).adj_time(), Some(expected));
            assert_eq!(shift.punch(out_index).rounded_time(), Some(expected));
        }

        /// `LunchStartTimeAndLengthRuleImplTest`, "break length greater than
        /// shift length".
        #[test]
        fn no_adjustment_when_the_break_is_longer_than_the_shift() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 5, 6, 0, 0, 0);
            let end = LocalDateTime::of(2014, 5, 6, 0, 30, 0);
            let mut shift = shift_with(Some(start), Some(end));
            let params = props("0.0", "1.0", "00:00:00", "23:59:00", "1.0", "true");

            LunchStartTimeAndLengthRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }
    }
}
