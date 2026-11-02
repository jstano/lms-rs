//! Port of `DaysWorkedDailyDifferentialRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/DaysWorkedDailyDifferentialRuleImpl.java`.
//!
//! Pays a **fixed dollar amount** for a day worked in a week in which the
//! employee has worked *fewer than* `weeklyDaysWorkedThreshold` distinct days —
//! a premium for a short work week. Which of two amounts depends on the day's
//! hours against `dailyHoursThreshold`: `firstDailyHoursFixedCost` up to and
//! including it, `secondDailyHoursFixedCost` above.
//!
//! # What counts
//!
//! - **Days worked** in the property's work week containing the date:
//!   the dates of shifts with no persisted errors, plus the dates of earnings
//!   whose type is `REGULAR` or `PREMIUM`. A day with both counts once.
//! - **Hours for the day**: if anything is scheduled that date, the
//!   *scheduled* net hours (so a short actual shift on a long scheduled day
//!   still earns the second amount); otherwise the worked hours, which are the
//!   rounded shift net hours plus the rounded hours of that day's regular and
//!   premium earnings. A day with no worked hours pays nothing.
//!
//! # No period check
//!
//! Unlike the slice-1 rules this one does **not** test
//! `isOpenForEditingOn`; it only requires the employee to be active and to
//! have a home job on the date.
//!
//! The earning is dated and paid on the day, attributed to the home job,
//! appended to the card, then saved.

