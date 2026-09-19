//! Port of `com.unifocus.watson.server.hibernate.entity.ShiftCategory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/ShiftCategory.java`.
//!
//! A per-property tag a shift may carry — "double shift", "training", and the
//! like. `ShiftCategoryRegRateRuleImpl` and `ShiftCategoryMinWageRegRateRuleImpl`
//! are the first rules to read one, and they only ever compare its id against a
//! configured list, so [`EmployeeShift`](crate::entity::employee_shift::EmployeeShift)
//! carries the id alone rather than a reference to this entity — the same
//! shape as `job_id`. The Java entity's productivity/training/contract flags
//! and its property back-reference are not read by any ported rule.

/// A shift tag, configured per property. `ShiftCategory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShiftCategory {
    id: i32,
    property_id: i32,
    name: String,
}

impl ShiftCategory {
    /// Build a shift category.
    pub fn new(id: i32, property_id: i32, name: impl Into<String>) -> Self {
        Self {
            id,
            property_id,
            name: name.into(),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shift_category_carries_its_id_and_name() {
        let category = ShiftCategory::new(4, 11, "Double Shift");
        assert_eq!(category.id(), 4);
        assert_eq!(category.property_id(), 11);
        assert_eq!(category.name(), "Double Shift");
    }
}
