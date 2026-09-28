//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeShiftError`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/
//! EmployeeShiftError.java`. Only `errorType` is modeled — `id`/`employeeShift`/`property` are
//! all either database plumbing or a back-reference to the `EmployeeShift` that now owns this
//! error directly (`EmployeeShift::errors`), so storing them here would be self-referential for
//! no ported reader.

use crate::entity::shift_error_type::ShiftErrorType;

/// `EmployeeShiftError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmployeeShiftError {
    error_type: ShiftErrorType,
}

impl EmployeeShiftError {
    pub fn new(error_type: ShiftErrorType) -> Self {
        Self { error_type }
    }

    /// `getErrorType()`.
    pub fn error_type(&self) -> ShiftErrorType {
        self.error_type
    }
}
