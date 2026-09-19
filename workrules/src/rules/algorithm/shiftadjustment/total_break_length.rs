//! Port of `TotalBreakLengthRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/TotalBreakLengthRuleImpl.java`.
//!
//! Rounds the shift's *total* break time to a configured multiple, then pays
//! back the shortfall if the rounded total is still under `minimumBreakLength`,
//! or otherwise adjusts by whatever the rounding itself changed. Unlike
//! `MinBreakRuleImpl`/`PaidBreakRuleImpl`, which adjust per break, this
//! reasons about one number — the sum of every break on the shift — and
//! writes at most one adjustment.
//!
//! # A seconds value run through an hours-precision round
//!
//! `TDouble.roundHours(minimumBreakLengthInSeconds - totalBreakTime)` rounds
//! a **seconds** difference the way the rest of the tree rounds **hours** —
//! two decimal places — before dividing by `SECONDS_PER_HOUR`. Reproduced
//! exactly as written; the rounding has no real effect here since the
//! operand is already a whole number of seconds, but the call is faithful to
//! the source rather than simplified away.
//!
//! Ported cases: `TotalBreakLengthRuleImplTest.groovy` (one `where:` table,
//! 24 rows) — the four `shift: null` rows are not transcribed, since this
//! port's `execute` takes `&mut EmployeeShift`, not a nullable reference;
//! every other row is.

use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::common::numbers::{round_hours, round_raw_hours};
use crate::common::rounding::RoundingOption;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::ShiftAdjustmentRule;
use crate::rules::algorithm::shiftadjustment::config::{
    MINIMUM_BREAK_LENGTH, ROUNDING_OPTION, ROUND_TO, TotalBreakLengthRuleConfig,
};
use crate::rules::rule_config::RuleConfig;

const SECONDS_PER_MINUTE: i32 = 60;
const SECONDS_PER_HOUR: f64 = 3600.0;

/// `TotalBreakLengthRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TotalBreakLengthRule;

