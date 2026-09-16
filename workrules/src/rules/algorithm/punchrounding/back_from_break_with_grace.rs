//! Port of `BackFromBreakWithGraceRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/BackFromBreakWithGraceRuleImpl.java`.
//!
//! If a break came out close to its expected length, snap it to exactly that
//! length. An employee who took 32 minutes of a 30-minute break, within a
//! 5-minute grace, comes back at the 30-minute mark.
//!
//! Three things set this rule apart from the rest of the family:
//!
//! * it **ignores the manual and clock gates** — its config does not extend
//!   `PunchRoundingRuleConfig`, so there are none to read, and it never calls
//!   `canRound`;
//! * it gates on the shift's **persisted** errors via `hasErrors()`, then its
//!   own break-finding gates on them a second time;
//! * its break-finding is *not* `ShiftUtil.getBreaks`. It pairs the same
//!   punches positionally, but takes the break's start from `roundedTime` and
//!   its end from `adjTime` — a deliberate mix, because the end is the punch
//!   this rule is about to move and must still be compared unrounded.

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::{EmployeeShift, PunchCursor};
use crate::rules::algorithm::punchrounding::PunchRoundingRule;
use crate::rules::algorithm::punchrounding::config::{
    BREAK_LENGTH, BackFromBreakWithGraceRuleConfig, GRACE_PERIOD,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// Snap a back punch to a full break when it is within grace.
/// `BackFromBreakWithGraceRuleImpl`.
pub struct BackFromBreakWithGraceRule;

impl PunchRoundingRule for BackFromBreakWithGraceRule {
    fn execute(&self, punch: &mut PunchCursor<'_>, params: &RuleParams) {
        if punch.shift().has_errors() || punch.punch_type() != PunchType::Back {
            return;
        }

        let params = params.fixed(&BackFromBreakWithGraceRuleConfig.default_values());
        let break_length = params.int_at(BREAK_LENGTH);
        let grace_period = params.int_at(GRACE_PERIOD);

        let Some(adj_time) = punch.adj_time() else {
            return;
        };

        // Java loops every break and rounds on each that ends at this punch.
        // At most one can, since a break's end is a single punch time.
        let rounded = breaks(punch.shift())
            .into_iter()
            .filter(|(_, end)| *end == adj_time)
            .find_map(|(start, end)| {
                let break_minutes = break_minutes(start, end);
                falls_within_threshold(break_minutes, break_length, grace_period)
                    .then(|| start.plus_minutes(i64::from(break_length)))
            });

        if let Some(rounded) = rounded {
            punch.set_rounded_time(Some(rounded));
        }
    }
}

/// The shift's breaks, as (start, end) pairs.
///
/// `BackFromBreakWithGraceRuleImpl.getBreaks` — its own, not
/// `ShiftUtil.getBreaks`. Same positional pairing and same `size() > 2` and
/// error guards, but the start comes from `roundedTime` and the end from
/// `adjTime`.
fn breaks(shift: &EmployeeShift) -> Vec<(LocalDateTime, LocalDateTime)> {
    if shift.has_errors() || shift.punch_count() <= 2 {
        return Vec::new();
    }

    let order = shift.punch_order_for_breaks();
    let mut breaks = Vec::new();

    let mut index = 1;
    while index < order.len().saturating_sub(2) {
        let start = shift.punch(order[index]).rounded_time();
        let end = shift.punch(order[index + 1]).adj_time();
        if let (Some(start), Some(end)) = (start, end) {
            breaks.push((start, end));
        }
        index += 2;
    }

    breaks
}

/// A break's length in whole minutes.
///
/// Java computes `getDurationInFractionalHours() * MINUTES_PER_HOUR` and casts
/// to `int`, which truncates toward zero.
fn break_minutes(start: LocalDateTime, end: LocalDateTime) -> i32 {
    let seconds = end.epoch_seconds() - start.epoch_seconds();
    let fractional_hours = seconds as f64 / 3600.0;
    (fractional_hours * 60.0) as i32
}

/// Is the break close enough to its expected length?
/// `breakFallsWithinThreshold` — inclusive at the grace boundary, and symmetric,
/// so an over-long break snaps back just as an over-short one snaps forward.
fn falls_within_threshold(break_minutes: i32, break_length: i32, grace_period: i32) -> bool {
    (break_minutes - break_length).abs() <= grace_period
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::rule_params;
    use joda_rs::LocalDate;
    use rstest::rstest;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    /// In 8:00, Break 12:00, Back at the given time, Out 16:00.
    fn shift(back_at: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(8, 0)),
                EmployeeShiftPunch::new(2, PunchType::Break, PunchSource::Clock, at(12, 0)),
                EmployeeShiftPunch::new(3, PunchType::Back, PunchSource::Clock, back_at),
                EmployeeShiftPunch::new(4, PunchType::Out, PunchSource::Clock, at(16, 0)),
            ],
        )
    }

    fn run(back_at: LocalDateTime, params: RuleParams) -> LocalDateTime {
        let mut shift = shift(back_at);
        BackFromBreakWithGraceRule.execute(&mut shift.punch_cursor(2), &params);
        shift.punch(2).rounded_time().unwrap()
    }

    #[rstest]
    // A 30 minute break is already exact.
    #[case(at(12, 30), at(12, 30))]
    // Within the 5 minute grace, either side: snapped to 30 minutes.
    #[case(at(12, 32), at(12, 30))]
    #[case(at(12, 35), at(12, 30))]
    #[case(at(12, 26), at(12, 30))]
    #[case(at(12, 25), at(12, 30))]
    fn a_break_within_grace_snaps_to_its_full_length(
        #[case] back_at: LocalDateTime,
        #[case] expected: LocalDateTime,
    ) {
        assert_eq!(run(back_at, RuleParams::new()), expected);
    }

    #[rstest]
    // Outside the grace on either side: left alone.
    #[case(at(12, 36))]
    #[case(at(12, 24))]
    #[case(at(13, 30))]
    fn a_break_outside_grace_is_left_alone(#[case] back_at: LocalDateTime) {
        assert_eq!(run(back_at, RuleParams::new()), back_at);
    }

    #[test]
    fn the_grace_boundary_is_inclusive() {
        // Exactly 5 minutes over snaps; 6 does not.
        assert_eq!(run(at(12, 35), RuleParams::new()), at(12, 30));
        assert_eq!(run(at(12, 36), RuleParams::new()), at(12, 36));
    }

    #[test]
    fn the_break_length_and_grace_are_configurable() {
        // A 60 minute break with 10 minutes of grace.
        let params = rule_params! { BREAK_LENGTH => "60", GRACE_PERIOD => "10" };

        assert_eq!(run(at(13, 5), params.clone()), at(13, 0));
        assert_eq!(run(at(13, 11), params), at(13, 11));
    }

    #[test]
    fn only_a_back_punch_is_touched() {
        let mut shift = shift(at(12, 32));

        for index in [0, 1, 3] {
            BackFromBreakWithGraceRule.execute(&mut shift.punch_cursor(index), &RuleParams::new());
        }

        assert_eq!(shift.punch(0).rounded_time(), Some(at(8, 0)));
        assert_eq!(shift.punch(1).rounded_time(), Some(at(12, 0)));
        assert_eq!(shift.punch(3).rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn a_shift_with_saved_errors_is_skipped_entirely() {
        // hasErrors() reads the persisted set, not the derived one.
        let mut shift = shift(at(12, 32)).with_errors(vec![ShiftErrorType::MissingOut]);

        BackFromBreakWithGraceRule.execute(&mut shift.punch_cursor(2), &RuleParams::new());

        assert_eq!(shift.punch(2).rounded_time(), Some(at(12, 32)));
    }

    #[test]
    fn a_shift_with_no_break_has_nothing_to_snap_to() {
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(8, 0)),
                EmployeeShiftPunch::new(2, PunchType::Back, PunchSource::Clock, at(12, 32)),
            ],
        );

        BackFromBreakWithGraceRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(shift.punch(1).rounded_time(), Some(at(12, 32)));
    }

    #[test]
    fn the_rule_ignores_the_manual_and_clock_gates() {
        // Its config carries none, and it never calls canRound. Passing them
        // anyway must change nothing.
        let params = rule_params! {
            crate::rules::algorithm::punchrounding::config::MANUAL_BACK => "false",
            crate::rules::algorithm::punchrounding::config::CLOCK_BACK => "false",
        };

        assert_eq!(run(at(12, 32), params), at(12, 30));
    }

    #[test]
    fn snapping_the_back_punch_recomputes_the_shifts_hours() {
        let mut shift = shift(at(12, 32));
        shift.calc_worked_hours();
        let before = shift.worked_hours();

        BackFromBreakWithGraceRule.execute(&mut shift.punch_cursor(2), &RuleParams::new());

        assert_ne!(shift.worked_hours(), before, "the break got longer");
        assert_eq!(shift.worked_hours(), 7.5);
    }

    #[rstest]
    #[case(30, 30, 5, true)]
    #[case(35, 30, 5, true)]
    #[case(36, 30, 5, false)]
    #[case(25, 30, 5, true)]
    #[case(24, 30, 5, false)]
    fn the_threshold_test_is_symmetric(
        #[case] break_minutes: i32,
        #[case] break_length: i32,
        #[case] grace: i32,
        #[case] expected: bool,
    ) {
        assert_eq!(
            falls_within_threshold(break_minutes, break_length, grace),
            expected
        );
    }

    #[test]
    fn break_minutes_truncate_rather_than_round() {
        // Java casts the fractional hours product to int.
        assert_eq!(break_minutes(at(12, 0), at(12, 30)), 30);
        assert_eq!(
            break_minutes(at(12, 0), LocalDateTime::of(2010, 1, 2, 12, 30, 59)),
            30,
            "the trailing 59 seconds are dropped"
        );
    }
}
