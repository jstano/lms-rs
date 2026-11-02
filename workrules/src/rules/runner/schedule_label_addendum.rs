//! Port of `ScheduleLabelAddendumRunner` and `ScheduleLabelAddendumExtraDataRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ScheduleLabelAddendumRunner.java`
//! and `…/ScheduleLabelAddendumExtraDataRunner.java`.
//!
//! The first finds the employee's **last home job** in the period, resolves that
//! job's `SCHEDULE_LABEL_ADDENDUM` rule set as of the earlier of the job's end
//! and the period's end, and runs **every** rule item (no priority sort, unlike
//! its sibling label runner) over the labels, merging what they return. The
//! second runs the first for one employee and writes each result into its
//! label's `extraData`, then saves the labelled ones.
//!
//! A rule returns `(label index, text)` pairs (see the family doc). When two rule
//! items label the same label, Java's `toMap` throws `IllegalStateException`; the
//! port keeps the **later** one (divergence 120). An employee with no home job in
//! the period (Java: `NullPointerException`) yields nothing.

use crate::entity::employee::Employee;
use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
use crate::entity::rule_set::RuleSet;
use crate::rules::algorithm::schedulelabeladdendum::ScheduleLabelAddendumRule;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::EmployeeScheduleLabelPort;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// Runs the label-addendum rules for one employee's labels.
pub struct ScheduleLabelAddendumRunner<'r> {
    registry: &'r dyn RuleRegistry<dyn ScheduleLabelAddendumRule>,
}

impl<'r> ScheduleLabelAddendumRunner<'r> {
    /// Build the runner over the family's dispatch.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ScheduleLabelAddendumRule>) -> Self {
        Self { registry }
    }

    /// `runRules(Employee, DateRange, List<EmployeeScheduleLabel>)`.
    /// `rule_set_for(job_id, date)` is `ruleUtils.getRuleSet(employee, job, date, SCHEDULE_LABEL_ADDENDUM)`.
    pub fn run_rules<'a>(
        &self,
        employee: &Employee,
        period: &DateRange,
        labels: &[EmployeeScheduleLabel],
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) -> Vec<(usize, String)> {
        let Some(job_status) = employee.last_home_employee_job_status_for_period(period) else {
            return Vec::new();
        };
        let rule_set_date = if job_status.end_date() < period.end_date() {
            job_status.end_date()
        } else {
            period.end_date()
        };
        let Some(rule_set) = rule_set_for(job_status.job_id(), rule_set_date) else {
            return Vec::new();
        };

        let mut merged: Vec<(usize, String)> = Vec::new();
        for item in rule_set.rule_items() {
            let Some(rule) = self.registry.rule_for(item.rule_class()) else {
                continue;
            };
            for (index, text) in rule.execute(employee, period, labels, item) {
                match merged.iter_mut().find(|(i, _)| *i == index) {
                    Some(existing) => existing.1 = text,
                    None => merged.push((index, text)),
                }
            }
        }
        merged
    }

    /// `runRules(DateRange, Map<Integer, List<EmployeeScheduleLabel>>)`: several
    /// employees at once. `employee_for(id)` is `employeeDAO.findByID`. Returns
    /// `(employee id, label index within that employee's list, text)`.
    pub fn run_rules_for_employees<'a>(
        &self,
        period: &DateRange,
        employee_labels: &[(i32, Vec<EmployeeScheduleLabel>)],
        employee_for: impl Fn(i32) -> Option<Employee>,
        rule_set_for: impl Fn(&Employee, i32, LocalDate) -> Option<&'a RuleSet>,
    ) -> Vec<(i32, usize, String)> {
        let mut all = Vec::new();
        for (employee_id, labels) in employee_labels {
            let Some(employee) = employee_for(*employee_id) else {
                continue;
            };
            let results = self.run_rules(&employee, period, labels, |job, date| {
                rule_set_for(&employee, job, date)
            });
            all.extend(results.into_iter().map(|(i, t)| (*employee_id, i, t)));
        }
        all
    }
}

/// Runs the addendum rules and stores the results in each label's `extraData`.
pub struct ScheduleLabelAddendumExtraDataRunner<L: EmployeeScheduleLabelPort> {
    labels: L,
}

impl<L: EmployeeScheduleLabelPort> ScheduleLabelAddendumExtraDataRunner<L> {
    /// Build the runner over the port that saves labels.
    pub fn new(labels: L) -> Self {
        Self { labels }
    }

