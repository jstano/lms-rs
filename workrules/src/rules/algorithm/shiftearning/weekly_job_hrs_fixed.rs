//! Port of `WeeklyJobHrsFixedRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/WeeklyJobHrsFixedRuleImpl.java`.
//!
//! Pays a fixed dollar amount, dated the work week's **last day**, once per
//! *job* per week, when the hours worked **on that shift's job** in the week
//! fall in `[minHours, maxHours]`.
//!
//! The week is the 7 days ending on the property's period-end weekday. It runs
//! for every shift, but pays at most once per (rule item, week-end date, job):
//! an earning of this rule item already on the card for that date and job stops
//! it. Only the week-end date must be open for editing — not the shift's own
//! date — and shifts with errors are skipped.
//!
//! The earning is attached to **no** shift (`setShift(null)`).
//!
//! No Groovy spec; behaviour tests are written from the Java. Java reads the
//! parameters without `fixMap`; the port backfills (divergence 100).

use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, EARNING, MAX_HRS_PROP, MIN_HRS_PROP,
    WeeklyJobHrsFixedRuleConfig,
};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::ports::{EmployeeEarningPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use std::cell::RefCell;

/// `WeeklyJobHrsFixedRuleImpl`.
pub struct WeeklyJobHrsFixedRule<E: EmployeeEarningPort, P: PropertyPort> {
    earnings: RefCell<E>,
    property: P,
}

impl<E: EmployeeEarningPort, P: PropertyPort> WeeklyJobHrsFixedRule<E, P> {
    /// Build the rule over the earning writer and the property's calendar.
    pub fn new(earnings: E, property: P) -> Self {
        Self {
            earnings: RefCell::new(earnings),
            property,
        }
    }
}

impl<E: EmployeeEarningPort, P: PropertyPort> ShiftEarningRule for WeeklyJobHrsFixedRule<E, P> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let Some(employee) = time_card.employee() else {
            return;
        };
        let week =
            WeeklyDateRange::with_end_date(self.property.period_end_date(employee.property_id()))
                .range_containing_date(shift.shift_date());
        let end = week.end_date();

        if shift.has_errors() || !time_card.is_open_for_editing_on(end) {
            return;
        }
        if time_card.earnings().iter().any(|e| {
            e.rule_item_id() == Some(rule_item.id())
                && e.earning_date() == end
                && e.job_id() == shift.job_id()
        }) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&WeeklyJobHrsFixedRuleConfig.default_values());
        let (min, max) = (
            params.double_at(MIN_HRS_PROP),
            params.double_at(MAX_HRS_PROP),
        );
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let mut total = 0.0;
        for other in time_card.shifts() {
            if week.contains_date(other.shift_date())
                && other.job_id() == shift.job_id()
                && !other.has_errors()
            {
                total = round_to(total + other.net_hours_using(use_actual), 2);
            }
        }

        if total >= min && total <= max {
            let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, false);
            earning.set_earning_date_and_pay_date(end);
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
    use crate::entity::employee::Employee;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    struct WeekEnds(LocalDate);
    impl PropertyPort for WeekEnds {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            self.0
        }
    }

    /// The week is Mon 2014-05-05 .. Sun 2014-05-11.
    fn sunday() -> LocalDate {
        LocalDate::of(2014, 5, 11)
    }
    fn shift(id: i32, day: i32, job: i32, hours: f64) -> EmployeeShift {
        EmployeeShift::new(
            id,
            1,
            job,
            LocalDate::of(2014, 5, day),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(hours)
    }
    fn item() -> RuleItem {
        RuleItem::new(
            2,
            1,
            "WJHF",
            RuleClass::WklyJobHrsFixedDif,
            rule_params! { DOLLARS_PROP => "30", EARNING => "3", MIN_HRS_PROP => "20", MAX_HRS_PROP => "40", ACTUAL_PUNCH_PROP => "false" },
        )
    }
    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_shifts(shifts)
    }
    fn run(s: &EmployeeShift, c: &mut TimeCardData) -> SavedEarnings {
        let saved = SavedEarnings::default();
        WeeklyJobHrsFixedRule::new(saved.clone(), WeekEnds(sunday())).execute(s, c, &item());
        saved
    }

    #[test]
    fn a_job_in_the_band_earns_on_the_week_end_attached_to_no_shift() {
        let s = shift(1, 6, 9, 25.0);
        let mut c = card(vec![s.clone()]);
        let saved = run(&s, &mut c);
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), sunday());
        assert_eq!(e.pay_date(), Some(sunday()));
        assert_eq!(
            (e.dollars(), e.rate(), e.job_id(), e.shift_id()),
            (30.0, 0.0, 9, None)
        );
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn only_the_shifts_own_job_counts() {
        let s = shift(1, 6, 9, 10.0);
        let other_job = shift(2, 7, 8, 30.0);
        let mut c = card(vec![s.clone(), other_job]);
        run(&s, &mut c);
        assert!(
            c.earnings().is_empty(),
            "10h on job 9 is under the 20h floor"
        );
    }

    #[test]
    fn it_pays_once_per_job_per_week() {
        let a = shift(1, 5, 9, 12.0);
        let b = shift(2, 6, 9, 12.0);
        let mut c = card(vec![a.clone(), b.clone()]);
        run(&a, &mut c);
        run(&b, &mut c);
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn different_jobs_each_earn_once() {
        let a = shift(1, 5, 9, 25.0);
        let b = shift(2, 6, 8, 25.0);
        let mut c = card(vec![a.clone(), b.clone()]);
        run(&a, &mut c);
        run(&b, &mut c);
        assert_eq!(c.earnings().len(), 2);
    }

    #[test]
    fn the_band_is_inclusive_and_errors_and_a_closed_week_end_stop_it() {
        for (hours, expected) in [(20.0, 1), (40.0, 1), (19.99, 0), (40.01, 0)] {
            let s = shift(1, 6, 9, hours);
            let mut c = card(vec![s.clone()]);
            run(&s, &mut c);
            assert_eq!(c.earnings().len(), expected, "{hours}");
        }
        let bad = shift(1, 6, 9, 25.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = card(vec![bad.clone()]);
        run(&bad, &mut c);
        assert!(c.earnings().is_empty());

        let s = shift(1, 6, 9, 25.0);
        let mut closed =
            card(vec![s.clone()]).with_calculation_start_date(LocalDate::of(2014, 5, 12));
        run(&s, &mut closed);
        assert!(closed.earnings().is_empty());
    }
}
