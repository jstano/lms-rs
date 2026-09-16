//! Port of the rule-set resolution in
//! `com.unifocus.watson.server.labor.rules.RuleUtils`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/RuleUtils.java`.
//!
//! Every runner starts here: given an employee, a job, a date and a
//! [`RuleType`], which rule set applies? Three levels, in order:
//!
//! 1. **Employee.** A rule set configured directly against the employee wins
//!    outright — the other levels are not even consulted.
//! 2. **Employee sets.** Otherwise, every employee set the employee qualifies
//!    for in this job on this date is examined, and the highest salience wins.
//! 3. **Property.** Failing both, the property's default for the type.
//!
//! Finding nothing is normal: runners fall back to a hardcoded default rule.
//!
//! # Two resolutions that disagree on ties
//!
//! `getRuleSet` and `getSourcedRuleSets` are not the same function with
//! different return types. On a salience tie between employee sets:
//!
//! * `getRuleSet` keeps the **first** it saw — its test is `>`, so an equal
//!   salience never displaces the incumbent;
//! * `getSourcedRuleSets` collects **all** of them — its test is `==` to append
//!   and `>` to reset.
//!
//! Both are ported, and the difference is preserved.

use crate::entity::assignment::Assignment;
use crate::entity::employee::Employee;
use crate::entity::property::Property;
use crate::entity::rule_set::{RuleSet, RuleSetSource, SourcedRuleSet};
use crate::rules::rule_type::RuleType;
use joda_rs::LocalDate;

/// Supplies the employee sets an employee qualifies for.
///
/// Stands in for `RuleUtils.getEmployeeSetsForEmployeeAndJob`, which reaches
/// `EmployeeSetDAO` and the Hibernate session — the one part of resolution that
/// is I/O rather than logic. Keeping it behind a trait leaves the cascade itself
/// pure and testable.
///
/// Implementations return each qualifying set's name paired with its rule set
/// for the type, already filtered — `None` rule sets are simply not returned.
pub trait EmployeeSetProvider {
    /// The employee sets that cover this employee, in this job, on this date,
    /// and have a rule set of this type.
    fn employee_set_rule_sets(
        &self,
        employee_id: i32,
        job_id: i32,
        date: LocalDate,
        rule_type: RuleType,
    ) -> Vec<(&str, &RuleSet)>;
}

/// No employee sets at all.
///
/// The common shape in tests, and correct for a site that configures rules only
/// at the employee and property levels.
pub struct NoEmployeeSets;

impl EmployeeSetProvider for NoEmployeeSets {
    fn employee_set_rule_sets(
        &self,
        _employee_id: i32,
        _job_id: i32,
        _date: LocalDate,
        _rule_type: RuleType,
    ) -> Vec<(&str, &RuleSet)> {
        Vec::new()
    }
}

/// The rule set that applies. `RuleUtils.getRuleSet`.
///
/// On a salience tie between employee sets the **first** encountered wins, as
/// Java's `>` comparison does.
///
/// Java iterates a `HashSet<EmployeeSet>`, so its tie-break is not actually
/// deterministic between runs; here the order is whatever the provider returns,
/// which at least makes it reproducible. Sites that hit this are relying on
/// unspecified behaviour either way.
pub fn resolve_rule_set<'a>(
    employee: &'a Employee,
    job: &Assignment,
    property: &'a Property,
    date: LocalDate,
    rule_type: RuleType,
    employee_sets: &'a dyn EmployeeSetProvider,
) -> Option<&'a RuleSet> {
    if let Some(rule_set) = employee.rule_set_for_type(rule_type) {
        return Some(rule_set);
    }

    let candidates = employee_sets.employee_set_rule_sets(employee.id(), job.id(), date, rule_type);

    let mut winner: Option<&RuleSet> = None;
    for (_, rule_set) in candidates {
        match winner {
            None => winner = Some(rule_set),
            // Strictly greater: a tie leaves the incumbent in place.
            Some(current) if rule_set.salience() > current.salience() => winner = Some(rule_set),
            Some(_) => {}
        }
    }

    if winner.is_some() {
        return winner;
    }

    property.rule_set_for_type(rule_type)
}

