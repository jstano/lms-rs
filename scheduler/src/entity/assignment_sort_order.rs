//! Port of `com.unifocus.watson.server.hibernate.entity.AssignmentSortOrder`.
//!
//! Ground truth not read directly — inferred entirely from `EmployeeSeniorityComparator`'s usage
//! (`assignmentSortOrder.getType()`), which is the only field the engine reads off it so far.

use crate::entity::jc_sort_order_type::JcSortOrderType;

/// One entry in a job's ordered seniority tie-break chain. `AssignmentSortOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignmentSortOrder {
    sort_type: JcSortOrderType,
}

impl AssignmentSortOrder {
    pub fn new(sort_type: JcSortOrderType) -> Self {
        Self { sort_type }
    }

    /// `getType()`.
    pub fn sort_type(&self) -> JcSortOrderType {
        self.sort_type
    }
}
