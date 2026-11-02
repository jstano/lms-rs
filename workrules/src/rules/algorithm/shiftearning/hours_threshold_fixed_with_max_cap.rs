//! Port of `HoursThresholdFixedWithMaxCapRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/HoursThresholdFixedWithMaxCapRuleImpl.java`.
//!
//! Like `HoursThresholdFixed` — a fixed dollar amount for each shift of at
//! least `minHours` — but at most `maxOccurrences` times per work week.
//!
//! # The cap counts by rule *class*, not by rule item
//!
//! The occurrences already used are the earnings dated in the shift's work week
//! whose rule item is non-null and has **the same rule class** as this one —
//! so a second rule set carrying the same rule class shares the cap, while
//! earnings of another class (or none) do not. (`weeklyearning`'s sibling counts
//! by item identity instead.) Open or closed period makes no difference here.
//!
//! The work week is the 7 days ending on the property's period-end weekday.
//! Unlike its weekly sibling there is no per-run counter: each `execute` re-counts
//! from the card, so earnings this run already wrote count.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, EARNING, HoursThresholdFixedWithMaxCapRuleConfig,
    MAX_OCCURRENCES, MIN_HRS_PROP,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EmployeeEarningPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use std::cell::RefCell;

/// `HoursThresholdFixedWithMaxCapRuleImpl`.
pub struct HoursThresholdFixedWithMaxCapRule<E: EmployeeEarningPort, P: PropertyPort> {
    earnings: RefCell<E>,
    property: P,
}

impl<E: EmployeeEarningPort, P: PropertyPort> HoursThresholdFixedWithMaxCapRule<E, P> {
    /// Build the rule over the earning writer and the property's calendar.
    pub fn new(earnings: E, property: P) -> Self {
        Self {
            earnings: RefCell::new(earnings),
            property,
        }
    }
}

