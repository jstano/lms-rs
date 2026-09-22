//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeAssignment`.
//!
//! Ground truth not read directly — only the fields `AssignmentOrderComparator`/
//! `AssignmentRankComparator` read.

/// One employee's standing on one assignment. `EmployeeAssignment`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeAssignment {
    assignment_id: i32,
    order_no: i32,
    rank: i32,
    is_active: bool,
}

impl EmployeeAssignment {
    pub fn new(assignment_id: i32, order_no: i32, rank: i32, is_active: bool) -> Self {
        Self {
            assignment_id,
            order_no,
            rank,
            is_active,
        }
    }

    /// The assignment this standing is for — `Employee.getAssignment(int)` looks this collection
    /// up by it.
    pub fn assignment_id(&self) -> i32 {
        self.assignment_id
    }

    /// `getOrderNo()`.
    pub fn order_no(&self) -> i32 {
        self.order_no
    }

    /// `getRank()`.
    pub fn rank(&self) -> i32 {
        self.rank
    }

    /// `isActive()`.
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}
