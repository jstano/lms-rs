//! Port of `MinuteRoundingRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/MinuteRoundingRuleImpl.java`.
//!
//! Round each punch to the nearest N minutes, with N configured per punch type.
//! The simplest rule in the engine, and the only one of the six that never
//! touches its shift.

use crate::common::dates::round_date_time_to_threshold_minutes;
use crate::common::enums::punch_type::PunchType;
use crate::common::rounding::RoundingOption;
use crate::entity::employee_shift::PunchCursor;
use crate::rules::algorithm::punchrounding::config::{
    BACK_PUNCH_ROUND_TO, BREAK_PUNCH_ROUND_TO, IN_PUNCH_ROUND_TO, MinuteRoundingRuleConfig,
    OUT_PUNCH_ROUND_TO,
};
use crate::rules::algorithm::punchrounding::{PunchRoundingRule, can_round};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;

/// Round each punch to the nearest configured number of minutes.
/// `MinuteRoundingRuleImpl`.
pub struct MinuteRoundingRule;

impl PunchRoundingRule for MinuteRoundingRule {
    fn execute(&self, punch: &mut PunchCursor<'_>, params: &RuleParams) {
        let params = params.fixed(&MinuteRoundingRuleConfig.default_values());

        if !can_round(punch, &params) {
            return;
        }

        let rounded = punch.adj_time().map(|adj_time| {
            round_date_time_to_threshold_minutes(
                adj_time,
                threshold(punch.punch_type(), &params),
                RoundingOption::Nearest,
            )
        });

        punch.set_rounded_time(rounded);
    }
}

