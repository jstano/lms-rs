//! Port of `ScheduleChangeValidationRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ScheduleChangeValidationRunner.java`.
//!
//! Prices one schedule edit. Given the shift before and after (either may be
//! absent: an add or a delete), it:
//!
//! 1. asks whether the user may **overwrite** the penalties
//!    (`SCHEDULING_FAIR_WORK_WEEK_CHANGES`) at the shift's property — the updated
//!    shift's, else the original's, else the employee's;
//! 2. if at least one shift exists and the card is open for editing for it (the
//!    updated shift, else the original), resolves the `SCHEDULE_CHANGE_VALIDATION`
//!    rule set for the employee, the **original** shift's job and date (else the
//!    updated's), sorts its items by priority and runs each;
//! 3. returns every non-empty [`EarningRequest`] with the permission flag.
//!
//! The priority sort is applied but cannot reorder anything: the three configs are
//! not `PriorityRuleConfig`s, so every item ranks at the default and keeps
//! rule-set order.

use crate::entity::earning_request::EarningRequest;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulechangevalidation::ScheduleChangeValidationRule;
use crate::rules::algorithm::schedulechangevalidation::config::{
    AddHoursPenaltyRuleConfig, RemoveHoursPenaltyRuleConfig, ShiftModificationPenaltyRuleConfig,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::SchedulingPermissionPort;
use crate::rules::priority::sort_by_priority;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;

/// `ScheduleChangeValidationResultDTO`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleChangeValidationResult {
    /// The penalties the edit incurs.
    pub earning_requests: Vec<EarningRequest>,
    /// Whether the user may overwrite them.
    pub has_overwrite_permission: bool,
}

/// Runs the schedule-change-validation family over one edit.
pub struct ScheduleChangeValidationRunner<'r, P: SchedulingPermissionPort> {
    registry: &'r dyn RuleRegistry<dyn ScheduleChangeValidationRule>,
    permission: P,
}

impl<'r, P: SchedulingPermissionPort> ScheduleChangeValidationRunner<'r, P> {
    /// Build the runner over the family's dispatch and the permission lookup.
    pub fn new(
        registry: &'r dyn RuleRegistry<dyn ScheduleChangeValidationRule>,
        permission: P,
    ) -> Self {
        Self {
            registry,
            permission,
        }
    }

    /// `runRules(calcDataSet, originalShift, updatedShift, penaltyDescription)`.
    /// `rule_set_for(shift)` is `ruleUtils.getRuleSet(employee, shift.job, shift.date, SCHEDULE_CHANGE_VALIDATION)`.
    pub fn run_rules<'a>(
        &self,
        time_card: &dyn TimeCard,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        penalty_description: Option<&str>,
        rule_set_for: impl Fn(&EmployeeShift) -> Option<&'a RuleSet>,
    ) -> ScheduleChangeValidationResult {
        let property_id = updated
            .or(original)
            .map(|s| s.property_id())
            .or_else(|| time_card.employee().map(|e| e.property_id()))
            .unwrap_or_default();
        let has_permission = self
            .permission
            .can_overwrite_fair_work_week_changes(property_id);

        let mut earning_requests = Vec::new();
        if let Some(edited) = updated.or(original)
            && time_card.is_open_for_editing_for_shift(edited)
            && let Some(basis) = original.or(updated)
            && let Some(rule_set) = rule_set_for(basis)
        {
            let mut items: Vec<&RuleItem> = rule_set.rule_items().iter().collect();
            sort_by_priority(&mut items, |item| config_for(item.rule_class()));
            for item in items {
                if let Some(rule) = self.registry.rule_for(item.rule_class())
                    && let Some(request) =
                        rule.execute(original, updated, item, item.params(), penalty_description)
                {
                    earning_requests.push(request);
                }
            }
        }
        ScheduleChangeValidationResult {
            earning_requests,
            has_overwrite_permission: has_permission,
        }
    }
}

