//! Port of `DailySalariedAllowanceRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/DailySalariedAllowanceRuleImpl.java`.
//!
//! Tops up a salaried-exempt employee's day with a fixed-dollar allowance when
//! the day's salary-distribution hours fall short of a standard work day.
//! Fires only when **all** hold: the date's weekday is among the configured
//! `dows`; the employee is active; the date is open for editing; the card has
//! at least one shift that day (any shift, errors or not); the home job's pay
//! type is salaried-exempt; and the hours of that day's earnings whose type is
//! `includeInSalaryDistribution` total *less than* `standardWorkDay`.
//!
//! # The weekday numbering
//!
//! `dows` uses the database convention Sunday = 1 … Saturday = 7; the date's
//! ISO weekday is translated with
//! [`translate_dow_from_iso`](crate::common::dates::translate_dow_from_iso).
//!
//! # The earning is saved but never added to the card
//!
//! Unlike every other rule in this family, `execute` ends in
//! `employeeEarningDAO.save(earning)` with **no** `getEarnings().add(...)`. The
//! card is not told, so a later pass over `getEarnings()` on the same run does
//! not see it (and the "less than a standard day" test cannot be defeated by
//! the rule's own earlier output). No note is set.
//!
//! Spec: `DailySalariedAllowanceRuleImplTest.groovy` — **filed under the
//! `weeklyearning` test directory**, which is why it was missed at first.

