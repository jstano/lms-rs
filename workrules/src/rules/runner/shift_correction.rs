//! Port of `ShiftCorrectionRunner`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ShiftCorrectionRunner.java`.
//!
//! For each shift on the time card that is open for editing, resolves its
//! `SHIFT_CORRECTION` rule set, sorts the items by priority, and runs each
//! against the shift. A shift in a closed period, or with no rule set (or an
//! empty one), is skipped.
//!
//! # It sorts — unlike nearly every other runner
//!
//! `Collections.sort(ruleItems, new RuleItemPriorityComparator())`. The sort
//! is stable and ranks configs that are not priority-configurable at
//! [`DEFAULT_PRIORITY`](crate::rules::rule_config::DEFAULT_PRIORITY), so in
//! the Groovy spec `SHIFT_CATEGORY_SCR` (no priority) runs between the
//! `SPLIT_TO_SCHEDULE_SCR` items of priority 2 and 7.
//!
//! # The shift being corrected is a working copy (divergence 80)
//!
//! Java iterates a *copy of the list* and hands each rule the live shift plus
//! the live time card. A `&mut EmployeeShift` borrowed out of the time card
//! cannot coexist with the `&mut dyn TimeCard` `SplitToSchedule` needs to
//! append to, so the runner clones the shift, runs the rules against the
//! clone, and writes it back **at the same index**. Shifts a rule appends
//! land after the originals, and are not visited this pass — exactly Java's
//! behaviour, since its iteration list was copied first.
//!
//! # Resolution is per shift
//!
//! Java asks `ruleUtils.getRuleSet(employee, shift.getJob(), shift.getShiftDate(),
//! SHIFT_CORRECTION)` for each shift, so a different job or date can select a
//! different set. `rule_set_for` is that call.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftcorrection::ShiftCorrectionRule;
use crate::rules::algorithm::shiftcorrection::config::{
    OutPunchCorrectionRuleConfig, ShiftCategoryCorrectionRuleConfig,
    SplitToScheduleCorrectionRuleConfig,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::priority::sort_by_priority;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;

/// Runs the shift-correction family over a time card. `ShiftCorrectionRunner`.
pub struct ShiftCorrectionRunner<'r> {
    registry: &'r dyn RuleRegistry<dyn ShiftCorrectionRule>,
}

impl<'r> ShiftCorrectionRunner<'r> {
    /// Build the runner over the family's dispatch.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ShiftCorrectionRule>) -> Self {
        Self { registry }
    }

    /// `runRules(ActualsTimeCard)`. `rule_set_for` resolves the
    /// `SHIFT_CORRECTION` rule set for one shift.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rule_set_for: impl Fn(&EmployeeShift) -> Option<&'a RuleSet>,
    ) {
        // Java iterates a snapshot of the list; rules may append to the card.
        let count = time_card.shifts().len();

        for index in 0..count {
            if !time_card.is_open_for_editing_for_shift(&time_card.shifts()[index]) {
                continue;
            }
            let Some(rule_set) = rule_set_for(&time_card.shifts()[index]) else {
                continue;
            };
            if rule_set.rule_items().is_empty() {
                continue;
            }

            let mut items: Vec<&RuleItem> = rule_set.rule_items().iter().collect();
            sort_by_priority(&mut items, |item| config_for(item.rule_class()));

            let mut shift = time_card.shifts()[index].clone();
            for item in items {
                if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                    rule.execute(&mut shift, time_card, item);
                }
            }
            time_card.shifts_mut()[index] = shift;
        }
    }
}

