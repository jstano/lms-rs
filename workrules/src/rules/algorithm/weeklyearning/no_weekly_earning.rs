//! Port of `NoWeeklyEarningRuleImpl` — a rule that deliberately does nothing.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/NoWeeklyEarningRuleImpl.java`.
//! No Groovy spec.

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::WeeklyEarningRule;
use date_range_rs::DateRange;

/// `NoWeeklyEarningRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWeeklyEarningRule;

impl WeeklyEarningRule for NoWeeklyEarningRule {
    fn execute(&self, _tc: &mut dyn TimeCard, _w: &DateRange, _i: &RuleItem, _d: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    #[test]
    fn it_never_adds_an_earning() {
        let mut card = TimeCardData::new();
        let week = DateRange::new(LocalDate::of(2014, 5, 5), LocalDate::of(2014, 5, 11));
        let item = RuleItem::new(1, 1, "none", RuleClass::NoWdf, RuleParams::new());
        NoWeeklyEarningRule.execute(&mut card, &week, &item, "d");
        assert!(card.earnings().is_empty());
    }
}
