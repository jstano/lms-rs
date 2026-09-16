//! Punch rounding. `RuleType::PunchRounding` — six rules.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/`.
//!
//! The first family ported, chosen because it is small, self-contained, and has
//! a fully table-driven test suite. It also turned out to contain the hardest
//! ownership problem in the engine — see
//! [`EmployeeShift`](crate::entity::employee_shift) for the punch/shift cursor
//! this family forced.
//!
//! Every rule reads a punch's `adjTime` and writes its `roundedTime`. Nothing
//! reads `roundedTime` as an input, so a rule is idempotent and re-running the
//! family rounds from the same base each time.

pub mod back_from_break_with_grace;
pub mod config;
pub mod minute_rounding;
pub mod property_data_rounding;
pub mod round_to_schedule;
pub mod worked_hours_rounding;

use crate::entity::employee_shift::PunchCursor;
use crate::rules::params::RuleParams;

/// A punch rounding rule.
///
/// `PunchRoundingRuleImpl.execute(EmployeeShiftPunch, TimeCard, Map)`. The
/// punch arrives as a [`PunchCursor`] rather than a bare punch, so that writing
/// a rounded time still fires the shift callback Java gets from the punch's
/// back-reference.
///
/// The `TimeCard` argument is not on this trait. Four of the six rules never
/// consult it — the Groovy tests pass `null` — and the two that do,
/// [`RoundInToScheduleRule`] and [`RoundOutToScheduleRule`], take it on their
/// own `execute` instead of burdening the other four with an argument they
/// ignore. The runner knows which shape each rule class has.
///
/// [`RoundInToScheduleRule`]: round_to_schedule::RoundInToScheduleRule
/// [`RoundOutToScheduleRule`]: round_to_schedule::RoundOutToScheduleRule
pub trait PunchRoundingRule {
    /// Round this punch, if the rule applies to it.
    ///
    /// Doing nothing is the normal outcome for a punch the rule does not cover
    /// — a wrong punch type, or a source the rule set has gated off.
    fn execute(&self, punch: &mut PunchCursor<'_>, params: &RuleParams);
}

/// Is this punch one the rule set allows rounding?
///
/// `PunchRoundingRuleImpl.canRound` together with `setParams`. Java reads the
/// eight gates into fields and then switches; the two steps collapse here
/// because there is no per-request bean to hold the fields.
///
/// A punch type outside in/out/break/back is never rounded — Java's `switch`
/// falls through to `default: return false`.
pub fn can_round(punch: &PunchCursor<'_>, params: &RuleParams) -> bool {
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use config::{
        CLOCK_BACK, CLOCK_BREAK, CLOCK_IN, CLOCK_OUT, MANUAL_BACK, MANUAL_BREAK, MANUAL_IN,
        MANUAL_OUT,
    };

    let from_clock = punch.source() == PunchSource::Clock;

    let key = match punch.punch_type() {
        PunchType::In if from_clock => CLOCK_IN,
        PunchType::In => MANUAL_IN,
        PunchType::Out if from_clock => CLOCK_OUT,
        PunchType::Out => MANUAL_OUT,
        PunchType::Break if from_clock => CLOCK_BREAK,
        PunchType::Break => MANUAL_BREAK,
        PunchType::Back if from_clock => CLOCK_BACK,
        PunchType::Back => MANUAL_BACK,
        _ => return false,
    };

    params.bool_at(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::rules::rule_config::RuleConfig;
    use config::MinuteRoundingRuleConfig;
    use joda_rs::{LocalDate, LocalDateTime};
    use rstest::rstest;

    fn shift_with(punch_type: PunchType, source: PunchSource) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![EmployeeShiftPunch::new(
                1,
                punch_type,
                source,
                LocalDateTime::of(2010, 1, 2, 7, 40, 0),
            )],
        )
    }

    #[rstest]
    #[case(PunchType::In, PunchSource::Clock)]
    #[case(PunchType::In, PunchSource::Manual)]
    #[case(PunchType::Out, PunchSource::Clock)]
    #[case(PunchType::Out, PunchSource::Manual)]
    #[case(PunchType::Break, PunchSource::Clock)]
    #[case(PunchType::Break, PunchSource::Manual)]
    #[case(PunchType::Back, PunchSource::Clock)]
    #[case(PunchType::Back, PunchSource::Manual)]
    fn every_time_punch_rounds_by_default(
        #[case] punch_type: PunchType,
        #[case] source: PunchSource,
    ) {
        let mut shift = shift_with(punch_type, source);
        let params = MinuteRoundingRuleConfig.default_values();

        assert!(can_round(&shift.punch_cursor(0), &params));
    }

    #[rstest]
    #[case(PunchType::In, PunchSource::Manual, config::MANUAL_IN)]
    #[case(PunchType::Out, PunchSource::Manual, config::MANUAL_OUT)]
    #[case(PunchType::Break, PunchSource::Manual, config::MANUAL_BREAK)]
    #[case(PunchType::Back, PunchSource::Manual, config::MANUAL_BACK)]
    #[case(PunchType::In, PunchSource::Clock, config::CLOCK_IN)]
    #[case(PunchType::Out, PunchSource::Clock, config::CLOCK_OUT)]
    #[case(PunchType::Break, PunchSource::Clock, config::CLOCK_BREAK)]
    #[case(PunchType::Back, PunchSource::Clock, config::CLOCK_BACK)]
    fn each_punch_type_and_source_reads_its_own_gate(
        #[case] punch_type: PunchType,
        #[case] source: PunchSource,
        #[case] gate: &str,
    ) {
        let mut shift = shift_with(punch_type, source);
        let mut params = MinuteRoundingRuleConfig.default_values();
        params.set(gate, "false");

        assert!(!can_round(&shift.punch_cursor(0), &params));
    }

    #[test]
    fn a_manual_gate_does_not_affect_a_clock_punch() {
        let mut shift = shift_with(PunchType::In, PunchSource::Clock);
        let mut params = MinuteRoundingRuleConfig.default_values();
        params.set(config::MANUAL_IN, "false");

        assert!(can_round(&shift.punch_cursor(0), &params));
    }

    #[test]
    fn an_auto_punch_is_treated_as_manual() {
        // Java tests `source == CLOCK`, so anything else takes the manual gate.
        let mut shift = shift_with(PunchType::In, PunchSource::Auto);
        let mut params = MinuteRoundingRuleConfig.default_values();
        params.set(config::MANUAL_IN, "false");

        assert!(!can_round(&shift.punch_cursor(0), &params));
    }

    #[rstest]
    #[case(PunchType::Tips)]
    #[case(PunchType::Meal)]
    #[case(PunchType::OnSite)]
    fn a_punch_that_is_not_a_time_punch_is_never_rounded(#[case] punch_type: PunchType) {
        let mut shift = shift_with(punch_type, PunchSource::Clock);
        let params = MinuteRoundingRuleConfig.default_values();

        assert!(!can_round(&shift.punch_cursor(0), &params));
    }
}