use crate::common::dates::translate_dow_from_iso;
use crate::common::enums::earning_source::EarningSource;
use crate::common::json_ids::ids_for_key;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::DailyEarningRule;
use crate::rules::algorithm::dailyearning::config::{
    AMOUNT, DOW_MULTIPLE, DailySalariedAllowanceRuleConfig, EARNING, STANDARD_WORK_DAY,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `DailySalariedAllowanceRuleImpl`.
pub struct DailySalariedAllowanceRule<T: EarningTypePort, E: EmployeeEarningPort> {
    earning_types: T,
    earnings: RefCell<E>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort> DailySalariedAllowanceRule<T, E> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort> DailyEarningRule
    for DailySalariedAllowanceRule<T, E>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&DailySalariedAllowanceRuleConfig.default_values());

        let Some(employee) = time_card.employee() else {
            return;
        };

        let included_days = ids_for_key(DOW_MULTIPLE, &params);
        let on_included_day =
            included_days.contains(&translate_dow_from_iso(date.day_of_week().value()));
        if !(on_included_day
            && employee.is_active_on_date(date)
            && time_card.is_open_for_editing_on(date)
            && !time_card.shifts_for_day(date).is_empty())
        {
            return;
        }
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        if !home.pay_type().is_salaried_exempt() {
            return;
        }

        let salaried_hours: f64 = time_card
            .earnings_for_period(&DateRange::new(date, date))
            .iter()
            .filter(|earning| {
                self.earning_types
                    .find_by_id(earning.earning_type_id())
                    .is_some_and(|t| t.include_in_salary_distribution())
            })
            .map(|earning| earning.hours())
            .sum();
        if salaried_hours >= params.double_at(STANDARD_WORK_DAY) {
            return;
        }

        // The earning type is only ever *stored*, never inspected, so the
        // configured id is used as is. The Groovy spec stubs
        // `findByID` to return null and still expects exactly one save.
        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            home.job_id(),
            params.int_at(EARNING),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_dollars(params.double_at(AMOUNT));
        earning.calc_and_set_total_dollars();

        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::uom::UOM;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use std::collections::HashMap;

    /// 2014-05-06 is a Tuesday: ISO 2, database convention 3.
    const TUESDAY: &str = "[3]";

    struct Types(HashMap<i32, EarningType>);
    impl EarningTypePort for Types {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }

    fn types() -> Types {
        let allowance = EarningType::new(1, 1, "Allowance", EarnType::Earning, UOM::Dollars);
        let salary = EarningType::new(2, 1, "Salary", EarnType::Regular, UOM::Hours)
            .with_flags(false, false, true, false);
        let other = EarningType::new(3, 1, "Other", EarnType::Regular, UOM::Hours);
        Types(HashMap::from([(1, allowance), (2, salary), (3, other)]))
    }

    fn employee(pay_type: EmployeePayType) -> Employee {
        let d = today();
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                d.minus_years(1),
                d.plus_years(1),
                pay_type,
                0.0,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            d.minus_years(1),
            d.plus_years(1),
            EmployeeStatusType::Active,
        )])
    }

    fn item(dows: &str, standard_day: &str) -> RuleItem {
        RuleItem::new(
            4,
            1,
            "Allowance",
            RuleClass::DlySalAllowDdf,
            rule_params! {
                EARNING => "1", AMOUNT => "50", STANDARD_WORK_DAY => standard_day, DOW_MULTIPLE => dows
            },
        )
    }

    fn worked_card(pay_type: EmployeePayType) -> TimeCardData {
        card(employee(pay_type), vec![shift_with_hours(today(), 4.0)])
    }

    fn run(card: &mut TimeCardData, item: &RuleItem) -> SavedEarnings {
        let saved = SavedEarnings::default();
        DailySalariedAllowanceRule::new(types(), saved.clone()).execute(card, today(), item);
        saved
    }

    #[test]
    fn a_salaried_exempt_short_day_on_an_included_weekday_is_paid() {
        let mut c = worked_card(EmployeePayType::SalariedExempt);
        let saved = run(&mut c, &item(TUESDAY, "8"));
        let saved = saved.0.borrow();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].dollars(), 50.0);
        assert_eq!(saved[0].total_dollars(), 50.0);
        assert_eq!(saved[0].earning_date(), today());
        assert_eq!(saved[0].pay_date(), Some(today()));
        assert_eq!(saved[0].job_id(), JOB);
        assert_eq!(saved[0].rule_item_id(), Some(4));
    }

    #[test]
    fn the_earning_is_saved_but_not_added_to_the_card() {
        let mut c = worked_card(EmployeePayType::SalariedExempt);
        run(&mut c, &item(TUESDAY, "8"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_weekday_not_in_the_list_is_skipped() {
        let mut c = worked_card(EmployeePayType::SalariedExempt);
        // Sunday and Saturday only.
        assert!(run(&mut c, &item("[1,7]", "8")).0.borrow().is_empty());
    }

    #[test]
    fn hourly_and_non_exempt_salaried_employees_are_skipped() {
        for pay_type in [EmployeePayType::Hourly, EmployeePayType::SalariedNonExempt] {
            let mut c = worked_card(pay_type);
            assert!(run(&mut c, &item(TUESDAY, "8")).0.borrow().is_empty());
        }
    }

    #[test]
    fn a_day_with_no_shift_is_skipped() {
        let mut c = card(employee(EmployeePayType::SalariedExempt), Vec::new());
        assert!(run(&mut c, &item(TUESDAY, "8")).0.borrow().is_empty());
    }

    #[test]
    fn a_closed_period_is_skipped() {
        let mut c = worked_card(EmployeePayType::SalariedExempt)
            .with_calculation_start_date(today().plus_days(1));
        assert!(run(&mut c, &item(TUESDAY, "8")).0.borrow().is_empty());
    }

    #[test]
    fn salary_distribution_hours_at_the_standard_day_suppress_the_allowance() {
        let salary = |hours| {
            crate::entity::employee_earning::EmployeeEarning::new(
                1,
                1,
                JOB,
                2,
                today(),
                hours,
                0.0,
                EarningSource::Auto,
            )
        };
        let mut full =
            worked_card(EmployeePayType::SalariedExempt).with_earnings(vec![salary(8.0)]);
        assert!(run(&mut full, &item(TUESDAY, "8")).0.borrow().is_empty());

        let mut short =
            worked_card(EmployeePayType::SalariedExempt).with_earnings(vec![salary(7.99)]);
        assert_eq!(run(&mut short, &item(TUESDAY, "8")).0.borrow().len(), 1);
    }

    #[test]
    fn earnings_not_in_the_salary_distribution_do_not_count() {
        let other = crate::entity::employee_earning::EmployeeEarning::new(
            1,
            1,
            JOB,
            3,
            today(),
            8.0,
            0.0,
            EarningSource::Auto,
        );
        let mut c = worked_card(EmployeePayType::SalariedExempt).with_earnings(vec![other]);
        assert_eq!(run(&mut c, &item(TUESDAY, "8")).0.borrow().len(), 1);
    }

    #[test]
    fn an_inactive_employee_is_skipped() {
        let mut c = card(
            Employee::new(1, 1, "Pat", Vec::new()),
            vec![shift_with_hours(today(), 4.0)],
        );
        assert!(run(&mut c, &item(TUESDAY, "8")).0.borrow().is_empty());
    }

    // ---- java_parity_tests: DailySalariedAllowanceRuleImplTest.groovy ----
    // (2016-09-19 is a Monday: ISO 1, database convention 2.)

    fn monday() -> LocalDate {
        LocalDate::of(2016, 9, 19)
    }

    fn spec_employee(
        status: EmployeeStatusType,
        home: bool,
        pay_type: EmployeePayType,
    ) -> Employee {
        let d = monday();
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                1,
                d.minus_years(1),
                d.plus_years(1),
                pay_type,
                0.0,
                home,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            d.minus_years(1),
            d.plus_years(1),
            status,
        )])
    }

    fn spec_item(dows: &str) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "x",
            RuleClass::DlySalAllowDdf,
            rule_params! { STANDARD_WORK_DAY => "8", DOW_MULTIPLE => dows },
        )
    }

    fn spec_shift() -> crate::entity::employee_shift::EmployeeShift {
        crate::entity::employee_shift::EmployeeShift::new(
            0,
            1,
            1,
            monday(),
            crate::common::enums::shift_type::ShiftType::Actual,
            Vec::new(),
        )
    }

    /// The spec's `earningTypeDAO` is a bare mock that answers null.
    struct NullTypes;
    impl EarningTypePort for NullTypes {
        fn find_by_id(&self, _id: i32) -> Option<EarningType> {
            None
        }
    }

    fn spec_run(card: &mut TimeCardData, dows: &str) -> usize {
        let saved = SavedEarnings::default();
        DailySalariedAllowanceRule::new(NullTypes, saved.clone()).execute(
            card,
            monday(),
            &spec_item(dows),
        );
        saved.0.borrow().len()
    }

    #[test]
    fn spec_should_not_pay_if_date_is_not_included_in_configured_dows() {
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Active,
                true,
                EmployeePayType::SalariedExempt,
            ))
            .with_shifts(vec![spec_shift()]);
        assert_eq!(spec_run(&mut c, "[1,3,4,5,6,7]"), 0);
    }

    #[test]
    fn spec_should_not_pay_if_no_shifts_for_day_exist() {
        let mut c = TimeCardData::new().with_employee(spec_employee(
            EmployeeStatusType::Active,
            true,
            EmployeePayType::SalariedExempt,
        ));
        assert_eq!(spec_run(&mut c, "[2,3,4,5,6,7]"), 0);
    }

    #[test]
    fn spec_should_not_pay_if_employee_is_not_active_on_date() {
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Terminated,
                true,
                EmployeePayType::SalariedExempt,
            ))
            .with_shifts(vec![spec_shift()]);
        // Terminated a year ago: active only on its start date.
        assert_eq!(spec_run(&mut c, "[2,3,4,5,6,7]"), 0);
    }

    #[test]
    fn spec_should_not_pay_if_employee_has_no_home_job_on_date() {
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Active,
                false,
                EmployeePayType::SalariedExempt,
            ))
            .with_shifts(vec![spec_shift()]);
        assert_eq!(spec_run(&mut c, "[2,3,4,5,6,7]"), 0);
    }

    #[test]
    fn spec_should_not_pay_if_the_home_job_is_not_salaried() {
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Active,
                true,
                EmployeePayType::Hourly,
            ))
            .with_shifts(vec![spec_shift()]);
        assert_eq!(spec_run(&mut c, "[2,3,4,5,6,7]"), 0);
    }

    #[test]
    fn spec_should_not_pay_if_salary_distribution_earnings_exceed_the_standard_day() {
        struct SalaryType;
        impl EarningTypePort for SalaryType {
            fn find_by_id(&self, _id: i32) -> Option<EarningType> {
                Some(
                    EarningType::new(0, 1, "s", EarnType::Regular, UOM::Hours)
                        .with_flags(false, false, true, false),
                )
            }
        }
        let nine = crate::entity::employee_earning::EmployeeEarning::new(
            0,
            1,
            1,
            0,
            monday(),
            9.0,
            0.0,
            EarningSource::Auto,
        );
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Active,
                true,
                EmployeePayType::SalariedExempt,
            ))
            .with_shifts(vec![spec_shift()])
            .with_earnings(vec![nine]);
        let saved = SavedEarnings::default();
        DailySalariedAllowanceRule::new(SalaryType, saved.clone()).execute(
            &mut c,
            monday(),
            &spec_item("[1,2,3,4,5,6,7]"),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn spec_should_pay_if_all_parameters_are_just_right() {
        let mut c = TimeCardData::new()
            .with_employee(spec_employee(
                EmployeeStatusType::Active,
                true,
                EmployeePayType::SalariedExempt,
            ))
            .with_shifts(vec![spec_shift()]);
        assert_eq!(
            spec_run(&mut c, "[1,2,3,4,5,6,7]"),
            1,
            "even though the earning type resolves to null"
        );
    }
}
