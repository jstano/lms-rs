//! Port of `ScheduleLabelRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ScheduleLabelRunner.java`.
//!
//! Drives the `schedulelabel` family over a schedule calculation, one day at a
//! time:
//!
//! 1. **Clear.** Remove the card's rule-sourced labels and tell the label port to
//!    delete the rule-generated labels it holds for the employee over the
//!    calculation's date range.
//! 2. For each date in the range on which the employee has a **home job** and is
//!    **active**: resolve the `SCHEDULE_LABEL` rule set for the home job and date,
//!    sort its items by priority and run each.
//!
//! # The sort is the config's, not the raw parameter
//!
//! Java's comparator parses `params.get("priority")` directly with
//! `Integer.valueOf`, so a rule item with no `priority` parameter throws
//! `NumberFormatException`. Here the priority comes from the config with defaults
//! filled (`LabelRuleConfig`'s default is 1), so a missing one ranks 1 instead of
//! failing (divergence 118). A given priority ranks identically.

use crate::common::enums::label_source::LabelSource;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::ScheduleLabelRule;
use crate::rules::algorithm::schedulelabel::config::{
    ApprovedTimeOffLabelRuleConfig, AvailableNotScheduledRuleConfig, NoLabelRuleConfig,
    NotScheduledOrOffRuleConfig, UnavailableAllDayRuleConfig,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::EmployeeScheduleLabelPort;
use crate::rules::priority::sort_by_priority;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// Runs the schedule-label family over a schedule calculation.
pub struct ScheduleLabelRunner<'r, L: EmployeeScheduleLabelPort> {
    registry: &'r dyn RuleRegistry<dyn ScheduleLabelRule>,
    labels: RefCell<L>,
}

impl<'r, L: EmployeeScheduleLabelPort> ScheduleLabelRunner<'r, L> {
    /// Build the runner over the family's dispatch and the label port.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ScheduleLabelRule>, labels: L) -> Self {
        Self {
            registry,
            labels: RefCell::new(labels),
        }
    }

    /// `runRules(ScheduleCalcDataSet)`. `rule_set_for(job_id, date)` is
    /// `ruleUtils.getRuleSet(employee, job, date, SCHEDULE_LABEL)`.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) {
        let Some(employee_id) = time_card.employee().map(|e| e.id()) else {
            return;
        };
        let range = *time_card.date_range();

        // clearLabels
        time_card
            .schedule_labels_mut()
            .retain(|label| label.source() != LabelSource::Rule);
        self.labels
            .borrow_mut()
            .remove_rule_generated_labels_for_period(employee_id, &range);

        // findAndRunRuleSet
        for date in range.dates() {
            let Some(home_job_id) = time_card.employee().and_then(|employee| {
                employee
                    .home_employee_job_status(date)
                    .filter(|_| employee.is_active_on_date(date))
                    .map(|home| home.job_id())
            }) else {
                continue;
            };
            let Some(rule_set) = rule_set_for(home_job_id, date) else {
                continue;
            };

            let mut items: Vec<&RuleItem> = rule_set.rule_items().iter().collect();
            sort_by_priority(&mut items, |item| config_for(item.rule_class()));
            for item in items {
                if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                    rule.execute(time_card, item, date);
                }
            }
        }
    }
}

