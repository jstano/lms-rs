//! `RuleType::ShiftCorrection` — three concrete rules behind `ShiftCorrectionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftcorrection/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftcorrection/`,
//! runner at `.../server/labor/calcshift/rules/ShiftCorrectionRunner.java`.
//!
//! Fourth of the seven families prioritized for scheduling. Unlike the
//! adjustment families, these rules **repair the shift's structure** rather
//! than write an adjustment: one invents a missing OUT punch, one re-tags a
//! shift with the category of the schedule it matches, and one cuts a long
//! shift into pieces along contiguous schedules.
//!
//! # `execute` takes the time card mutably
//!
//! Every other family's `execute` takes `&dyn TimeCard`. `SplitToSchedule`
//! does `timeCard.getShifts().add(split)`, so the trait takes
//! `&mut dyn TimeCard`. See divergence 80 for how the runner keeps the shift
//! being corrected and the time card that owns it from aliasing.
//!
//! # The runner sorts, unlike nearly every other runner
//!
//! `ShiftCorrectionRunner` calls `Collections.sort(ruleItems, new
//! RuleItemPriorityComparator())`. `priority.rs` used to say only
//! `PostPunchRuleRunner` does; it is one of five, and the comment is corrected.
//!
//! Ported cases: `SplitToScheduleCorrectionRuleImplTest.groovy` (all 14),
//! `SplitToScheduleCorrectionRuleConfigTest.groovy`, and
//! `ShiftCorrectionRunnerTest.groovy`. `OutPunchCorrectionRuleImpl` and
//! `ShiftCategoryCorrectionRuleImpl` have no Groovy spec; their behaviour
//! tests are written from the Java.

pub mod config;
pub mod out_punch;
pub mod shift_category;
pub mod split_to_schedule;

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A shift-correction rule. `ShiftCorrectionRuleImpl`.
pub trait ShiftCorrectionRule {
    /// `execute(EmployeeShift, TimeCard, Map<String, String>)` — the params
    /// travel inside the [`RuleItem`], as in every other family.
    fn execute(
        &self,
        shift: &mut EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
    );
}

use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::EmployeeShiftSavePort;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use out_punch::OutPunchCorrectionRule;
use shift_category::ShiftCategoryCorrectionRule;
use split_to_schedule::SplitToScheduleCorrectionRule;

/// Dispatch for the three shift-correction rules. `RuleImplFactory`, narrowed
/// to this family.
pub struct ShiftCorrectionRegistry<S: EmployeeShiftSavePort> {
    out_punch: OutPunchCorrectionRule,
    shift_category: ShiftCategoryCorrectionRule,
    split_to_schedule: SplitToScheduleCorrectionRule<S>,
}

impl<S: EmployeeShiftSavePort> ShiftCorrectionRegistry<S> {
    /// Build the registry over the port `SplitToSchedule` persists through.
    pub fn new(employee_shift: S) -> Self {
        Self {
            out_punch: OutPunchCorrectionRule,
            shift_category: ShiftCategoryCorrectionRule,
            split_to_schedule: SplitToScheduleCorrectionRule::new(employee_shift),
        }
    }
}

impl<S: EmployeeShiftSavePort + 'static> RuleRegistry<dyn ShiftCorrectionRule>
    for ShiftCorrectionRegistry<S>
{
    fn rule_type(&self) -> RuleType {
        RuleType::ShiftCorrection
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ShiftCorrectionRule + 'static)> {
        match rule_class {
            RuleClass::OutPunchScr => Some(&self.out_punch),
            RuleClass::ShiftCategoryScr => Some(&self.shift_category),
            RuleClass::SplitToScheduleScr => Some(&self.split_to_schedule),
            _ => None,
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::entity::employee_shift::EmployeeShift;

    struct NoSave;
    impl EmployeeShiftSavePort for NoSave {
        fn save(&self, _shift: &mut EmployeeShift) {}
    }

    /// All three catalogue entries dispatch, none is missing.
    #[test]
    fn every_shift_correction_rule_class_dispatches() {
        let registry = ShiftCorrectionRegistry::new(NoSave);
        assert_eq!(registry.dispatchable().len(), 3);
        assert!(registry.missing().is_empty());
    }
}
