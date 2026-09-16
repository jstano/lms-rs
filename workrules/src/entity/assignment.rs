//! Port of `com.unifocus.watson.server.hibernate.entity.Assignment`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Assignment.java`.
//!
//! A job. The Java entity is 1,133 lines with 17 `@OneToMany` relations — the
//! heaviest relation graph in the model — but rules read seven fields of it:
//! identity, its property, its code, and its parent in the labour hierarchy.
//! The scheduling and standards half is untouched.

/// A job. `Assignment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    id: i32,
    property_id: i32,
    name: String,
    code: String,
    parent_assignment_id: Option<i32>,
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
        }
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
}
