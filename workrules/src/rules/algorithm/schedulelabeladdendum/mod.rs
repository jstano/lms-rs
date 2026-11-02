//! `RuleType::ScheduleLabelAddendum` — three rules that attach a number or note
//! ("3rd vacation day this year") to labels already on a schedule.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabeladdendum/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelabeladdendum/`,
//! runners `ScheduleLabelAddendumRunner` and `ScheduleLabelAddendumExtraDataRunner`.
//!
//! Unlike every other family these rules **return** their result: a map from
//! label to the addendum text, for the caller to display or store. Java keys the
//! map by the `EmployeeScheduleLabel` object; here a rule returns the **index**
//! of the label in the slice it was given, which names the same object without
//! needing label identity (the specs build labels with no id).
//!
//! # `AccrualBalanceLabelAddendumRuleImpl` sorts the caller's list
//!
//! `labels.sort(...)` reorders the list it was handed, newest first. The port
//! sorts an index list instead and leaves the caller's slice alone (divergence
//! 119); nothing downstream relies on the side effect.

pub mod accrual_balance;
pub mod between_dates;
pub mod config;
pub mod reference_range;

use crate::entity::employee::Employee;
use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
use crate::entity::rule_item::RuleItem;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{AccrualBalancePort, EmployeeScheduleLabelPort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use accrual_balance::AccrualBalanceLabelAddendumRule;
use between_dates::BetweenDatesScheduleLabelCountRule;
use date_range_rs::DateRange;

/// `ScheduleLabelAddendumRule`.
pub trait ScheduleLabelAddendumRule {
    /// `execute(employee, period, labels, ruleItem)`: `(label index, text)` pairs.
    fn execute(
        &self,
        employee: &Employee,
        period: &DateRange,
        labels: &[EmployeeScheduleLabel],
        rule_item: &RuleItem,
    ) -> Vec<(usize, String)>;
}

/// `NoOpScheduleLabelAddendumRuleImpl`.
pub struct NoOpScheduleLabelAddendumRule;

impl ScheduleLabelAddendumRule for NoOpScheduleLabelAddendumRule {
    fn execute(
        &self,
        _employee: &Employee,
        _period: &DateRange,
        _labels: &[EmployeeScheduleLabel],
        _rule_item: &RuleItem,
    ) -> Vec<(usize, String)> {
        Vec::new()
    }
}

/// Dispatch for the three addendum rules. `RuleImplFactory`, narrowed to this family.
pub struct ScheduleLabelAddendumRegistry<L: EmployeeScheduleLabelPort, A: AccrualBalancePort> {
    no_op: NoOpScheduleLabelAddendumRule,
    between_dates: BetweenDatesScheduleLabelCountRule<L>,
    accrual_balance: AccrualBalanceLabelAddendumRule<L, A>,
}

impl<L: EmployeeScheduleLabelPort + Clone, A: AccrualBalancePort>
    ScheduleLabelAddendumRegistry<L, A>
{
    /// Build every rule over the shared ports.
    pub fn new(labels: L, accruals: A) -> Self {
        Self {
            no_op: NoOpScheduleLabelAddendumRule,
            between_dates: BetweenDatesScheduleLabelCountRule::new(labels.clone()),
            accrual_balance: AccrualBalanceLabelAddendumRule::new(labels, accruals),
        }
    }
}

impl<L, A> RuleRegistry<dyn ScheduleLabelAddendumRule> for ScheduleLabelAddendumRegistry<L, A>
where
    L: EmployeeScheduleLabelPort + 'static,
    A: AccrualBalancePort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ScheduleLabelAddendum
    }

    fn rule_for(
        &self,
        rule_class: RuleClass,
    ) -> Option<&(dyn ScheduleLabelAddendumRule + 'static)> {
        match rule_class {
            RuleClass::NoOpSchAddnm => Some(&self.no_op),
            RuleClass::BtwnDtsLblCntSchAddnm => Some(&self.between_dates),
            RuleClass::AccrualBalanceSchAddnm => Some(&self.accrual_balance),
            _ => None,
        }
    }
}

/// Shared test scaffolding.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::rules::algorithm::schedulelabel::fixtures::SavedLabels;
    use crate::rules::ports::AccrualEndingBalance;
    use joda_rs::LocalDate;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Counts stored labels by answering a fixed number and recording each query.
    #[derive(Clone, Default)]
    pub struct CountingLabels {
        pub count: i32,
        pub queries: Rc<RefCell<Vec<(i32, String, DateRange)>>>,
        pub saved: SavedLabels,
    }
    impl EmployeeScheduleLabelPort for CountingLabels {
        fn save(&mut self, label: &EmployeeScheduleLabel) {
            self.saved.save(label);
        }
        fn remove_rule_generated_labels_for_period(&mut self, e: i32, p: &DateRange) {
            self.saved.remove_rule_generated_labels_for_period(e, p);
        }
        fn count_employee_labels_matching_text(&self, e: i32, t: &str, p: &DateRange) -> i32 {
            self.queries.borrow_mut().push((e, t.to_string(), *p));
            self.count
        }
        fn save_all(&mut self, labels: &[EmployeeScheduleLabel]) {
            self.saved.save_all(labels);
        }
    }

    /// An accrual balance answered whatever the query.
    pub struct FixedBalance(pub Option<AccrualEndingBalance>);
    impl AccrualBalancePort for FixedBalance {
        fn earliest_accrual_balance_on_or_after(
            &self,
            _e: i32,
            _a: i32,
            _d: LocalDate,
        ) -> Option<AccrualEndingBalance> {
            self.0
        }
    }

    // ---- java_parity_tests: NoOpScheduleLabelAddendumRuleImplTest.groovy ----

    #[test]
    fn the_no_op_rule_returns_an_empty_map() {
        let employee = Employee::new(0, 1, "x", Vec::new());
        let period = DateRange::new(LocalDate::of(2017, 4, 1), LocalDate::of(2017, 4, 7));
        let rule_item = RuleItem::new(1, 1, "n", RuleClass::NoOpSchAddnm, Default::default());
        assert!(
            NoOpScheduleLabelAddendumRule
                .execute(&employee, &period, &[], &rule_item)
                .is_empty()
        );
    }

    #[test]
    fn the_registry_dispatches_every_addendum_rule_class() {
        let registry =
            ScheduleLabelAddendumRegistry::new(CountingLabels::default(), FixedBalance(None));
        assert_eq!(registry.dispatchable().len(), 3);
        assert!(registry.missing().is_empty());
    }
}
