//! Port of
//! `com.unifocus.watson.server.labor.rules.algorithm.utility.HoursDistributionFactory`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/utility/HoursDistributionFactory.java`.
//!
//! How every distribution family builds a new [`HoursDistribution`]. A static
//! factory in Java, and free functions here.
//!
//! # A premium distribution has zero original hours
//!
//! `createPremiumDistribution` inherits the date, property and **base rate** of
//! the regular distribution it is derived from, and then sets
//! `originalHours` to **0** while `hours` carries the premium amount:
//!
//! ```java
//! return createDistribution(regularBaseDistribution.getDate(), regularBaseDistribution.getPropertyID(),
//!                           hoursDistributionTypeId, hours,
//!                           0, premiumRate, regularBaseDistribution.getBaseRate(),
//!                           hoursRuleItemID, rateRuleItemID);
//! ```
//!
//! That is not an oversight. `originalHours` is what the accumulators sum when
//! measuring a week against its limits, so premium hours have to read as zero
//! there or every overtime hour would be counted twice — once as the regular
//! hour it came from and once as itself. The constructor
//! [`HoursDistribution::new`] sets `original_hours` equal to `hours`, which is
//! right for a fresh regular distribution and wrong here, so this builds the
//! struct field by field.
//!
//! # Not ported yet
//!
//! `createSplitDistributions` and `createDistributionOfConfiguredType`, which
//! the day-split families use. They need `BreaksAndAdjustmentsCalculator` and
//! `SingleDistributionTypeRuleConfig`, neither of which any ported rule
//! touches. They arrive with the family that calls them.

use crate::entity::hours_distribution::HoursDistribution;
use joda_rs::LocalDate;

/// Build a distribution with every field spelled out. `createDistribution`.
#[allow(clippy::too_many_arguments)]
pub fn create_distribution(
    distribution_date: LocalDate,
    property_id: i32,
    hours_distribution_type_id: i32,
    hours: f64,
    original_hours: f64,
    premium_rate: f64,
    base_rate: f64,
    hours_rule_item_id: Option<i32>,
    rate_rule_item_id: Option<i32>,
) -> HoursDistribution {
    let mut distribution = HoursDistribution::new(
        property_id,
        distribution_date,
        Some(hours_distribution_type_id),
        hours,
        base_rate,
    );
    distribution.set_original_hours(original_hours);
    distribution.set_premium_rate(premium_rate);
    distribution.set_hours_rule_item_id(hours_rule_item_id);
    distribution.set_rate_rule_item_id(rate_rule_item_id);
    distribution
}

/// Derive a premium distribution from the regular one it is taken out of.
/// `createPremiumDistribution(HoursDistribution, int, double, Integer)` — the
/// four-argument overload, which is the one the rules call; it fixes the
/// premium rate at zero and leaves the rate rule item unset, for a rate family
/// to fill in later.
pub fn create_premium_distribution(
    regular_distribution: &HoursDistribution,
    hours_distribution_type_id: i32,
    hours: f64,
    hours_rule_item_id: Option<i32>,
) -> HoursDistribution {
    create_premium_distribution_at_rate(
        regular_distribution,
        hours,
        0.0,
        hours_distribution_type_id,
        hours_rule_item_id,
        None,
    )
}

/// The six-argument `createPremiumDistribution`, which also sets a premium
/// rate and a rate rule item.
pub fn create_premium_distribution_at_rate(
    regular_base_distribution: &HoursDistribution,
    hours: f64,
    premium_rate: f64,
    hours_distribution_type_id: i32,
    hours_rule_item_id: Option<i32>,
    rate_rule_item_id: Option<i32>,
) -> HoursDistribution {
    create_distribution(
        regular_base_distribution.date(),
        regular_base_distribution.property_id(),
        hours_distribution_type_id,
        hours,
        // Zero, deliberately — see the module note.
        0.0,
        premium_rate,
        regular_base_distribution.base_rate(),
        hours_rule_item_id,
        rate_rule_item_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::hours_distribution_type::HoursDistributionType;

    fn regular() -> HoursDistribution {
        HoursDistribution::new(
            11,
            LocalDate::of(2010, 1, 4),
            Some(HoursDistributionType::REGULAR_ID),
            8.0,
            10.0,
        )
    }

    #[test]
    fn a_premium_distribution_inherits_date_property_and_base_rate() {
        let premium = create_premium_distribution(
            &regular(),
            HoursDistributionType::OVERTIME_ID,
            2.0,
            Some(5),
        );

        assert_eq!(premium.date(), LocalDate::of(2010, 1, 4));
        assert_eq!(premium.property_id(), 11);
        assert_eq!(premium.base_rate(), 10.0);
        assert_eq!(
            premium.hours_distribution_type_id(),
            Some(HoursDistributionType::OVERTIME_ID)
        );
        assert_eq!(premium.hours(), 2.0);
    }

    #[test]
    fn a_premium_distribution_has_zero_original_hours() {
        // What keeps the accumulators from counting an overtime hour twice.
        let premium = create_premium_distribution(
            &regular(),
            HoursDistributionType::OVERTIME_ID,
            2.0,
            Some(5),
        );

        assert_eq!(premium.original_hours(), 0.0);
        assert_eq!(premium.hours(), 2.0, "while the hours are the premium ones");
    }

    #[test]
    fn the_four_argument_form_sets_no_premium_rate_or_rate_rule_item() {
        let premium = create_premium_distribution(&regular(), 3, 2.0, Some(5));

        assert_eq!(premium.premium_rate(), 0.0);
        assert_eq!(premium.hours_rule_item_id(), Some(5));
        assert_eq!(
            premium.rate_rule_item_id(),
            None,
            "a rate family fills this in later"
        );
    }

    #[test]
    fn the_cost_follows_from_the_inherited_base_rate() {
        let premium =
            create_premium_distribution_at_rate(&regular(), 2.0, 5.0, 2, Some(5), Some(6));

        assert_eq!(premium.premium_rate(), 5.0);
        assert_eq!(premium.total_costs(), 2.0 * (10.0 + 5.0));
        assert_eq!(premium.rate_rule_item_id(), Some(6));
    }

    #[test]
    fn a_distribution_built_outright_keeps_the_original_hours_it_is_given() {
        let distribution = create_distribution(
            LocalDate::of(2010, 1, 4),
            11,
            HoursDistributionType::REGULAR_ID,
            6.0,
            8.0,
            0.0,
            10.0,
            Some(5),
            None,
        );

        assert_eq!(distribution.hours(), 6.0);
        assert_eq!(distribution.original_hours(), 8.0);
        assert_eq!(distribution.total_costs(), 60.0);
    }
}
