//! Port of `MinBreakRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/MinBreakRuleImpl.java`.
//!
//! Pays back the shortfall on every break shorter than `Minimum Break
//! Length` — one adjustment per qualifying break, not one for the shift as
//! a whole. `ShiftUtil.getBreaks(shift, false)` pairs punches positionally
//! (see [`EmployeeShift::breaks`](crate::entity::employee_shift::EmployeeShift::breaks)),
//! so this only sees breaks at all once the shift has more than two
//! punches.
//!
//! Ported cases: `MinBreakRuleImplTest.groovy` (five cases).

use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::common::numbers::round_raw_hours;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::config::{
    MIN_BREAK_MIN_HRS_WORKED_PROP, MIN_BRK_LENGTH_PROP, MinBreakRuleConfig,
};
use crate::rules::algorithm::shiftadjustment::{ShiftAdjustmentRule, shift_is_valid};
use crate::rules::rule_config::RuleConfig;

/// `MinBreakRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinBreakRule;

impl ShiftAdjustmentRule for MinBreakRule {
    fn execute(&self, shift: &mut EmployeeShift, _dataset: &dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_valid(shift) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&MinBreakRuleConfig.default_values());
        let min_hrs = params.double_at(MIN_BREAK_MIN_HRS_WORKED_PROP);
        let min_brk = params.double_at(MIN_BRK_LENGTH_PROP);

        if shift.worked_hours() <= min_hrs {
            return;
        }

        let short_breaks: Vec<f64> = shift
            .breaks()
            .iter()
            .map(|range| range.duration().fractional_hours())
            .filter(|duration| *duration < min_brk)
            .collect();

        for break_duration in short_breaks {
            shift.apply_adjustment(round_raw_hours(-break_duration), ShiftAdjustType::Break);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
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
        RuleItem::new(1, 1, "Min break", RuleClass::MinBreakSad, params)
    }

    fn punch(id: i32, punch_type: PunchType, time: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Manual, time)
    }

    /// The Groovy fixture: IN 8:00, BREAK +133m, BACK +61m, OUT +4h — a
    /// 61-minute break (1.0167 hours).
    fn shift_with_break() -> EmployeeShift {
        let in_time = LocalDateTime::of(2014, 12, 17, 8, 0, 0);
        let break_time = in_time.plus_minutes(133);
        let back_time = break_time.plus_minutes(61);
        let out_time = back_time.plus_hours(4);

        EmployeeShift::new(
            1,
            100,
            1,
            in_time.to_local_date(),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, in_time),
                punch(2, PunchType::Break, break_time),
                punch(3, PunchType::Back, back_time),
                punch(4, PunchType::Out, out_time),
            ],
        )
        .with_times(Some(in_time), Some(out_time))
        .with_worked_hours(8.0)
    }

    mod java_parity_tests {
        use super::*;

        /// `MinBreakRuleImplTest`: "shift with errors gets no adjustment".
        #[test]
        fn shift_with_errors_gets_no_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break().with_errors(vec![ShiftErrorType::MissingOut]);

            MinBreakRule.execute(
                &mut shift,
                &dataset,
                &rule_item(MinBreakRuleConfig.default_values()),
            );

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `MinBreakRuleImplTest`: "shift without both times gets to
        /// adjustment" [sic — no adjustment].
        #[test]
        fn shift_without_both_times_gets_no_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break()
                .with_times(None, Some(LocalDateTime::of(2014, 12, 17, 17, 0, 0)));

            MinBreakRule.execute(
                &mut shift,
                &dataset,
                &rule_item(MinBreakRuleConfig.default_values()),
            );

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `MinBreakRuleImplTest`: "shift with worked hours less than min
        /// hours gets no adjustment".
        #[test]
        fn shift_with_worked_hours_under_the_minimum_gets_no_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break();
            let params =
                rule_params! { MIN_BREAK_MIN_HRS_WORKED_PROP => "10", MIN_BRK_LENGTH_PROP => "3" };

            MinBreakRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `MinBreakRuleImplTest`: "shift with break duration less than min
        /// break gets no adjustment".
        #[test]
        fn shift_with_a_break_already_over_the_minimum_gets_no_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break();
            let params =
                rule_params! { MIN_BREAK_MIN_HRS_WORKED_PROP => "5", MIN_BRK_LENGTH_PROP => ".5" };

            MinBreakRule.execute(&mut shift, &dataset, &rule_item(params));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `MinBreakRuleImplTest`: "shift with worked hours over minimum and
        /// break duration over minimum gets adjustment of break duration".
        #[test]
        fn an_adjustment_of_the_break_duration_is_applied() {
            let dataset = TimeCardData::new();
            let mut shift = shift_with_break();
            let params =
                rule_params! { MIN_BREAK_MIN_HRS_WORKED_PROP => "5", MIN_BRK_LENGTH_PROP => "2" };

            MinBreakRule.execute(&mut shift, &dataset, &rule_item(params));

            // adjHours -= roundRawHours(-1.0167) == +1.0167
            assert_eq!(shift.adj_hours(), 1.0167);
        }
    }
}
