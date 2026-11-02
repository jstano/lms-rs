//! Port of `NoEarningRuleImpl` — a rule that deliberately does nothing.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/NoEarningRuleImpl.java`.
//! No Groovy spec.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::ShiftEarningRule;

/// `NoEarningRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEarningRule;

impl ShiftEarningRule for NoEarningRule {
    fn execute(&self, _s: &EmployeeShift, _tc: &mut dyn TimeCard, _i: &RuleItem) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    #[test]
    fn it_never_adds_an_earning() {
        let mut card = TimeCardData::new();
        let shift = EmployeeShift::new(
            1,
            1,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        );
        NoEarningRule.execute(
            &shift,
            &mut card,
            &RuleItem::new(1, 1, "n", RuleClass::NoDif, RuleParams::new()),
        );
        assert!(card.earnings().is_empty());
    }
}
