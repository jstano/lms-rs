//! Port of `NoAdjustmentRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/NoAdjustmentRuleImpl.java`.
//!
//! The family's no-op — an empty `execute` body. The catalogue's default
//! fallback, the same role `NoRestrictionRuleImpl` plays in
//! `schedulerestriction` and `RegHrsOnlyRuleImpl` plays in `hoursdistribution`.
//!
//! No Groovy spec; the one behaviour worth pinning is that nothing changes.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::ShiftAdjustmentRule;

/// `NoAdjustmentRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAdjustmentRule;

impl ShiftAdjustmentRule for NoAdjustmentRule {
    fn execute(&self, _shift: &mut EmployeeShift, _dataset: &dyn TimeCard, _rule_item: &RuleItem) {}
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
    fn nothing_changes() {
        let dataset = TimeCardData::new();
        let mut shift = EmployeeShift::new(
            1,
            100,
            1,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_worked_hours(8.0)
        .with_net_hours(8.0);
        let rule_item = RuleItem::new(1, 1, "No adjustment", RuleClass::NoSad, RuleParams::new());

        NoAdjustmentRule.execute(&mut shift, &dataset, &rule_item);

        assert_eq!(shift.adj_hours(), 0.0);
        assert_eq!(shift.net_hours(), 8.0);
    }
}
