//! Port of `AutoBreakRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/AutoBreakRuleImpl.java`.
//!
//! Automatically credits `hrsAdjustment` hours back onto a shift that worked
//! through what should have been an unpaid break — see the family's module
//! doc for why the stored `-hrsAdjustment` value, combined with
//! `calcAdjustments`' `BREAK` subtraction, works out to an *increase* in net
//! hours rather than the decrease the name suggests.
//!
//! Skips shifts a `schedulelunch` rule already adjusted (see
//! [`EmployeeShift::has_schedule_lunch_adjustment`]) and, for time-and-attendance
//! calculations only, shifts with anything other than exactly one IN and one
//! OUT punch.
//!
//! [`EmployeeShift::has_schedule_lunch_adjustment`]: crate::entity::employee_shift::EmployeeShift::has_schedule_lunch_adjustment
//!
//! Ported cases: `AutoBreakRuleImplTest.groovy` (three `where:` tables) —
//! narrowed where the Java spec asserts adjustment-list/audit-field detail
//! this crate's `adj_hours` stand-in cannot represent; see the family's
//! module doc.

use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::config::{
    AUTO_BREAK_HRS_ADJUSTMENT_PROP, AUTO_BREAK_MIN_HOURS_PROP, AutoBreakRuleConfig,
};
use crate::rules::algorithm::shiftadjustment::{ShiftAdjustmentRule, shift_is_valid};
use crate::rules::rule_config::RuleConfig;

/// `AutoBreakRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoBreakRule;

impl AutoBreakRule {
    /// `shouldAddAdjustment`.
    fn should_add_adjustment(
        &self,
        shift: &EmployeeShift,
        dataset: &dyn TimeCard,
        min_hours: f64,
    ) -> bool {
        (dataset.calculation_mode() != EmployeeCalculationMode::Ta || shift.punch_count() == 2)
            && shift.worked_hours() > min_hours
            && !shift.has_schedule_lunch_adjustment()
    }
}

impl ShiftAdjustmentRule for AutoBreakRule {
    fn execute(&self, shift: &mut EmployeeShift, dataset: &dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_valid(shift) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&AutoBreakRuleConfig.default_values());
        let min_hours = params.double_at(AUTO_BREAK_MIN_HOURS_PROP);
        let hrs_adjustment = params.double_at(AUTO_BREAK_HRS_ADJUSTMENT_PROP);

        if self.should_add_adjustment(shift, dataset, min_hours) {
            shift.apply_adjustment(-hrs_adjustment, ShiftAdjustType::Break);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(15, 1, "Auto break", RuleClass::AutoBreakSad, params)
    }

    fn valid_shift(worked_hours: f64) -> EmployeeShift {
        let start = LocalDateTime::of(2014, 2, 19, 8, 0, 0);
        let end = LocalDateTime::of(2014, 2, 19, 16, 0, 0);
        EmployeeShift::new(
            1,
            100,
            1,
            start.to_local_date(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(
                    1,
                    crate::common::enums::punch_type::PunchType::In,
                    crate::common::enums::punch_source::PunchSource::Clock,
                    start,
                ),
                EmployeeShiftPunch::new(
                    2,
                    crate::common::enums::punch_type::PunchType::Out,
                    crate::common::enums::punch_source::PunchSource::Clock,
                    end,
                ),
            ],
        )
        .with_times(Some(start), Some(end))
        .with_worked_hours(worked_hours)
        .with_net_hours(worked_hours)
    }

    mod java_parity_tests {
        use super::*;

        /// `AutoBreakRuleImplTest`: "should not add if shift contains a
        /// schedule lunch adjustment" — narrowed to the flag this crate
        /// carries in place of a provenance-tagged adjustments list.
        #[test]
        fn no_adjustment_when_a_schedule_lunch_rule_already_touched_the_shift() {
            let dataset = TimeCardData::new();
            let mut shift = valid_shift(8.0).with_schedule_lunch_adjustment();

            AutoBreakRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

            assert_eq!(shift.adj_hours(), 0.0);
        }

        /// `AutoBreakRuleImplTest`: same case, `noAdjustmentAdded = false`
        /// rows — a prior non-schedule-lunch adjustment, or none at all,
        /// does not block the rule.
        #[test]
        fn an_adjustment_is_added_without_a_prior_schedule_lunch_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = valid_shift(8.0);

            AutoBreakRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

            assert_ne!(shift.adj_hours(), 0.0);
        }

        /// `AutoBreakRuleImplTest`: "the rule should not be run when the
        /// shift has errors or is missing times".
        #[test]
        fn no_adjustment_when_the_shift_has_errors_or_is_missing_times() {
            let dataset = TimeCardData::new();
            let start = LocalDateTime::of(2014, 2, 19, 8, 0, 0);
            let end = LocalDateTime::of(2014, 2, 19, 13, 0, 0);

            type Case = (
                Vec<ShiftErrorType>,
                Option<LocalDateTime>,
                Option<LocalDateTime>,
                bool,
            );
            let cases: [Case; 4] = [
                (Vec::new(), Some(start), Some(end), true),
                (
                    vec![ShiftErrorType::MissingOut],
                    Some(start),
                    Some(end),
                    false,
                ),
                (Vec::new(), Some(start), None, false),
                (Vec::new(), None, Some(end), false),
            ];

            for (errors, s, e, should_run) in cases {
                let mut shift = valid_shift(8.0).with_errors(errors).with_times(s, e);
                AutoBreakRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));
                assert_eq!(shift.adj_hours() != 0.0, should_run);
            }
        }

        /// `AutoBreakRuleImplTest`: "adjustments added should be of type
        /// BREAK with -hrsAdjustment".
        #[test]
        fn the_stored_adjustment_is_negative_hrs_adjustment() {
            let dataset = TimeCardData::new();
            let mut shift = valid_shift(8.0);
            let params = rule_params! { AUTO_BREAK_HRS_ADJUSTMENT_PROP => "8.5" };

            AutoBreakRule.execute(&mut shift, &dataset, &rule_item(params));

            // adjHours -= (-8.5) == +8.5
            assert_eq!(shift.adj_hours(), 8.5);
        }
    }
}
