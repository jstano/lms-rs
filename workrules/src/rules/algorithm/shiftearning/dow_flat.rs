//! Port of `DOWFlatRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/DOWFlatRuleImpl.java`.
//!
//! On a shift whose date falls on the configured weekday (database numbering,
//! Sunday = 1), pays `round(rate × netHours, 2)` **dollars** — a flat hourly
//! premium for working that day. The hours are `ShiftUtil.getNetHours(shift,
//! actualPunchTime)`; the `>= 0.0` guard only rejects a negative-hours shift.
//! Hours and rate on the earning are zero; the amount is all in `dollars`.
//!
//! `dow = 0` (which config validation accepts) matches no weekday.
//!
//! Java reads the parameters without `fixMap`; the port backfills (divergence
//! 100). No Groovy spec.

use crate::common::dates::translate_dow_to_iso;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DOW_PROP, DOWFlatRuleConfig, EARNING, RATE_PROP,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `DOWFlatRuleImpl`.
pub struct DOWFlatRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> DOWFlatRule<E> {
    /// Build the rule over the earning writer.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> ShiftEarningRule for DOWFlatRule<E> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&DOWFlatRuleConfig.default_values());
        let dow = params.int_at(DOW_PROP);

        // `translateDOWToISO(0)` is -1, which no ISO weekday equals.
        if shift.shift_date().day_of_week().value() != translate_dow_to_iso(dow) {
            return;
        }
        let net_hours = shift.net_hours_using(params.bool_at(ACTUAL_PUNCH_PROP));
        if net_hours >= 0.0 {
            let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, true);
            earning.set_dollars(round_to(params.double_at(RATE_PROP) * net_hours, 2));
            earning.set_rate(0.0);
            earning.set_hours(0.0);
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
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    /// 2014-05-06 is a Tuesday: database weekday 3.
    fn shift(hours: f64) -> EmployeeShift {
        EmployeeShift::new(
            4,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(hours)
    }
    fn run(dow: &str, s: &EmployeeShift) -> (TimeCardData, SavedEarnings) {
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        let saved = SavedEarnings::default();
        let item = RuleItem::new(
            2,
            1,
            "Flat",
            RuleClass::DowFlatDif,
            rule_params! { RATE_PROP => "2.5", EARNING => "3", DOW_PROP => dow, ACTUAL_PUNCH_PROP => "false" },
        );
        DOWFlatRule::new(saved.clone()).execute(s, &mut c, &item);
        (c, saved)
    }

    #[test]
    fn the_configured_weekday_pays_rate_times_hours_in_dollars() {
        let (c, saved) = run("3", &shift(7.5));
        let e = &c.earnings()[0];
        assert_eq!(
            (e.dollars(), e.hours(), e.rate(), e.total_dollars()),
            (18.75, 0.0, 0.0, 18.75)
        );
        assert_eq!(e.shift_id(), Some(4));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn other_weekdays_and_dow_zero_pay_nothing() {
        for dow in ["1", "2", "4", "7", "0"] {
            let (c, _) = run(dow, &shift(7.5));
            assert!(c.earnings().is_empty(), "dow {dow}");
        }
    }

    #[test]
    fn a_negative_hours_shift_pays_nothing_but_zero_hours_pays_zero_dollars() {
        let (neg, _) = run("3", &shift(-1.0));
        assert!(neg.earnings().is_empty());
        let (zero, _) = run("3", &shift(0.0));
        assert_eq!(zero.earnings()[0].dollars(), 0.0);
    }

    #[test]
    fn dollars_are_rounded_to_cents() {
        let (c, _) = run("3", &shift(1.33));
        assert_eq!(c.earnings()[0].dollars(), 3.33);
    }
}
