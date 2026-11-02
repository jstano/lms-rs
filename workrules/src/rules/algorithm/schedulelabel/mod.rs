//! `RuleType::ScheduleLabel` — five rules that stamp a short label on an
//! employee's day during schedule calculation.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulelabel/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelabel/`,
//! runner `…/calcshift/rules/ScheduleLabelRunner.java`.
//!
//! A label is a tag like `OFF` or `FLEX` the scheduler shows against a day. The
//! rules decide whether a given day earns one; the runner clears the previous
//! run's rule-made labels and walks the calculation range a day at a time.
//!
//! # Rules take a [`TimeCard`], not a `ScheduleCalcDataSet`
//!
//! Same single-struct decision as divergence 23. What these rules read from the
//! data set — labels, schedules, shifts, time-off requests, the employee — are
//! all [`TimeCard`] methods now.
//!
//! # Labels carry the rule item id; the label list is written through the card
//!
//! `createAndSaveLabel` adds the label to the data set **and** saves it through
//! the DAO ([`EmployeeScheduleLabelPort`]).
//!
//! # `Property.getWorkWeekForDate` is the weekly range ending on the period end
//!
//! "Max labels" counts matching labels (text compared trimmed and
//! case-insensitively) across the work week containing the date, anchored on the
//! property's pay-period end date through [`PropertyPort`], as
//! `hours_threshold_fixed_with_max_cap` does.

pub mod approved_time_off;
pub mod available_not_scheduled;
pub mod config;
pub mod not_scheduled_or_off;
pub mod unavailable_all_day;

use crate::common::enums::label_source::LabelSource;
use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulelabel::config::SCHEDULE_LABEL;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::params::RuleParams;
use crate::rules::ports::{EmployeeScheduleLabelPort, PropertyPort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use approved_time_off::ApprovedTimeOffLabelRule;
use available_not_scheduled::AvailableNotScheduledRule;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;
use not_scheduled_or_off::NotScheduledOrOffRule;
use std::cell::RefCell;
use unavailable_all_day::UnavailableAllDayRule;

/// `ScheduleLabelRule`: decide whether `date` earns a label and make it.
pub trait ScheduleLabelRule {
    /// `execute(ScheduleCalcDataSet, RuleItem, LocalDate)`.
    fn execute(&self, time_card: &mut dyn TimeCard, rule_item: &RuleItem, date: LocalDate);
}

/// `createAndSaveLabel`: a `RULE`-sourced label on `date` carrying the rule
/// item's id and the `scheduleLabel` text of `params` (defaults filled), added
/// to the card and saved.
pub(crate) fn create_and_save_label<L: EmployeeScheduleLabelPort>(
    labels: &RefCell<L>,
    time_card: &mut dyn TimeCard,
    rule_item: &RuleItem,
    params: &RuleParams,
    date: LocalDate,
) {
    let Some(employee_id) = time_card.employee().map(|e| e.id()) else {
        return;
    };
    // The text comes from the parameters *after* defaults are filled in.
    let text = params.get(SCHEDULE_LABEL).unwrap_or("");
    let label = EmployeeScheduleLabel::new(employee_id, date, text, LabelSource::Rule)
        .with_rule_item_id(rule_item.id());
    time_card.schedule_labels_mut().push(label.clone());
    labels.borrow_mut().save(&label);
}

/// How many labels with this text the employee already has in the work week
/// containing `date`. `getMatchingLabelsCount`.
pub(crate) fn matching_labels_count<P: PropertyPort>(
    property: &P,
    time_card: &dyn TimeCard,
    date: LocalDate,
    text: &str,
) -> usize {
    let Some(employee) = time_card.employee() else {
        return 0;
    };
    let week = WeeklyDateRange::with_end_date(property.period_end_date(employee.property_id()))
        .range_containing_date(date);
    time_card
        .schedule_labels_for_date_range(&week)
        .into_iter()
        .filter(|label| label.label().trim().to_lowercase() == text.trim().to_lowercase())
        .count()
}

/// `NoLabelRuleImpl`: configured to do nothing.
pub struct NoLabelRule;

impl ScheduleLabelRule for NoLabelRule {
    fn execute(&self, _time_card: &mut dyn TimeCard, _rule_item: &RuleItem, _date: LocalDate) {}
}

/// Dispatch for the five schedule-label rules. `RuleImplFactory`, narrowed to
/// this family.
pub struct ScheduleLabelRegistry<L: EmployeeScheduleLabelPort, P: PropertyPort> {
    no_label: NoLabelRule,
    approved_time_off: ApprovedTimeOffLabelRule<L>,
    available_not_scheduled: AvailableNotScheduledRule<L, P>,
    not_scheduled_or_off: NotScheduledOrOffRule<L>,
    unavailable_all_day: UnavailableAllDayRule<L, P>,
}

impl<L: EmployeeScheduleLabelPort + Clone, P: PropertyPort + Clone> ScheduleLabelRegistry<L, P> {
    /// Build every rule over the shared ports.
    pub fn new(labels: L, property: P) -> Self {
        Self {
            no_label: NoLabelRule,
            approved_time_off: ApprovedTimeOffLabelRule::new(labels.clone()),
            available_not_scheduled: AvailableNotScheduledRule::new(
                labels.clone(),
                property.clone(),
            ),
            not_scheduled_or_off: NotScheduledOrOffRule::new(labels.clone()),
            unavailable_all_day: UnavailableAllDayRule::new(labels, property),
        }
    }
}

impl<L, P> RuleRegistry<dyn ScheduleLabelRule> for ScheduleLabelRegistry<L, P>
where
    L: EmployeeScheduleLabelPort + 'static,
    P: PropertyPort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ScheduleLabel
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ScheduleLabelRule + 'static)> {
        match rule_class {
            RuleClass::NoLabelLb => Some(&self.no_label),
            RuleClass::ApprovedTimeOffLb => Some(&self.approved_time_off),
            RuleClass::AvailNotSchedLb => Some(&self.available_not_scheduled),
            RuleClass::NotSchedLb => Some(&self.not_scheduled_or_off),
            RuleClass::UnavailLb => Some(&self.unavailable_all_day),
            _ => None,
        }
    }
}

