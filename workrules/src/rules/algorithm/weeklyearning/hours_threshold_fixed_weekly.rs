//! Port of `HoursThresholdFixedWeeklyRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/HoursThresholdFixedWeeklyRuleImpl.java`.
//!
//! Pays a fixed dollar amount **per shift** of at least `minHours` — but at
//! most `maxOccurrences` times in the work week. The count starts from the
//! earnings this rule item already wrote this week that sit in a **closed**
//! period (they can no longer be recomputed, so they still use up the cap);
//! each new earning raises it.
//!
//! Shifts are taken in card order from the week's shifts (by shift date), and
//! must have no persisted errors and be open for editing. Each earning is dated
//! on the shift's date, attached to the shift, and on the shift's job and
//! employee. Hours are `ShiftUtil.getNetHours(shift, actualPunchTime)`.
//!
//! "This rule item" is Java identity; the port compares rule item ids.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, EARNING, HoursThresholdFixedWeeklyRuleConfig, MAX_OCCURRENCES,
    MIN_HRS_PROP,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, rule_earning};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use std::cell::RefCell;

/// `HoursThresholdFixedWeeklyRuleImpl`.
pub struct HoursThresholdFixedWeeklyRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> HoursThresholdFixedWeeklyRule<E> {
    /// Build the rule over the earning writer.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> WeeklyEarningRule for HoursThresholdFixedWeeklyRule<E> {
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        _rule_description: &str,
    ) {
        let params = rule_item
            .params()
            .fixed(&HoursThresholdFixedWeeklyRuleConfig.default_values());
        let earning_type_id = params.int_at(EARNING);
        let dollars = params.double_at(DOLLARS_PROP);
        let min_hours = params.double_at(MIN_HRS_PROP);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let max_occurrences = params.int_at(MAX_OCCURRENCES);

        let mut occurrences = time_card
            .earnings()
            .iter()
            .filter(|e| {
                work_week.contains_date(e.earning_date())
                    && !time_card.is_open_for_editing_for_earning(e)
                    && e.rule_item_id() == Some(rule_item.id())
            })
            .count() as i32;

        // `getShiftsForDateRange` is a snapshot; the card grows below.
        let shifts: Vec<EmployeeShift> = time_card
            .shifts_for_period(work_week)
            .into_iter()
            .cloned()
            .collect();
        for shift in shifts {
            let shift_hours = shift.net_hours_using(use_actual);
            let eligible = !shift.has_errors()
                && time_card.is_open_for_editing_for_shift(&shift)
                && shift_hours >= min_hours
                && occurrences < max_occurrences;
            if !eligible {
                continue;
            }

            let mut earning = rule_earning(
                shift.employee_id(),
                shift.job_id(),
                earning_type_id,
                shift.shift_date(),
                rule_item,
                Some(shift.id()),
            );
            earning.set_dollars(dollars);
            earning.set_rate(0.0);
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_item.name());

            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
            occurrences += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
    use joda_rs::LocalDate;

    const JOB: i32 = 27;
    const NOTE: &str = "Rule: ruleSet - ruleItem";

    fn week() -> DateRange {
        WeeklyDateRange::with_start_date(LocalDate::of(2015, 2, 2))
    }

    fn params() -> RuleParams {
        rule_params! {
            EARNING => "22", DOLLARS_PROP => "10.0", MIN_HRS_PROP => "6.0",
            ACTUAL_PUNCH_PROP => "false", MAX_OCCURRENCES => "5"
        }
    }

    fn item() -> RuleItem {
        RuleItem::new(15, 1, NOTE, RuleClass::HrsThreshFixedWdf, params())
    }

    fn shift(id: i32, date: LocalDate, hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new()).with_net_hours(hours)
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_shifts(shifts)
            .with_calculation_start_date(week().start_date())
    }

    fn run(c: &mut TimeCardData) -> SavedEarnings {
        let saved = SavedEarnings::default();
        HoursThresholdFixedWeeklyRule::new(saved.clone()).execute(c, &week(), &item(), "something");
        saved
    }

    /// `verifyEarning`.
    fn verify(e: &EmployeeEarning, shift: &EmployeeShift) -> bool {
        e.earning_date() == shift.shift_date()
            && e.pay_date() == Some(shift.shift_date())
            && e.source() == EarningSource::Rule
            && e.earning_type_id() == 22
            && e.rule_item_id() == Some(15)
            && e.note() == NOTE
            && e.employee_id() == shift.employee_id()
            && e.job_id() == shift.job_id()
            && e.dollars() == 10.0
            && e.hours() == 0.0
            && e.rate() == 0.0
            && e.shift_id() == Some(shift.id())
    }

    fn day(n: i64) -> LocalDate {
        week().start_date().plus_days(n)
    }

    // ---- java_parity_tests: HoursThresholdFixedWeeklyRuleImplTest.groovy ----

    #[test]
    fn no_shifts_in_week() {
        let mut c = card(Vec::new());
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn should_ignore_shifts_with_errors() {
        let bad = shift(1, day(0), 6.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = card(vec![bad]);
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn shifts_on_or_over_minimum_hours_should_get_earning() {
        let s = shift(1, day(0), 6.0);
        let mut c = card(vec![s.clone()]);
        let saved = run(&mut c);
        assert_eq!(c.earnings().len(), 1);
        assert!(verify(&c.earnings()[0], &s));
        assert_eq!(saved.0.borrow().len(), 1);
        assert!(verify(&saved.0.borrow()[0], &s));
    }

    #[test]
    fn shifts_under_minimum_hours_should_not_get_an_earning() {
        let mut c = card(vec![shift(1, day(0), 5.0)]);
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn shifts_in_closed_pay_period_in_week_should_not_get_an_earning() {
        let mut c = card(vec![shift(1, day(0), 6.0)]).with_calculation_start_date(day(3));
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn shifts_outside_the_work_week_in_open_pay_period_should_not_get_an_earning() {
        let mut c = card(vec![
            shift(1, day(0).minus_days(1), 6.0),
            shift(2, day(9), 6.0),
        ])
        .with_calculation_start_date(day(3));
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn should_only_create_up_to_the_max_occurrence_earnings() {
        let shifts: Vec<_> = (0..6).map(|n| shift(n as i32 + 1, day(n), 6.0)).collect();
        let mut c = card(shifts.clone());
        let saved = run(&mut c);
        assert_eq!(c.earnings().len(), 5);
        for s in &shifts[..5] {
            assert!(c.earnings().iter().any(|e| verify(e, s)));
            assert_eq!(saved.0.borrow().iter().filter(|e| verify(e, s)).count(), 1);
        }
        assert!(!c.earnings().iter().any(|e| verify(e, &shifts[5])));
    }

    #[test]
    fn earnings_in_the_week_in_a_closed_period_for_this_rule_count_toward_the_occurrences() {
        let shifts: Vec<_> = (0..6).map(|n| shift(n as i32 + 1, day(n), 6.0)).collect();
        let old = |date, rule_item: i32, shift_id: Option<i32>| {
            EmployeeEarning::new(0, 1, JOB, 22, date, 0.0, 0.0, EarningSource::Rule)
                .from_rule(rule_item, shift_id)
        };
        let earnings = vec![
            old(day(0).minus_days(1), 15, None),
            old(day(0), 15, Some(1)),
            old(day(1), 15, Some(2)),
            old(day(1), 0, None),
        ];
        let mut c = card(shifts.clone())
            .with_earnings(earnings)
            .with_calculation_start_date(day(2));
        let saved = run(&mut c);
        assert_eq!(c.earnings().len(), 7);
        for s in &shifts[2..5] {
            assert!(c.earnings().iter().any(|e| verify(e, s)));
            assert_eq!(saved.0.borrow().iter().filter(|e| verify(e, s)).count(), 1);
        }
        assert_eq!(saved.0.borrow().len(), 3);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn exactly_the_minimum_hours_qualifies_and_just_under_does_not() {
        for (hours, expected) in [(6.0, 1), (5.99, 0)] {
            let mut c = card(vec![shift(1, day(0), hours)]);
            run(&mut c);
            assert_eq!(c.earnings().len(), expected, "{hours}");
        }
    }
}
