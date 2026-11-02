//! Port of `TimeBetweenShiftsRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/TimeBetweenShiftsRuleImpl.java`.
//!
//! A "quick turnaround" premium: when the gap between the end of the
//! employee's **previous shift** and the start of this one is shorter than
//! `Hours Between Shifts`, pays a fixed dollar amount, dated this shift's date
//! and attached to this shift.
//!
//! - The previous shift is, among the card's shifts for the same employee that
//!   end **at or before** this shift's start, the one that ends latest (a tie
//!   keeps the first). Equal timestamps count as a previous shift.
//! - The gap is `round_hours(milliseconds / 3,600,000)` between that end and this
//!   start; strictly less than the minimum pays.
//! - The shift must be fully populated (both times, a shift date, an
//!   employee, a job), have no errors and be open for editing. The comment in
//!   Java says this makes the rule run only once an OUT punch is saved.
//! - If the configured earning type cannot be found, nothing is written.
//!
//! The rule's keys are the capitalized phrases `Hours Between Shifts`,
//! `Dollars` and `Earning`. No Groovy spec; behaviour tests are written from the
//! Java.

use crate::common::dates::duration_in_fractional_hours;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    HOURS_BETWEEN_SHIFTS_PROP, TIME_BETWEEN_DOLLARS_PROP, TIME_BETWEEN_EARNING_PROP,
    TimeBetweenShiftsRuleConfig,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `TimeBetweenShiftsRuleImpl`.
pub struct TimeBetweenShiftsRule<T: EarningTypePort, E: EmployeeEarningPort> {
    earning_types: T,
    earnings: RefCell<E>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort> TimeBetweenShiftsRule<T, E> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort> ShiftEarningRule for TimeBetweenShiftsRule<T, E> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let (Some(start), Some(_end)) = (shift.start_date_time(), shift.end_date_time()) else {
            return;
        };

        let params = rule_item
            .params()
            .fixed(&TimeBetweenShiftsRuleConfig.default_values());
        let min_hours = params.double_at(HOURS_BETWEEN_SHIFTS_PROP);

        // Latest end among this employee's shifts that finish by `start`.
        let mut previous_end = None;
        for other in time_card.shifts() {
            if other.employee_id() != shift.employee_id() {
                continue;
            }
            let Some(end) = other.end_date_time() else {
                continue;
            };
            if end <= start && previous_end.is_none_or(|latest| end > latest) {
                previous_end = Some(end);
            }
        }
        let Some(previous_end) = previous_end else {
            return;
        };

        let hours_between = round_hours(duration_in_fractional_hours(previous_end, start));
        if hours_between >= min_hours {
            return;
        }

