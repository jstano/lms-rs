//! Port of `RegHrsOnlyRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/RegHrsOnlyRuleImpl.java`.
//!
//! `REG_ONLY_HDR`, the family's default rule, and **a genuine no-op**:
//!
//! ```java
//! public void execute(TimeCard timeCard, LegacyDatePeriod workWeek, RuleItem ruleItem) {}
//! ```
//!
//! That is the whole body. Regular hours are already distributed onto the
//! shifts by the calc pipeline before any hours-distribution rule runs; this
//! rule is what a property configures when it wants none of them moved into
//! overtime or double time. Doing nothing is the correct behaviour, not a
//! missing implementation — do not go looking for a body to port.
//!
//! It is also the fallback a runner reaches for when a rule set configures no
//! hours-distribution rule at all, which is why it exists as a catalogue entry
//! rather than as an absence.

use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use date_range_rs::DateRange;

/// Leave every distributed hour where it is. `RegHrsOnlyRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegHrsOnlyRule;

impl HoursDistributionRule for RegHrsOnlyRule {
    fn execute(
        &self,
        _time_card: &mut dyn TimeCard,
        _work_week: &DateRange,
        _rule_item: &RuleItem,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    #[test]
    fn the_default_rule_moves_nothing() {
        let date = LocalDate::of(2010, 1, 4);
        let mut card = TimeCardData::new().with_shifts(vec![
            EmployeeShift::new(1, 100, 200, date, ShiftType::Actual, Vec::new())
                .with_hours_distributions(vec![HoursDistribution::new(
                    11,
                    date,
                    Some(HoursDistributionType::REGULAR_ID),
                    12.0,
                    10.0,
                )]),
        ]);
        let before = card.shifts()[0].hours_distributions().to_vec();

        RegHrsOnlyRule.execute(
            &mut card,
            &DateRange::new(LocalDate::of(2010, 1, 3), LocalDate::of(2010, 1, 9)),
            &RuleItem::new(
                1,
                1,
                "Regular hours only",
                RuleClass::RegOnlyHdr,
                RuleParams::new(),
            ),
        );

        assert_eq!(card.shifts()[0].hours_distributions(), before.as_slice());
    }
}
