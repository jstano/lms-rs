//! Port of `AccrualBalanceLabelAddendumRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabeladdendum/AccrualBalanceLabelAddendumRuleImpl.java`.
//!
//! Numbers the matching labels by the employee's **accrual balance**: "VAC 11,
//! VAC 10 …" counting down the days they have left, or up the days used, from the
//! balance on the first date on or after the period's end.
//!
//! - The balance is truncated to a whole number. When it is struck **after** the
//!   period's end, the stored labels with this text between the day after the
//!   period and the balance date are added to it (they are in the future of the
//!   period and not yet reflected).
//! - The labels are taken **newest first**; each one whose text equals
//!   `scheduleLabel` ignoring case (but **not** trimmed) gets the running count,
//!   then the count rises by one — so the latest label shows the base number and
//!   earlier ones higher.
//! - No labels, or no balance on or after the end date, returns nothing.

use crate::entity::employee::Employee;
use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::schedulelabel::config::SCHEDULE_LABEL;
use crate::rules::algorithm::schedulelabeladdendum::ScheduleLabelAddendumRule;
use crate::rules::algorithm::schedulelabeladdendum::config::{
    ACCRUAL_PROP, AccrualBalanceLabelAddendumRuleConfig,
};
use crate::rules::ports::{AccrualBalancePort, EmployeeScheduleLabelPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// `AccrualBalanceLabelAddendumRuleImpl`.
pub struct AccrualBalanceLabelAddendumRule<L: EmployeeScheduleLabelPort, A: AccrualBalancePort> {
    labels: L,
    accruals: A,
}

impl<L: EmployeeScheduleLabelPort, A: AccrualBalancePort> AccrualBalanceLabelAddendumRule<L, A> {
    /// Build the rule over the label counter and the benefit query.
    pub fn new(labels: L, accruals: A) -> Self {
        Self { labels, accruals }
    }
}

impl<L: EmployeeScheduleLabelPort, A: AccrualBalancePort> ScheduleLabelAddendumRule
    for AccrualBalanceLabelAddendumRule<L, A>
{
    fn execute(
        &self,
        employee: &Employee,
        period: &DateRange,
        labels: &[EmployeeScheduleLabel],
        rule_item: &RuleItem,
    ) -> Vec<(usize, String)> {
        if labels.is_empty() {
            return Vec::new();
        }
        let params = rule_item
            .params()
            .fixed(&AccrualBalanceLabelAddendumRuleConfig.default_values());
        let text = params.get(SCHEDULE_LABEL).unwrap_or("");

        let Some(balance) = self.accruals.earliest_accrual_balance_on_or_after(
            employee.id(),
            params.int_at(ACCRUAL_PROP),
            period.end_date(),
        ) else {
            return Vec::new();
        };

        // `((Double)…).intValue()` truncates toward zero.
        let mut count = balance.ending_total_balance as i32;
        if period.end_date() < balance.as_of_date {
            let to_load = DateRange::new(period.end_date().plus_days(1), balance.as_of_date);
            count += self
                .labels
                .count_employee_labels_matching_text(employee.id(), text, &to_load);
        }

        // Newest first; equal dates keep list order (a stable sort, as Java's).
        let mut order: Vec<usize> = (0..labels.len()).collect();
        order.sort_by(|&a, &b| labels[b].date().cmp(&labels[a].date()));

        let mut result = Vec::new();
        for index in order {
            if labels[index].label().to_lowercase() == text.to_lowercase() {
                result.push((index, count.to_string()));
                count += 1;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::label_source::LabelSource;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabeladdendum::fixtures::{CountingLabels, FixedBalance};
    use crate::rules::ports::AccrualEndingBalance;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn today() -> LocalDate {
        LocalDate::of(2017, 4, 1)
    }
    fn label(text: &str, day: i64) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(0, today().plus_days(day), text, LabelSource::Rule)
    }
    fn item(params: crate::rules::params::RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "x", RuleClass::AccrualBalanceSchAddnm, params)
    }
    fn employee() -> Employee {
        Employee::new(0, 1, "x", Vec::new())
    }
    fn period() -> DateRange {
        DateRange::new(today(), today().plus_days(7))
    }
    fn balance(total: f64, as_of_days: i64) -> FixedBalance {
        FixedBalance(Some(AccrualEndingBalance {
            ending_total_balance: total,
            as_of_date: today().plus_days(as_of_days),
        }))
    }
    fn three() -> Vec<EmployeeScheduleLabel> {
        vec![label("label", 1), label("label", 2), label("no match", 4)]
    }

    // ---- java_parity_tests: AccrualBalanceLabelAddendumRuleImplTest.groovy ----

    #[test]
    fn rule_returns_empty_map_if_there_are_no_labels() {
        let rule =
            AccrualBalanceLabelAddendumRule::new(CountingLabels::default(), balance(10.0, 7));
        assert!(
            rule.execute(&employee(), &period(), &[], &item(Default::default()))
                .is_empty()
        );
    }

    #[test]
    fn rule_returns_empty_map_if_there_is_no_matching_accrual_balance() {
        let rule =
            AccrualBalanceLabelAddendumRule::new(CountingLabels::default(), FixedBalance(None));
        let labels = vec![EmployeeScheduleLabel::new(
            0,
            today(),
            "",
            LabelSource::Rule,
        )];
        assert!(
            rule.execute(&employee(), &period(), &labels, &item(Default::default()))
                .is_empty()
        );
    }

    #[test]
    fn rule_returns_a_map_of_label_to_addendum_number_if_there_is_a_match() {
        let rule =
            AccrualBalanceLabelAddendumRule::new(CountingLabels::default(), balance(10.0, 7));
        let labels = three();
        let out = rule.execute(
            &employee(),
            &period(),
            &labels,
            &item(rule_params! { SCHEDULE_LABEL => "label" }),
        );
        assert_eq!(out.len(), 2);
        assert!(out.contains(&(0, "11".to_string())));
        assert!(out.contains(&(1, "10".to_string())));
    }

    #[test]
    fn rule_queries_to_find_matching_labels_and_updates_the_count_for_missing_data() {
        let stored = CountingLabels {
            count: 5,
            ..Default::default()
        };
        let rule = AccrualBalanceLabelAddendumRule::new(stored.clone(), balance(10.0, 13));
        let labels = three();
        let out = rule.execute(
            &employee(),
            &period(),
            &labels,
            &item(rule_params! { SCHEDULE_LABEL => "label" }),
        );
        assert_eq!(out.len(), 2);
        assert!(out.contains(&(0, "16".to_string())));
        assert!(out.contains(&(1, "15".to_string())));
        let queries = stored.queries.borrow();
        assert_eq!(queries[0].1, "label");
        assert_eq!(
            queries[0].2,
            DateRange::new(today().plus_days(8), today().plus_days(13))
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_balance_is_truncated_and_text_is_matched_ignoring_case_but_not_whitespace() {
        let rule =
            AccrualBalanceLabelAddendumRule::new(CountingLabels::default(), balance(9.99, 7));
        let labels = vec![label("LABEL", 1), label("label ", 2)];
        let out = rule.execute(
            &employee(),
            &period(),
            &labels,
            &item(rule_params! { SCHEDULE_LABEL => "label" }),
        );
        assert_eq!(out, vec![(0, "9".to_string())]);
    }
}