fn config_for(rule_class: RuleClass) -> &'static dyn RuleConfig {
    match rule_class {
        RuleClass::UnavailLb => &UnavailableAllDayRuleConfig,
        RuleClass::AvailNotSchedLb => &AvailableNotScheduledRuleConfig,
        RuleClass::NotSchedLb => &NotScheduledOrOffRuleConfig,
        RuleClass::ApprovedTimeOffLb => &ApprovedTimeOffLabelRuleConfig,
        _ => &NoLabelRuleConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabel::fixtures::SavedLabels;
    use crate::rules::rule_config::PRIORITY;
    use crate::rules::rule_type::RuleType;
    use date_range_rs::DateRange;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }

    /// Records `(rule item id, date)` for every execution.
    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(RuleClass, i32, LocalDate)>>,
    }
    impl ScheduleLabelRule for Recorder {
        fn execute(&self, _tc: &mut dyn TimeCard, item: &RuleItem, date: LocalDate) {
            self.calls
                .borrow_mut()
                .push((item.rule_class(), item.id(), date));
        }
    }
    struct Registry(Recorder);
    impl RuleRegistry<dyn ScheduleLabelRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::ScheduleLabel
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ScheduleLabelRule + 'static)> {
            Some(&self.0)
        }
    }

    fn active() -> Vec<EmployeeStatus> {
        vec![EmployeeStatus::new(
            today().minus_years(1),
            today().plus_years(1),
            EmployeeStatusType::Active,
        )]
    }
    fn home_job(start: LocalDate, end: LocalDate) -> EmployeeJobStatus {
        EmployeeJobStatus::new(1, 1, 1, start, end, EmployeePayType::Hourly, 0.0, true)
    }

    // ---- java_parity_tests: ScheduleLabelRunnerTest.groovy ----

    #[test]
    fn labels_are_cleared_prior_to_rules_running() {
        let registry = Registry(Recorder::default());
        let labels = SavedLabels::default();
        let range = DateRange::new(today(), today().plus_days(6));
        let mut c = TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()).with_statuses(active()))
            .with_date_range(range)
            .with_schedule_labels(vec![
                EmployeeScheduleLabel::new(1, today(), "R", LabelSource::Rule),
                EmployeeScheduleLabel::new(1, today(), "M", LabelSource::Manual),
            ]);
        ScheduleLabelRunner::new(&registry, labels.clone()).run_rules(&mut c, |_, _| None);
        assert_eq!(*labels.removed.borrow(), vec![(1, range)]);
        // Rule-made labels leave the card; others stay.
        assert_eq!(c.schedule_labels().len(), 1);
        assert_eq!(c.schedule_labels()[0].source(), LabelSource::Manual);
    }

    #[test]
    fn label_rules_are_not_run_if_the_employee_is_not_active_on_date() {
        let registry = Registry(Recorder::default());
        let set = RuleSet::new(
            1,
            1,
            "s",
            RuleType::ScheduleLabel,
            0,
            vec![RuleItem::new(
                1,
                1,
                "a",
                RuleClass::NoLabelLb,
                Default::default(),
            )],
        );
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![home_job(today().minus_weeks(1), today().minus_days(1))],
        );
        let mut c = TimeCardData::new()
            .with_employee(employee)
            .with_date_range(DateRange::new(today(), today().plus_days(6)));
        let asked = RefCell::new(0);
        ScheduleLabelRunner::new(&registry, SavedLabels::default()).run_rules(&mut c, |_, _| {
            *asked.borrow_mut() += 1;
            Some(&set)
        });
        assert_eq!(*asked.borrow(), 0);
        assert!(registry.0.calls.borrow().is_empty());
    }

    #[test]
    fn the_runner_correctly_runs_all_configured_label_rules_in_the_correct_order() {
        let registry = Registry(Recorder::default());
        // Listed in the "wrong" order to show the priority sort.
        let low = RuleItem::new(
            2,
            1,
            "b",
            RuleClass::NoLabelLb,
            rule_params! { PRIORITY => "2" },
        );
        let high = RuleItem::new(
            1,
            1,
            "a",
            RuleClass::UnavailLb,
            rule_params! { PRIORITY => "1" },
        );
        let set = RuleSet::new(1, 1, "s", RuleType::ScheduleLabel, 0, vec![low, high]);
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![home_job(today().minus_days(10), today().plus_years(1))],
        )
        .with_statuses(active());
        let mut c = TimeCardData::new()
            .with_employee(employee)
            .with_date_range(DateRange::new(today(), today()));
        ScheduleLabelRunner::new(&registry, SavedLabels::default())
            .run_rules(&mut c, |job, _| (job == 1).then_some(&set));
        assert_eq!(
            *registry.0.calls.borrow(),
            vec![
                (RuleClass::UnavailLb, 1, today()),
                (RuleClass::NoLabelLb, 2, today())
            ]
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_rule_item_without_a_priority_ranks_one_instead_of_throwing() {
        let registry = Registry(Recorder::default());
        let unset = RuleItem::new(1, 1, "a", RuleClass::UnavailLb, Default::default());
        let two = RuleItem::new(
            2,
            1,
            "b",
            RuleClass::NoLabelLb,
            rule_params! { PRIORITY => "2" },
        );
        let set = RuleSet::new(1, 1, "s", RuleType::ScheduleLabel, 0, vec![two, unset]);
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![home_job(today().minus_days(10), today().plus_years(1))],
        )
        .with_statuses(active());
        let mut c = TimeCardData::new()
            .with_employee(employee)
            .with_date_range(DateRange::new(today(), today()));
        ScheduleLabelRunner::new(&registry, SavedLabels::default())
            .run_rules(&mut c, |_, _| Some(&set));
        let ids: Vec<i32> = registry.0.calls.borrow().iter().map(|c| c.1).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
