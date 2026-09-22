//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! EmployeeSeniorityDateComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! EmployeeSeniorityDateComparator.java`. Java's `Comparator<EmployeeShiftRequestDetail>`.

use crate::entity::employee_shift_request_detail::EmployeeShiftRequestDetail;
use std::cmp::Ordering;

/// `EmployeeSeniorityDateComparator`.
pub struct EmployeeSeniorityDateComparator;

impl EmployeeSeniorityDateComparator {
    /// `compare(EmployeeShiftRequestDetail, EmployeeShiftRequestDetail)`.
    pub fn compare(
        &self,
        detail1: &EmployeeShiftRequestDetail,
        detail2: &EmployeeShiftRequestDetail,
    ) -> Ordering {
        detail1
            .trade_to_employee_seniority_date()
            .cmp(&detail2.trade_to_employee_seniority_date())
    }
}
