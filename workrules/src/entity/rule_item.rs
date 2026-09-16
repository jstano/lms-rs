//! Port of `com.unifocus.watson.server.hibernate.entity.RuleItem`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/RuleItem.java`.
//!
//! One configured rule inside a rule set: which [`RuleClass`] to run, and the
//! parameters to run it with. The most widely referenced entity in the engine —
//! 24 of the 32 rule families take one.
//!
//! The Java entity's `params` is a Hibernate `@ElementCollection` joined off
//! `RuleItemParam(RuleItemID, Name, Value)`, which is why every rule parameter
//! is a string regardless of what it represents.
//!
//! Not ported: the `Auditable` implementation (`findMasterAuditID`,
//! `findAuditResourceKey` and friends) — audit trails are outside the engine.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;

/// A configured rule within a rule set. `RuleItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleItem {
    id: i32,
    rule_set_id: i32,
    name: String,
    rule_class: RuleClass,
    params: RuleParams,
}

impl RuleItem {
    /// Build a rule item.
    ///
    /// Java's entity has a no-arg constructor and setters, because Hibernate
    /// needs them. Nothing in the engine mutates a rule item — rules only read
    /// them — so this takes everything up front.
    pub fn new(
        id: i32,
        rule_set_id: i32,
        name: impl Into<String>,
        rule_class: RuleClass,
        params: RuleParams,
    ) -> Self {
        Self {
            id,
            rule_set_id,
            name: name.into(),
            rule_class,
            params,
        }
    }

    /// `getID()`. A legacy integer database key, not a UUID.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// The id of the rule set holding this item.
    ///
    /// Java navigates to the whole `RuleSet` via `getRuleSet()`. Holding the id
    /// instead keeps the ownership one-way — a rule set owns its items — since
    /// there is no lazy-loading session here to resolve a cycle.
    pub fn rule_set_id(&self) -> i32 {
        self.rule_set_id
    }

    /// `getName()`. Rules that record what produced an earning use this.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Which algorithm to run. `getRuleClass()`.
    pub fn rule_class(&self) -> RuleClass {
        self.rule_class
    }

    /// The parameters to run it with. `getParams()`.
    pub fn params(&self) -> &RuleParams {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;
    use crate::rules::rule_type::RuleType;

    fn rule_item() -> RuleItem {
        RuleItem::new(
            10,
            1,
            "Round to 15",
            RuleClass::MinutePrr,
            rule_params! { "inPunchRoundTo" => "15" },
        )
    }

    #[test]
    fn a_rule_item_carries_its_class_and_parameters() {
        let item = rule_item();

        assert_eq!(item.id(), 10);
        assert_eq!(item.rule_set_id(), 1);
        assert_eq!(item.name(), "Round to 15");
        assert_eq!(item.rule_class(), RuleClass::MinutePrr);
        assert_eq!(item.params().int_at("inPunchRoundTo"), 15);
    }

    #[test]
    fn the_rule_class_reaches_its_rule_type() {
        assert_eq!(
            rule_item().rule_class().rule_type(),
            RuleType::PunchRounding
        );
    }

    #[test]
    fn a_rule_item_may_carry_no_parameters_at_all() {
        // Then the algorithm runs entirely on its config's defaults.
        let item = RuleItem::new(1, 1, "Defaults", RuleClass::MinutePrr, RuleParams::new());
        assert!(item.params().is_empty());
    }
}