impl ShiftAdjustmentRule for TotalBreakLengthRule {
    fn execute(&self, shift: &mut EmployeeShift, _dataset: &dyn TimeCard, rule_item: &RuleItem) {
        if shift.has_errors() || !shift.has_both_times() {
            return;
        }

        let total_break_time = shift.total_break_time_in_minutes() * SECONDS_PER_MINUTE;
        if total_break_time == 0 {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&TotalBreakLengthRuleConfig.default_values());
        let minimum_break_length_seconds = params.int_at(MINIMUM_BREAK_LENGTH) * SECONDS_PER_MINUTE;
        let round_time_to_seconds = params.int_at(ROUND_TO) * SECONDS_PER_MINUTE;
        let rounding_option =
            RoundingOption::from_code(params.get(ROUNDING_OPTION).unwrap_or("NEAR"))
                .expect("unrecognized rounding option");
        let rounded_break_time = rounding_option.round(total_break_time, round_time_to_seconds);

        let adjusted_hours = if rounded_break_time < minimum_break_length_seconds {
            round_hours(f64::from(minimum_break_length_seconds - total_break_time))
                / SECONDS_PER_HOUR
        } else if total_break_time != rounded_break_time {
            round_hours(f64::from(rounded_break_time - total_break_time)) / SECONDS_PER_HOUR
        } else {
            return;
        };

        shift.apply_adjustment(round_raw_hours(adjusted_hours), ShiftAdjustType::Break);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "Total break length",
            RuleClass::TotalBreakLengthSad,
            params,
        )
    }

    fn props(round_to: i32, rounding_option: &str, minimum_break_length: i32) -> RuleParams {
        rule_params! {
            ROUND_TO => round_to.to_string(),
            ROUNDING_OPTION => rounding_option,
            MINIMUM_BREAK_LENGTH => minimum_break_length.to_string()
        }
    }

    /// `shiftWithBreak(breakDurationMinutes)`.
    fn shift_with_break(break_duration_minutes: i64) -> EmployeeShift {
        let start = LocalDateTime::of(2014, 1, 1, 8, 10, 0);
        let end = LocalDateTime::of(2014, 1, 1, 17, 0, 0);
        let break_time = LocalDateTime::of(2014, 1, 1, 12, 0, 0);

        EmployeeShift::new(
            1,
            100,
            1,
            start.to_local_date(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(
                    1,
                    PunchType::In,
                    crate::common::enums::punch_source::PunchSource::Manual,
                    start,
                ),
                EmployeeShiftPunch::new(
                    2,
                    PunchType::Break,
                    crate::common::enums::punch_source::PunchSource::Manual,
                    break_time,
                ),
                EmployeeShiftPunch::new(
                    3,
                    PunchType::Back,
                    crate::common::enums::punch_source::PunchSource::Manual,
                    break_time.plus_minutes(break_duration_minutes),
                ),
                EmployeeShiftPunch::new(
                    4,
                    PunchType::Out,
                    crate::common::enums::punch_source::PunchSource::Manual,
                    end,
                ),
            ],
        )
        .with_times(Some(start), Some(end))
    }

    mod java_parity_tests {
        use super::*;

        /// `TotalBreakLengthRuleImplTest`, rows with a zero-length break —
        /// "not applicable".
        #[test]
        fn no_adjustment_with_no_break_time() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break(0);

            TotalBreakLengthRule.execute(&mut shift, &dataset, &rule_item(props(10, "NONE", 5)));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `TotalBreakLengthRuleImplTest`, row: shift has errors — "not
        /// applicable".
        #[test]
        fn no_adjustment_when_the_shift_has_errors() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break(6).with_errors(vec![ShiftErrorType::MissingOut]);

            TotalBreakLengthRule.execute(&mut shift, &dataset, &rule_item(props(10, "NONE", 5)));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `TotalBreakLengthRuleImplTest`: `props(10, NEAREST, 10) |
        /// shiftWithBreak(3) | 7.0 / 60`.
        #[test]
        fn a_short_break_is_topped_up_to_the_minimum() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break(3);

            TotalBreakLengthRule.execute(&mut shift, &dataset, &rule_item(props(10, "NEAR", 10)));

            // stored +0.1167 (7 minutes), aggregate is the negation.
            assert_eq!(shift.adj_hours(), -0.1167);
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, NONE, 10) |
        /// shiftWithBreak(6) | 4.0 / 60` (also asserted for UP/DOWN/NEAREST
        /// at the same rounding, since a 6-minute break rounds to the same
        /// 5-minute multiple under every option here).
        #[test]
        fn a_break_under_the_minimum_after_rounding_is_topped_up() {
            let dataset = TimeCardData::new();

            for rounding_option in ["NONE", "UP", "DOWN", "NEAR"] {
                let mut shift = shift_with_break(6);
                TotalBreakLengthRule.execute(
                    &mut shift,
                    &dataset,
                    &rule_item(props(5, rounding_option, 10)),
                );
                assert_eq!(shift.adj_hours(), -0.0667, "rounding={rounding_option}");
            }
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, UP, 11) |
        /// shiftWithBreak(6) | 5.0 / 60`.
        #[test]
        fn rounding_up_to_a_wider_multiple_changes_the_shortfall() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break(6);

            TotalBreakLengthRule.execute(&mut shift, &dataset, &rule_item(props(5, "UP", 11)));

            assert_eq!(shift.adj_hours(), -0.0833);
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, DOWN, 10) |
        /// shiftWithBreak(4) | 6.0 / 60` (also NEAREST).
        #[test]
        fn a_four_minute_break_is_topped_up_to_ten() {
            let dataset = TimeCardData::new();

            for rounding_option in ["DOWN", "NEAR"] {
                let mut shift = shift_with_break(4);
                TotalBreakLengthRule.execute(
                    &mut shift,
                    &dataset,
                    &rule_item(props(5, rounding_option, 10)),
                );
                assert_eq!(shift.adj_hours(), -0.1, "rounding={rounding_option}");
            }
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, NONE, 10) |
        /// shiftWithBreak(12) | 0 adjustments — "not applicable"` (12 is
        /// already over the 10-minute minimum, and NONE rounding leaves it
        /// unchanged).
        #[test]
        fn no_adjustment_when_already_over_minimum_and_unrounded() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break(12);

            TotalBreakLengthRule.execute(&mut shift, &dataset, &rule_item(props(5, "NONE", 10)));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, UP, 10) |
        /// shiftWithBreak(12) | 3.0 / 60` and `props(5, DOWN/NEAREST, 10) |
        /// shiftWithBreak(12) | -2.0 / 60` — rounding a 12-minute break to a
        /// 5-minute multiple changes it even though it is already over the
        /// minimum.
        #[test]
        fn rounding_a_break_already_over_minimum_still_adjusts() {
            let dataset = TimeCardData::new();

            let mut up = shift_with_break(12);
            TotalBreakLengthRule.execute(
                &mut up,
                &TimeCardData::new(),
                &rule_item(props(5, "UP", 10)),
            );
            assert_eq!(up.adj_hours(), -0.05);

            for rounding_option in ["DOWN", "NEAR"] {
                let mut shift = shift_with_break(12);
                TotalBreakLengthRule.execute(
                    &mut shift,
                    &dataset,
                    &rule_item(props(5, rounding_option, 10)),
                );
                assert_eq!(shift.adj_hours(), 0.0333, "rounding={rounding_option}");
            }
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, NEAREST, 0) |
        /// shiftWithBreak(12) | -2.0 / 60` and `shiftWithBreak(13) | 2.0 /
        /// 60` — with `minimumBreakLength = 0`, only the rounding delta
        /// matters.
        #[test]
        fn with_no_minimum_only_rounding_produces_an_adjustment() {
            let dataset = TimeCardData::new();

            let mut shift12 = shift_with_break(12);
            TotalBreakLengthRule.execute(&mut shift12, &dataset, &rule_item(props(5, "NEAR", 0)));
            assert_eq!(shift12.adj_hours(), 0.0333);

            let mut shift13 = shift_with_break(13);
            TotalBreakLengthRule.execute(&mut shift13, &dataset, &rule_item(props(5, "NEAR", 0)));
            assert_eq!(shift13.adj_hours(), -0.0333);
        }

        /// `TotalBreakLengthRuleImplTest`: `props(5, UP, 5) |
        /// shiftWithBreak(60) | 0 adjustments` and `shiftWithBreak(58) | 2.0
        /// / 60`.
        #[test]
        fn a_break_already_a_multiple_of_five_needs_no_rounding_adjustment() {
            let dataset = TimeCardData::new();

            let mut shift60 = shift_with_break(60);
            TotalBreakLengthRule.execute(&mut shift60, &dataset, &rule_item(props(5, "UP", 5)));
            assert_eq!(shift60.adj_hours(), 0.0);

            let mut shift58 = shift_with_break(58);
            TotalBreakLengthRule.execute(&mut shift58, &dataset, &rule_item(props(5, "UP", 5)));
            assert_eq!(shift58.adj_hours(), -0.0333);
        }
    }
}
