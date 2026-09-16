//! Port of `PunchRoundToSchedule` and its two subclasses,
//! `RoundInToScheduleRuleImpl` and `RoundOutToScheduleRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/`.
//!
//! Snap a punch to a scheduled shift's boundary when one sits close enough:
//! an in punch to a scheduled start, an out punch to a scheduled end. Where no
//! schedule matches, fall back to plain threshold rounding.
//!
//! Java expresses the pair as a template-method base with four abstract hooks.
//! The two differ only in which end of a schedule they aim at and which punch
//! type they accept, so here the base is one algorithm parameterised by a
//! [`ScheduleEnd`] — the four hooks collapse into it.
//!
//! # The grace parameters are crossed
//!
//! `PunchRoundToSchedule.createMatchingPeriod` reads:
//!
//! ```java
//! LocalDateTime preShift  = punch.getAdjTime().minusMinutes(gracePost);
//! LocalDateTime postShift = punch.getAdjTime().plusMinutes(gracePre);
//! ```
//!
//! The *post* grace is subtracted and the *pre* grace added — the opposite of
//! what the names suggest. So `gracePreSchedStart` governs how far **after** a
//! punch a schedule may sit, and `gracePostSchedStart` how far before.
//! Reproduced exactly; both default to 15, so a site that never changed them
//! cannot tell.

use crate::common::dates::{duration_in_fractional_hours, round_date_time_to_threshold_minutes};
use crate::common::enums::punch_source::PunchSource;
use crate::common::enums::punch_type::PunchType;
use crate::common::rounding::RoundingOption;
use crate::entity::employee_shift::{EmployeeShift, PunchCursor};
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::punchrounding::config::{
    CLOCK_IN, CLOCK_OUT, GRACE_POST_END, GRACE_POST_START, GRACE_PRE_END, GRACE_PRE_START,
    MANUAL_IN, MANUAL_OUT, MATCH_JOB, ROUND_TO, ROUNDING_OPTION, RoundInToScheduleRuleConfig,
    RoundOutToScheduleRuleConfig,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// Which end of a scheduled shift a rule aims at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleEnd {
    /// `RoundInToScheduleRuleImpl` — an in punch to a scheduled start.
    Start,
    /// `RoundOutToScheduleRuleImpl` — an out punch to a scheduled end.
    End,
}

impl ScheduleEnd {
    /// The boundary this rule aims at. `matchingPeriodContainsTarget`'s target
    /// and `isCloserTime`'s reference point.
    fn of(&self, schedule: &EmployeeShift) -> Option<LocalDateTime> {
        match self {
            Self::Start => schedule.start_date_time(),
            Self::End => schedule.end_date_time(),
        }
    }

    /// The punch type this rule rounds. Everything else falls through Java's
    /// `switch` to `default: return false`.
    fn punch_type(&self) -> PunchType {
        match self {
            Self::Start => PunchType::In,
            Self::End => PunchType::Out,
        }
    }

    /// The two gates this rule reads — it carries only its own direction.
    fn gates(&self) -> (&'static str, &'static str) {
        match self {
            Self::Start => (MANUAL_IN, CLOCK_IN),
            Self::End => (MANUAL_OUT, CLOCK_OUT),
        }
    }

    /// The two grace keys, named for this rule's direction.
    fn grace_keys(&self) -> (&'static str, &'static str) {
        match self {
            Self::Start => (GRACE_PRE_START, GRACE_POST_START),
            Self::End => (GRACE_PRE_END, GRACE_POST_END),
        }
    }
}

/// Round an in punch to a matching scheduled start.
/// `RoundInToScheduleRuleImpl`.
pub struct RoundInToScheduleRule;

/// Round an out punch to a matching scheduled end.
/// `RoundOutToScheduleRuleImpl`.
pub struct RoundOutToScheduleRule;

