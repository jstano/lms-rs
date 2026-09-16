//! Port of `com.unifocus.watson.server.hibernate.entity.RuleSet` and
//! `com.unifocus.watson.server.labor.rules.{RuleSetSource, SourcedRuleSet}`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/RuleSet.java`
//! and `taps/src/java/com/unifocus/watson/server/labor/rules/`.
//!
//! A rule set is what gets configured against an employee, an employee set, or
//! a property: a [`RuleType`], a salience used to pick between competing sets,
//! and the [`RuleItem`]s to run.

use crate::entity::rule_item::RuleItem;
use crate::rules::rule_type::RuleType;

/// Where a rule set was found during resolution. `RuleSetSource`.
///
/// Resolution walks these in order — an employee-level set wins outright, then
/// the employee's sets by salience, then the property's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleSetSource {
    /// Configured directly against the employee.
    Employee,
    /// Configured against an employee set the employee belongs to.
    EmployeeSet,
    /// The property-level default.
    Property,
}

/// A configured set of rules. `RuleSet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    id: i32,
    property_id: i32,
    name: String,
    description: String,
    rule_type: RuleType,
    req_approval: bool,
    salience: i32,
    rule_items: Vec<RuleItem>,
}

impl RuleSet {
    /// Build a rule set.
    pub fn new(
        id: i32,
        property_id: i32,
        name: impl Into<String>,
        rule_type: RuleType,
        salience: i32,
        rule_items: Vec<RuleItem>,
    ) -> Self {
        Self {
            id,
            property_id,
            name: name.into(),
            description: String::new(),
            rule_type,
            req_approval: false,
            salience,
            rule_items,
        }
    }

    /// Set the description and approval flag, neither of which the engine reads.
    ///
    /// Kept apart from [`new`](Self::new) so the common case — building a rule
    /// set to run — does not have to supply two fields no algorithm consults.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>, req_approval: bool) -> Self {
        self.description = description.into();
        self.req_approval = req_approval;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getProperty().getID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getDescription()`.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Which category of rule this set configures. `getRuleType()`.
    pub fn rule_type(&self) -> RuleType {
        self.rule_type
    }

    /// `getReqApproval()`.
    pub fn req_approval(&self) -> bool {
        self.req_approval
    }

    /// How strongly this set claims the employee. `getSalience()`.
    ///
    /// Only meaningful between employee-set-level sets, where the highest wins.
    pub fn salience(&self) -> i32 {
        self.salience
    }

    /// The rules in this set, in the order Hibernate returned them.
    /// `getRuleItems()`.
    ///
    /// **This order is load-bearing.** Only `PostPunchRuleRunner` sorts by
    /// priority; every other runner iterates as-is.
    pub fn rule_items(&self) -> &[RuleItem] {
        &self.rule_items
    }

    /// Is this set empty of rules?
    ///
    /// Runners check this: `PunchRoundingRunner` falls back to a hardcoded
    /// default rule when a set exists but has no items.
    pub fn is_empty(&self) -> bool {
        self.rule_items.is_empty()
    }
}

/// A rule set together with where resolution found it. `SourcedRuleSet`.
///
/// Java models this as a subclass of `RuleSet` that copies every field across;
/// composition says the same thing without the copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcedRuleSet {
    rule_set: RuleSet,
    source: RuleSetSource,
    set_name: String,
}

impl SourcedRuleSet {
    /// Tag a rule set with where it was found.
    ///
    /// `set_name` is the employee set's name when `source` is
    /// [`RuleSetSource::EmployeeSet`], and empty otherwise — which is what Java
    /// passes.
    pub fn new(rule_set: RuleSet, source: RuleSetSource, set_name: impl Into<String>) -> Self {
        Self {
            rule_set,
            source,
            set_name: set_name.into(),
        }
    }

    /// The rule set itself.
    pub fn rule_set(&self) -> &RuleSet {
        &self.rule_set
    }

    /// Where it was found. `getRuleSetSource()`.
    pub fn source(&self) -> RuleSetSource {
        self.source
    }

    /// The employee set's name, when it came from one. `getSetName()`.
    pub fn set_name(&self) -> &str {
        &self.set_name
    }

    /// Unwrap to the rule set, discarding the provenance.
    pub fn into_rule_set(self) -> RuleSet {
        self.rule_set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    fn item(id: i32, rule_class: RuleClass) -> RuleItem {
        RuleItem::new(id, 1, format!("item {id}"), rule_class, RuleParams::new())
    }

    fn rule_set(items: Vec<RuleItem>) -> RuleSet {
        RuleSet::new(1, 11, "Rounding", RuleType::PunchRounding, 100, items)
    }

    #[test]
    fn a_rule_set_carries_its_type_salience_and_items() {
        let set = rule_set(vec![item(10, RuleClass::MinutePrr)]);

        assert_eq!(set.id(), 1);
        assert_eq!(set.property_id(), 11);
        assert_eq!(set.name(), "Rounding");
        assert_eq!(set.rule_type(), RuleType::PunchRounding);
        assert_eq!(set.salience(), 100);
        assert_eq!(set.rule_items().len(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn a_rule_set_can_exist_with_no_rules_in_it() {
        // Runners distinguish this from "no rule set at all" — both fall back
        // to a default rule, but only after checking both conditions.
        let set = rule_set(Vec::new());
        assert!(set.is_empty());
    }

    #[test]
    fn rule_items_keep_the_order_they_were_given() {
        let set = rule_set(vec![
            item(10, RuleClass::MinutePrr),
            item(20, RuleClass::WorkedHoursPrr),
        ]);

        let ids: Vec<_> = set.rule_items().iter().map(RuleItem::id).collect();
        assert_eq!(ids, vec![10, 20]);
    }

    #[test]
    fn description_and_approval_default_to_empty_and_false() {
        let set = rule_set(Vec::new());
        assert_eq!(set.description(), "");
        assert!(!set.req_approval());
    }

    #[test]
    fn description_and_approval_can_be_attached() {
        let set = rule_set(Vec::new()).with_description("Property default", true);
        assert_eq!(set.description(), "Property default");
        assert!(set.req_approval());
    }

    #[test]
    fn a_sourced_rule_set_remembers_where_it_came_from() {
        let sourced = SourcedRuleSet::new(
            rule_set(Vec::new()),
            RuleSetSource::EmployeeSet,
            "Night shift",
        );

        assert_eq!(sourced.source(), RuleSetSource::EmployeeSet);
        assert_eq!(sourced.set_name(), "Night shift");
        assert_eq!(sourced.rule_set().rule_type(), RuleType::PunchRounding);
    }

    #[test]
    fn an_employee_level_set_carries_no_set_name() {
        // Java passes "" for anything that did not come from an employee set.
        let sourced = SourcedRuleSet::new(rule_set(Vec::new()), RuleSetSource::Employee, "");
        assert_eq!(sourced.set_name(), "");
    }

    #[test]
    fn a_sourced_rule_set_unwraps_to_its_rule_set() {
        let set = rule_set(vec![item(10, RuleClass::MinutePrr)]);
        let sourced = SourcedRuleSet::new(set.clone(), RuleSetSource::Property, "");
        assert_eq!(sourced.into_rule_set(), set);
    }

    #[test]
    fn rule_items_carry_their_own_parameters() {
        let set = rule_set(vec![RuleItem::new(
            10,
            1,
            "Round to 5",
            RuleClass::MinutePrr,
            rule_params! { "inPunchRoundTo" => "5" },
        )]);

        assert_eq!(set.rule_items()[0].params().int_at("inPunchRoundTo"), 5);
    }
}
