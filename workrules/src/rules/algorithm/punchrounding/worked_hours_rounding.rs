//! Port of `WorkedHoursRoundingRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/WorkedHoursRoundingRuleImpl.java`.
//!
//! Rather than rounding each punch, this rounds the shift's **total** worked
//! hours, and moves the out punch by whatever that adjustment comes to.
//!
//! It is the rule that forced the punch/shift cursor. Java:
//!
//! ```java
//! final EmployeeShift shift = punch.getEmployeeShift();
//! for (EmployeeShiftPunch otherPunch : shift.getPunches()) {
//!    otherPunch.setRoundedTime(otherPunch.getAdjTime());
//! }
//! double workedHours = shift.getWorkedHours();
//! ```
//!
//! The reset loop is not housekeeping — every `setRoundedTime` fires the shift
//! callback, and `getWorkedHours()` is only correct because they did. Reading
//! the hours before the loop, or resetting the punches without the callback,
//! gives a different answer.
//!
//! Note it does **not** call `canRound`: the manual and clock gates it inherits
//! from its config are never consulted. Only the punch type gates it, and only
//! on `Out`.

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::PunchCursor;
use crate::rules::algorithm::punchrounding::PunchRoundingRule;
use crate::rules::algorithm::punchrounding::config::{ROUND_TO, WorkedHoursRoundingRuleConfig};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;

const SECONDS_PER_MINUTE: f64 = 60.0;
const MINUTES_PER_HOUR: f64 = 60.0;

/// Round the shift's worked hours, adjusting the out punch to suit.
/// `WorkedHoursRoundingRuleImpl`.
pub struct WorkedHoursRoundingRule;

impl PunchRoundingRule for WorkedHoursRoundingRule {
    fn execute(&self, punch: &mut PunchCursor<'_>, params: &RuleParams) {
        let params = params.fixed(&WorkedHoursRoundingRuleConfig.default_values());

        if punch.punch_type() != PunchType::Out {
            return;
        }

        // Reset every punch to its adjusted time. Each write fires the shift
        // callback, which is what makes the worked hours read below correct.
        punch.for_each_punch(|other| {
            let adj_time = other.adj_time();
            other.set_rounded_time(adj_time);
        });

        let worked_hours = punch.shift().worked_hours();
        let worked_seconds = worked_hours * MINUTES_PER_HOUR * SECONDS_PER_MINUTE;

        let round_to_seconds = params.int_at(ROUND_TO) * 60;

        // Java's Math.round: half toward positive infinity, then an int cast.
        let rounded_seconds =
            java_round(worked_seconds / f64::from(round_to_seconds)) * round_to_seconds;

        // Java truncates workedSeconds with an (int) cast, not a round.
        let seconds_adjustment = rounded_seconds - worked_seconds as i32;

        let rounded = punch
            .adj_time()
            .map(|adj_time| adj_time.plus_seconds(i64::from(seconds_adjustment)));

        punch.set_rounded_time(rounded);
    }
}