    /// `runRules(Employee, DateRange, List<EmployeeScheduleLabel>)`.
    pub fn run_rules<'a>(
        &mut self,
        addendum_runner: &ScheduleLabelAddendumRunner<'_>,
        employee: &Employee,
        period: &DateRange,
        labels: &mut [EmployeeScheduleLabel],
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) {
        let results = addendum_runner.run_rules(employee, period, labels, rule_set_for);
        for (index, text) in &results {
            labels[*index].set_extra_data(text.clone());
        }
        let mut indices: Vec<usize> = results.iter().map(|(i, _)| *i).collect();
        indices.sort_unstable();
        let changed: Vec<EmployeeScheduleLabel> =
            indices.iter().map(|&i| labels[i].clone()).collect();
        self.labels.save_all(&changed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::label_source::LabelSource;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::rule_item::RuleItem;
    use crate::rules::algorithm::schedulelabel::fixtures::SavedLabels;
    use crate::rules::rule_class::RuleClass;
    use crate::rules::rule_type::RuleType;
    use std::cell::RefCell;

    fn today() -> LocalDate {
        LocalDate::of(2017, 4, 1)
    }
    fn employee(id: i32) -> Employee {
        Employee::new(
            id,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                id,
                10,
                today().minus_years(1),
                today().plus_years(1),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }
    fn label(id: i32, text: &str) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(1, today(), text, LabelSource::Rule).with_id(id)
    }

    /// Answers a fixed list of `(index, text)` and records the dates asked.
    struct Fixed(Vec<(usize, String)>);
    impl ScheduleLabelAddendumRule for Fixed {
        fn execute(
            &self,
            _e: &Employee,
            _p: &DateRange,
            _l: &[EmployeeScheduleLabel],
            _r: &RuleItem,
        ) -> Vec<(usize, String)> {
            self.0.clone()
        }
    }
    struct Registry(Fixed);
    impl RuleRegistry<dyn ScheduleLabelAddendumRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::ScheduleLabelAddendum
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ScheduleLabelAddendumRule + 'static)> {
            Some(&self.0)
        }
    }
    fn set() -> RuleSet {
        RuleSet::new(
            1,
            1,
            "s",
            RuleType::ScheduleLabelAddendum,
            0,
            vec![RuleItem::new(
                1,
                1,
                "a",
                RuleClass::BtwnDtsLblCntSchAddnm,
                Default::default(),
            )],
        )
    }

    // ---- java_parity_tests: ScheduleLabelAddendumRunnerTest.groovy ----

    #[test]
    fn run_rules_returns_an_empty_map_if_a_proper_rule_set_cannot_be_found() {
        let registry = Registry(Fixed(vec![(0, "1".into())]));
        let period = DateRange::new(today(), today().plus_days(6));
        let out = ScheduleLabelAddendumRunner::new(&registry).run_rules(
            &employee(1),
            &period,
            &[],
            |_, _| None,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn run_rules_returns_a_map_of_labels_to_addendum_text_for_each_rule_run() {
        let registry = Registry(Fixed(vec![(0, "1".into())]));
        let period = DateRange::new(today(), today().plus_days(6));
        let rule_set = set();
        let asked = RefCell::new(Vec::new());
        let out = ScheduleLabelAddendumRunner::new(&registry).run_rules(
            &employee(103),
            &period,
            &[label(1, "x")],
            |job, date| {
                asked.borrow_mut().push((job, date));
                Some(&rule_set)
            },
        );
        assert!(!out.is_empty());
        // The rule set is resolved for the home job, as of the period's end.
        assert_eq!(*asked.borrow(), vec![(10, period.end_date())]);
    }

    // ---- java_parity_tests: ScheduleLabelAddendumExtraDataRunnerTest.groovy ----

    #[test]
    fn runner_calls_the_addendum_runner_then_saves_the_results_into_extra_data() {
        let registry = Registry(Fixed(vec![(0, "1".into()), (2, "33".into())]));
        let addendum = ScheduleLabelAddendumRunner::new(&registry);
        let rule_set = set();
        let mut labels = vec![label(1, "One"), label(2, "Two"), label(3, "Three")];
        let saved = SavedLabels::default();
        let period = DateRange::new(today(), today());
        ScheduleLabelAddendumExtraDataRunner::new(saved.clone()).run_rules(
            &addendum,
            &employee(1),
            &period,
            &mut labels,
            |_, _| Some(&rule_set),
        );
        assert_eq!(labels[0].extra_data(), Some("1"));
        assert_eq!(labels[1].extra_data(), None);
        assert_eq!(labels[2].extra_data(), Some("33"));
        let ids: Vec<i32> = saved.saved.borrow().iter().map(|l| l.id()).collect();
        assert_eq!(ids, vec![1, 3]);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_rule_set_date_is_the_job_end_when_it_ends_before_the_period() {
        let registry = Registry(Fixed(Vec::new()));
        let ended = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                10,
                today().minus_years(1),
                today().plus_days(2),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        );
        let rule_set = set();
        let asked = RefCell::new(Vec::new());
        let period = DateRange::new(today(), today().plus_days(6));
        ScheduleLabelAddendumRunner::new(&registry).run_rules(&ended, &period, &[], |_, d| {
            asked.borrow_mut().push(d);
            Some(&rule_set)
        });
        assert_eq!(*asked.borrow(), vec![today().plus_days(2)]);
    }

    #[test]
    fn a_label_returned_by_two_rules_keeps_the_later_text() {
        let registry = Registry(Fixed(vec![(0, "first".into())]));
        let two_items = RuleSet::new(
            1,
            1,
            "s",
            RuleType::ScheduleLabelAddendum,
            0,
            vec![
                RuleItem::new(
                    1,
                    1,
                    "a",
                    RuleClass::BtwnDtsLblCntSchAddnm,
                    Default::default(),
                ),
                RuleItem::new(
                    2,
                    1,
                    "b",
                    RuleClass::BtwnDtsLblCntSchAddnm,
                    Default::default(),
                ),
            ],
        );
        let period = DateRange::new(today(), today());
        let out = ScheduleLabelAddendumRunner::new(&registry).run_rules(
            &employee(1),
            &period,
            &[label(1, "x")],
            |_, _| Some(&two_items),
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn several_employees_are_run_and_tagged_by_employee() {
        let registry = Registry(Fixed(vec![(0, "1".into())]));
        let rule_set = set();
        let period = DateRange::new(today(), today());
        let input = vec![(1, vec![label(1, "x")]), (2, vec![label(2, "y")])];
        let out = ScheduleLabelAddendumRunner::new(&registry).run_rules_for_employees(
            &period,
            &input,
            |id| Some(employee(id)),
            |_, _, _| Some(&rule_set),
        );
        assert_eq!(out, vec![(1, 0, "1".to_string()), (2, 0, "1".to_string())]);
    }
}
