//! Port of `OutPunchCorrectionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftcorrection/OutPunchCorrectionRuleImpl.java`.
//!
//! A shift with an IN and no OUT gets a synthetic OUT punch a configured
//! number of hours after the IN. Gated on the shift's **persisted** errors
//! (`getErrors()`, not the derived `getErrorsOld()`): `MISSING_OUT` present
//! and `MISSING_IN` absent.
//!
//! The new punch is `MANUAL`, stamped `rawPunch = "Rule"`, and takes the IN's
//! badge and `rounded` flag. Each of its three times is the IN's matching time
//! plus `intValue(round(dailyHours * 60, 2))` minutes — the minutes truncate,
//! so `7.999` hours is 479 minutes, not 480.
//!
//! The IN used is the **last** IN punch on the shift.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::punch_source::PunchSource;
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_shift_punch::EmployeeShiftPunch;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftcorrection::ShiftCorrectionRule;
use crate::rules::algorithm::shiftcorrection::config::{
    DAILY_HRS_PROP, OutPunchCorrectionRuleConfig,
};
use crate::rules::rule_config::RuleConfig;

/// `OutPunchCorrectionRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OutPunchCorrectionRule;

impl ShiftCorrectionRule for OutPunchCorrectionRule {
    fn execute(
        &self,
        shift: &mut EmployeeShift,
        _time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        if !shift.has_error(ShiftErrorType::MissingOut)
            || shift.has_error(ShiftErrorType::MissingIn)
        {
            return;
        }

        // Java never calls fixMap here and would throw on a missing
        // parameter; backfilling from the defaults is divergence 81.
        let params = rule_item
            .params()
            .fixed(&OutPunchCorrectionRuleConfig.default_values());
        let daily_hours = params.double_at(DAILY_HRS_PROP);

        let Some(in_index) = shift
            .punches()
            .iter()
            .rposition(|p| p.punch_type() == PunchType::In)
        else {
            return;
        };
        let in_punch = shift.punch(in_index).clone();

        let minutes = round_to(daily_hours * 60.0, 2) as i64;
        let plus = |t: Option<joda_rs::LocalDateTime>| t.map(|t| t.plus_minutes(minutes));

        let mut out_punch = EmployeeShiftPunch::new(
            0,
            PunchType::Out,
            PunchSource::Manual,
            in_punch
                .punch_time()
                .unwrap_or_else(|| joda_rs::LocalDateTime::of(1970, 1, 1, 0, 0, 0)),
        )
        .with_times(
            plus(in_punch.punch_time()),
            plus(in_punch.adj_time()),
            plus(in_punch.rounded_time()),
        );
        out_punch.set_raw_punch("Rule");
        out_punch.set_badge_no(in_punch.badge_no());
        out_punch.set_rounded(in_punch.rounded());

        shift.add_punch(out_punch);
        // Java sets the rounded time while the punch already points at the
        // shift, which moves the shift's end to it.
        let new_index = shift.punch_count() - 1;
        shift.reset_start_and_end_times_from_punch(new_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, h, m, 0)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "out punch", RuleClass::OutPunchScr, params)
    }

    fn shift_with_in(errors: Vec<ShiftErrorType>) -> EmployeeShift {
        let mut in_punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(8, 0));
        in_punch.set_badge_no("B7");
        in_punch.set_rounded(true);
        EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![in_punch],
        )
        .with_times(Some(at(8, 0)), None)
        .with_errors(errors)
    }

    fn run(shift: &mut EmployeeShift, params: RuleParams) {
        OutPunchCorrectionRule.execute(shift, &mut TimeCardData::new(), &rule_item(params));
    }

    #[test]
    fn a_missing_out_gets_an_out_punch_the_configured_hours_after_the_in() {
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut]);
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "8.0" });

        assert_eq!(shift.punch_count(), 2);
        let out = shift.punch(1);
        assert_eq!(out.punch_type(), PunchType::Out);
        assert_eq!(out.source(), PunchSource::Manual);
        assert_eq!(out.raw_punch(), "Rule");
        assert_eq!(out.badge_no(), "B7");
        assert!(out.rounded());
        assert_eq!(out.punch_time(), Some(at(16, 0)));
        assert_eq!(out.adj_time(), Some(at(16, 0)));
        assert_eq!(out.rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn the_shift_end_and_hours_follow_the_new_punch() {
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut]);
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "8.0" });

        assert_eq!(shift.end_date_time(), Some(at(16, 0)));
        assert_eq!(shift.start_date_time(), Some(at(8, 0)));
    }

    #[test]
    fn minutes_truncate() {
        // 7.999 h = 479.94 min -> 479.
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut]);
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "7.999" });
        assert_eq!(shift.punch(1).rounded_time(), Some(at(15, 59)));
    }

    #[test]
    fn nothing_happens_without_a_missing_out_error() {
        let mut shift = shift_with_in(Vec::new());
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "8.0" });
        assert_eq!(shift.punch_count(), 1);
    }

    #[test]
    fn nothing_happens_when_the_in_is_also_missing() {
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut, ShiftErrorType::MissingIn]);
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "8.0" });
        assert_eq!(shift.punch_count(), 1);
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
        )
        .with_errors(vec![ShiftErrorType::MissingOut]);
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "8.0" });
        assert_eq!(shift.punch_count(), 0);
    }

    #[test]
    fn the_last_in_punch_is_the_one_used() {
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut]);
        shift.add_punch(EmployeeShiftPunch::new(
            2,
            PunchType::In,
            PunchSource::Clock,
            at(10, 0),
        ));
        run(&mut shift, rule_params! { DAILY_HRS_PROP => "1.0" });
        assert_eq!(shift.punch(2).rounded_time(), Some(at(11, 0)));
    }

    #[test]
    fn an_unset_parameter_falls_back_to_eight_hours() {
        let mut shift = shift_with_in(vec![ShiftErrorType::MissingOut]);
        run(&mut shift, RuleParams::new());
        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
    }
}