/// `Math.round(double)` — half toward positive infinity, cast to `int`.
fn java_round(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::rule_params;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn at_sec(hour: i32, minute: i32, second: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, second)
    }

    /// An in/out pair, with the shift's times and hours already computed — the
    /// state it arrives in from the database, and what the rule depends on
    /// since its reset loop fires no callback when nothing actually changes.
    fn shift(in_at: LocalDateTime, out_at: LocalDateTime) -> EmployeeShift {
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, in_at),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, out_at),
            ],
        )
        .with_times(Some(in_at), Some(out_at));
        shift.calc_worked_hours();
        shift
    }

    #[test]
    fn a_shift_already_on_the_boundary_is_not_moved() {
        // Eight hours exactly, rounding to 15 minutes: no adjustment.
        let mut shift = shift(at(8, 0), at(16, 0));

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn worked_hours_round_up_and_the_out_punch_follows() {
        // 8h08m rounds up to 8h15m. The adjustment is 421 seconds rather than
        // 420 — see `the_four_decimal_worked_hours_leak_a_second` below.
        let mut shift = shift(at(8, 0), at(16, 8));

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(shift.punch(1).rounded_time(), Some(at_sec(16, 15, 1)));
    }

    #[test]
    fn worked_hours_round_down_and_the_out_punch_follows() {
        // 8h07m rounds down to 8h00m, a clean -420 seconds.
        let mut shift = shift(at(8, 0), at(16, 7));

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn the_threshold_is_configurable() {
        // 8h08m to the nearest 30 minutes rounds down to 8h00m.
        let mut shift = shift(at(8, 0), at(16, 8));

        WorkedHoursRoundingRule.execute(
            &mut shift.punch_cursor(1),
            &rule_params! { ROUND_TO => "30" },
        );

        assert_eq!(shift.punch(1).rounded_time(), Some(at_sec(16, 0, 1)));
    }

    #[test]
    fn the_four_decimal_worked_hours_leak_a_second() {
        // `shift.getWorkedHours()` is rounded to four decimals by
        // TDouble.roundRawHours, so 488 minutes reads back as 8.1333 hours =
        // 29279.88 seconds, not 29280. Java then truncates that with an (int)
        // cast, and the adjustment comes out one second long. Faithful, and
        // worth pinning: it is the kind of artefact a "tidier" port would
        // quietly fix and thereby diverge.
        let mut shift = shift(at(8, 0), at(16, 8));
        assert_eq!(shift.worked_hours(), 8.1333);

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        let rounded = shift.punch(1).rounded_time().unwrap();
        assert_eq!(rounded, at_sec(16, 15, 1));
        assert_ne!(rounded, at(16, 15), "not the clean quarter hour");
    }

    #[test]
    fn an_in_punch_is_left_alone() {
        let mut shift = shift(at(8, 0), at(16, 8));

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(0), &RuleParams::new());

        assert_eq!(shift.punch(0).rounded_time(), Some(at(8, 0)));
        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 8)));
    }

    #[test]
    fn the_reset_loop_restores_every_punch_to_its_adjusted_time() {
        // A prior rule has already moved the in punch; this rule undoes that
        // before measuring, so the hours it rounds are the unrounded ones.
        let mut shift = shift(at(8, 0), at(16, 8));
        shift.punch_cursor(0).set_rounded_time(Some(at(9, 0)));
        assert_eq!(shift.worked_hours(), 7.1333, "the shift shrank");

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(
            shift.punch(0).rounded_time(),
            Some(at(8, 0)),
            "the in punch is back at its adjusted time"
        );
        // The rule's own final write to the out punch fires the callback once
        // more, so the hours left on the shift describe the *rounded* span
        // (8:00 to 16:15:01), not the measured one it rounded from.
        assert_eq!(shift.worked_hours(), 8.25);
    }

    #[test]
    fn the_callback_fires_for_the_out_punch() {
        let mut shift = shift(at(8, 0), at(16, 8));

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(1), &RuleParams::new());

        assert_eq!(shift.end_date_time(), Some(at_sec(16, 15, 1)));
    }

    #[test]
    fn a_shift_with_a_break_rounds_its_net_worked_hours() {
        // 8h08m elapsed less a 30 minute break is 7h38m, rounding to 7h45m.
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(8, 0)),
                EmployeeShiftPunch::new(2, PunchType::Break, PunchSource::Clock, at(12, 0)),
                EmployeeShiftPunch::new(3, PunchType::Back, PunchSource::Clock, at(12, 30)),
                EmployeeShiftPunch::new(4, PunchType::Out, PunchSource::Clock, at(16, 8)),
            ],
        );
        shift.calc_worked_hours();
        assert_eq!(shift.worked_hours(), 7.6333);

        WorkedHoursRoundingRule.execute(&mut shift.punch_cursor(3), &RuleParams::new());

        assert_eq!(shift.punch(3).rounded_time(), Some(at_sec(16, 15, 1)));
    }

    #[test]
    fn java_round_goes_half_toward_positive_infinity() {
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(1.5), 2);
        assert_eq!(java_round(-0.5), 0, "not -1");
        assert_eq!(java_round(0.49), 0);
    }
}
