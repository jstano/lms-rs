//! Port of `BetweenDatesScheduleLabelCountRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabeladdendum/BetweenDatesScheduleLabelCountRuleImpl.java`.
//!
//! Numbers the labels that match `scheduleLabel` (case-insensitive) within each
//! yearly reference range that overlaps the period — "this is the 3rd `VAC` since
//! 15 April". A label counts only if it falls inside some reference range;
//! one in a gap between ranges gets nothing. Within a range the labels are
//! numbered in date order, continuing from the **stored** labels with that text
//! between the range's start and the day before the period starts when the range
//! began before the period.

use crate::entity::employee::Employee;
use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::schedulelabeladdendum::ScheduleLabelAddendumRule;
use crate::rules::algorithm::schedulelabeladdendum::config::{
    BetweenDatesScheduleLabelCountRuleConfig, REFERENCE_DATE_END_PROP, REFERENCE_DATE_START_PROP,
    SCHEDULE_LABEL_PROP, try_parse_date,
};
use crate::rules::algorithm::schedulelabeladdendum::reference_range::reference_ranges_overlapping_period;
use crate::rules::ports::EmployeeScheduleLabelPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// `BetweenDatesScheduleLabelCountRuleImpl`.
pub struct BetweenDatesScheduleLabelCountRule<L: EmployeeScheduleLabelPort> {
    labels: L,
}

impl<L: EmployeeScheduleLabelPort> BetweenDatesScheduleLabelCountRule<L> {
    /// Build the rule over the port that counts stored labels.
    pub fn new(labels: L) -> Self {
        Self { labels }
    }
}

impl<L: EmployeeScheduleLabelPort> ScheduleLabelAddendumRule
    for BetweenDatesScheduleLabelCountRule<L>
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
            .fixed(&BetweenDatesScheduleLabelCountRuleConfig.default_values());
        let (Some(start), Some(end)) = (
            params
                .get(REFERENCE_DATE_START_PROP)
                .and_then(try_parse_date),
            params.get(REFERENCE_DATE_END_PROP).and_then(try_parse_date),
        ) else {
            return Vec::new();
        };
        let to_match = params.get(SCHEDULE_LABEL_PROP).unwrap_or("");

        let ranges = reference_ranges_overlapping_period(period, start, end);

        // Matching labels in date order (stable), each with the first range holding it.
        let mut matching: Vec<usize> = (0..labels.len())
            .filter(|&i| {
                labels[i].label().to_lowercase() == to_match.to_lowercase()
                    && ranges.iter().any(|r| r.contains_date(labels[i].date()))
            })
            .collect();
        matching.sort_by_key(|&i| labels[i].date());

        let mut result = Vec::new();
        for range in &ranges {
            let in_range: Vec<usize> = matching
                .iter()
                .copied()
                .filter(|&i| {
                    ranges.iter().find(|r| r.contains_date(labels[i].date())) == Some(range)
                })
                .collect();
            if in_range.is_empty() {
                continue;
            }
            let mut count = if range.start_date() < period.start_date() {
                self.labels.count_employee_labels_matching_text(
                    employee.id(),
                    to_match,
                    &DateRange::new(range.start_date(), period.start_date().minus_days(1)),
                )
            } else {
                0
            };
            for index in in_range {
                count += 1;
                result.push((index, count.to_string()));
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
    use crate::rules::algorithm::schedulelabeladdendum::fixtures::CountingLabels;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn date() -> LocalDate {
        LocalDate::of(2017, 4, 1)
    }
    fn label(id: i32, d: LocalDate, text: &str) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(102, d, text, LabelSource::Rule).with_id(id)
    }
    fn item(params: crate::rules::params::RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "x", RuleClass::BtwnDtsLblCntSchAddnm, params)
    }

    // ---- java_parity_tests: BetweenDatesScheduleLabelCountRuleImplTest.groovy ----

    #[test]
    fn execute_returns_an_empty_map_if_no_labels_are_passed_in() {
        let rule = BetweenDatesScheduleLabelCountRule::new(CountingLabels::default());
        let employee = Employee::new(0, 1, "x", Vec::new());
        let out = rule.execute(
            &employee,
            &DateRange::new(date(), date()),
            &[],
            &item(Default::default()),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn execute_maps_schedule_labels_to_their_label_count_for_the_period() {
        let params = rule_params! {
            SCHEDULE_LABEL_PROP => "label",
            REFERENCE_DATE_START_PROP => "2017-04-15",
            REFERENCE_DATE_END_PROP => "2018-04-12"
        };
        let employee = Employee::new(102, 1, "x", Vec::new());
        let period = DateRange::new(date(), LocalDate::of(2017, 4, 30));
        let labels = vec![
            label(2, date(), "label"),
            label(3, date().plus_days(1), "label"),
            label(4, date().plus_days(2), "otherLabel"),
            label(5, date().plus_weeks(3), "label"),
            label(6, LocalDate::of(2017, 4, 13), "label"),
        ];
        let stored = CountingLabels {
            count: 10,
            ..Default::default()
        };
        let out = BetweenDatesScheduleLabelCountRule::new(stored.clone()).execute(
            &employee,
            &period,
            &labels,
            &item(params),
        );

        assert_eq!(out.len(), 3);
        let by_id = |id: i32| {
            out.iter()
                .find(|(i, _)| labels[*i].id() == id)
                .map(|(_, t)| t.as_str())
        };
        // After the first reference range ends the count restarts at 1.
        assert_eq!(by_id(5), Some("1"));
        // Within the first range the stored 10 come first.
        assert_eq!(by_id(2), Some("11"));
        assert_eq!(by_id(3), Some("12"));
        // The 13 April label is in the gap between ranges: not counted.
        assert_eq!(by_id(6), None);
        // The stored count was asked once, for the first range up to the day before the period.
        let queries = stored.queries.borrow();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].0, 102);
        assert_eq!(
            queries[0].2,
            DateRange::new(LocalDate::of(2016, 4, 15), LocalDate::of(2017, 3, 31))
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn labels_match_ignoring_case_and_numbering_follows_date_order_not_list_order() {
        let params = rule_params! {
            SCHEDULE_LABEL_PROP => "VAC", REFERENCE_DATE_START_PROP => "2017-04-01",
            REFERENCE_DATE_END_PROP => "2018-03-31"
        };
        let employee = Employee::new(102, 1, "x", Vec::new());
        let period = DateRange::new(date(), LocalDate::of(2017, 4, 30));
        let labels = vec![
            label(1, date().plus_days(5), "vac"),
            label(2, date().plus_days(1), "Vac"),
        ];
        let out = BetweenDatesScheduleLabelCountRule::new(CountingLabels::default()).execute(
            &employee,
            &period,
            &labels,
            &item(params),
        );
        // The range starts on the period's first day, so no stored labels are added.
        let mut sorted = out.clone();
        sorted.sort();
        assert_eq!(sorted, vec![(0, "2".to_string()), (1, "1".to_string())]);
    }
}
