//! Port of `com.unifocus.watson.server.hibernate.entity.Property`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Property.java`.
//!
//! A site. Rules reach it mainly for two things: the period end date that
//! `AbstractRunner` builds its work weeks from, and the property-level rule
//! sets that resolution falls back to when neither the employee nor any
//! employee set has one.

use crate::entity::rule_set::RuleSet;
use crate::rules::rule_type::RuleType;
use joda_rs::LocalDate;

/// A site. `Property`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    id: i32,
    name: String,
    period_end_date: LocalDate,
    week_end_day: i32,
    rule_sets: Vec<RuleSet>,
}

impl Property {
    /// Build a property with its default rule sets.
    pub fn new(
        id: i32,
        name: impl Into<String>,
        period_end_date: LocalDate,
        week_end_day: i32,
        rule_sets: Vec<RuleSet>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            period_end_date,
            week_end_day,
            rule_sets,
        }
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The end of the current pay period. `getPeriodEndDate()`.
    ///
    /// `AbstractRunner.runRulesForEachWeekAgainst` anchors its `WeeklyDateRange`
    /// on this.
    pub fn period_end_date(&self) -> LocalDate {
        self.period_end_date
    }

    /// Which day the week ends on. `getWeekEndDay()`.
    ///
    /// Stored in the engine's own Sun=1..Sat=7 convention, not ISO — Java has a
    /// standing comment about the mismatch. Use
    /// [`translate_dow_to_iso`](crate::common::dates::translate_dow_to_iso)
    /// before handing it to anything date-aware.
    pub fn week_end_day(&self) -> i32 {
        self.week_end_day
    }

    /// The property-level default rule set for a type, if configured.
    /// `getRuleSetForType(RuleType)` — the last step of resolution.
    pub fn rule_set_for_type(&self, rule_type: RuleType) -> Option<&RuleSet> {
        self.rule_sets
            .iter()
            .find(|rule_set| rule_set.rule_type() == rule_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property(rule_sets: Vec<RuleSet>) -> Property {
        Property::new(11, "Downtown", LocalDate::of(2010, 1, 15), 1, rule_sets)
    }

    #[test]
    fn a_property_carries_its_period_end_date() {
        let p = property(Vec::new());
        assert_eq!(p.id(), 11);
        assert_eq!(p.name(), "Downtown");
        assert_eq!(p.period_end_date(), LocalDate::of(2010, 1, 15));
        assert_eq!(p.week_end_day(), 1);
    }

    #[test]
    fn a_rule_set_is_found_by_its_type() {
        let p = property(vec![RuleSet::new(
            1,
            11,
            "Rounding",
            RuleType::PunchRounding,
            0,
            Vec::new(),
        )]);

        assert!(p.rule_set_for_type(RuleType::PunchRounding).is_some());
        assert!(p.rule_set_for_type(RuleType::BenefitAccrual).is_none());
    }

    #[test]
    fn the_first_rule_set_of_a_type_wins() {
        // Java returns on the first match; salience is not consulted here.
        let p = property(vec![
            RuleSet::new(1, 11, "First", RuleType::PunchRounding, 0, Vec::new()),
            RuleSet::new(2, 11, "Second", RuleType::PunchRounding, 999, Vec::new()),
        ]);

        assert_eq!(
            p.rule_set_for_type(RuleType::PunchRounding).unwrap().name(),
            "First"
        );
    }
}
