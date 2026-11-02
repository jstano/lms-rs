//! Port of `HoursThresholdFixedRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/HoursThresholdFixedRuleImpl.java`.
//!
//! Pays a fixed dollar amount for **each** shift of at least `minHours`
//! (`ShiftUtil.getNetHours(shift, actualPunchTime)`), dated the shift's date
//! and attached to the shift. No cap on how many — compare
//! `HoursThresholdFixedWithMaxCap`.
//!
//! Java reads the parameters straight off the rule item with no `fixMap`; the
//! port backfills (divergence 100). No Groovy spec.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, EARNING, HoursThresholdFixedRuleConfig, MIN_HRS_PROP,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `HoursThresholdFixedRuleImpl`.
pub struct HoursThresholdFixedRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> HoursThresholdFixedRule<E> {
    /// Build the rule over the earning writer (the earning type is only stored).
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> ShiftEarningRule for HoursThresholdFixedRule<E> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&HoursThresholdFixedRuleConfig.default_values());

        if shift.net_hours_using(params.bool_at(ACTUAL_PUNCH_PROP))
            >= params.double_at(MIN_HRS_PROP)
        {
            let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, true);
            earning.set_dollars(params.double_at(DOLLARS_PROP));
            earning.set_rate(0.0);
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_item.name());

            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn day() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }
    fn shift(hours: f64) -> EmployeeShift {
        EmployeeShift::new(4, 1, 9, day(), ShiftType::Actual, Vec::new()).with_net_hours(hours)
    }
    fn item() -> RuleItem {
        RuleItem::new(
            2,
            1,
            "HTF",
            RuleClass::HrsThreshFixedDif,
            rule_params! { DOLLARS_PROP => "12", EARNING => "3", MIN_HRS_PROP => "6", ACTUAL_PUNCH_PROP => "false" },
        )
    }
    fn run(s: &EmployeeShift, card: &mut TimeCardData) -> SavedEarnings {
        let saved = SavedEarnings::default();
        HoursThresholdFixedRule::new(saved.clone()).execute(s, card, &item());
        saved
    }

    #[test]
    fn a_shift_at_the_threshold_earns_the_fixed_amount() {
        let s = shift(6.0);
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        let saved = run(&s, &mut c);
        let e = &c.earnings()[0];
        assert_eq!(
            (e.dollars(), e.total_dollars(), e.hours(), e.rate()),
            (12.0, 12.0, 0.0, 0.0)
        );
        assert_eq!(e.earning_date(), day());
        assert_eq!(e.pay_date(), Some(day()));
        assert_eq!(
            (e.employee_id(), e.job_id(), e.earning_type_id()),
            (1, 9, 3)
        );
        assert_eq!(e.shift_id(), Some(4));
        assert_eq!(e.rule_item_id(), Some(2));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn a_shorter_shift_earns_nothing() {
        let s = shift(5.99);
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        run(&s, &mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_shift_with_errors_or_in_a_closed_period_earns_nothing() {
        let bad = shift(8.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = TimeCardData::new().with_shifts(vec![bad.clone()]);
        run(&bad, &mut c);
        assert!(c.earnings().is_empty());

        let s = shift(8.0);
        let mut closed = TimeCardData::new()
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(day().plus_days(1));
        run(&s, &mut closed);
        assert!(closed.earnings().is_empty());
    }

    #[test]
    fn missing_parameters_fall_back_to_the_defaults() {
        let s = shift(6.0);
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        let saved = SavedEarnings::default();
        HoursThresholdFixedRule::new(saved).execute(
            &s,
            &mut c,
            &RuleItem::new(
                2,
                1,
                "x",
                RuleClass::HrsThreshFixedDif,
                rule_params! { ACTUAL_PUNCH_PROP => "false" },
            ),
        );
        assert_eq!(c.earnings()[0].dollars(), 10.0, "default dollars");
    }
}