impl<E: EmployeeEarningPort, P: PropertyPort> ShiftEarningRule
    for HoursThresholdFixedWithMaxCapRule<E, P>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&HoursThresholdFixedWithMaxCapRuleConfig.default_values());

        let Some(employee) = time_card.employee() else {
            return;
        };
        let work_week =
            WeeklyDateRange::with_end_date(self.property.period_end_date(employee.property_id()))
                .range_containing_date(shift.shift_date());

        let occurrences = time_card
            .earnings()
            .iter()
            .filter(|e| {
                work_week.contains_date(e.earning_date())
                    && e.rule_class() == Some(rule_item.rule_class())
            })
            .count() as i32;

        let eligible = {
            let shift_hours = shift.net_hours_using(params.bool_at(ACTUAL_PUNCH_PROP));
            shift_is_eligible(shift, time_card)
                && shift_hours >= params.double_at(MIN_HRS_PROP)
                && occurrences < params.int_at(MAX_OCCURRENCES)
        };
        if !eligible {
            return;
        }

        let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, true);
        earning.set_dollars(params.double_at(DOLLARS_PROP));
        earning.set_rate(0.0);
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
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
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    const JOB: i32 = 27;
    const NOTE: &str = "Rule: ruleSet - ruleItem";

    fn today() -> LocalDate {
        LocalDate::of(2015, 3, 6)
    }
    fn week() -> DateRange {
        DateRange::new(today(), today().plus_days(6))
    }
    struct WeekEnds(LocalDate);
    impl PropertyPort for WeekEnds {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            self.0
        }
    }

    fn params() -> RuleParams {
        rule_params! {
            EARNING => "22", DOLLARS_PROP => "10.0", MIN_HRS_PROP => "6.0",
            ACTUAL_PUNCH_PROP => "false", MAX_OCCURRENCES => "5"
        }
    }
    fn item(class: RuleClass) -> RuleItem {
        RuleItem::new(15, 1, NOTE, class, params())
    }
    fn shift(date: LocalDate, hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new()).with_net_hours(hours)
    }
    fn card(s: &EmployeeShift, calc_start: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(calc_start)
    }
    /// Period end is the week's end (the Groovy's first test uses the 7th,
    /// whose week contains `today` too).
    fn run(s: &EmployeeShift, c: &mut TimeCardData) -> SavedEarnings {
        let saved = SavedEarnings::default();
        HoursThresholdFixedWithMaxCapRule::new(saved.clone(), WeekEnds(week().end_date())).execute(
            s,
            c,
            &item(RuleClass::HrsThreshFixedWithMaxCapDif),
        );
        saved
    }
    fn verify(e: &EmployeeEarning, s: &EmployeeShift) -> bool {
        e.earning_date() == s.shift_date()
            && e.pay_date() == Some(s.shift_date())
            && e.source() == EarningSource::Rule
            && e.earning_type_id() == 22
            && e.rule_item_id() == Some(15)
            && e.note() == NOTE
            && e.employee_id() == s.employee_id()
            && e.job_id() == s.job_id()
            && e.dollars() == 10.0
            && e.hours() == 0.0
            && e.rate() == 0.0
            && e.shift_id() == Some(s.id())
    }
    fn prior(class: RuleClass, date: LocalDate) -> EmployeeEarning {
        EmployeeEarning::new(0, 1, JOB, 22, date, 0.0, 0.0, EarningSource::Rule)
            .from_rule_item(&RuleItem::new(15, 1, "x", class, RuleParams::new()), None)
    }

    // ---- java_parity_tests: HoursThresholdFixedWithMaxCapRuleImplTest.groovy ----

    #[test]
    fn should_ignore_shifts_with_errors() {
        let s = shift(today(), 8.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = card(&s, today());
        run(&s, &mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn shifts_on_or_over_minimum_hours_should_get_earning() {
        let s = shift(today(), 6.0);
        let mut c = card(&s, today());
        let saved = run(&s, &mut c);
        assert_eq!(c.earnings().len(), 1);
        assert!(verify(&c.earnings()[0], &s));
        assert_eq!(saved.0.borrow().len(), 1);
        assert!(verify(&saved.0.borrow()[0], &s));
    }

    #[test]
    fn shifts_under_minimum_hours_should_not_get_an_earning() {
        let s = shift(today(), 5.0);
        let mut c = card(&s, today());
        run(&s, &mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn shifts_before_the_calculation_start_date_should_not_get_an_earning() {
        let s = shift(today(), 8.0);
        let mut c = card(&s, today().plus_days(3));
        run(&s, &mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn should_not_create_more_than_the_max_occurrence_of_the_earning() {
        let s = shift(today().plus_days(1), 6.0);
        let mut c = card(&s, today().plus_days(1)).with_earnings(
            (0..5)
                .map(|_| prior(RuleClass::HrsThreshFixedWithMaxCapDif, today()))
                .collect(),
        );
        run(&s, &mut c);
        assert_eq!(c.earnings().len(), 5, "5 earnings already exist");
    }

    #[test]
    fn should_not_count_earnings_from_another_rule_class_as_part_of_the_max_occurrences() {
        let s = shift(today().plus_days(1), 6.0);
        let mut earnings: Vec<_> = (0..4)
            .map(|_| prior(RuleClass::HrsThreshFixedWithMaxCapDif, today()))
            .collect();
        // Same rule item id (15), different class — and one with no rule item.
        earnings.push(prior(RuleClass::MinimumHoursWorkedShiftDif, today()));
        earnings.push(EmployeeEarning::new(
            0,
            1,
            JOB,
            22,
            today(),
            0.0,
            0.0,
            EarningSource::Auto,
        ));
        let mut c = card(&s, today().plus_days(1)).with_earnings(earnings);
        let saved = run(&s, &mut c);
        assert_eq!(c.earnings().len(), 7);
        assert!(c.earnings().iter().any(|e| verify(e, &s)));
        assert_eq!(saved.0.borrow().iter().filter(|e| verify(e, &s)).count(), 1);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn earnings_outside_the_shifts_work_week_do_not_count() {
        let s = shift(today().plus_days(1), 6.0);
        let mut c = card(&s, today().plus_days(1)).with_earnings(
            (0..5)
                .map(|_| {
                    prior(
                        RuleClass::HrsThreshFixedWithMaxCapDif,
                        today().minus_days(1),
                    )
                })
                .collect(),
        );
        run(&s, &mut c);
        assert_eq!(c.earnings().len(), 6);
    }

    #[test]
    fn a_missing_parameter_falls_back_to_the_defaults() {
        let s = shift(today(), 6.0);
        let mut c = card(&s, today());
        HoursThresholdFixedWithMaxCapRule::new(
            SavedEarnings::default(),
            WeekEnds(week().end_date()),
        )
        .execute(
            &s,
            &mut c,
            &RuleItem::new(
                15,
                1,
                "x",
                RuleClass::HrsThreshFixedWithMaxCapDif,
                rule_params! { ACTUAL_PUNCH_PROP => "false" },
            ),
        );
        assert_eq!(c.earnings()[0].dollars(), 10.0);
    }
}
