//! Port of
//! `com.unifocus.watson.server.labor.calcshift.rules.RuleItemPriorityComparator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/RuleItemPriorityComparator.java`.
//!
//! # This is not the general ordering mechanism
//!
//! It looks like one, and it is easy to assume every runner sorts its rule
//! items this way. **Only `PostPunchRuleRunner` calls it.** Every other runner
//! iterates `ruleSet.getRuleItems()` in whatever order Hibernate returned, and
//! reproducing Java means *not* sorting there either.
//!
//! It mostly does not matter, because seven of the 32 rule types have
//! `allowCategories == false` and so hold exactly one rule item — see
//! [`RuleType::allow_categories`](crate::rules::rule_type::RuleType::allow_categories).
//! For the rest, the Hibernate order is the behaviour.
//!
//! Java resolves each item's config through `RuleConfigFactory` and asks
//! `instanceof PriorityRuleConfig`. Here the config is looked up through the
//! same registry dispatch the engine uses elsewhere, and
//! [`RuleConfig::priority`] answers the same question without `instanceof`.
//!
//! [`RuleConfig::priority`]: crate::rules::rule_config::RuleConfig::priority

use crate::entity::rule_item::RuleItem;
use crate::rules::rule_config::{DEFAULT_PRIORITY, RuleConfig};

/// A rule item's execution order within its rule set.
///
/// `RuleItemPriorityComparator` reads this from the item's config: a config
/// that extends `PriorityRuleConfig` parses the `"priority"` parameter, and
/// everything else ranks at [`DEFAULT_PRIORITY`]. Lower sorts first.
pub fn priority_of(rule_item: &RuleItem, config: &dyn RuleConfig) -> i32 {
    config
        .priority(rule_item.params())
        .unwrap_or(DEFAULT_PRIORITY)
}

/// Sort rule items by priority, lowest first.
///
/// `PostPunchRuleRunner.sortRuleItemsByPriority`. `config_for` supplies each
/// item's config, standing in for Java's `RuleConfigFactory.createRuleConfig`.
///
/// The sort is **stable**, so items of equal priority keep their rule-set
/// order. Java's `Collections.sort` is stable too, and since ties are common —
/// everything that is not priority-configurable ranks at
/// [`DEFAULT_PRIORITY`] — that is what keeps the result predictable.
pub fn sort_by_priority<'a, F>(rule_items: &mut [&'a RuleItem], config_for: F)
where
    F: Fn(&RuleItem) -> &'a dyn RuleConfig,
{
    rule_items.sort_by_key(|item| priority_of(item, config_for(item)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use crate::rules::rule_config::{
        PRIORITY, ValidationResults, priority_default_values, priority_of as config_priority_of,
    };

    /// Stands in for a Java config that does not extend `PriorityRuleConfig`.
    struct PlainConfig;

    impl RuleConfig for PlainConfig {
        fn rule_class(&self) -> RuleClass {
            RuleClass::MinutePrr
        }
        fn default_values(&self) -> RuleParams {
            RuleParams::new()
        }
        fn validate(&self, _params: &RuleParams) -> ValidationResults {
            ValidationResults::new()
        }
    }

    /// Stands in for a Java `extends PriorityRuleConfig`.
    struct PriorityConfig;

    impl RuleConfig for PriorityConfig {
        fn rule_class(&self) -> RuleClass {
            RuleClass::ShortBreakPpr
        }
        fn default_values(&self) -> RuleParams {
            priority_default_values()
        }
        fn priority(&self, params: &RuleParams) -> Option<i32> {
            Some(config_priority_of(self, params))
        }
    }

    fn item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(
            id,
            1,
            format!("item {id}"),
            RuleClass::ShortBreakPpr,
            params,
        )
    }

    #[test]
    fn a_rule_without_a_priority_config_ranks_at_the_default() {
        let item = item(1, RuleParams::new());
        assert_eq!(priority_of(&item, &PlainConfig), DEFAULT_PRIORITY);
    }

    #[test]
    fn a_priority_config_with_no_parameter_also_ranks_at_the_default() {
        // Its own default_values supplies "5", so both paths agree.
        let item = item(1, RuleParams::new());
        assert_eq!(priority_of(&item, &PriorityConfig), 5);
    }

    #[test]
    fn a_configured_priority_is_read_from_the_parameters() {
        let item = item(1, rule_params! { PRIORITY => "2" });
        assert_eq!(priority_of(&item, &PriorityConfig), 2);
    }

    #[test]
    fn a_priority_parameter_is_ignored_when_the_config_is_not_priority_based() {
        // Java would not look at it either: the comparator checks the config
        // type, not the parameters.
        let item = item(1, rule_params! { PRIORITY => "1" });
        assert_eq!(priority_of(&item, &PlainConfig), DEFAULT_PRIORITY);
    }

    #[test]
    fn sorting_puts_the_lowest_priority_first() {
        let third = item(3, rule_params! { PRIORITY => "9" });
        let first = item(1, rule_params! { PRIORITY => "1" });
        let second = item(2, rule_params! { PRIORITY => "5" });
        let mut items = vec![&third, &first, &second];

        sort_by_priority(&mut items, |_| &PriorityConfig);

        let ids: Vec<_> = items.iter().map(|i| i.id()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn ties_keep_their_original_order() {
        // Every item ranks at 5, so the sort must not disturb them.
        let a = item(1, RuleParams::new());
        let b = item(2, RuleParams::new());
        let c = item(3, RuleParams::new());
        let mut items = vec![&c, &a, &b];

        sort_by_priority(&mut items, |_| &PlainConfig);

        let ids: Vec<_> = items.iter().map(|i| i.id()).collect();
        assert_eq!(ids, vec![3, 1, 2], "a stable sort must preserve tie order");
    }

    #[test]
    fn priority_and_non_priority_rules_interleave_around_the_default() {
        let urgent = item(1, rule_params! { PRIORITY => "1" });
        let plain = item(2, RuleParams::new());
        let late = item(3, rule_params! { PRIORITY => "9" });
        let mut items = vec![&late, &plain, &urgent];

        sort_by_priority(&mut items, |rule_item| {
            if rule_item.params().contains(PRIORITY) {
                &PriorityConfig
            } else {
                &PlainConfig
            }
        });

        let ids: Vec<_> = items.iter().map(|i| i.id()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn sorting_an_empty_or_single_list_is_a_no_op() {
        let mut empty: Vec<&RuleItem> = Vec::new();
        sort_by_priority(&mut empty, |_| &PlainConfig);
        assert!(empty.is_empty());

        let only = item(1, RuleParams::new());
        let mut one = vec![&only];
        sort_by_priority(&mut one, |_| &PlainConfig);
        assert_eq!(one[0].id(), 1);
    }
}