/// Shared test scaffolding for the family.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use date_range_rs::DateRange;
    use joda_rs::LocalDateTime;
    use std::rc::Rc;

    pub fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }
    pub fn now() -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, 10, 30, 0)
    }

    /// Records every saved label through a handle the test keeps.
    #[derive(Clone, Default)]
    pub struct SavedLabels {
        pub saved: Rc<RefCell<Vec<EmployeeScheduleLabel>>>,
        pub removed: Rc<RefCell<Vec<(i32, DateRange)>>>,
    }

    impl EmployeeScheduleLabelPort for SavedLabels {
        fn save(&mut self, label: &EmployeeScheduleLabel) {
            self.saved.borrow_mut().push(label.clone());
        }
        fn remove_rule_generated_labels_for_period(
            &mut self,
            employee_id: i32,
            period: &DateRange,
        ) {
            self.removed.borrow_mut().push((employee_id, *period));
        }
    }

    /// The work week is the seven days from `today()`.
    #[derive(Clone)]
    pub struct WeekFromToday;
    impl PropertyPort for WeekFromToday {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            today().plus_days(6)
        }
    }

    pub fn employee() -> Employee {
        Employee::new(1111, 1, "Pat", Vec::new())
    }

    pub fn card(employee: Employee) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee)
            .with_date_range(DateRange::new(today(), today().plus_days(7)))
    }

    pub fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1234, 1, "label", RuleClass::NoLabelLb, params)
    }

    #[test]
    fn the_registry_dispatches_every_label_rule_class() {
        let registry = ScheduleLabelRegistry::new(SavedLabels::default(), WeekFromToday);
        assert_eq!(registry.dispatchable().len(), 5);
        assert!(registry.missing().is_empty());
    }
}
