//! Port of `ShiftCategoryCorrectionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftcorrection/ShiftCategoryCorrectionRuleImpl.java`.
//!
//! Re-tags a worked shift with the category of the schedule it was meant to
//! cover: the schedule whose start falls inside a window around the IN
//! punch's adjusted time, and, among those, whose start is nearest it.
//!
//! # The window's two graces are swapped
//!
//! Java names them backwards and the behaviour follows the code, not the
//! names: the window opens `gracePostSchedStart` minutes **before** the
//! punch (`preShiftStart = adjTime - gracePost`) and closes
//! `gracePreSchedStart` minutes **after** it (`postShiftStart = adjTime +
//! gracePre`). With the default 60/60 nobody can tell; with unequal values
//! the "pre" parameter extends the window *later*. Reproduced as written —
//! divergence 82.
//!
//! The window is inclusive at both ends (`DateTimePeriod.containsDateTime`).
//! A tie on distance keeps the **earlier-listed** schedule (strict `<`).
//!
//! The rule does nothing if any punch on the shift is `MANUAL`: it only
//! corrects shifts the clock produced.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::dates::{duration_in_fractional_hours, is_between};
use crate::common::enums::punch_source::PunchSource;
use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftcorrection::ShiftCorrectionRule;
use crate::rules::algorithm::shiftcorrection::config::{
    GRACE_POST_START_PROP, GRACE_PRE_START_PROP, MATCH_JOB, ShiftCategoryCorrectionRuleConfig,
};
use crate::rules::rule_config::RuleConfig;

/// `ShiftCategoryCorrectionRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftCategoryCorrectionRule;

impl ShiftCorrectionRule for ShiftCategoryCorrectionRule {
    fn execute(
        &self,
        shift: &mut EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&ShiftCategoryCorrectionRuleConfig.default_values());
        let grace_pre_start = params.int_at(GRACE_PRE_START_PROP);
        let grace_post_start = params.int_at(GRACE_POST_START_PROP);
        let match_job = params.bool_at(MATCH_JOB);

        let mut in_adj_time = None;
        for punch in shift.punches() {
            if punch.source() == PunchSource::Manual {
                return;
            }
            if punch.punch_type() == PunchType::In {
                in_adj_time = Some(punch.adj_time());
            }
        }
        // Java dereferences the IN's adjTime unconditionally.
        let Some(Some(in_adj)) = in_adj_time else {
            return;
        };

        let window_start = in_adj.minus_minutes(i64::from(grace_post_start));
        let window_end = in_adj.plus_minutes(i64::from(grace_pre_start));

        let mut closest: Option<&EmployeeShift> = None;
        for schedule in time_card.schedules() {
            let Some(start) = schedule.start_date_time() else {
                continue;
            };
            if !is_between(start, window_start, window_end) {
                continue;
            }
            if match_job && shift.job_id() != schedule.job_id() {
                continue;
            }
            let is_closer = |candidate: &EmployeeShift, current: &EmployeeShift| {
                let distance = |s: &EmployeeShift| {
                    s.start_date_time().map_or(f64::INFINITY, |t| {
                        duration_in_fractional_hours(in_adj, t).abs()
                    })
                };
                distance(candidate) < distance(current)
            };
            if closest.is_none_or(|c| is_closer(schedule, c)) {
                closest = Some(schedule);
            }
        }

