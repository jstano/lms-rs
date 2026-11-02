//! Port of `UnavailableAllDayRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabel/UnavailableAllDayRuleImpl.java`.
//!
//! Labels a day the employee is **required off all day** (a whole-day
//! `REQUIRED_OFF` availability row for that weekday), provided the day has no
//! label and fewer than `maxLabels` labels with the same text (trimmed,
//! case-insensitive) fall in the work week containing it.

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::config::{
    MAX_LABELS, SCHEDULE_LABEL, UnavailableAllDayRuleConfig,
};
use crate::rules::algorithm::schedulelabel::{
    ScheduleLabelRule, create_and_save_label, matching_labels_count,
};
use crate::rules::ports::{EmployeeScheduleLabelPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `UnavailableAllDayRuleImpl`.
pub struct UnavailableAllDayRule<L: EmployeeScheduleLabelPort, P: PropertyPort> {
    labels: RefCell<L>,
    property: P,
}

impl<L: EmployeeScheduleLabelPort, P: PropertyPort> UnavailableAllDayRule<L, P> {
    /// Build the rule over the label port and the property calendar.
    pub fn new(labels: L, property: P) -> Self {
        Self {
            labels: RefCell::new(labels),
            property,
        }
    }
}

impl<L: EmployeeScheduleLabelPort, P: PropertyPort> ScheduleLabelRule
    for UnavailableAllDayRule<L, P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, rule_item: &RuleItem, date: LocalDate) {
        let params = rule_item
            .params()
            .fixed(&UnavailableAllDayRuleConfig.default_values());
        let Some(employee) = time_card.employee() else {
            return;
        };
        if !employee.is_required_off_for_day(date)
            || time_card.schedule_label_for_date(date).is_some()
        {
            return;
        }
        let text = params.get(SCHEDULE_LABEL).unwrap_or("");
        let count = matching_labels_count(&self.property, time_card, date, text);
        if (count as i64) < i64::from(params.int_at(MAX_LABELS)) {
            create_and_save_label(&self.labels, time_card, rule_item, &params, date);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_avail_type::EmployeeAvailType;
    use crate::common::enums::label_source::LabelSource;
    use crate::entity::employee_availability::EmployeeAvailability;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabel::fixtures::*;
    use crate::rules::params::RuleParams;
    use joda_rs::LocalTime;

    fn employee_with(kind: EmployeeAvailType) -> crate::entity::employee::Employee {
        let midnight = LocalTime::of(0, 0, 0);
        employee().with_availability(vec![EmployeeAvailability::new(
            today().day_of_week().value(),
            midnight,
            midnight,
            kind,
        )])
    }
    fn label(text: &str, day: i64) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(1111, today().plus_days(day), text, LabelSource::Rule)
    }
    fn run(
        kind: EmployeeAvailType,
        labels: Vec<EmployeeScheduleLabel>,
        params: RuleParams,
    ) -> usize {
        let saved = SavedLabels::default();
        let mut c = card(employee_with(kind)).with_schedule_labels(labels);
        UnavailableAllDayRule::new(saved.clone(), WeekFromToday).execute(
            &mut c,
            &item(params),
            today(),
        );
        saved.saved.borrow().len()
    }

    // ---- java_parity_tests: UnavailableAllDayRuleImplTest.groovy ----

    #[test]
    fn rule_should_create_labels_on_an_unavailable_day() {
        let n = run(
            EmployeeAvailType::RequiredOff,
            vec![],
            rule_params! { SCHEDULE_LABEL => "FLEX" },
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn rule_should_not_create_more_labels_than_the_configured_max() {
        let n = run(
            EmployeeAvailType::RequiredOff,
            vec![label("FLEX", 1), label("FLEX", 2)],
            rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "2" },
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn rule_should_not_create_a_label_if_there_is_an_existing_label_on_the_day() {
        let n = run(
            EmployeeAvailType::RequiredOff,
            vec![label("", 0)],
            RuleParams::new(),
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn labels_only_count_toward_the_max_if_their_text_matches() {
        let n = run(
            EmployeeAvailType::RequiredOff,
            vec![label("NOMATCH", 1)],
            rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "1" },
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn labels_are_not_created_on_days_where_the_employee_is_available() {
        let n = run(
            EmployeeAvailType::AvailableToWork,
            vec![],
            rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "1" },
        );
        assert_eq!(n, 0);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn matching_text_is_trimmed_and_case_insensitive() {
        let n = run(
            EmployeeAvailType::RequiredOff,
            vec![label("  flex ", 1)],
            rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "1" },
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn only_a_whole_day_required_off_row_counts() {
        let partial = employee().with_availability(vec![EmployeeAvailability::new(
            today().day_of_week().value(),
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
            EmployeeAvailType::RequiredOff,
        )]);
        let saved = SavedLabels::default();
        let mut c = card(partial);
        UnavailableAllDayRule::new(saved.clone(), WeekFromToday).execute(
            &mut c,
            &item(rule_params! { SCHEDULE_LABEL => "FLEX" }),
            today(),
        );
        assert!(saved.saved.borrow().is_empty());
    }
}