impl RoundInToScheduleRule {
    /// Round this punch against the schedules on the time card.
    ///
    /// Separate from the family's [`PunchRoundingRule`] trait because this rule
    /// needs the time card, which most of the family ignores.
    ///
    /// [`PunchRoundingRule`]: crate::rules::algorithm::punchrounding::PunchRoundingRule
    pub fn execute(
        &self,
        punch: &mut PunchCursor<'_>,
        time_card: Option<&dyn TimeCard>,
        params: &RuleParams,
    ) {
        let params = params.fixed(&RoundInToScheduleRuleConfig.default_values());
        round_to_schedule(ScheduleEnd::Start, punch, time_card, &params);
    }
}

impl RoundOutToScheduleRule {
    /// Round this punch against the schedules on the time card.
    pub fn execute(
        &self,
        punch: &mut PunchCursor<'_>,
        time_card: Option<&dyn TimeCard>,
        params: &RuleParams,
    ) {
        let params = params.fixed(&RoundOutToScheduleRuleConfig.default_values());
        round_to_schedule(ScheduleEnd::End, punch, time_card, &params);
    }
}

/// `PunchRoundToSchedule.execute`.
fn round_to_schedule(
    end: ScheduleEnd,
    punch: &mut PunchCursor<'_>,
    time_card: Option<&dyn TimeCard>,
    params: &RuleParams,
) {
    if !can_round(end, punch, params) {
        return;
    }

    let Some(adj_time) = punch.adj_time() else {
        return;
    };

    let closest = time_card
        .and_then(|time_card| closest_schedule(end, punch, time_card, adj_time, params))
        .and_then(|schedule| end.of(schedule));

    // With no schedule to snap to, fall back to plain threshold rounding.
    let rounded = closest.unwrap_or_else(|| {
        let option = params
            .get(ROUNDING_OPTION)
            .and_then(RoundingOption::from_code)
            .unwrap_or(RoundingOption::Nearest);
        round_date_time_to_threshold_minutes(adj_time, params.int_at(ROUND_TO), option)
    });

    punch.set_rounded_time(Some(rounded));
}

/// `PunchRoundToSchedule.canRound`, as each subclass overrides it.
fn can_round(end: ScheduleEnd, punch: &PunchCursor<'_>, params: &RuleParams) -> bool {
    if punch.punch_type() != end.punch_type() {
        return false;
    }

    let (manual, clock) = end.gates();
    let key = if punch.source() == PunchSource::Clock {
        clock
    } else {
        manual
    };

    params.bool_at(key)
}