/// The threshold for a punch type. `MinuteRoundingRuleImpl.getThreshold`.
///
/// # Panics
///
/// On a punch type that is not in/out/break/back, matching Java's
/// `IllegalArgumentException`. Unreachable in practice: [`can_round`] has
/// already rejected every other type.
fn threshold(punch_type: PunchType, params: &RuleParams) -> i32 {
    let key = match punch_type {
        PunchType::In => IN_PUNCH_ROUND_TO,
        PunchType::Out => OUT_PUNCH_ROUND_TO,
        PunchType::Break => BREAK_PUNCH_ROUND_TO,
        PunchType::Back => BACK_PUNCH_ROUND_TO,
        other => panic!("the punch type is not valid: {other:?}"),
    };

    params.int_at(key)
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

    fn shift_with(punch: EmployeeShiftPunch) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![punch],
        )
    }

    #[test]
    fn a_punch_the_rule_set_gates_off_is_left_alone() {
        let punch = EmployeeShiftPunch::new(
            1,
            PunchType::In,
            PunchSource::Manual,
            LocalDateTime::of(2010, 1, 2, 7, 40, 0),
        );
        let mut shift = shift_with(punch);

        MinuteRoundingRule.execute(
            &mut shift.punch_cursor(0),
            &rule_params! { crate::rules::algorithm::punchrounding::config::MANUAL_IN => "false" },
        );

        assert_eq!(
            shift.punch(0).rounded_time(),
            Some(LocalDateTime::of(2010, 1, 2, 7, 40, 0))
        );
    }

    #[test]
    fn rounding_an_in_punch_moves_the_shift_start() {
        // The rule writes only the punch; the shift follows through the cursor.
        let punch = EmployeeShiftPunch::new(
            1,
            PunchType::In,
            PunchSource::Clock,
            LocalDateTime::of(2010, 1, 2, 7, 40, 0),
        );
        let mut shift = shift_with(punch);

        MinuteRoundingRule.execute(&mut shift.punch_cursor(0), &RuleParams::new());

        assert_eq!(
            shift.start_date_time(),
            Some(LocalDateTime::of(2010, 1, 2, 7, 45, 0))
        );
    }

    #[test]
    fn the_rule_is_idempotent_because_it_rounds_from_the_adjusted_time() {
        let punch = EmployeeShiftPunch::new(
            1,
            PunchType::In,
            PunchSource::Clock,
            LocalDateTime::of(2010, 1, 2, 7, 40, 0),
        );
        let mut shift = shift_with(punch);

        MinuteRoundingRule.execute(&mut shift.punch_cursor(0), &RuleParams::new());
        let once = shift.punch(0).rounded_time();
        MinuteRoundingRule.execute(&mut shift.punch_cursor(0), &RuleParams::new());

        assert_eq!(shift.punch(0).rounded_time(), once);
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// Every row below is transcribed unchanged from the `@Parameterized` table in
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/MinuteRoundingRuleImplTest.groovy`,
/// including its own section comments. A disagreement here is a disagreement
/// with the engine being ported.
///
/// The Groovy test asserts on `punch.getRoundedTime()`; here the assertion
/// reads through the shift that owns the punch, since that is where the cursor
/// writes. Nothing else about the cases changed.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::rule_params;
    use crate::rules::algorithm::punchrounding::config::{
        BACK_PUNCH_ROUND_TO, BREAK_PUNCH_ROUND_TO, CLOCK_BACK, CLOCK_BREAK, CLOCK_IN, CLOCK_OUT,
        IN_PUNCH_ROUND_TO, MANUAL_BACK, MANUAL_BREAK, MANUAL_IN, MANUAL_OUT, OUT_PUNCH_ROUND_TO,
    };
    use joda_rs::{LocalDate, LocalDateTime};
    use rstest::rstest;

    /// Parse the ISO strings the Groovy table is written in.
    fn dt(text: &str) -> LocalDateTime {
        let (date, time) = text.split_once('T').expect("an ISO date-time");
        let date: Vec<i32> = date.split('-').map(|p| p.parse().unwrap()).collect();
        let time: Vec<i32> = time.split(':').map(|p| p.parse().unwrap()).collect();
        LocalDateTime::of(date[0], date[1], date[2], time[0], time[1], time[2])
    }

    /// The Groovy `setUp` plus `testExecute` preamble: a punch whose punchTime,
    /// adjTime and roundedTime all start at the recorded time, on a bare shift.
    fn run(
        punch_time: &str,
        punch_type: PunchType,
        source: PunchSource,
        params: RuleParams,
    ) -> LocalDateTime {
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![EmployeeShiftPunch::new(
                1,
                punch_type,
                source,
                dt(punch_time),
            )],
        );

        MinuteRoundingRule.execute(&mut shift.punch_cursor(0), &params);

        shift.punch(0).rounded_time().expect("a rounded time")
    }

    #[rstest]
    // test defaults round to 15 forward
    #[case("2010-01-02T07:40:00", PunchType::In, PunchSource::Manual, rule_params! {}, "2010-01-02T07:45:00")]
    #[case("2010-01-02T07:40:00", PunchType::Out, PunchSource::Manual, rule_params! {}, "2010-01-02T07:45:00")]
    #[case("2010-01-02T07:40:00", PunchType::Break, PunchSource::Manual, rule_params! {}, "2010-01-02T07:45:00")]
    #[case("2010-01-02T07:40:00", PunchType::Back, PunchSource::Manual, rule_params! {}, "2010-01-02T07:45:00")]
    // test don't round punch type manual
    #[case("2010-01-02T07:40:00", PunchType::In, PunchSource::Manual, rule_params! { MANUAL_IN => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Out, PunchSource::Manual, rule_params! { MANUAL_OUT => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Break, PunchSource::Manual, rule_params! { MANUAL_BREAK => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Back, PunchSource::Manual, rule_params! { MANUAL_BACK => "false" }, "2010-01-02T07:40:00")]
    // test don't round punch type clock
    #[case("2010-01-02T07:40:00", PunchType::In, PunchSource::Clock, rule_params! { CLOCK_IN => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Out, PunchSource::Clock, rule_params! { CLOCK_OUT => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Break, PunchSource::Clock, rule_params! { CLOCK_BREAK => "false" }, "2010-01-02T07:40:00")]
    #[case("2010-01-02T07:40:00", PunchType::Back, PunchSource::Clock, rule_params! { CLOCK_BACK => "false" }, "2010-01-02T07:40:00")]
    // test defaults rounds to 15 back
    #[case("2010-01-02T07:35:00", PunchType::In, PunchSource::Manual, rule_params! { MANUAL_IN => "true" }, "2010-01-02T07:30:00")]
    #[case("2010-01-02T07:35:00", PunchType::Out, PunchSource::Manual, rule_params! { MANUAL_OUT => "true" }, "2010-01-02T07:30:00")]
    #[case("2010-01-02T07:35:00", PunchType::Break, PunchSource::Manual, rule_params! { MANUAL_BREAK => "true" }, "2010-01-02T07:30:00")]
    #[case("2010-01-02T07:35:00", PunchType::Back, PunchSource::Manual, rule_params! { MANUAL_BACK => "true" }, "2010-01-02T07:30:00")]
    // test round to 1
    #[case("2010-01-02T07:47:00", PunchType::In, PunchSource::Manual, rule_params! { MANUAL_IN => "true", IN_PUNCH_ROUND_TO => "1" }, "2010-01-02T07:47:00")]
    #[case("2010-01-02T07:47:00", PunchType::Out, PunchSource::Manual, rule_params! { MANUAL_OUT => "true", OUT_PUNCH_ROUND_TO => "1" }, "2010-01-02T07:47:00")]
    #[case("2010-01-02T07:47:00", PunchType::Break, PunchSource::Manual, rule_params! { MANUAL_BREAK => "true", BREAK_PUNCH_ROUND_TO => "1" }, "2010-01-02T07:47:00")]
    #[case("2010-01-02T07:47:00", PunchType::Back, PunchSource::Manual, rule_params! { MANUAL_BACK => "true", BACK_PUNCH_ROUND_TO => "1" }, "2010-01-02T07:47:00")]
    // test round to 5
    #[case("2010-01-02T07:48:00", PunchType::In, PunchSource::Manual, rule_params! { MANUAL_IN => "true", IN_PUNCH_ROUND_TO => "5" }, "2010-01-02T07:50:00")]
    #[case("2010-01-02T07:48:00", PunchType::Out, PunchSource::Manual, rule_params! { MANUAL_OUT => "true", OUT_PUNCH_ROUND_TO => "5" }, "2010-01-02T07:50:00")]
    #[case("2010-01-02T07:48:00", PunchType::Break, PunchSource::Manual, rule_params! { MANUAL_BREAK => "true", BREAK_PUNCH_ROUND_TO => "5" }, "2010-01-02T07:50:00")]
    #[case("2010-01-02T07:48:00", PunchType::Back, PunchSource::Manual, rule_params! { MANUAL_BACK => "true", BACK_PUNCH_ROUND_TO => "5" }, "2010-01-02T07:50:00")]
    // test round to midnight
    #[case("2013-04-01T23:56:00", PunchType::In, PunchSource::Manual, rule_params! {}, "2013-04-02T00:00:00")]
    fn matches_the_java_rounding_table(
        #[case] punch_time: &str,
        #[case] punch_type: PunchType,
        #[case] source: PunchSource,
        #[case] params: RuleParams,
        #[case] expected: &str,
    ) {
        assert_eq!(run(punch_time, punch_type, source, params), dt(expected));
    }
}