use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_hours;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::DailyEarningRule;
use crate::rules::algorithm::dailyearning::config::{
    DAILY_HOURS_THRESHOLD, DaysWorkedDailyDifferentialRuleConfig, EARNING,
    FIRST_DAILY_HOURS_FIXED_COST, SECOND_DAILY_HOURS_FIXED_COST, WEEKLY_DAYS_WORKED_THRESHOLD,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::HashSet;

/// `DaysWorkedDailyDifferentialRuleImpl`.
pub struct DaysWorkedDailyDifferentialRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    P: PropertyPort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    property: P,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, P: PropertyPort>
    DaysWorkedDailyDifferentialRule<T, E, P>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, property: P) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            property,
        }
    }

    /// `isEarningRegularOrPremium`. An earning whose type cannot be resolved
    /// is not counted (Java would throw).
    fn is_regular_or_premium(&self, earning: &EmployeeEarning) -> bool {
        self.earning_types
            .find_by_id(earning.earning_type_id())
            .is_some_and(|t| t.earn_type().is_regular() || t.earn_type().is_premium())
    }

    fn days_worked_in_range(&self, time_card: &dyn TimeCard, range: &DateRange) -> usize {
        let mut days: HashSet<LocalDate> = HashSet::new();
        for shift in time_card.shifts() {
            if range.contains_date(shift.shift_date()) && !shift.has_errors() {
                days.insert(shift.shift_date());
            }
        }
        for earning in time_card.earnings() {
            if range.contains_date(earning.earning_date()) && self.is_regular_or_premium(earning) {
                days.insert(earning.earning_date());
            }
        }
        days.len()
    }

    fn hours_worked_on_date(&self, time_card: &dyn TimeCard, date: LocalDate) -> f64 {
        let shift_hours: f64 = time_card
            .shifts()
            .iter()
            .filter(|s| s.shift_date() == date && !s.has_errors())
            .map(|s| s.net_hours())
            .sum();
        let earning_hours: f64 = time_card
            .earnings()
            .iter()
            .filter(|e| e.earning_date() == date && self.is_regular_or_premium(e))
            .map(|e| e.hours())
            .sum();
        round_hours(round_hours(shift_hours) + round_hours(earning_hours))
    }

    fn dollars_for(&self, time_card: &dyn TimeCard, date: LocalDate, params: &RuleParams) -> f64 {
        let scheduled = round_hours(
            time_card
                .schedules()
                .iter()
                .filter(|s| s.shift_date() == date)
                .map(|s| s.net_hours())
                .sum(),
        );
        let worked = round_hours(self.hours_worked_on_date(time_card, date));
        if worked <= 0.0 {
            return 0.0;
        }
        let hours = if scheduled <= 0.0 { worked } else { scheduled };
        if hours <= params.double_at(DAILY_HOURS_THRESHOLD) {
            params.double_at(FIRST_DAILY_HOURS_FIXED_COST)
        } else {
            params.double_at(SECOND_DAILY_HOURS_FIXED_COST)
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, P: PropertyPort> DailyEarningRule
    for DaysWorkedDailyDifferentialRule<T, E, P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        let Some(employee) = time_card.employee() else {
            return;
        };
        if !employee.is_active_on_date(date) {
            return;
        }
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        let (employee_id, home_job_id, property_id) =
            (employee.id(), home.job_id(), employee.property_id());

        // Java reads the parameters without `fixMap`; the defaults here stand in
        // for the NullPointerException a missing one would throw (divergence 89).
        let params = rule_item
            .params()
            .fixed(&DaysWorkedDailyDifferentialRuleConfig.default_values());

        let work_week = WeeklyDateRange::with_end_date(self.property.period_end_date(property_id))
            .range_containing_date(date);
        let threshold = params.int_at(WEEKLY_DAYS_WORKED_THRESHOLD);
        if threshold as i64 <= self.days_worked_in_range(time_card, &work_week) as i64 {
            return;
        }

        let dollars = self.dollars_for(time_card, date, &params);
        if dollars <= 0.0 {
            return;
        }
        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };

        let mut earning = EmployeeEarning::new(
            0,
            employee_id,
            home_job_id,
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_dollars(dollars);
        earning.calc_and_set_total_dollars();
        // `"Rule: " + ruleItem.toString()`, and `RuleItem.toString()` is
        // `"RuleItem: " + name`.
        earning.set_note(format!("Rule: RuleItem: {}", rule_item.name()));

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use std::collections::HashMap;

    const FIRST_COST: f64 = 1.5;
    const SECOND_COST: f64 = 3.0;
    /// Resolves the earning type the rule is configured with (id 1) and a
    /// fixed type per earning id the fixtures use.
    struct Types(HashMap<i32, EarningType>);

    impl EarningTypePort for Types {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }

    struct WeekEndsOn(LocalDate);

    impl PropertyPort for WeekEndsOn {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.0
        }
    }

    fn week_start() -> LocalDate {
        today()
    }

    fn week_end() -> LocalDate {
        today().plus_days(6)
    }

    fn types() -> Types {
        let t = |id, earn_type| EarningType::new(id, 1, "t", earn_type, UOM::Hours);
        Types(HashMap::from([
            (1, t(1, EarnType::Earning)),
            (100, t(100, EarnType::Earning)),
            (101, t(101, EarnType::Regular)),
            (102, t(102, EarnType::Premium)),
        ]))
    }

    fn employee() -> Employee {
        let job = EmployeeJobStatus::new(
            1,
            1,
            JOB,
            week_start(),
            week_end(),
            EmployeePayType::Hourly,
            17.5,
            true,
        );
        Employee::new(1, 1, "Pat", vec![job]).with_statuses(vec![
            EmployeeStatus::new(week_start(), week_end(), EmployeeStatusType::Active),
            EmployeeStatus::new(
                week_end().plus_days(1),
                LocalDate::of(9999, 12, 31),
                EmployeeStatusType::Terminated,
            ),
        ])
    }

    fn item() -> RuleItem {
        RuleItem::new(
            9,
            1,
            "Days",
            RuleClass::DaysWorkedDdf,
            rule_params! {
                WEEKLY_DAYS_WORKED_THRESHOLD => "5", EARNING => "1",
                DAILY_HOURS_THRESHOLD => "6.0",
                FIRST_DAILY_HOURS_FIXED_COST => "1.5", SECOND_DAILY_HOURS_FIXED_COST => "3.0"
            },
        )
    }

    fn shift(date: LocalDate, net_hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new()).with_net_hours(net_hours)
    }

    fn schedule(date: LocalDate, net_hours: f64) -> EmployeeShift {
        EmployeeShift::new(2, 1, JOB, date, ShiftType::Schedule, Vec::new())
            .with_net_hours(net_hours)
    }

    fn earning(date: LocalDate, type_id: i32, hours: f64) -> EmployeeEarning {
        EmployeeEarning::new(0, 1, JOB, type_id, date, hours, 0.0, EarningSource::Auto)
    }

    fn run(card: &mut TimeCardData) -> SavedEarnings {
        run_with(card, item())
    }

    fn run_with(card: &mut TimeCardData, item: RuleItem) -> SavedEarnings {
        let saved = SavedEarnings::default();
        DaysWorkedDailyDifferentialRule::new(types(), saved.clone(), WeekEndsOn(week_end()))
            .execute(card, week_start(), &item);
        saved
    }

    fn rule_earnings(card: &TimeCardData) -> Vec<&EmployeeEarning> {
        card.earnings()
            .iter()
            .filter(|e| e.source() == EarningSource::Rule)
            .collect()
    }

    fn base() -> TimeCardData {
        TimeCardData::new().with_employee(employee())
    }

    // ---- java_parity_tests: DaysWorkedDailyDifferentialRuleImplTest.groovy ----

    #[test]
    fn no_extra_earning_if_the_employee_did_not_work_at_all() {
        let mut c = base();
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_fixed_earning_for_unscheduled_work_based_on_worked_hours() {
        for (hours, expected) in [
            (5.0, FIRST_COST),
            (6.0, FIRST_COST),
            (7.0, SECOND_COST),
            (8.0, SECOND_COST),
        ] {
            let mut c = base().with_shifts(vec![shift(week_start(), hours)]);
            let saved = run(&mut c);
            assert_eq!(c.earnings().len(), 1, "{hours}");
            assert_eq!(c.earnings()[0].total_dollars(), expected, "{hours}");
            assert_eq!(saved.0.borrow().len(), 1);
        }
    }

    #[test]
    fn a_fixed_earning_for_scheduled_work_based_on_scheduled_hours() {
        for (hours, expected) in [
            (5.0, FIRST_COST),
            (6.0, FIRST_COST),
            (7.0, SECOND_COST),
            (8.0, SECOND_COST),
        ] {
            let mut c = base()
                .with_shifts(vec![shift(week_start(), 4.0)])
                .with_schedules(vec![schedule(week_start(), hours)]);
            run(&mut c);
            assert_eq!(c.earnings().len(), 1, "{hours}");
            assert_eq!(c.earnings()[0].total_dollars(), expected, "{hours}");
        }
    }

    #[test]
    fn no_earning_if_inactive_on_the_date() {
        let mut inactive = employee();
        inactive = inactive.with_statuses(vec![EmployeeStatus::new(
            week_start().plus_days(1),
            week_end(),
            EmployeeStatusType::Active,
        )]);
        let mut c = TimeCardData::new()
            .with_employee(inactive)
            .with_shifts(vec![shift(week_start(), 8.0)]);
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_earning_without_a_home_job_on_the_date() {
        let job = EmployeeJobStatus::new(
            1,
            1,
            JOB,
            week_start().plus_days(1),
            week_end(),
            EmployeePayType::Hourly,
            17.5,
            true,
        );
        let e = Employee::new(1, 1, "Pat", vec![job]).with_statuses(vec![EmployeeStatus::new(
            week_start(),
            week_end(),
            EmployeeStatusType::Active,
        )]);
        let mut c = TimeCardData::new()
            .with_employee(e)
            .with_shifts(vec![shift(week_start(), 8.0)]);
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_earning_once_the_threshold_of_days_has_been_worked() {
        let shifts = (0..5)
            .map(|d| shift(week_start().plus_days(d), 8.0))
            .collect();
        let mut c = base().with_shifts(shifts);
        run(&mut c);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn regular_or_premium_earnings_on_other_days_count_toward_the_threshold() {
        for (type_id, should_have_earning) in [(100, true), (101, false), (102, false)] {
            let mut c = base()
                .with_shifts(vec![shift(week_start(), 8.0)])
                .with_earnings(
                    (1..=4)
                        .map(|d| earning(week_start().plus_days(d), type_id, 8.0))
                        .collect(),
                );
            run(&mut c);
            assert_eq!(
                !rule_earnings(&c).is_empty(),
                should_have_earning,
                "{type_id}"
            );
        }
    }

    #[test]
    fn unscheduled_work_sums_shift_hours_and_regular_earning_hours() {
        let mut c = base()
            .with_shifts(vec![shift(week_start(), 4.0)])
            .with_earnings(vec![earning(week_start(), 101, 4.0)]);
        run(&mut c);
        assert_eq!(c.earnings().len(), 2);
        assert_eq!(rule_earnings(&c)[0].total_dollars(), SECOND_COST);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_day_with_a_shift_and_a_regular_earning_counts_once() {
        // 4 days with shifts + the same 4 days with regular earnings = 4, not 8,
        // so threshold 5 is still unmet and the day pays.
        let shifts: Vec<_> = (0..4)
            .map(|d| shift(week_start().plus_days(d), 8.0))
            .collect();
        let earnings: Vec<_> = (0..4)
            .map(|d| earning(week_start().plus_days(d), 101, 8.0))
            .collect();
        let mut c = base().with_shifts(shifts).with_earnings(earnings);
        run(&mut c);
        assert_eq!(rule_earnings(&c).len(), 1);
    }

    #[test]
    fn shifts_with_errors_do_not_count_as_days_worked() {
        let mut shifts: Vec<_> = (0..5)
            .map(|d| shift(week_start().plus_days(d), 8.0))
            .collect();
        shifts[4] = shifts[4]
            .clone()
            .with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = base().with_shifts(shifts);
        run(&mut c);
        assert_eq!(rule_earnings(&c).len(), 1);
    }

    #[test]
    fn the_period_need_not_be_open_for_editing() {
        let mut c = base()
            .with_shifts(vec![shift(week_start(), 8.0)])
            .with_calculation_start_date(week_end().plus_days(30));
        run(&mut c);
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn the_earning_is_dated_attributed_and_noted() {
        let mut c = base().with_shifts(vec![shift(week_start(), 8.0)]);
        run(&mut c);
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), week_start());
        assert_eq!(e.pay_date(), Some(week_start()));
        assert_eq!(e.job_id(), JOB);
        assert_eq!(e.dollars(), SECOND_COST);
        assert_eq!(e.rule_item_id(), Some(9));
        assert_eq!(e.note(), "Rule: RuleItem: Days");
    }
}
