//! Port of `SalariedMinWageRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/SalariedMinWageRuleImpl.java`.
//!
//! A safety net for salaried employees who work enough hours that their salary
//! falls below the minimum wage. On each date of the week that is the **last day
//! of a pay period** (and open for editing) it compares the period's effective
//! hourly wage with `minWage` and, if lower, pays the difference in dollars.
//!
//! - **Hours** are the net hours of every shift dated in the period (errors
//!   included, raw `netHours`), plus the hours of earnings whose type is in
//!   `hourlyEarnings`.
//! - **Salary** is `PayRateCalculator.getPeriodSalary` plus the `totalDollars`
//!   of earnings whose type is in `dollarsEarnings`.
//! - If `salary / hours < minWage`, the amount owed is
//!   `round_currency(minWage × hours − salary)`, paid only when positive. Zero
//!   hours divides to infinity or NaN, neither of which is `< minWage`, so a
//!   period with no hours pays nothing — same as Java's `double` arithmetic.
//!
//! The earning is dollars only, dated the period end, on the home job.
//!
//! Needs [`PeriodSalaryPort`](crate::rules::ports::PeriodSalaryPort) — a
//! single-method stand-in for `PayRateCalculator`, which is not ported.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_currency;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    DOLLARS_EARNINGS, EARNING, HOURLY_EARNINGS, MIN_WAGE_PROP, SalariedMinWageRuleConfig,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, rule_earning};
use crate::rules::ports::{EmployeeEarningPort, PeriodSalaryPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use std::cell::RefCell;

/// `SalariedMinWageRuleImpl`.
pub struct SalariedMinWageRule<E: EmployeeEarningPort, S: PeriodSalaryPort> {
    earnings: RefCell<E>,
    salary: S,
}

impl<E: EmployeeEarningPort, S: PeriodSalaryPort> SalariedMinWageRule<E, S> {
    /// Build the rule over the earning writer and the salary calculator.
    pub fn new(earnings: E, salary: S) -> Self {
        Self {
            earnings: RefCell::new(earnings),
            salary,
        }
    }
}

impl<E: EmployeeEarningPort, S: PeriodSalaryPort> WeeklyEarningRule for SalariedMinWageRule<E, S> {
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        rule_description: &str,
    ) {
        let params = rule_item
            .params()
            .fixed(&SalariedMinWageRuleConfig.default_values());
        let earning_type_id = params.int_at(EARNING);
        let min_wage = params.double_at(MIN_WAGE_PROP);
        let hourly_ids = ids_for_key(HOURLY_EARNINGS, &params);
        let dollars_ids = ids_for_key(DOLLARS_EARNINGS, &params);

        for date in work_week.dates() {
            let period = time_card.pay_period_containing(date);
            if !(period.end_date() == date && time_card.is_open_for_editing_on(date)) {
                continue;
            }

            let mut actual_hours: f64 = time_card
                .shifts()
                .iter()
                .filter(|s| period.contains_date(s.shift_date()))
                .map(|s| s.net_hours())
                .sum();
            actual_hours += time_card
                .earnings()
                .iter()
                .filter(|e| {
                    period.contains_date(e.earning_date())
                        && hourly_ids.contains(&e.earning_type_id())
                })
                .map(|e| e.hours())
                .sum::<f64>();
            let earning_dollars: f64 = time_card
                .earnings()
                .iter()
                .filter(|e| {
                    period.contains_date(e.earning_date())
                        && dollars_ids.contains(&e.earning_type_id())
                })
                .map(|e| e.total_dollars())
                .sum();

            let Some(employee) = time_card.employee() else {
                return;
            };
            let period_salary =
                self.salary.period_salary(employee.id(), period.end_date()) + earning_dollars;

            let period_wage = period_salary / actual_hours;
            if period_wage < min_wage {
                let owed = round_currency(min_wage * actual_hours - period_salary);
                if owed > 0.0 {
                    let Some(home) = employee.home_employee_job_status(date) else {
                        continue;
                    };
                    let mut earning = rule_earning(
                        employee.id(),
                        home.job_id(),
                        earning_type_id,
                        date,
                        rule_item,
                        None,
                    );
                    earning.set_hours(0.0);
                    earning.set_dollars(owed);
                    earning.set_rate(0.0);
                    earning.calc_and_set_total_dollars();
                    earning.set_note(rule_description);

                    time_card.earnings_mut().push(earning.clone());
                    self.earnings.borrow_mut().save(earning);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const JOB: i32 = 3;

    struct Salary(f64);
    impl PeriodSalaryPort for Salary {
        fn period_salary(&self, _e: i32, _end: LocalDate) -> f64 {
            self.0
        }
    }

    fn d(day: i32) -> LocalDate {
        LocalDate::of(2014, 5, day)
    }
    /// The pay period is a week, Mon 5th .. Sun 11th.
    fn period() -> DateRange {
        DateRange::new(d(5), d(11))
    }

    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                d(1),
                LocalDate::of(2015, 1, 1),
                EmployeePayType::SalariedExempt,
                0.0,
                true,
            )],
        )
    }

    fn card(hours_per_day: f64, days: i32) -> TimeCardData {
        let shifts = (0..days)
            .map(|n| {
                EmployeeShift::new(n + 1, 1, JOB, d(5 + n), ShiftType::Actual, Vec::new())
                    .with_net_hours(hours_per_day)
            })
            .collect();
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_current_pay_period(period())
    }

    fn params() -> RuleParams {
        rule_params! {
            MIN_WAGE_PROP => "7.25", EARNING => "9",
            HOURLY_EARNINGS => "[40]", DOLLARS_EARNINGS => "[41]"
        }
    }

    fn run(c: &mut TimeCardData, salary: f64) -> SavedEarnings {
        let saved = SavedEarnings::default();
        SalariedMinWageRule::new(saved.clone(), Salary(salary)).execute(
            c,
            &period(),
            &RuleItem::new(2, 1, "SMW", RuleClass::SalaryMinWageWdf, params()),
            "Rule: x - SMW",
        );
        saved
    }

    #[test]
    fn a_salary_under_the_minimum_wage_is_topped_up_on_the_period_end() {
        // 5 x 10 = 50h; min wage 7.25 -> 362.50 owed against a 300 salary.
        let mut c = card(10.0, 5);
        let saved = run(&mut c, 300.0);
        let e = &c.earnings()[0];
        assert_eq!(e.dollars(), 62.5);
        assert_eq!(e.total_dollars(), 62.5);
        assert_eq!((e.hours(), e.rate()), (0.0, 0.0));
        assert_eq!(e.earning_date(), d(11));
        assert_eq!(e.pay_date(), Some(d(11)));
        assert_eq!(e.earning_type_id(), 9);
        assert_eq!(e.job_id(), JOB);
        assert_eq!(e.note(), "Rule: x - SMW");
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn a_salary_at_or_above_the_minimum_wage_pays_nothing() {
        for salary in [362.5, 400.0] {
            let mut c = card(10.0, 5);
            run(&mut c, salary);
            assert!(c.earnings().is_empty(), "{salary}");
        }
    }

    #[test]
    fn a_period_with_no_hours_pays_nothing() {
        let mut c = card(0.0, 0);
        run(&mut c, 300.0);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn dollar_earnings_in_the_configured_types_count_toward_the_salary() {
        let mut c = card(10.0, 5).with_earnings(vec![EmployeeEarning::new(
            0,
            1,
            JOB,
            41,
            d(7),
            0.0,
            0.0,
            EarningSource::Auto,
        )]);
        c.earnings_mut()[0].set_dollars(62.5);
        run(&mut c, 300.0);
        assert_eq!(
            c.earnings().len(),
            1,
            "300 + 62.50 meets the minimum: nothing added"
        );
    }

    #[test]
    fn hourly_earnings_in_the_configured_types_add_hours() {
        // 50h of shifts + 10 earning hours = 60h -> 435 owed against 300.
        let mut c = card(10.0, 5).with_earnings(vec![EmployeeEarning::new(
            0,
            1,
            JOB,
            40,
            d(7),
            10.0,
            0.0,
            EarningSource::Auto,
        )]);
        run(&mut c, 300.0);
        assert_eq!(c.earnings().last().unwrap().dollars(), 135.0);
    }

    #[test]
    fn only_the_period_end_date_is_considered_and_it_must_be_open() {
        let mut mid_week = card(10.0, 5);
        let saved = SavedEarnings::default();
        SalariedMinWageRule::new(saved.clone(), Salary(300.0)).execute(
            &mut mid_week,
            &DateRange::new(d(5), d(10)),
            &RuleItem::new(2, 1, "SMW", RuleClass::SalaryMinWageWdf, params()),
            "d",
        );
        assert!(saved.0.borrow().is_empty(), "the 11th is outside this week");

        let mut closed = card(10.0, 5).with_calculation_start_date(d(12));
        run(&mut closed, 300.0);
        assert!(closed.earnings().is_empty());
    }
}