        let earning_type_id = params.int_at(TIME_BETWEEN_EARNING_PROP);
        // Only existence matters: Java stores the found type, whose id is the one
        // it was found by.
        if self.earning_types.find_by_id(earning_type_id).is_none() {
            return;
        }
        let mut earning = new_shift_earning(shift, earning_type_id, rule_item, true);
        earning.set_dollars(round_currency(params.double_at(TIME_BETWEEN_DOLLARS_PROP)));
        earning.set_rate(0.0);
        earning.set_hours(0.0);
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(day: i32, h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, day, h, m, 0)
    }
    fn shift(id: i32, employee: i32, start: LocalDateTime, end: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            id,
            employee,
            9,
            start.to_local_date(),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(start), Some(end))
    }
    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(2, 1, "TBS", RuleClass::TimeBetweenShiftsDif, params)
    }
    fn params(hours: &str) -> RuleParams {
        rule_params! {
            HOURS_BETWEEN_SHIFTS_PROP => hours, TIME_BETWEEN_DOLLARS_PROP => "4.5", TIME_BETWEEN_EARNING_PROP => "6"
        }
    }
    fn run(
        current: &EmployeeShift,
        others: Vec<EmployeeShift>,
        p: RuleParams,
    ) -> (TimeCardData, SavedEarnings) {
        let mut shifts = others;
        shifts.push(current.clone());
        let mut c = TimeCardData::new().with_shifts(shifts);
        let saved = SavedEarnings::default();
        TimeBetweenShiftsRule::new(FixedEarningType(earning_type(UOM::Dollars)), saved.clone())
            .execute(current, &mut c, &item(p));
        (c, saved)
    }

    #[test]
    fn a_short_gap_after_the_previous_shift_pays() {
        let previous = shift(1, 1, at(5, 8, 0), at(5, 16, 0));
        let current = shift(2, 1, at(6, 0, 0), at(6, 8, 0)); // 8h gap -> not short
        let (c, _) = run(&current, vec![previous.clone()], params("8"));
        assert!(c.earnings().is_empty(), "exactly the minimum is not short");

        let current = shift(2, 1, at(5, 23, 0), at(6, 7, 0)); // 7h gap
        let (c, saved) = run(&current, vec![previous], params("8"));
        let e = &c.earnings()[0];
        assert_eq!(
            (e.dollars(), e.hours(), e.rate(), e.total_dollars()),
            (4.5, 0.0, 0.0, 4.5)
        );
        assert_eq!(e.shift_id(), Some(2));
        assert_eq!(e.earning_type_id(), 6);
        assert_eq!(e.earning_date(), LocalDate::of(2014, 5, 5));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn a_previous_shift_ending_exactly_at_the_start_counts_as_a_zero_gap() {
        let previous = shift(1, 1, at(5, 8, 0), at(5, 16, 0));
        let current = shift(2, 1, at(5, 16, 0), at(5, 22, 0));
        let (c, _) = run(&current, vec![previous], params("8"));
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn the_latest_ending_previous_shift_is_the_one_measured() {
        let early = shift(1, 1, at(5, 1, 0), at(5, 5, 0));
        let late = shift(3, 1, at(5, 9, 0), at(5, 15, 0));
        let current = shift(2, 1, at(5, 22, 0), at(6, 4, 0)); // 7h after `late`, 17h after `early`
        let (c, _) = run(&current, vec![early, late], params("8"));
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn other_employees_shifts_and_later_shifts_are_ignored() {
        let someone_else = shift(1, 2, at(5, 8, 0), at(5, 22, 0));
        let later = shift(3, 1, at(7, 8, 0), at(7, 16, 0));
        let current = shift(2, 1, at(6, 0, 0), at(6, 8, 0));
        let (c, _) = run(&current, vec![someone_else, later], params("8"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_previous_shift_or_an_unfinished_shift_pays_nothing() {
        let current = shift(2, 1, at(6, 0, 0), at(6, 8, 0));
        let (c, _) = run(&current, vec![], params("8"));
        assert!(c.earnings().is_empty());

        let open = EmployeeShift::new(
            5,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(at(6, 0, 0)), None);
        let (c, _) = run(
            &open,
            vec![shift(1, 1, at(5, 20, 0), at(5, 23, 0))],
            params("8"),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_gap_is_rounded_to_two_places_before_comparing() {
        // 7h59m59s is 7.9997h -> rounds to 8.00, which is not under 8.
        let previous = shift(1, 1, at(5, 8, 0), LocalDateTime::of(2014, 5, 5, 16, 0, 1));
        let current = shift(2, 1, at(6, 0, 0), at(6, 8, 0));
        let (c, _) = run(&current, vec![previous], params("8"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn an_unknown_earning_type_writes_nothing() {
        struct None_;
        impl EarningTypePort for None_ {
            fn find_by_id(&self, _id: i32) -> Option<crate::entity::earning_type::EarningType> {
                None
            }
        }
        let previous = shift(1, 1, at(5, 8, 0), at(5, 16, 0));
        let current = shift(2, 1, at(5, 20, 0), at(6, 4, 0));
        let mut c = TimeCardData::new().with_shifts(vec![previous, current.clone()]);
        TimeBetweenShiftsRule::new(None_, SavedEarnings::default()).execute(
            &current,
            &mut c,
            &item(params("8")),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn dollars_are_rounded_to_four_places_and_the_total_to_cents() {
        let previous = shift(1, 1, at(5, 8, 0), at(5, 16, 0));
        let current = shift(2, 1, at(5, 20, 0), at(6, 4, 0));
        let p = rule_params! { HOURS_BETWEEN_SHIFTS_PROP => "8", TIME_BETWEEN_DOLLARS_PROP => "1.006", TIME_BETWEEN_EARNING_PROP => "6" };
        let (c, _) = run(&current, vec![previous], p);
        assert_eq!(
            c.earnings()[0].dollars(),
            1.006,
            "round_currency keeps four places"
        );
        assert_eq!(c.earnings()[0].total_dollars(), 1.01);
    }
}
