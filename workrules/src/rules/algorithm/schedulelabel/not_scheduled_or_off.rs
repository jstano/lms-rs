//! Port of `NotScheduledOrOffRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabel/NotScheduledOrOffRuleImpl.java`.
//!
//! Labels a day on which the employee has **no shift**, no label yet, and no
//! approved time-off request (of a configured type) covering the date.
//! Unlike `AvailableNotScheduled` it looks at the actual shifts
//! (`getShiftsForDay`), ignores availability, and has no maximum.

use crate::common::enums::time_off_status::TimeOffStatus;
use crate::common::json_ids::ids_for_key;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::config::{NotScheduledOrOffRuleConfig, TIME_OFF_TYPES};
use crate::rules::algorithm::schedulelabel::{ScheduleLabelRule, create_and_save_label};
use crate::rules::ports::EmployeeScheduleLabelPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `NotScheduledOrOffRuleImpl`.
pub struct NotScheduledOrOffRule<L: EmployeeScheduleLabelPort> {
    labels: RefCell<L>,
}

impl<L: EmployeeScheduleLabelPort> NotScheduledOrOffRule<L> {
    /// Build the rule over the port that saves labels.
    pub fn new(labels: L) -> Self {
        Self {
            labels: RefCell::new(labels),
        }
    }
}

impl<L: EmployeeScheduleLabelPort> ScheduleLabelRule for NotScheduledOrOffRule<L> {
    fn execute(&self, time_card: &mut dyn TimeCard, rule_item: &RuleItem, date: LocalDate) {
        let params = rule_item
            .params()
            .fixed(&NotScheduledOrOffRuleConfig.default_values());
        let type_ids = ids_for_key(TIME_OFF_TYPES, &params);

        let wanted = time_card.shifts_for_day(date).is_empty()
            && time_card.schedule_label_for_date(date).is_none()
            && !time_card.time_off_requests().iter().any(|request| {
                request.covers_date(date)
                    && request.status() == TimeOffStatus::Approved
                    && type_ids.contains(&request.time_off_type_id())
            });
        if wanted {
            create_and_save_label(&self.labels, time_card, rule_item, &params, date);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::label_source::LabelSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_time_off::EmployeeTimeOff;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabel::config::SCHEDULE_LABEL;
    use crate::rules::algorithm::schedulelabel::fixtures::*;
    use crate::rules::params::RuleParams;
    use joda_rs::LocalDateTime;

    fn run(
        shifts: Vec<EmployeeShift>,
        labels: Vec<EmployeeScheduleLabel>,
        requests: Vec<EmployeeTimeOff>,
        params: RuleParams,
    ) -> (usize, SavedLabels) {
        let saved = SavedLabels::default();
        let mut c = card(employee())
            .with_shifts(shifts)
            .with_schedule_labels(labels)
            .with_time_off_requests(requests);
        NotScheduledOrOffRule::new(saved.clone()).execute(&mut c, &item(params), today());
        let n = saved.saved.borrow().len();
        (n, saved)
    }
    fn test_label() -> RuleParams {
        rule_params! { SCHEDULE_LABEL => "test" }
    }

    // ---- java_parity_tests: NotScheduledOrOffRuleImplTest.groovy ----

    #[test]
    fn label_is_created_correctly() {
        let (n, saved) = run(vec![], vec![], vec![], test_label());
        assert_eq!(n, 1);
        let label = saved.saved.borrow()[0].clone();
        assert_eq!(label.rule_item_id(), Some(1234));
        assert_eq!(label.source(), LabelSource::Rule);
        assert_eq!(label.date(), today());
        assert_eq!(label.employee_id(), 1111);
        assert_eq!(label.label(), "test");
    }

    #[test]
    fn defaults_are_filled_before_save_is_called() {
        // No parameters at all: the "" label and "[]" types defaults apply.
        let (n, saved) = run(vec![], vec![], vec![], RuleParams::new());
        assert_eq!(n, 1);
        assert_eq!(saved.saved.borrow()[0].label(), "");
    }

    #[test]
    fn label_is_not_created_if_employee_is_scheduled() {
        let shift = EmployeeShift::new(1, 1111, 1, today(), ShiftType::Actual, Vec::new());
        assert_eq!(run(vec![shift], vec![], vec![], test_label()).0, 0);
    }

    #[test]
    fn label_is_not_created_if_one_already_exists() {
        let existing = EmployeeScheduleLabel::new(1111, today(), "x", LabelSource::Manual);
        assert_eq!(run(vec![], vec![existing], vec![], test_label()).0, 0);
    }

    #[test]
    fn rule_checks_if_employee_is_off() {
        use crate::common::enums::time_off_status::TimeOffStatus::{
            Approved as A, Denied as D, Pending as P,
        };
        let d: LocalDateTime = now();
        let (back, fwd) = (d.minus_days(1), d.plus_days(1));
        // (should create, start, end, status, type) — configured types are [1,2,3,4]
        let rows = [
            (false, d, d, A, 1),
            (true, d, d, A, 11),
            (true, d, d, P, 1),
            (true, d, d, D, 1),
            (true, d, d, P, 11),
            (false, back, d, A, 1),
            (false, d, fwd, A, 1),
            (false, back, fwd, A, 1),
            (true, back, back, A, 1),
            (true, fwd, fwd, A, 1),
            (true, fwd, fwd, P, 1),
            (true, fwd, fwd, A, 11),
            (true, back, back, P, 11),
        ];
        for (i, (create, start, end, status, kind)) in rows.into_iter().enumerate() {
            let params = rule_params! { SCHEDULE_LABEL => "test", TIME_OFF_TYPES => "[1,2,3,4]" };
            let (n, _) = run(
                vec![],
                vec![],
                vec![EmployeeTimeOff::new(start, end, status, kind)],
                params,
            );
            assert_eq!(n, usize::from(create), "row {i}");
        }
    }
}
