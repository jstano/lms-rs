//! Port of `ClearTimeCardEarnings`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ClearTimeCardEarnings.java`.
//!
//! Shared by the differential runners: before a family re-runs it removes
//! what its previous run wrote, so a recalculation is idempotent. An earning
//! is removed when **all** hold:
//!
//! - it is open for editing;
//! - it was written by a rule (`source == RULE`) whose rule item belongs to one
//!   of the given rule types;
//! - it has no shift, or its shift is also open for editing.
//!
//! Removed earnings leave the card and are deleted through the earning port.
//!
//! # Two things the Rust entities cannot say (divergences 92 and 93)
//!
//! An `EmployeeEarning` carries only its rule item's **id**, so the caller
//! supplies `rule_type_of_item` for what Java reads as
//! `earning.getRuleItem().getRuleClass().getRuleType()`. And an earning with
//! id 0 has never been persisted: it is dropped from the card without a delete.
//! An earning whose shift is not on the card counts as open.

use crate::common::enums::earning_source::EarningSource;
use crate::entity::time_card::TimeCard;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_type::RuleType;

/// `clearEarnings(TimeCard, List<RuleType>)`.
pub fn clear_time_card_earnings(
    time_card: &mut dyn TimeCard,
    earnings: &mut dyn EmployeeEarningPort,
    rule_types: &[RuleType],
    rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
) {
    let doomed: Vec<usize> = time_card
        .earnings()
        .iter()
        .enumerate()
        .filter(|(_, earning)| {
            time_card.is_open_for_editing_for_earning(earning)
                && earning.source() == EarningSource::Rule
                && earning
                    .rule_item_id()
                    .and_then(rule_type_of_item)
                    .is_some_and(|t| rule_types.contains(&t))
                && earning.shift_id().is_none_or(|shift_id| {
                    time_card
                        .shifts()
                        .iter()
                        .find(|s| s.id() == shift_id)
                        .is_none_or(|s| time_card.is_open_for_editing_for_shift(s))
                })
        })
        .map(|(i, _)| i)
        .collect();

    let mut removed = Vec::with_capacity(doomed.len());
    for index in doomed.into_iter().rev() {
        removed.push(time_card.earnings_mut().remove(index));
    }
    for earning in removed.into_iter().rev() {
        if earning.id() != 0 {
            earnings.remove(earning.id());
        }
    }
}
