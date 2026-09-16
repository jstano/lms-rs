//! Port of `com.unifocus.watson.server.labor.rules.RuleImplFactory` and
//! `com.unifocus.watson.common.labor.rules.RuleConfigFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/RuleImplFactory.java`.
//!
//! Java resolves a [`RuleClass`] to its algorithm by string surgery and a
//! Spring bean lookup:
//!
//! ```java
//! String className = ruleClass.getRuleConfigClass().getSimpleName(); // MinuteRoundingRuleConfig
//! className = className.replace("Config", "Impl");                   // MinuteRoundingRuleImpl
//! className = Introspector.decapitalize(className);                  // minuteRoundingRuleImpl
//! return (T)applicationContext.getBean(className);
//! ```
//!
//! There is no registry and no type safety: the factory returns a raw
//! `RuleImpl`, the caller casts it to whatever its family's interface is, and a
//! missing bean or a mismatched cast is a runtime failure. It works only
//! because every `*RuleConfig` has a sibling `*RuleImpl` in the mirrored
//! package with Spring's default bean name.
//!
//! Here dispatch is a `match` on the enum, per family. A family's registry maps
//! only the rule classes of its own [`RuleType`], so the cast disappears: the
//! returned trait object is already the family's trait.
//!
//! This module holds the shape; each family supplies its own registry as it is
//! ported.

use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;

/// Resolves the rule classes of one family to their algorithms.
///
/// `R` is the family's own rule trait — the thing Java's caller casts the raw
/// `RuleImpl` to.
pub trait RuleRegistry<R: ?Sized> {
    /// Which family this registry covers.
    fn rule_type(&self) -> RuleType;

    /// The algorithm for a rule class, or `None` if it does not belong to this
    /// family or is not ported yet.
    ///
    /// Java throws a `RuntimeException` wrapping whatever Spring raised. An
    /// `Option` says the same thing without deciding how the caller should
    /// react — a runner that finds nothing can fall back to its default rule,
    /// which is behaviour Java's runners already have.
    fn rule_for(&self, rule_class: RuleClass) -> Option<&R>;

    /// Every rule class this registry can dispatch.
    ///
    /// Used to check a family's coverage against
    /// [`RuleClass::of_type`], so a rule class that exists in the catalogue but
    /// has no implementation is visible rather than silently skipped.
    fn dispatchable(&self) -> Vec<RuleClass> {
        RuleClass::of_type(self.rule_type())
            .into_iter()
            .filter(|rule_class| self.rule_for(*rule_class).is_some())
            .collect()
    }

    /// The rule classes of this family that have no implementation yet.
    fn missing(&self) -> Vec<RuleClass> {
        RuleClass::of_type(self.rule_type())
            .into_iter()
            .filter(|rule_class| self.rule_for(*rule_class).is_none())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Rounder {
        fn name(&self) -> &'static str;
    }

    struct Minute;
    impl Rounder for Minute {
        fn name(&self) -> &'static str {
            "minute"
        }
    }

    struct WorkedHours;
    impl Rounder for WorkedHours {
        fn name(&self) -> &'static str {
            "worked hours"
        }
    }

    /// A partial registry — two of the six punch-rounding rules.
    struct PartialRounding {
        minute: Minute,
        worked_hours: WorkedHours,
    }

    impl RuleRegistry<dyn Rounder> for PartialRounding {
        fn rule_type(&self) -> RuleType {
            RuleType::PunchRounding
        }

        fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn Rounder + 'static)> {
            match rule_class {
                RuleClass::MinutePrr => Some(&self.minute),
                RuleClass::WorkedHoursPrr => Some(&self.worked_hours),
                _ => None,
            }
        }
    }

    fn registry() -> PartialRounding {
        PartialRounding {
            minute: Minute,
            worked_hours: WorkedHours,
        }
    }

    #[test]
    fn a_rule_class_dispatches_to_its_algorithm() {
        assert_eq!(
            registry().rule_for(RuleClass::MinutePrr).unwrap().name(),
            "minute"
        );
        assert_eq!(
            registry()
                .rule_for(RuleClass::WorkedHoursPrr)
                .unwrap()
                .name(),
            "worked hours"
        );
    }

    #[test]
    fn an_unported_rule_class_dispatches_to_nothing() {
        assert!(registry().rule_for(RuleClass::BackGracePrr).is_none());
    }

    #[test]
    fn a_rule_class_from_another_family_dispatches_to_nothing() {
        assert!(registry().rule_for(RuleClass::ShortBreakPpr).is_none());
    }

    #[test]
    fn coverage_reports_what_is_ported_and_what_is_missing() {
        let registry = registry();

        assert_eq!(registry.dispatchable().len(), 2);
        assert_eq!(registry.missing().len(), 4);
        assert_eq!(
            registry.dispatchable().len() + registry.missing().len(),
            RuleClass::of_type(RuleType::PunchRounding).len()
        );
    }

    #[test]
    fn missing_names_the_rule_classes_still_to_port() {
        assert!(registry().missing().contains(&RuleClass::PropertyDataPrr));
        assert!(!registry().missing().contains(&RuleClass::MinutePrr));
    }
}