/// The schedule whose boundary sits nearest the punch, within grace.
/// `PunchRoundToSchedule.getClosestSchedule`.
fn closest_schedule<'a>(
    end: ScheduleEnd,
    punch: &PunchCursor<'_>,
    time_card: &'a dyn TimeCard,
    adj_time: LocalDateTime,
    params: &RuleParams,
) -> Option<&'a EmployeeShift> {
    let (pre_key, post_key) = end.grace_keys();
    // Crossed on purpose — see the module docs.
    let period_start = adj_time.minus_minutes(i64::from(params.int_at(post_key)));
    let period_end = adj_time.plus_minutes(i64::from(params.int_at(pre_key)));

    let match_job = params.bool_at(MATCH_JOB);
    let punch_job_id = punch.shift().job_id();

    let mut closest: Option<&EmployeeShift> = None;

    for schedule in time_card.schedules() {
        let Some(target) = end.of(schedule) else {
            continue;
        };

        // matchingPeriod.containsDateTime — inclusive at both ends.
        if target.is_before(period_start) || target.is_after(period_end) {
            continue;
        }

        if match_job && punch_job_id != schedule.job_id() {
            continue;
        }

        let closer = match closest {
            None => true,
            Some(current) => match end.of(current) {
                Some(current_target) => {
                    duration_in_fractional_hours(adj_time, target).abs()
                        < duration_in_fractional_hours(adj_time, current_target).abs()
                }
                None => true,
            },
        };

        if closer {
            closest = Some(schedule);
        }
    }

    closest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use joda_rs::LocalDate;
    use rstest::rstest;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    /// A worked shift with a single punch of the given type.
    fn shift(punch_type: PunchType, time: LocalDateTime, job_id: i32) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            job_id,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![EmployeeShiftPunch::new(
                1,
                punch_type,
                PunchSource::Clock,
                time,
            )],
        )
    }

    /// A scheduled shift spanning the given times.
    fn schedule(job_id: i32, start: LocalDateTime, end: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            9,
            100,
            job_id,
            LocalDate::of(2010, 1, 2),
            ShiftType::Schedule,
            Vec::new(),
        )
        .with_times(Some(start), Some(end))
    }

    fn round_in(
        punch_at: LocalDateTime,
        schedules: Vec<EmployeeShift>,
        params: RuleParams,
    ) -> LocalDateTime {
        let mut worked = shift(PunchType::In, punch_at, 200);
        let card = TimeCardData::new().with_schedules(schedules);
        RoundInToScheduleRule.execute(&mut worked.punch_cursor(0), Some(&card), &params);
        worked.punch(0).rounded_time().unwrap()
    }

    fn round_out(
        punch_at: LocalDateTime,
        schedules: Vec<EmployeeShift>,
        params: RuleParams,
    ) -> LocalDateTime {
        let mut worked = shift(PunchType::Out, punch_at, 200);
        let card = TimeCardData::new().with_schedules(schedules);
        RoundOutToScheduleRule.execute(&mut worked.punch_cursor(0), Some(&card), &params);
        worked.punch(0).rounded_time().unwrap()
    }

    #[test]
    fn an_in_punch_snaps_to_a_nearby_scheduled_start() {
        let schedules = vec![schedule(200, at(8, 0), at(16, 0))];
        assert_eq!(round_in(at(7, 52), schedules, RuleParams::new()), at(8, 0));
    }

    #[test]
    fn an_out_punch_snaps_to_a_nearby_scheduled_end() {
        let schedules = vec![schedule(200, at(8, 0), at(16, 0))];
        assert_eq!(
            round_out(at(16, 7), schedules, RuleParams::new()),
            at(16, 0)
        );
    }

    #[test]
    fn with_no_schedule_at_all_the_rule_falls_back_to_threshold_rounding() {
        // 07:52 to the nearest 15 minutes is 07:45.
        assert_eq!(
            round_in(at(7, 52), Vec::new(), RuleParams::new()),
            at(7, 45)
        );
    }

    #[test]
    fn a_schedule_outside_grace_is_not_matched() {
        // The schedule starts 40 minutes after the punch, beyond the 15 minute
        // grace, so threshold rounding applies instead.
        let schedules = vec![schedule(200, at(8, 30), at(16, 0))];
        assert_eq!(round_in(at(7, 50), schedules, RuleParams::new()), at(7, 45));
    }

    #[test]
    fn the_nearest_of_several_schedules_wins() {
        let schedules = vec![
            schedule(200, at(8, 10), at(16, 0)),
            schedule(200, at(8, 2), at(16, 0)),
            schedule(200, at(8, 12), at(16, 0)),
        ];
        assert_eq!(round_in(at(8, 0), schedules, RuleParams::new()), at(8, 2));
    }

    #[test]
    fn match_job_excludes_a_schedule_for_another_job() {
        let schedules = vec![schedule(999, at(8, 0), at(16, 0))];

        // With matching on, the other job's schedule is ignored.
        assert_eq!(
            round_in(at(7, 52), schedules.clone(), RuleParams::new()),
            at(7, 45)
        );

        // With it off, it matches.
        assert_eq!(
            round_in(at(7, 52), schedules, rule_params! { MATCH_JOB => "false" }),
            at(8, 0)
        );
    }

    #[test]
    fn the_grace_parameters_are_crossed() {
        // gracePre governs how far AFTER the punch a schedule may sit, and
        // gracePost how far before — the opposite of the names. Set pre wide
        // and post to zero: a schedule 30 minutes later still matches.
        let later = vec![schedule(200, at(8, 30), at(16, 0))];
        assert_eq!(
            round_in(
                at(8, 0),
                later,
                rule_params! { GRACE_PRE_START => "30", GRACE_POST_START => "0" }
            ),
            at(8, 30)
        );

        // And one 30 minutes earlier does not.
        let earlier = vec![schedule(200, at(7, 30), at(16, 0))];
        assert_eq!(
            round_in(
                at(8, 0),
                earlier,
                rule_params! { GRACE_PRE_START => "30", GRACE_POST_START => "0" }
            ),
            at(8, 0),
            "threshold rounding, since nothing matched"
        );
    }

    #[test]
    fn the_grace_window_is_inclusive_at_both_ends() {
        let exactly_at_grace = vec![schedule(200, at(8, 15), at(16, 0))];
        assert_eq!(
            round_in(at(8, 0), exactly_at_grace, RuleParams::new()),
            at(8, 15)
        );

        let just_past = vec![schedule(200, at(8, 16), at(16, 0))];
        assert_eq!(round_in(at(8, 0), just_past, RuleParams::new()), at(8, 0));
    }

    #[rstest]
    #[case(PunchType::Out)]
    #[case(PunchType::Break)]
    #[case(PunchType::Back)]
    fn the_in_rule_only_touches_in_punches(#[case] punch_type: PunchType) {
        let mut worked = shift(punch_type, at(7, 52), 200);
        let card = TimeCardData::new().with_schedules(vec![schedule(200, at(8, 0), at(16, 0))]);

        RoundInToScheduleRule.execute(&mut worked.punch_cursor(0), Some(&card), &RuleParams::new());

        assert_eq!(worked.punch(0).rounded_time(), Some(at(7, 52)));
    }

    #[test]
    fn the_out_rule_only_touches_out_punches() {
        let mut worked = shift(PunchType::In, at(16, 7), 200);
        let card = TimeCardData::new().with_schedules(vec![schedule(200, at(8, 0), at(16, 0))]);

        RoundOutToScheduleRule.execute(
            &mut worked.punch_cursor(0),
            Some(&card),
            &RuleParams::new(),
        );

        assert_eq!(worked.punch(0).rounded_time(), Some(at(16, 7)));
    }

    #[test]
    fn a_gated_off_punch_is_left_alone() {
        let mut worked = shift(PunchType::In, at(7, 52), 200);
        let card = TimeCardData::new().with_schedules(vec![schedule(200, at(8, 0), at(16, 0))]);

        RoundInToScheduleRule.execute(
            &mut worked.punch_cursor(0),
            Some(&card),
            &rule_params! { CLOCK_IN => "false" },
        );

        assert_eq!(worked.punch(0).rounded_time(), Some(at(7, 52)));
    }

    #[test]
    fn a_missing_time_card_still_threshold_rounds() {
        // Java would throw on timeCard.getSchedules(); reaching the rule with
        // no time card means there are no schedules to match, which is the same
        // outcome as an empty list.
        let mut worked = shift(PunchType::In, at(7, 52), 200);

        RoundInToScheduleRule.execute(&mut worked.punch_cursor(0), None, &RuleParams::new());

        assert_eq!(worked.punch(0).rounded_time(), Some(at(7, 45)));
    }

    #[test]
    fn the_rounding_option_governs_the_fallback() {
        // No schedule, so the fallback rounds — up rather than to nearest.
        assert_eq!(
            round_in(
                at(7, 46),
                Vec::new(),
                rule_params! { ROUNDING_OPTION => "UP" }
            ),
            at(8, 0)
        );
        assert_eq!(
            round_in(
                at(7, 59),
                Vec::new(),
                rule_params! { ROUNDING_OPTION => "DOWN" }
            ),
            at(7, 45)
        );
    }

    #[test]
    fn snapping_an_in_punch_moves_the_shift_start() {
        let mut worked = shift(PunchType::In, at(7, 52), 200);
        let card = TimeCardData::new().with_schedules(vec![schedule(200, at(8, 0), at(16, 0))]);

        RoundInToScheduleRule.execute(&mut worked.punch_cursor(0), Some(&card), &RuleParams::new());

        assert_eq!(worked.start_date_time(), Some(at(8, 0)));
    }

    #[test]
    fn a_schedule_with_no_boundary_is_skipped() {
        let no_times = EmployeeShift::new(
            9,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Schedule,
            Vec::new(),
        );
        assert_eq!(
            round_in(at(7, 52), vec![no_times], RuleParams::new()),
            at(7, 45)
        );
    }
}