fn config_for(rule_class: RuleClass) -> &'static dyn RuleConfig {
    match rule_class {
        RuleClass::AddHoursSchedValidation => &AddHoursPenaltyRuleConfig,
        RuleClass::RemoveHoursSchedValidation => &RemoveHoursPenaltyRuleConfig,
        _ => &ShiftModificationPenaltyRuleConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee::Employee;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::schedulechangevalidation::ScheduleChangeValidationRegistry;
    use crate::rules::algorithm::schedulechangevalidation::config::EARNING_TYPE;
    use crate::rules::algorithm::schedulechangevalidation::fixtures::*;
    use crate::rules::rule_type::RuleType;
    use joda_rs::{LocalDate, LocalDateTime};
    use std::cell::RefCell;

    struct Permission(bool, RefCell<Vec<i32>>);
    impl SchedulingPermissionPort for Permission {
        fn can_overwrite_fair_work_week_changes(&self, property_id: i32) -> bool {
            self.1.borrow_mut().push(property_id);
            self.0
        }
    }

    fn registry() -> ScheduleChangeValidationRegistry<Types, Now> {
        ScheduleChangeValidationRegistry::new(
            Types(UOM::Hours),
            Now(LocalDateTime::of(2020, 12, 30, 0, 0, 0)),
        )
    }
    fn set(classes: &[(i32, RuleClass)]) -> RuleSet {
        RuleSet::new(
            1,
            1,
            "s",
            RuleType::ScheduleChangeValidation,
            0,
            classes
                .iter()
                .map(|(id, c)| {
                    RuleItem::new(*id, 1, "x", *c, rule_params! { EARNING_TYPE => "21" })
                })
                .collect(),
        )
    }
    fn card() -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(11, 7, "Pat", Vec::new()))
            .with_calculation_start_date(LocalDate::of(2020, 12, 1))
    }

    #[test]
    fn every_rule_that_produces_a_request_contributes_one() {
        let (original, updated) = (shift(6.0, (9, 0), (15, 0)), shift(8.0, (9, 0), (17, 0)));
        let rule_set = set(&[
            (1, RuleClass::AddHoursSchedValidation),
            (2, RuleClass::RemoveHoursSchedValidation),
            (3, RuleClass::ShiftModificationSchedValidation),
        ]);
        let registry = registry();
        let result =
            ScheduleChangeValidationRunner::new(&registry, Permission(true, RefCell::default()))
                .run_rules(
                    &card(),
                    Some(&original),
                    Some(&updated),
                    Some("temp"),
                    |_| Some(&rule_set),
                );
        // Only "add hours" applies: the shift got longer and was not otherwise moved.
        assert_eq!(result.earning_requests, vec![expected(1.0)]);
        assert!(result.has_overwrite_permission);
    }

    #[test]
    fn the_permission_is_asked_for_the_updated_shifts_property_first() {
        let registry = registry();
        let permission = Permission(false, RefCell::default());
        let runner = ScheduleChangeValidationRunner::new(&registry, permission);
        let original = shift(8.0, (9, 0), (17, 0)).with_property_id(3);
        let updated = shift(8.0, (9, 0), (17, 0)).with_property_id(4);
        let c = card();
        let r = runner.run_rules(&c, Some(&original), Some(&updated), None, |_| None);
        assert!(!r.has_overwrite_permission && r.earning_requests.is_empty());
        runner.run_rules(&c, Some(&original), None, None, |_| None);
        runner.run_rules(&c, None, None, None, |_| None);
        assert_eq!(*runner.permission.1.borrow(), vec![4, 3, 7]);
    }

    #[test]
    fn the_rule_set_is_resolved_for_the_original_shift_when_there_is_one() {
        let registry = registry();
        let runner =
            ScheduleChangeValidationRunner::new(&registry, Permission(true, RefCell::default()));
        let original = EmployeeShift::new(
            1,
            11,
            34,
            LocalDate::of(2021, 1, 1),
            ShiftType::Schedule,
            Vec::new(),
        );
        let updated = EmployeeShift::new(
            2,
            11,
            35,
            LocalDate::of(2021, 1, 2),
            ShiftType::Schedule,
            Vec::new(),
        );
        let asked = RefCell::new(Vec::new());
        runner.run_rules(&card(), Some(&original), Some(&updated), None, |s| {
            asked.borrow_mut().push((s.job_id(), s.shift_date()));
            None
        });
        assert_eq!(*asked.borrow(), vec![(34, LocalDate::of(2021, 1, 1))]);
    }

    #[test]
    fn nothing_runs_when_the_period_is_closed_or_both_shifts_are_absent() {
        let registry = registry();
        let runner =
            ScheduleChangeValidationRunner::new(&registry, Permission(true, RefCell::default()));
        let rule_set = set(&[(1, RuleClass::AddHoursSchedValidation)]);
        let updated = shift(8.0, (9, 0), (17, 0));
        let closed = card().with_calculation_start_date(LocalDate::of(2021, 2, 1));
        let r = runner.run_rules(&closed, None, Some(&updated), None, |_| Some(&rule_set));
        assert!(r.earning_requests.is_empty());
        let r = runner.run_rules(&card(), None, None, None, |_| Some(&rule_set));
        assert!(r.earning_requests.is_empty());
    }
}
