//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeStatus`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeStatus.java`.
//! Only `startDate`/`endDate`/`statusType` are modeled — `id`/`employee`/`statusReason`/`note`
//! aren't read by any ported call site (`Employee::status_for_date`, its first real caller).

use crate::entity::employee_status_type::EmployeeStatusType;
use joda_rs::LocalDate;

/// `EmployeeStatus`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeStatus {
    start_date: LocalDate,
    end_date: LocalDate,
    status_type: EmployeeStatusType,
}

impl EmployeeStatus {
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

    /// `startDate.isOnOrBefore(date) && endDate.isOnOrAfter(date)` — the inline range check
    /// `Employee.getStatusForDate` uses instead of calling `getDateRange().containsDate(date)`.
    pub fn contains_date(&self, date: LocalDate) -> bool {
        self.start_date.is_on_or_before(date) && self.end_date.is_on_or_after(date)
    }
}
