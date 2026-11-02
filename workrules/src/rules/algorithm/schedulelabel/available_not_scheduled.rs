//! Port of `AvailableNotScheduledRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabel/AvailableNotScheduledRuleImpl.java`.
//!
//! Labels a day the employee is **free to be scheduled but is not**. Every one
//! must hold:
//!
//! - no whole-day required-off availability for the weekday;
//! - no **approved** time-off request whose range overlaps the day (the
//!   *exclusive* overlap of `DateTimeRange.allDay(date)`, so a request that merely
//!   touches midnight does not count; a full-day request runs to the end of its
//!   last day);
//! - no scheduled shift (`getSchedules()`, not the actual shifts) dated that day;
//! - no label on the day, and fewer than `maxLabels` labels with the same text
//!   (trimmed, case-insensitive) in the work week.

use crate::common::enums::time_off_status::TimeOffStatus;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::config::{
    AvailableNotScheduledRuleConfig, MAX_LABELS, SCHEDULE_LABEL,
};
use crate::rules::algorithm::schedulelabel::{
    ScheduleLabelRule, create_and_save_label, matching_labels_count,
};
use crate::rules::ports::{EmployeeScheduleLabelPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `AvailableNotScheduledRuleImpl`.
pub struct AvailableNotScheduledRule<L: EmployeeScheduleLabelPort, P: PropertyPort> {
    labels: RefCell<L>,
    property: P,
}

impl<L: EmployeeScheduleLabelPort, P: PropertyPort> AvailableNotScheduledRule<L, P> {
    /// Build the rule over the label port and the property calendar.
    pub fn new(labels: L, property: P) -> Self {
        Self {
            labels: RefCell::new(labels),
            property,
        }
    }
}

impl<L: EmployeeScheduleLabelPort, P: PropertyPort> ScheduleLabelRule
    for AvailableNotScheduledRule<L, P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, rule_item: &RuleItem, date: LocalDate) {
        let params = rule_item
            .params()
            .fixed(&AvailableNotScheduledRuleConfig.default_values());
        let Some(employee) = time_card.employee() else {
            return;
        };

        let day = DateTimeRange::all_day(date);
        let available = !employee.is_required_off_for_day(date)
            && !time_card.time_off_requests().iter().any(|request| {
                request.status() == TimeOffStatus::Approved
                    && day.overlaps_exclusive(&request.to_date_time_range())
            })
            && !time_card
                .schedules()
                .iter()
                .any(|shift| shift.shift_date() == date);
        if !available || time_card.schedule_label_for_date(date).is_some() {
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
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_availability::EmployeeAvailability;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_time_off::EmployeeTimeOff;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabel::fixtures::*;
    use crate::rules::params::RuleParams;
    use joda_rs::{LocalDateTime, LocalTime};

    fn label(text: &str, day: i64) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(1111, today().plus_days(day), text, LabelSource::Rule)
    }
    fn run(c: &mut TimeCardData, params: RuleParams) -> usize {
        let saved = SavedLabels::default();
        AvailableNotScheduledRule::new(saved.clone(), WeekFromToday).execute(
            c,
            &item(params),
            today(),
        );
        saved.saved.borrow().len()
    }
    fn flex() -> RuleParams {
        rule_params! { SCHEDULE_LABEL => "FLEX" }
    }

    // ---- java_parity_tests: AvailableNotScheduledRuleImplTest.groovy ----

    #[test]
    fn rule_should_create_a_label_if_the_employee_is_available_and_under_the_limit() {
        let mut c = card(employee());
        assert_eq!(run(&mut c, flex()), 1);
    }

    #[test]
    fn rule_should_not_create_a_label_if_there_is_an_existing_tor() {
        let start = today().at_start_of_day();
        let end = LocalDateTime::of(2014, 5, 6, 23, 59, 59);
        let mut c = card(employee()).with_time_off_requests(vec![EmployeeTimeOff::new(
            start,
            end,
            TimeOffStatus::Approved,
            0,
        )]);
        assert_eq!(run(&mut c, flex()), 0);
    }

    #[test]
    fn rule_should_not_create_a_label_if_there_is_a_full_day_required_off() {
        let midnight = LocalTime::of(0, 0, 0);
        let off = employee().with_availability(vec![EmployeeAvailability::new(
            today().day_of_week().value(),
            midnight,
            midnight,
            EmployeeAvailType::RequiredOff,
        )]);
        let mut c = card(off);
        assert_eq!(run(&mut c, flex()), 0);
    }

    #[test]
    fn rule_should_not_create_a_label_if_there_is_a_shift_scheduled() {
        let shift = EmployeeShift::new(1, 1111, 1, today(), ShiftType::Schedule, Vec::new());
        let mut c = card(employee()).with_schedules(vec![shift]);
        assert_eq!(run(&mut c, flex()), 0);
    }

    #[test]
    fn rule_should_not_create_a_label_if_there_is_an_existing_label_on_the_day() {
        let mut c = card(employee()).with_schedule_labels(vec![label("", 0)]);
        assert_eq!(run(&mut c, flex()), 0);
    }

    #[test]
    fn rule_should_not_create_a_label_if_the_employee_is_over_the_max_label_limit() {
        let mut c = card(employee()).with_schedule_labels(vec![label("FLEX", 0), label("FLEX", 2)]);
        // The first shares the day; use a different day for the pair that counts.
        let mut c2 =
            card(employee()).with_schedule_labels(vec![label("FLEX", 1), label("FLEX", 2)]);
        let params = rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "2" };
        assert_eq!(run(&mut c, params.clone()), 0);
        assert_eq!(run(&mut c2, params), 0);
    }

    #[test]
    fn max_label_limit_only_counts_labels_with_matching_text() {
        let mut c = card(employee()).with_schedule_labels(vec![label("NOMATCH", 2)]);
        assert_eq!(
            run(
                &mut c,
                rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "1" }
            ),
            1
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_pending_request_does_not_make_the_employee_unavailable() {
        let start = today().at_start_of_day();
        let mut c = card(employee()).with_time_off_requests(vec![EmployeeTimeOff::new(
            start,
            start.plus_hours(8),
            TimeOffStatus::Pending,
            0,
        )]);
        assert_eq!(run(&mut c, flex()), 1);
    }

    #[test]
    fn a_request_that_only_touches_the_day_boundary_does_not_count() {
        // Ends exactly at midnight when the day starts: exclusive overlap is false.
        let end = today().at_start_of_day();
        let mut c = card(employee()).with_time_off_requests(vec![EmployeeTimeOff::new(
            end.minus_days(1),
            end,
            TimeOffStatus::Approved,
            0,
        )]);
        assert_eq!(run(&mut c, flex()), 1);
    }

    #[test]
    fn a_full_day_request_covers_its_whole_last_day() {
        let start = today().at_start_of_day();
        let mut c = card(employee()).with_time_off_requests(vec![
            EmployeeTimeOff::new(start, start, TimeOffStatus::Approved, 0).with_full_day(true),
        ]);
        assert_eq!(run(&mut c, flex()), 0);
    }
}
