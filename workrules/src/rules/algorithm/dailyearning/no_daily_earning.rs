//! Port of `NoDailyEarningRuleImpl` — a rule that deliberately does nothing,
//! so a rule set can be configured without paying anything.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/NoDailyEarningRuleImpl.java`.
//! No Groovy spec.

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::DailyEarningRule;
use joda_rs::LocalDate;

/// `NoDailyEarningRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDailyEarningRule;

impl DailyEarningRule for NoDailyEarningRule {
    fn execute(&self, _time_card: &mut dyn TimeCard, _date: LocalDate, _rule_item: &RuleItem) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    #[test]
    fn it_never_adds_an_earning() {
        let mut card = card(
            employee(10.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 8.0)],
        );
        let item = RuleItem::new(1, 1, "none", RuleClass::NoDdf, RuleParams::new());
        NoDailyEarningRule.execute(&mut card, today(), &item);
        assert!(crate::entity::time_card::TimeCard::earnings(&card).is_empty());
    }
}