        if let Some(schedule) = closest {
            let category = schedule.shift_category_id();
            shift.set_shift_category_id(category);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, h, m, 0)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "category", RuleClass::ShiftCategoryScr, params)
    }

    fn actual(in_at: LocalDateTime, source: PunchSource, job: i32) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            job,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, source, in_at),
                EmployeeShiftPunch::new(2, PunchType::Out, source, at(17, 0)),
            ],
        )
    }

    fn schedule(start: LocalDateTime, job: i32, category: i32) -> EmployeeShift {
        EmployeeShift::new(
            9,
            100,
            job,
            LocalDate::of(2014, 5, 6),
            ShiftType::Schedule,
            Vec::new(),
        )
        .with_times(Some(start), Some(start.plus_hours(8)))
        .with_shift_category_id(Some(category))
    }

    fn run(shift: &mut EmployeeShift, schedules: Vec<EmployeeShift>, params: RuleParams) {
        let mut card = TimeCardData::new().with_schedules(schedules);
        ShiftCategoryCorrectionRule.execute(shift, &mut card, &rule_item(params));
    }

    #[test]
    fn the_matching_schedules_category_is_applied() {
        let mut shift = actual(at(8, 10), PunchSource::Clock, 1);
        run(
            &mut shift,
            vec![schedule(at(8, 0), 1, 7)],
            RuleParams::new(),
        );
        assert_eq!(shift.shift_category_id(), Some(7));
    }

    #[test]
    fn the_nearest_start_wins() {
        let mut shift = actual(at(8, 20), PunchSource::Clock, 1);
        let far = schedule(at(8, 0), 1, 7);
        let near = schedule(at(8, 30), 1, 8);
        run(&mut shift, vec![far, near], RuleParams::new());
        assert_eq!(shift.shift_category_id(), Some(8));
    }

    #[test]
    fn a_tie_keeps_the_first_listed_schedule() {
        let mut shift = actual(at(8, 30), PunchSource::Clock, 1);
        let early = schedule(at(8, 0), 1, 7);
        let late = schedule(at(9, 0), 1, 8);
        run(&mut shift, vec![early, late], RuleParams::new());
        assert_eq!(shift.shift_category_id(), Some(7));
    }

    #[test]
    fn a_manual_punch_stops_the_rule() {
        let mut shift = actual(at(8, 0), PunchSource::Manual, 1);
        run(
            &mut shift,
            vec![schedule(at(8, 0), 1, 7)],
            RuleParams::new(),
        );
        assert_eq!(shift.shift_category_id(), None);
    }

    #[test]
    fn a_different_job_is_skipped_when_matching_on_job() {
        let mut shift = actual(at(8, 0), PunchSource::Clock, 1);
        run(
            &mut shift,
            vec![schedule(at(8, 0), 2, 7)],
            RuleParams::new(),
        );
        assert_eq!(shift.shift_category_id(), None);
    }

    #[test]
    fn a_different_job_matches_when_not_matching_on_job() {
        let mut shift = actual(at(8, 0), PunchSource::Clock, 1);
        run(
            &mut shift,
            vec![schedule(at(8, 0), 2, 7)],
            rule_params! { MATCH_JOB => "false" },
        );
        assert_eq!(shift.shift_category_id(), Some(7));
    }

    #[test]
    fn the_window_is_inclusive_at_both_ends() {
        let params = rule_params! { GRACE_PRE_START_PROP => "60", GRACE_POST_START_PROP => "60" };
        for start in [at(7, 0), at(9, 0)] {
            let mut shift = actual(at(8, 0), PunchSource::Clock, 1);
            run(&mut shift, vec![schedule(start, 1, 7)], params.clone());
            assert_eq!(shift.shift_category_id(), Some(7), "{start:?}");
        }
        let mut shift = actual(at(8, 0), PunchSource::Clock, 1);
        run(&mut shift, vec![schedule(at(9, 1), 1, 7)], params);
        assert_eq!(shift.shift_category_id(), None);
    }

    /// Divergence 82: the "pre" grace widens the window *after* the punch.
    #[test]
    fn the_pre_grace_extends_the_window_later_not_earlier() {
        let params = rule_params! { GRACE_PRE_START_PROP => "120", GRACE_POST_START_PROP => "0" };
        let mut later = actual(at(8, 0), PunchSource::Clock, 1);
        run(&mut later, vec![schedule(at(10, 0), 1, 7)], params.clone());
        assert_eq!(later.shift_category_id(), Some(7));

        let mut earlier = actual(at(8, 0), PunchSource::Clock, 1);
        run(&mut earlier, vec![schedule(at(6, 0), 1, 7)], params);
        assert_eq!(earlier.shift_category_id(), None);
    }

    #[test]
    fn a_shift_with_no_in_punch_is_left_alone() {
        let mut shift = EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        );
        run(
            &mut shift,
            vec![schedule(at(8, 0), 1, 7)],
            RuleParams::new(),
        );
        assert_eq!(shift.shift_category_id(), None);
    }
}
