//! Port of `NoRestrictionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/NoRestrictionRuleImpl.java`.
//!
//! The family's no-op: always OK, never strict. The catalogue's default
//! fallback, the same role `RegHrsOnlyRuleImpl` plays in `hoursdistribution`.
//!
//! Ported cases: `NoRestrictionRuleImplTest.groovy` (two cases).

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::{
    ScheduleRestrictionResult, ScheduleRestrictionRule,
};

/// `NoRestrictionRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRestrictionRule;

impl ScheduleRestrictionRule for NoRestrictionRule {
    fn can_employee_work_shift(
        &self,
        _dataset: &dyn TimeCard,
        _shift: &EmployeeShift,
        _rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult {
        ScheduleRestrictionResult::ok()
    }

    /// `isStrict` is hardcoded `false` — the one override in the family; see
    /// the trait's default-method doc for why every other rule can share it.
    fn is_strict(&self, _rule_item: &RuleItem) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn rule_item() -> RuleItem {
        RuleItem::new(1, 1, "No restriction", RuleClass::NoSrr, RuleParams::new())
    }

    mod java_parity_tests {
        use super::*;

        /// `NoRestrictionRuleImplTest`: "should always return OK with no
        /// message".
        #[test]
        fn should_always_return_ok_with_no_message() {
            let dataset = TimeCardData::new();
            let shift = EmployeeShift::new(
                1,
                100,
                1,
                LocalDate::of(2018, 2, 21),
                ShiftType::Schedule,
                Vec::new(),
            );

            let result = NoRestrictionRule.can_employee_work_shift(&dataset, &shift, &rule_item());

            assert!(result.is_ok());
            assert_eq!(result.message(), None);
            assert_eq!(result.shift_error_type(), None);
        }

        /// `NoRestrictionRuleImplTest`: "should not be strict".
        #[test]
        fn should_not_be_strict() {
            assert!(!NoRestrictionRule.is_strict(&rule_item()));
        }
    }
}
