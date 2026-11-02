//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeStatus`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeStatus.java`.
//!
//! One interval of an employee's employment: active, on leave, rehired,
//! terminated, transferred. Only the fields `Employee.isActiveOnDate` reads
//! come across — the reason, note and audit columns do not.

use crate::common::enums::employee_status_type::EmployeeStatusType;
use joda_rs::LocalDate;

/// An employment interval. `EmployeeStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmployeeStatus {
    start_date: LocalDate,
    end_date: LocalDate,
    status_type: EmployeeStatusType,
}

impl EmployeeStatus {
    /// Build a status covering `start_date..=end_date`.
    pub fn new(
        start_date: LocalDate,
        end_date: LocalDate,
        status_type: EmployeeStatusType,
    ) -> Self {
        Self {
            start_date,
            end_date,
            status_type,
        }
    }

    /// `getStartDate()`.
    pub fn start_date(&self) -> LocalDate {
        self.start_date
    }

    /// `getEndDate()`.
    pub fn end_date(&self) -> LocalDate {
        self.end_date
    }

    /// `getStatusType()`.
    pub fn status_type(&self) -> EmployeeStatusType {
        self.status_type
    }

    /// Does this interval cover `date`, both ends included?
    pub fn contains_date(&self, date: LocalDate) -> bool {
        self.start_date <= date && self.end_date >= date
    }
}
