//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeShiftRequestDetail`.
//!
//! Ground truth not read directly — only the one field `EmployeeSeniorityDateComparator` reads:
//! the trade-to employee's seniority date. `Employee` itself isn't nested here for the same
//! reason `EmployeeJobStatus.job` isn't a full `Assignment` — see `entity` module docs.

use joda_rs::LocalDate;

/// A shift-trade request between two employees. `EmployeeShiftRequestDetail`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeShiftRequestDetail {
    trade_to_employee_seniority_date: LocalDate,
}

impl EmployeeShiftRequestDetail {
    pub fn new(trade_to_employee_seniority_date: LocalDate) -> Self {
        Self {
            trade_to_employee_seniority_date,
        }
    }

    /// `getTradeToEmployee().getSeniorityDate()`.
    pub fn trade_to_employee_seniority_date(&self) -> LocalDate {
        self.trade_to_employee_seniority_date
    }
}
