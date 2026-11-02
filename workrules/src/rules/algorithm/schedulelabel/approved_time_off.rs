//! Port of `ApprovedTimeOffLabelRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabel/ApprovedTimeOffLabelRuleImpl.java`.
//!
//! Labels a day on which the employee has an **approved** time-off request of one
//! of the configured types, provided the day has no label yet. A request covers a
//! day when the day lies within the **date parts** of its start and end, both
//! inclusive — the times of day are ignored.

use crate::common::enums::time_off_status::TimeOffStatus;
use crate::common::json_ids::ids_for_key;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::config::{
    ApprovedTimeOffLabelRuleConfig, TIME_OFF_TYPES,
};
use crate::rules::algorithm::schedulelabel::{ScheduleLabelRule, create_and_save_label};
use crate::rules::ports::EmployeeScheduleLabelPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `ApprovedTimeOffLabelRuleImpl`.
pub struct ApprovedTimeOffLabelRule<L: EmployeeScheduleLabelPort> {
    labels: RefCell<L>,
}

impl<L: EmployeeScheduleLabelPort> ApprovedTimeOffLabelRule<L> {
    /// Build the rule over the port that saves labels.
    pub fn new(labels: L) -> Self {
        Self {
            labels: RefCell::new(labels),
        }
    }
}

impl<L: EmployeeScheduleLabelPort> ScheduleLabelRule for ApprovedTimeOffLabelRule<L> {
    fn execute(&self, time_card: &mut dyn TimeCard, rule_item: &RuleItem, date: LocalDate) {
        let params = rule_item
            .params()
            .fixed(&ApprovedTimeOffLabelRuleConfig.default_values());
        let type_ids = ids_for_key(TIME_OFF_TYPES, &params);

        let wanted = time_card.schedule_label_for_date(date).is_none()
            && time_card.time_off_requests().iter().any(|request| {
                request.status() == TimeOffStatus::Approved
                    && request.covers_date(date)
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
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::employee_time_off::EmployeeTimeOff;
    use crate::rule_params;
    use crate::rules::algorithm::schedulelabel::config::SCHEDULE_LABEL;
    use crate::rules::algorithm::schedulelabel::fixtures::*;
    use crate::rules::params::RuleParams;
    use joda_rs::LocalDateTime;

    const VALID: i32 = 1;
    const INVALID: i32 = 11;

    fn params() -> RuleParams {
        rule_params! { SCHEDULE_LABEL => "test", TIME_OFF_TYPES => "[1,2,3,4]" }
    }
    fn request(
        start: LocalDateTime,
        end: LocalDateTime,
        status: TimeOffStatus,
        kind: i32,
    ) -> EmployeeTimeOff {
        EmployeeTimeOff::new(start, end, status, kind)
    }
    fn run(
        requests: Vec<EmployeeTimeOff>,
        labels: Vec<EmployeeScheduleLabel>,
    ) -> (usize, SavedLabels) {
        let saved = SavedLabels::default();
        let mut c = card(employee())
            .with_time_off_requests(requests)
            .with_schedule_labels(labels);
        ApprovedTimeOffLabelRule::new(saved.clone()).execute(&mut c, &item(params()), today());
        let n = saved.saved.borrow().len();
        (n, saved)
    }

    // ---- java_parity_tests: ApprovedTimeOffLabelRuleImplTest.groovy ----

    #[test]
    fn label_is_created_correctly() {
        let (n, saved) = run(
            vec![request(now(), now(), TimeOffStatus::Approved, VALID)],
            vec![],
        );
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
        // No explicit parameters: the defaults ("" label, no types) apply, so
        // nothing is created and nothing throws.
        let saved = SavedLabels::default();
        let mut c = card(employee()).with_time_off_requests(vec![request(
            now(),
            now(),
            TimeOffStatus::Approved,
            VALID,
        )]);
        ApprovedTimeOffLabelRule::new(saved.clone()).execute(
            &mut c,
            &item(RuleParams::new()),
            today(),
        );
        assert!(saved.saved.borrow().is_empty());
    }

    #[test]
    fn label_is_not_created_if_one_already_exists() {
        let existing = EmployeeScheduleLabel::new(1111, today(), "x", LabelSource::Manual);
        let (n, _) = run(
            vec![request(now(), now(), TimeOffStatus::Approved, VALID)],
            vec![existing],
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn rule_checks_if_employee_is_off() {
        use TimeOffStatus::{Approved as A, Denied as D, Pending as P};
        let d = now();
        let (back, fwd) = (d.minus_days(1), d.plus_days(1));
        // (should create, start, end, status, type)
        let rows = [
            (true, d, d, A, VALID),
            (false, d, d, A, INVALID),
            (false, d, d, P, VALID),
            (false, d, d, D, VALID),
            (false, d, d, P, INVALID),
            (true, back, d, A, VALID),
            (true, d, fwd, A, VALID),
            (true, back, fwd, A, VALID),
            (false, back, back, A, VALID),
            (false, fwd, fwd, A, VALID),
            (false, fwd, fwd, P, VALID),
            (false, fwd, fwd, A, INVALID),
            (false, back, back, P, INVALID),
        ];
        for (i, (create, start, end, status, kind)) in rows.into_iter().enumerate() {
            let (n, _) = run(vec![request(start, end, status, kind)], vec![]);
            assert_eq!(n, usize::from(create), "row {i}");
        }
    }

    #[test]
    fn label_is_not_created_if_there_are_no_time_off_requests() {
        let (n, _) = run(vec![], vec![]);
        assert_eq!(n, 0);
    }
}
