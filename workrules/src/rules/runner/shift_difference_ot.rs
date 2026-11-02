//! Port of `ShiftDifferenceOTRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ShiftDifferenceOTRunner.java`.
//!
//! Runs the single-rule `SHIFT_DIFF_OT` family. Nothing happens for a card with
//! neither shifts nor earnings — not even clearing. Otherwise: clear the
//! earnings an earlier `SHIFT_DIFF_OT` run wrote, take the employee's **home
//! job on the calculation start date** (none, nothing runs), resolve that job's
//! `SHIFT_DIFF_OT` rule set for that date, and run each of its items in rule-set
//! order (no priority sort).

use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftdifferenceot::{OvertimeRateLookup, ShiftDifferenceOT};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_type::RuleType;
use crate::rules::runner::clear_time_card_earnings::clear_time_card_earnings;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// Runs the shift-difference-over-time rule over a time card.
pub struct ShiftDifferenceOTRunner<'r, E: EmployeeEarningPort> {
    registry: &'r dyn RuleRegistry<dyn ShiftDifferenceOT>,
    earnings: RefCell<E>,
}

impl<'r, E: EmployeeEarningPort> ShiftDifferenceOTRunner<'r, E> {
    /// Build the runner over the rule's dispatch and the earning port.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ShiftDifferenceOT>, earnings: E) -> Self {
        Self {
            registry,
            earnings: RefCell::new(earnings),
        }
    }

    /// `runRules(TimeCard, RuleSet defaultOTRuleSet)`. `rule_set_for(job_id,
    /// date)` is `ruleUtils.getRuleSet(employee, job, date, SHIFT_DIFF_OT)`; the
    /// overtime rate rule set, registry and default set arrive in `rates`.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rates: &OvertimeRateLookup<'_>,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        if !time_card.has_shifts_or_earnings() {
            return;
        }
        clear_time_card_earnings(
            time_card,
            &mut *self.earnings.borrow_mut(),
            &[RuleType::ShiftDiffOt],
            rule_type_of_item,
        );

        let date = time_card.calculation_start_date();
        let Some(home_job_id) = time_card
            .employee()
            .and_then(|e| e.home_employee_job_status(date))
            .map(|home| home.job_id())
        else {
            return;
        };
        let Some(rule_set) = rule_set_for(home_job_id, date) else {
            return;
        };
        for item in rule_set.rule_items() {
            if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                rule.execute(time_card, item, rates);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::rule_item::RuleItem;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::algorithm::overtimerate::OvertimeRateRule;
    use crate::rules::rule_class::RuleClass;
    use std::cell::Cell;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }
    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                10,
                today().minus_years(1),
                today().plus_years(1),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }

    #[derive(Default)]
    struct Counter(Cell<u32>);
    impl ShiftDifferenceOT for Counter {
        fn execute(&self, _tc: &mut dyn TimeCard, _item: &RuleItem, _r: &OvertimeRateLookup<'_>) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct Registry(Counter);
    impl RuleRegistry<dyn ShiftDifferenceOT> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::ShiftDiffOt
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ShiftDifferenceOT + 'static)> {
            Some(&self.0)
        }
    }
    struct NoRates;
    impl RuleRegistry<dyn OvertimeRateRule> for NoRates {
        fn rule_type(&self) -> RuleType {
            RuleType::OvertimeRate
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn OvertimeRateRule + 'static)> {
            None
        }
    }
    fn set() -> RuleSet {
        RuleSet::new(
            1,
            1,
            "s",
            RuleType::ShiftDiffOt,
            0,
            vec![RuleItem::new(
                1,
                1,
                "x",
                RuleClass::ShiftDifferenceOtSdo,
                Default::default(),
            )],
        )
    }

    // ---- java_parity_tests: ShiftDifferenceOTRunnerTest.groovy ----

    #[test]
    fn rules_shouldnt_run_if_no_earnings_or_shifts_exist_in_the_time_card() {
        let registry = Registry(Counter::default());
        let default_set = set();
        let none = |_: i32, _: LocalDate| -> Option<&RuleSet> { None };
        let lookup = OvertimeRateLookup {
            registry: &NoRates,
            rule_set_for: &none,
            default_rule_set: &default_set,
        };
        let asked = Cell::new(0);
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_calculation_start_date(today());
        ShiftDifferenceOTRunner::new(&registry, SavedEarnings::default()).run_rules(
            &mut c,
            &lookup,
            |_, _| {
                asked.set(asked.get() + 1);
                None
            },
            &|_| None,
        );
        assert_eq!(asked.get(), 0);
        assert_eq!(registry.0.0.get(), 0);
    }

    #[test]
    fn run_rules_will_run_the_rule_if_there_are_shifts_in_the_time_card() {
        let registry = Registry(Counter::default());
        let default_set = set();
        let none = |_: i32, _: LocalDate| -> Option<&RuleSet> { None };
        let lookup = OvertimeRateLookup {
            registry: &NoRates,
            rule_set_for: &none,
            default_rule_set: &default_set,
        };
        let rule_set = set();
        let shift = EmployeeShift::new(1, 1, 10, today(), ShiftType::Actual, Vec::new());
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift])
            .with_calculation_start_date(today());
        ShiftDifferenceOTRunner::new(&registry, SavedEarnings::default()).run_rules(
            &mut c,
            &lookup,
            |job, date| {
                assert_eq!((job, date), (10, today()));
                Some(&rule_set)
            },
            &|_| None,
        );
        assert_eq!(registry.0.0.get(), 1);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn no_home_job_on_the_calculation_start_date_means_nothing_runs() {
        let registry = Registry(Counter::default());
        let default_set = set();
        let none = |_: i32, _: LocalDate| -> Option<&RuleSet> { None };
        let lookup = OvertimeRateLookup {
            registry: &NoRates,
            rule_set_for: &none,
            default_rule_set: &default_set,
        };
        let rule_set = set();
        let shift = EmployeeShift::new(1, 1, 10, today(), ShiftType::Actual, Vec::new());
        let mut c = TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_shifts(vec![shift])
            .with_calculation_start_date(today());
        ShiftDifferenceOTRunner::new(&registry, SavedEarnings::default()).run_rules(
            &mut c,
            &lookup,
            |_, _| Some(&rule_set),
            &|_| None,
        );
        assert_eq!(registry.0.0.get(), 0);
    }
}