/// Every rule set that applies, tagged with where it came from.
/// `RuleUtils.getSourcedRuleSets`.
///
/// Unlike [`resolve_rule_set`], a salience tie between employee sets yields
/// **all** the tied sets rather than one. The employee and property levels
/// always yield at most one.
pub fn resolve_sourced_rule_sets(
    employee: &Employee,
    job: &Assignment,
    property: &Property,
    date: LocalDate,
    rule_type: RuleType,
    employee_sets: &dyn EmployeeSetProvider,
) -> Vec<SourcedRuleSet> {
    if let Some(rule_set) = employee.rule_set_for_type(rule_type) {
        return vec![SourcedRuleSet::new(
            rule_set.clone(),
            RuleSetSource::Employee,
            "",
        )];
    }

    let candidates = employee_sets.employee_set_rule_sets(employee.id(), job.id(), date, rule_type);

    let mut best: Option<i32> = None;
    let mut winners: Vec<SourcedRuleSet> = Vec::new();

    for (set_name, rule_set) in candidates {
        let sourced =
            || SourcedRuleSet::new(rule_set.clone(), RuleSetSource::EmployeeSet, set_name);

        match best {
            None => {
                best = Some(rule_set.salience());
                winners.push(sourced());
            }
            Some(current) if rule_set.salience() == current => winners.push(sourced()),
            Some(current) if rule_set.salience() > current => {
                best = Some(rule_set.salience());
                winners = vec![sourced()];
            }
            Some(_) => {}
        }
    }

    if !winners.is_empty() {
        return winners;
    }

    property
        .rule_set_for_type(rule_type)
        .map(|rule_set| {
            vec![SourcedRuleSet::new(
                rule_set.clone(),
                RuleSetSource::Property,
                "",
            )]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee_job_status::EmployeeJobStatus;

    /// Employee sets supplied directly, in the order given.
    struct FixedEmployeeSets(Vec<(String, RuleSet)>);

    impl EmployeeSetProvider for FixedEmployeeSets {
        fn employee_set_rule_sets(
            &self,
            _employee_id: i32,
            _job_id: i32,
            _date: LocalDate,
            rule_type: RuleType,
        ) -> Vec<(&str, &RuleSet)> {
            self.0
                .iter()
                .filter(|(_, rule_set)| rule_set.rule_type() == rule_type)
                .map(|(name, rule_set)| (name.as_str(), rule_set))
                .collect()
        }
    }

    fn rule_set(id: i32, name: &str, salience: i32) -> RuleSet {
        RuleSet::new(id, 11, name, RuleType::PunchRounding, salience, Vec::new())
    }

    fn date() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn job() -> Assignment {
        Assignment::new(200, 11, "Front Desk", "FD", None)
    }

    fn employee(rule_sets: Vec<RuleSet>) -> Employee {
        Employee::new(
            100,
            11,
            "Alex Kim",
            vec![EmployeeJobStatus::new(
                1,
                100,
                200,
                LocalDate::of(2010, 1, 1),
                LocalDate::of(2010, 12, 31),
                EmployeePayType::Hourly,
                12.50,
                true,
            )],
        )
        .with_rule_sets(rule_sets)
    }

    fn property(rule_sets: Vec<RuleSet>) -> Property {
        Property::new(11, "Downtown", LocalDate::of(2010, 1, 15), 1, rule_sets)
    }

    #[test]
    fn an_employee_level_rule_set_wins_outright() {
        let employee = employee(vec![rule_set(1, "Employee", 0)]);
        let property = property(vec![rule_set(3, "Property", 999)]);
        let sets = FixedEmployeeSets(vec![("Night".into(), rule_set(2, "Set", 999))]);

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(resolved.unwrap().name(), "Employee");
    }

    #[test]
    fn an_employee_set_beats_the_property() {
        let employee = employee(Vec::new());
        let property = property(vec![rule_set(3, "Property", 999)]);
        let sets = FixedEmployeeSets(vec![("Night".into(), rule_set(2, "Set", 0))]);

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(
            resolved.unwrap().name(),
            "Set",
            "salience does not compete across levels"
        );
    }

    #[test]
    fn the_property_is_the_last_resort() {
        let employee = employee(Vec::new());
        let property = property(vec![rule_set(3, "Property", 0)]);

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &NoEmployeeSets,
        );

        assert_eq!(resolved.unwrap().name(), "Property");
    }

    #[test]
    fn nothing_configured_resolves_to_nothing() {
        // Normal, not an error: the runner falls back to a default rule.
        let employee = employee(Vec::new());
        let property = property(Vec::new());

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &NoEmployeeSets,
        );

        assert!(resolved.is_none());
    }

    #[test]
    fn a_rule_set_of_another_type_is_not_resolved() {
        let employee = employee(vec![RuleSet::new(
            1,
            11,
            "Accruals",
            RuleType::BenefitAccrual,
            0,
            Vec::new(),
        )]);

        let property = property(Vec::new());
        assert!(
            resolve_rule_set(
                &employee,
                &job(),
                &property,
                date(),
                RuleType::PunchRounding,
                &NoEmployeeSets
            )
            .is_none()
        );
    }

    #[test]
    fn the_highest_salience_employee_set_wins() {
        let sets = FixedEmployeeSets(vec![
            ("Low".into(), rule_set(1, "Low", 10)),
            ("High".into(), rule_set(2, "High", 100)),
            ("Mid".into(), rule_set(3, "Mid", 50)),
        ]);

        let employee = employee(Vec::new());
        let property = property(Vec::new());

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(resolved.unwrap().name(), "High");
    }

    #[test]
    fn a_salience_tie_keeps_the_first_seen() {
        // Java's test is `>`, so an equal salience never displaces.
        let sets = FixedEmployeeSets(vec![
            ("First".into(), rule_set(1, "First", 50)),
            ("Second".into(), rule_set(2, "Second", 50)),
        ]);

        let employee = employee(Vec::new());
        let property = property(Vec::new());

        let resolved = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(resolved.unwrap().name(), "First");
    }

    #[test]
    fn the_sourced_form_collects_every_tied_set() {
        // Where resolve_rule_set keeps one, this keeps both.
        let sets = FixedEmployeeSets(vec![
            ("First".into(), rule_set(1, "First", 50)),
            ("Second".into(), rule_set(2, "Second", 50)),
            ("Loser".into(), rule_set(3, "Loser", 10)),
        ]);

        let resolved = resolve_sourced_rule_sets(
            &employee(Vec::new()),
            &job(),
            &property(Vec::new()),
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].set_name(), "First");
        assert_eq!(resolved[1].set_name(), "Second");
        assert!(
            resolved
                .iter()
                .all(|s| s.source() == RuleSetSource::EmployeeSet)
        );
    }

    #[test]
    fn a_later_higher_salience_resets_the_collected_sets() {
        let sets = FixedEmployeeSets(vec![
            ("Low".into(), rule_set(1, "Low", 10)),
            ("High".into(), rule_set(2, "High", 100)),
            ("AlsoHigh".into(), rule_set(3, "AlsoHigh", 100)),
        ]);

        let resolved = resolve_sourced_rule_sets(
            &employee(Vec::new()),
            &job(),
            &property(Vec::new()),
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].set_name(), "High");
        assert_eq!(resolved[1].set_name(), "AlsoHigh");
    }

    #[test]
    fn the_sourced_form_tags_the_employee_level() {
        let resolved = resolve_sourced_rule_sets(
            &employee(vec![rule_set(1, "Employee", 0)]),
            &job(),
            &property(vec![rule_set(3, "Property", 0)]),
            date(),
            RuleType::PunchRounding,
            &NoEmployeeSets,
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].source(), RuleSetSource::Employee);
        assert_eq!(
            resolved[0].set_name(),
            "",
            "no set name above the set level"
        );
    }

    #[test]
    fn the_sourced_form_tags_the_property_level() {
        let resolved = resolve_sourced_rule_sets(
            &employee(Vec::new()),
            &job(),
            &property(vec![rule_set(3, "Property", 0)]),
            date(),
            RuleType::PunchRounding,
            &NoEmployeeSets,
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].source(), RuleSetSource::Property);
    }

    #[test]
    fn the_sourced_form_returns_nothing_when_nothing_is_configured() {
        assert!(
            resolve_sourced_rule_sets(
                &employee(Vec::new()),
                &job(),
                &property(Vec::new()),
                date(),
                RuleType::PunchRounding,
                &NoEmployeeSets,
            )
            .is_empty()
        );
    }

    #[test]
    fn both_forms_agree_when_there_is_no_tie() {
        let sets = FixedEmployeeSets(vec![
            ("Low".into(), rule_set(1, "Low", 10)),
            ("High".into(), rule_set(2, "High", 100)),
        ]);
        let employee = employee(Vec::new());
        let property = property(Vec::new());

        let one = resolve_rule_set(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );
        let many = resolve_sourced_rule_sets(
            &employee,
            &job(),
            &property,
            date(),
            RuleType::PunchRounding,
            &sets,
        );

        assert_eq!(many.len(), 1);
        assert_eq!(one.unwrap().name(), many[0].rule_set().name());
    }
}
