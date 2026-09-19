//! Port of `com.unifocus.watson.server.hibernate.entity.Assignment`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Assignment.java`.
//!
//! A job. The Java entity is 1,133 lines with 17 `@OneToMany` relations — the
//! heaviest relation graph in the model — but rules read seven fields of it:
//! identity, its property, its code, and its parent in the labour hierarchy.
//! The scheduling and standards half is untouched.
//!
//! # Resolving the parent
//!
//! Java holds `parentAssignment` as a live reference and walks it with plain
//! field access. This crate's entity graph is one-way (see `entity`), so a
//! rule that needs the parent *object* — not just its id — resolves it through
//! [`AssignmentPort`](crate::rules::ports::AssignmentPort), the same port
//! [`effective_hourly_pay_rate`](Assignment::effective_hourly_pay_rate) uses to
//! walk the chain itself.

use crate::entity::assignment_pay_rate::AssignmentPayRate;
use crate::rules::ports::AssignmentPort;
use joda_rs::LocalDate;

/// A job. `Assignment`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    id: i32,
    property_id: i32,
    name: String,
    code: String,
    parent_assignment_id: Option<i32>,
    pay_rates: Vec<AssignmentPayRate>,
}

impl Assignment {
    /// Build a job.
    pub fn new(
        id: i32,
        property_id: i32,
        name: impl Into<String>,
        code: impl Into<String>,
        parent_assignment_id: Option<i32>,
    ) -> Self {
        Self {
            id,
            property_id,
            name: name.into(),
            code: code.into(),
            parent_assignment_id,
            pay_rates: Vec::new(),
        }
    }

    /// Attach the job's own dated pay rates. `setAssignmentPayRates()`.
    #[must_use]
    pub fn with_pay_rates(mut self, pay_rates: Vec<AssignmentPayRate>) -> Self {
        self.pay_rates = pay_rates;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getProperty().getID()`.
    ///
    /// Resolution needs this: the last fallback is the property's own rule set,
    /// reached in Java as `job.getProperty().getRuleSetForType(ruleType)`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getCode()`.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The parent in the labour hierarchy, if this is not a top-level job.
    /// `getParentAssignment().getID()`.
    pub fn parent_assignment_id(&self) -> Option<i32> {
        self.parent_assignment_id
    }

    /// The rate in force on a date, falling back up the parent-assignment
    /// chain when this job has none of its own. `getEffectiveHourlyPayRate(LocalDate)`.
    ///
    /// `0.0` where Java's `findEffectiveAssignmentPayRate` returns null — no
    /// rate on or before `effective_date` anywhere up the chain.
    pub fn effective_hourly_pay_rate(
        &self,
        effective_date: LocalDate,
        assignments: &dyn AssignmentPort,
    ) -> f64 {
        self.effective_pay_rate(effective_date, assignments)
            .map_or(0.0, |rate| rate.hourly_rate())
    }

    /// `findEffectiveAssignmentPayRate(LocalDate)`.
    fn effective_pay_rate(
        &self,
        effective_date: LocalDate,
        assignments: &dyn AssignmentPort,
    ) -> Option<AssignmentPayRate> {
        let own = self
            .pay_rates
            .iter()
            .filter(|rate| rate.effective_date().is_on_or_before(effective_date))
            .max_by_key(|rate| rate.effective_date())
            .copied();

        match own {
            Some(rate) => Some(rate),
            None => {
                let parent = assignments.find_by_id(self.parent_assignment_id?)?;
                parent.effective_pay_rate(effective_date, assignments)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_carries_its_property_and_code() {
        let job = Assignment::new(200, 11, "Front Desk", "FD", None);

        assert_eq!(job.id(), 200);
        assert_eq!(job.property_id(), 11);
        assert_eq!(job.name(), "Front Desk");
        assert_eq!(job.code(), "FD");
        assert_eq!(job.parent_assignment_id(), None);
    }

    #[test]
    fn a_job_may_sit_under_a_parent() {
        let job = Assignment::new(201, 11, "Night Audit", "NA", Some(200));
        assert_eq!(job.parent_assignment_id(), Some(200));
    }

    struct NoAssignments;
    impl AssignmentPort for NoAssignments {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }

    struct OneAssignment(Assignment);
    impl AssignmentPort for OneAssignment {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            (self.0.id() == id).then(|| self.0.clone())
        }
    }

    #[test]
    fn the_latest_rate_on_or_before_the_date_wins() {
        let job = Assignment::new(200, 11, "Front Desk", "FD", None).with_pay_rates(vec![
            AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5),
            AssignmentPayRate::new(LocalDate::of(2010, 1, 1), 8.0),
        ]);

        assert_eq!(
            job.effective_hourly_pay_rate(LocalDate::of(2005, 1, 1), &NoAssignments),
            6.5
        );
        assert_eq!(
            job.effective_hourly_pay_rate(LocalDate::of(2020, 1, 1), &NoAssignments),
            8.0
        );
    }

    #[test]
    fn a_job_with_no_rate_of_its_own_falls_back_to_the_parent() {
        let parent = Assignment::new(100, 11, "Department", "DEPT", None)
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let job = Assignment::new(200, 11, "Front Desk", "FD", Some(100));

        assert_eq!(
            job.effective_hourly_pay_rate(LocalDate::of(2010, 1, 1), &OneAssignment(parent)),
            6.5
        );
    }

    #[test]
    fn no_rate_anywhere_in_the_chain_is_zero() {
        let job = Assignment::new(200, 11, "Front Desk", "FD", None);
        assert_eq!(
            job.effective_hourly_pay_rate(LocalDate::of(2010, 1, 1), &NoAssignments),
            0.0
        );
    }

    #[test]
    fn a_rate_effective_after_the_date_does_not_count() {
        let job = Assignment::new(200, 11, "Front Desk", "FD", None)
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(2020, 1, 1), 8.0)]);

        assert_eq!(
            job.effective_hourly_pay_rate(LocalDate::of(2010, 1, 1), &NoAssignments),
            0.0
        );
    }
}