fn config_for(rule_class: RuleClass) -> &'static dyn RuleConfig {
    match rule_class {
        RuleClass::SplitToScheduleScr => &SplitToScheduleCorrectionRuleConfig,
        RuleClass::ShiftCategoryScr => &ShiftCategoryCorrectionRuleConfig,
        _ => &OutPunchCorrectionRuleConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_type::RuleType;
    use joda_rs::LocalDate;
    use std::cell::RefCell;

    /// Records which shift and which rule item each call received.
    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(i32, i32)>>,
    }

    impl ShiftCorrectionRule for Recorder {
        fn execute(
            &self,
            shift: &mut EmployeeShift,
            _time_card: &mut dyn TimeCard,
            rule_item: &RuleItem,
        ) {
            self.calls.borrow_mut().push((shift.id(), rule_item.id()));
        }
    }

    struct RecordingRegistry(Recorder);

    impl RuleRegistry<dyn ShiftCorrectionRule> for RecordingRegistry {
        fn rule_type(&self) -> RuleType {
            RuleType::ShiftCorrection
        }
        fn rule_for(&self, _rule_class: RuleClass) -> Option<&(dyn ShiftCorrectionRule + 'static)> {
            Some(&self.0)
        }
    }

    fn shift(id: i32, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 100, 1, date, ShiftType::Actual, Vec::new())
    }

    fn item(id: i32, class: RuleClass, priority: Option<&str>) -> RuleItem {
        let params = match priority {
            Some(p) => rule_params! { "priority" => p },
            None => rule_params! {},
        };
        RuleItem::new(id, 1, "item", class, params)
    }

    /// `ShiftCorrectionRunnerTest."run shift correction rules"`.
    #[test]
    fn closed_shifts_and_missing_rule_sets_are_skipped_and_items_run_in_priority_order() {
        let today = LocalDate::of(2014, 5, 6);
        let mut card = TimeCardData::new()
            .with_calculation_start_date(today)
            .with_shifts(vec![
                shift(1, today.minus_days(3)),
                shift(2, today),
                shift(3, today),
            ]);
        let rule_set = RuleSet::new(
            1,
            1,
            "sc",
            RuleType::ShiftCorrection,
            0,
            vec![
                item(11, RuleClass::SplitToScheduleScr, Some("7")),
                item(12, RuleClass::SplitToScheduleScr, Some("2")),
                item(13, RuleClass::ShiftCategoryScr, None),
            ],
        );
        let registry = RecordingRegistry(Recorder::default());
        let resolutions = RefCell::new(Vec::new());

        ShiftCorrectionRunner::new(&registry).run_rules(&mut card, |s| {
            resolutions.borrow_mut().push(s.id());
            if s.id() == 3 { Some(&rule_set) } else { None }
        });

        assert_eq!(
            *resolutions.borrow(),
            vec![2, 3],
            "closed shift 1 never resolves"
        );
        assert_eq!(
            *registry.0.calls.borrow(),
            vec![(3, 12), (3, 13), (3, 11)],
            "priority 2, default 5, priority 7"
        );
    }

    #[test]
    fn an_empty_rule_set_runs_nothing() {
        let today = LocalDate::of(2014, 5, 6);
        let mut card = TimeCardData::new()
            .with_calculation_start_date(today)
            .with_shifts(vec![shift(1, today)]);
        let empty = RuleSet::new(1, 1, "sc", RuleType::ShiftCorrection, 0, Vec::new());
        let registry = RecordingRegistry(Recorder::default());

        ShiftCorrectionRunner::new(&registry).run_rules(&mut card, |_| Some(&empty));

        assert!(registry.0.calls.borrow().is_empty());
    }

    #[test]
    fn equal_priorities_keep_rule_set_order() {
        let today = LocalDate::of(2014, 5, 6);
        let mut card = TimeCardData::new()
            .with_calculation_start_date(today)
            .with_shifts(vec![shift(1, today)]);
        let rule_set = RuleSet::new(
            1,
            1,
            "sc",
            RuleType::ShiftCorrection,
            0,
            vec![
                item(21, RuleClass::OutPunchScr, None),
                item(22, RuleClass::ShiftCategoryScr, None),
            ],
        );
        let registry = RecordingRegistry(Recorder::default());

        ShiftCorrectionRunner::new(&registry).run_rules(&mut card, |_| Some(&rule_set));

        assert_eq!(*registry.0.calls.borrow(), vec![(1, 21), (1, 22)]);
    }
}
