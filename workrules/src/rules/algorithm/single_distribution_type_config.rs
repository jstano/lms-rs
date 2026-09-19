//! Port of
//! `com.unifocus.watson.common.labor.rules.algorithm.SingleDistributionTypeRuleConfig`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/SingleDistributionTypeRuleConfig.java`.
//!
//! The shared base for the `regularhoursdistribution` configs — one parameter,
//! which distribution bucket a rule writes into. As with
//! [`RuleConfig`](crate::rules::rule_config::RuleConfig)'s other mixins
//! ([`priority_default_values`](crate::rules::rule_config::priority_default_values)),
//! Java's `extends` becomes a constant plus free functions a concrete config
//! calls from its own [`RuleConfig`](crate::rules::rule_config::RuleConfig)
//! impl. The l2fprod `getProperties` half is configuration UI and does not
//! come across, matching every other config in this crate.

use crate::rule_params;
use crate::rules::params::RuleParams;
use crate::rules::rule_config::ValidationResults;

/// Which distribution bucket the rule writes into.
/// `SingleDistributionTypeRuleConfig.HOURS_DISTRIBUTION_TYPE_ID`.
pub const HOURS_DISTRIBUTION_TYPE_ID: &str = "hoursDistributionTypeID";

/// Defaults to the regular bucket. `getDefaultValues()`.
pub fn single_distribution_type_default_values() -> RuleParams {
    rule_params! { HOURS_DISTRIBUTION_TYPE_ID => "1" }
}

/// The configured type must be a real bucket id. `validateProperties()`.
///
/// Java reads the value off an l2fprod `Property` and treats a missing or
/// non-numeric one as `null`; here an unparseable value already fails to
/// parse as `RuleParams::int_at`'s caller expects, so the only case left to
/// check is a value below 1.
pub fn single_distribution_type_validate(params: &RuleParams) -> ValidationResults {
    let hours_distribution_type_id = params.try_int_at(HOURS_DISTRIBUTION_TYPE_ID);
    match hours_distribution_type_id {
        Some(id) if id >= 1 => ValidationResults::new(),
        _ => vec![
            "Hours distribution type ID value is incorrect. Use Regular as a default value"
                .to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_regular_bucket() {
        assert_eq!(
            single_distribution_type_default_values().int_at(HOURS_DISTRIBUTION_TYPE_ID),
            1
        );
    }

    #[test]
    fn a_configured_type_of_one_or_more_is_valid() {
        let params = rule_params! { HOURS_DISTRIBUTION_TYPE_ID => "3" };
        assert!(single_distribution_type_validate(&params).is_empty());
    }

    #[test]
    fn a_type_below_one_is_invalid() {
        let params = rule_params! { HOURS_DISTRIBUTION_TYPE_ID => "0" };
        assert_eq!(single_distribution_type_validate(&params).len(), 1);
    }

    #[test]
    fn a_missing_type_is_invalid() {
        assert_eq!(
            single_distribution_type_validate(&RuleParams::new()).len(),
            1
        );
    }
}
