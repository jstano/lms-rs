//! Port of `ClosePayPeriodRuleRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/rms/timekeeping/domain/paygroup/closing/ClosePayPeriodRuleRunner.java`.
//!
//! For each employee's time card, finds the employee's **home job on the pay
//! period's last day**, resolves that job's `CLOSE_PAY_PERIOD` rule set as of that
//! day (an employee with no home job that day is skipped) and, for every rule
//! item in rule-set order, runs `undo` then `execute` (closing), or `undo` alone
//! (re-opening). Items run unsorted.
//!
//! Java loads the cards from a `TimeCardBatchRepository` and runs each in
//! "system processing mode"; here the caller hands the loaded cards in and the mode
//! has no equivalent (divergence 133).

use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::closepayperiod::ClosePayPeriodRule;
use crate::rules::dispatch::RuleRegistry;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// Runs the close-pay-period rules over loaded time cards.
pub struct ClosePayPeriodRuleRunner<'r> {
    registry: &'r dyn RuleRegistry<dyn ClosePayPeriodRule>,
}

impl<'r> ClosePayPeriodRuleRunner<'r> {
    /// Build the runner over the family's dispatch.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ClosePayPeriodRule>) -> Self {
        Self { registry }
    }

    /// `runClosePayPeriodRulesFor`: undo then execute each rule item.
    pub fn run_close_pay_period_rules<'a>(
        &self,
        time_cards: &mut [&mut dyn TimeCard],
        pay_period: &DateRange,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) {
        self.run(time_cards, pay_period, &rule_set_for, true);
    }

    /// `undoClosePayPeriodRulesFor`: undo only.
    pub fn undo_close_pay_period_rules<'a>(
        &self,
        time_cards: &mut [&mut dyn TimeCard],
        pay_period: &DateRange,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) {
        self.run(time_cards, pay_period, &rule_set_for, false);
    }

    fn run<'a>(
        &self,
        time_cards: &mut [&mut dyn TimeCard],
        pay_period: &DateRange,
        rule_set_for: &impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
        execute: bool,
    ) {
        let end = pay_period.end_date();
        for time_card in time_cards.iter_mut() {
            let Some(home_job_id) = time_card
                .employee()
                .and_then(|e| e.home_employee_job_status(end))
                .map(|home| home.job_id())
            else {
                continue;
            };
            let Some(rule_set) = rule_set_for(home_job_id, end) else {
                continue;
            };
            for item in rule_set.rule_items() {
                let Some(rule) = self.registry.rule_for(item.rule_class()) else {
                    continue;
                };
                rule.undo(pay_period, &mut **time_card, item);
                if execute {
                    rule.execute(pay_period, &mut **time_card, item, rule_set.name());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::rule_item::RuleItem;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::rule_class::RuleClass;
    use crate::rules::rule_type::RuleType;
    use std::cell::RefCell;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }

    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(&'static str, i32)>>,
    }
    impl ClosePayPeriodRule for Recorder {
        fn execute(&self, _p: &DateRange, _t: &mut dyn TimeCard, item: &RuleItem, _n: &str) {
            self.calls.borrow_mut().push(("execute", item.id()));
        }
        fn undo(&self, _p: &DateRange, _t: &mut dyn TimeCard, item: &RuleItem) {
            self.calls.borrow_mut().push(("undo", item.id()));
        }
    }
    struct Registry(Recorder);
    impl RuleRegistry<dyn ClosePayPeriodRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::ClosePayPeriod
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ClosePayPeriodRule + 'static)> {
            Some(&self.0)
        }
    }
    fn card() -> TimeCardData {
        TimeCardData::new().with_employee(Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                2,
                1,
                3,
                today().minus_years(1),
                today().plus_years(1),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        ))
    }
    fn rule_set() -> RuleSet {
        RuleSet::new(
            4,
            1,
            "s",
            RuleType::ClosePayPeriod,
            0,
            vec![RuleItem::new(
                5,
                4,
                "x",
                RuleClass::FlsaOrr,
                Default::default(),
            )],
        )
    }

    // ---- java_parity_tests: ClosePayPeriodRuleRunnerTest.groovy ----

    #[test]
    fn run_close_pay_period_rules_executes_undo_and_execute_on_each_rule_item() {
        let registry = Registry(Recorder::default());
        let set = rule_set();
        let period = DateRange::new(today(), today());
        let mut c = card();
        ClosePayPeriodRuleRunner::new(&registry).run_close_pay_period_rules(
            &mut [&mut c],
            &period,
            |job, date| {
                assert_eq!((job, date), (3, today()));
                Some(&set)
            },
        );
        assert_eq!(
            *registry.0.calls.borrow(),
            vec![("undo", 5), ("execute", 5)]
        );
    }

    #[test]
    fn undo_close_pay_period_rules_executes_undo_only() {
        let registry = Registry(Recorder::default());
        let set = rule_set();
        let period = DateRange::new(today(), today());
        let mut c = card();
        ClosePayPeriodRuleRunner::new(&registry).undo_close_pay_period_rules(
            &mut [&mut c],
            &period,
            |_, _| Some(&set),
        );
        assert_eq!(*registry.0.calls.borrow(), vec![("undo", 5)]);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn an_employee_with_no_home_job_on_the_last_day_is_skipped() {
        let registry = Registry(Recorder::default());
        let set = rule_set();
        let period = DateRange::new(today().plus_years(5), today().plus_years(5));
        let mut c = card();
        ClosePayPeriodRuleRunner::new(&registry).run_close_pay_period_rules(
            &mut [&mut c],
            &period,
            |_, _| Some(&set),
        );
        assert!(registry.0.calls.borrow().is_empty());
    }
}
