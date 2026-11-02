//! Port of `PrepayContractCPRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/closepayperiod/PrepayContractCPRuleImpl.java`.
//!
//! Pays a contract employee their **next** period's contract hours in advance, at
//! the close of this one, and reverses the prepayment already made for this period
//! if they leave.
//!
//! - **Execute** does two things, adjustment first:
//!   1. *Adjust*: if this rule item wrote a positive earning dated the day before
//!      the period started (last period's prepayment for this one), write its
//!      negation — dollars and hours negated, rate kept — on the employee's **last
//!      active (or on-leave) day** in the period, on the home job that day. Only the
//!      first such earning is adjusted. This claws the prepayment back for someone
//!      who left during the period; it is written even if they did not leave
//!      (the date is the period's last active day).
//!   2. *Prepay*: for the **current property's** pay period after next — i.e. the
//!      period following the property's current one — if the employee is active
//!      at all during it and their first home job in it is a `CONTRACT` pay type
//!      with non-zero contract hours (taken on the later of that period's start and
//!      the status's start), write one earning dated the closing period's last day:
//!      hours at the home-job rate (floored at minimum wage if the type says so), or
//!      for a `DOLLARS` type `round(rate × hours, 2)`.
//! - **Undo** removes every earning in the period written by this rule item,
//!   from the card and from storage.
//!
//! The note is `"Rule: <rule set> - <rule item>"` (divergence 91).

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::uom::UOM;
use crate::common::numbers::round_to;
use crate::entity::earning_type::EarningType;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::closepayperiod::ClosePayPeriodRule;
use crate::rules::algorithm::closepayperiod::config::PrepayContractCPRuleConfig;
use crate::rules::algorithm::dailyearning::config::EARNING;
use crate::rules::algorithm::shiftearning::contract_guarantee::employee_contract_hours;
use crate::rules::ports::{CurrentPropertyPort, EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use std::cell::RefCell;

/// `PrepayContractCPRuleImpl`.
pub struct PrepayContractCPRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    C: CurrentPropertyPort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
    current_property: C,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, C: CurrentPropertyPort>
    PrepayContractCPRule<T, E, M, C>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M, current_property: C) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
            current_property,
        }
    }

    fn earning_type(&self, rule_item: &RuleItem) -> Option<EarningType> {
        let params = rule_item
            .params()
            .fixed(&PrepayContractCPRuleConfig.default_values());
        self.earning_types.find_by_id(params.int_at(EARNING))
    }

    fn note(rule_set_name: &str, rule_item: &RuleItem) -> String {
        format!("Rule: {} - {}", rule_set_name, rule_item.name())
    }

    /// `adjustCurrentPeriodIfAlreadyPaid`.
    fn adjust_current_period(
        &self,
        pay_period: &DateRange,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    ) {
        let day_before = pay_period.start_date().minus_days(1);
        let Some(existing) = time_card
            .earnings()
            .iter()
            .find(|e| {
                e.earning_date() == day_before
                    && e.rule_item_id() == Some(rule_item.id())
                    && e.total_dollars() > 0.0
            })
            .cloned()
        else {
            return;
        };
        let Some(earning_type) = self.earning_type(rule_item) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(date) = employee.last_status_active_date(pay_period) else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };

        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            home.job_id(),
            earning_type.id(),
            date,
            -existing.hours(),
            existing.rate(),
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_note(Self::note(rule_set_name, rule_item));
        earning.set_dollars(-existing.dollars());
        earning.calc_and_set_total_dollars();

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }

    /// `getContractHours(employee, nextPayPeriod)`.
    fn contract_hours_for(employee: &Employee, time_card: &dyn TimeCard, next: &DateRange) -> f64 {
        let Some(home) = employee.first_home_employee_job_status_for_period(next) else {
            return 0.0;
        };
        if !home.pay_type().is_contract() {
            return 0.0;
        }
        let date = if home.start_date() > next.start_date() {
            home.start_date()
        } else {
            next.start_date()
        };
        employee_contract_hours(employee, time_card, date)
    }

    /// `getEarningRateHomeJob(earningType, period, dataSet)`.
    fn home_job_rate(
        &self,
        employee: &Employee,
        earning_type: &EarningType,
        period: &DateRange,
    ) -> f64 {
        let home = employee.first_home_employee_job_status_for_period(period);
        let rate = home.map_or(0.0, |h| h.hourly_rate());
        let wage = match home {
            Some(h) => self.min_wage.min_wage(
                employee.property_id(),
                Some(h.job_id()),
                period.start_date(),
            ),
            None => self
                .min_wage
                .min_wage(employee.property_id(), None, period.start_date()),
        };
        if earning_type.pay_at_least_min_wage() && rate < wage {
            wage
        } else {
            rate
        }
    }

    /// `prepayNextPeriod`.
    fn prepay_next_period(
        &self,
        pay_period: &DateRange,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    ) {
        let next = self.current_property.pay_period().next();
        let Some(employee) = time_card.employee() else {
            return;
        };
        if !employee.is_active_during_period(&next) {
            return;
        }
        let contract_hours = Self::contract_hours_for(employee, time_card, &next);
        if contract_hours == 0.0 {
            return;
        }
        let Some(earning_type) = self.earning_type(rule_item) else {
            return;
        };
        let rate = self.home_job_rate(employee, &earning_type, &next);
        let Some(home) = employee.home_employee_job_status(pay_period.end_date()) else {
            return;
        };

        let date = pay_period.end_date();
        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            home.job_id(),
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_note(Self::note(rule_set_name, rule_item));
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(round_to(rate * contract_hours, 2));
            earning.set_hours(0.0);
            earning.set_rate(0.0);
        } else {
            earning.set_dollars(0.0);
            earning.set_hours(contract_hours);
            earning.set_rate(rate);
        }
        earning.calc_and_set_total_dollars();

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, C: CurrentPropertyPort>
    ClosePayPeriodRule for PrepayContractCPRule<T, E, M, C>
{
    fn execute(
        &self,
        pay_period: &DateRange,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    ) {
        self.adjust_current_period(pay_period, time_card, rule_item, rule_set_name);
        self.prepay_next_period(pay_period, time_card, rule_item, rule_set_name);
    }

    fn undo(&self, pay_period: &DateRange, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let doomed: Vec<EmployeeEarning> = time_card
            .earnings()
            .iter()
            .filter(|e| {
                pay_period.contains_date(e.earning_date())
                    && e.rule_item_id() == Some(rule_item.id())
            })
            .cloned()
            .collect();
        time_card.earnings_mut().retain(|e| {
            !(pay_period.contains_date(e.earning_date())
                && e.rule_item_id() == Some(rule_item.id()))
        });
        let mut port = self.earnings.borrow_mut();
        for earning in doomed {
            port.remove(earning.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::pay_period_type::PayPeriodType;
    use crate::common::enums::schedule_mode::ScheduleMode;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings,
    };
    use crate::rules::rule_class::RuleClass;
    use date_range_rs::daterange::monthly_date_range::MonthlyDateRange;
    use joda_rs::LocalDate;

    fn d(y: i32, m: i32, day: i32) -> LocalDate {
        LocalDate::of(y, m, day)
    }
    fn march() -> DateRange {
        MonthlyDateRange::with_end_date_on_first(d(2014, 3, 31))
    }
    struct Property(DateRange);
    impl CurrentPropertyPort for Property {
        fn pay_period(&self) -> DateRange {
            self.0
        }
    }
    fn item() -> RuleItem {
        RuleItem::new(
            7,
            1,
            "dummy rule",
            RuleClass::PrepayContractCpr,
            rule_params! { EARNING => "12" },
        )
    }
    fn earning_type(uom: UOM) -> FixedEarningType {
        FixedEarningType(EarningType::new(12, 1, "Prepay", EarnType::Earning, uom))
    }
    fn prior_prepayment() -> EmployeeEarning {
        let mut e = EmployeeEarning::new(
            0,
            1,
            5,
            12,
            march().start_date().minus_days(1),
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(7, None);
        e.set_dollars(7.0);
        e.calc_and_set_total_dollars();
        e
    }
    fn employee(term: LocalDate, pay_type: EmployeePayType, start: LocalDate) -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![
                EmployeeJobStatus::new(1, 1, 5, start, d(9999, 12, 31), pay_type, 20.0, true)
                    .with_salaried_fields(0.0, 0.0, Some(40.0), None),
            ],
        )
        .with_statuses(vec![
            EmployeeStatus::new(
                d(2012, 1, 1),
                term.minus_days(1),
                EmployeeStatusType::Active,
            ),
            EmployeeStatus::new(term, d(9999, 12, 31), EmployeeStatusType::Terminated),
        ])
    }
    fn rule(
        uom: UOM,
        saved: SavedEarnings,
        period: DateRange,
    ) -> PrepayContractCPRule<FixedEarningType, SavedEarnings, FixedMinWage, Property> {
        PrepayContractCPRule::new(
            earning_type(uom),
            saved,
            FixedMinWage(0.0),
            Property(period),
        )
    }

    // ---- java_parity_tests: PrepayContractCPRuleImplTest.groovy ----

    #[test]
    fn should_add_an_adjusted_earning_on_the_last_active_day_if_terminated_during_the_period() {
        for term in [d(2014, 3, 1), d(2014, 3, 15), d(2014, 3, 31)] {
            let saved = SavedEarnings::default();
            let mut card = TimeCardData::new()
                .with_employee(employee(term, EmployeePayType::Hourly, d(2012, 1, 1)))
                .with_earnings(vec![prior_prepayment()]);
            rule(UOM::Hours, saved.clone(), march()).execute(
                &march(),
                &mut card,
                &item(),
                "dummy rule",
            );
            assert_eq!(saved.0.borrow().len(), 1, "term {term}");
            let adjustment = card
                .earnings()
                .iter()
                .find(|e| e.earning_date() == term && e.dollars() == -7.0)
                .unwrap_or_else(|| panic!("no adjustment for {term}"));
            assert_eq!(adjustment.rule_item_id(), Some(7));
            assert_eq!(adjustment.pay_date(), Some(term));
            assert_eq!(
                (adjustment.total_dollars(), adjustment.earning_type_id()),
                (-7.0, 12)
            );
        }
    }

    // ---- behaviour tests from the Java ----

    /// An employee who stays active and is on a contract.
    fn contract_employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![
                EmployeeJobStatus::new(
                    1,
                    1,
                    5,
                    d(2012, 1, 1),
                    d(9999, 12, 31),
                    EmployeePayType::Contract,
                    20.0,
                    true,
                )
                .with_salaried_fields(0.0, 0.0, Some(10.0), None),
            ],
        )
        .with_statuses(vec![EmployeeStatus::new(
            d(2012, 1, 1),
            d(9999, 12, 31),
            EmployeeStatusType::Active,
        )])
        .with_dates(Some(d(2012, 1, 1)), None, None)
    }

    #[test]
    fn a_contract_employee_is_prepaid_next_periods_contract_hours_on_the_closing_periods_last_day()
    {
        let saved = SavedEarnings::default();
        let mut card = TimeCardData::new()
            .with_employee(contract_employee())
            .with_contract_periods(PayPeriodType::Monthly, ScheduleMode::Monthly)
            .with_current_pay_period(march());
        rule(UOM::Hours, saved.clone(), march()).execute(
            &march(),
            &mut card,
            &item(),
            "dummy rule",
        );
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &card.earnings()[0];
        assert_eq!(e.earning_date(), d(2014, 3, 31));
        assert_eq!((e.rate(), e.dollars()), (20.0, 0.0));
        assert!(e.hours() > 0.0);
        assert_eq!(e.note(), "Rule: dummy rule - dummy rule");
    }

    #[test]
    fn a_dollars_type_prepays_a_rounded_amount_and_a_non_contract_employee_nothing() {
        let saved = SavedEarnings::default();
        let mut card = TimeCardData::new()
            .with_employee(contract_employee())
            .with_contract_periods(PayPeriodType::Monthly, ScheduleMode::Monthly)
            .with_current_pay_period(march());
        rule(UOM::Dollars, saved.clone(), march()).execute(&march(), &mut card, &item(), "s");
        let e = &card.earnings()[0];
        assert_eq!((e.hours(), e.rate()), (0.0, 0.0));
        assert!(e.dollars() > 0.0);

        let none = SavedEarnings::default();
        let mut hourly = TimeCardData::new().with_employee(employee(
            d(9998, 1, 1),
            EmployeePayType::Hourly,
            d(2012, 1, 1),
        ));
        rule(UOM::Hours, none.clone(), march()).execute(&march(), &mut hourly, &item(), "s");
        assert!(none.0.borrow().is_empty());
    }

    #[test]
    fn undo_removes_this_rule_items_earnings_in_the_period_from_the_card_and_storage() {
        let saved = SavedEarnings::default();
        let in_period =
            EmployeeEarning::new(9, 1, 5, 12, d(2014, 3, 31), 0.0, 0.0, EarningSource::Rule)
                .from_rule(7, None);
        let before = prior_prepayment(); // dated before the period
        let other =
            EmployeeEarning::new(10, 1, 5, 12, d(2014, 3, 31), 0.0, 0.0, EarningSource::Rule)
                .from_rule(99, None);
        let mut card = TimeCardData::new()
            .with_employee(contract_employee())
            .with_earnings(vec![in_period, before, other]);
        rule(UOM::Hours, saved, march()).undo(&march(), &mut card, &item());
        let ids: Vec<i32> = card.earnings().iter().map(|e| e.id()).collect();
        assert_eq!(ids, vec![0, 10]);
    }

    #[test]
    fn the_no_cp_rule_does_nothing() {
        let mut card = TimeCardData::new().with_earnings(vec![prior_prepayment()]);
        let rule_item = RuleItem::new(1, 1, "n", RuleClass::NoCpr, Default::default());
        crate::rules::algorithm::closepayperiod::NoCPRule.execute(
            &march(),
            &mut card,
            &rule_item,
            "s",
        );
        crate::rules::algorithm::closepayperiod::NoCPRule.undo(&march(), &mut card, &rule_item);
        assert_eq!(card.earnings().len(), 1);
    }
}
