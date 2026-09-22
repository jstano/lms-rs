//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeJobStatusChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeJobStatusChecker.java`. `Employee.isActiveOnDate` needs `EmployeeStatus`/
//! `EmployeeStatusType`, not modeled (see `entity::employee`'s doc) — resolved through
//! [`EmployeeActiveOnDatePort`], same "stub the deferred half" treatment as
//! `EmployeeOvertimeChecker`. `isActiveJobOnDate` **is** real: it's just
//! `getEmployeeJobStatus(job, date) != null`, already grounded.

use crate::entity::employee::Employee;
use joda_rs::LocalDate;

/// `Employee.isActiveOnDate(LocalDate)`.
pub trait EmployeeActiveOnDatePort {
    fn is_active_on_date(&self, employee: &Employee, date: LocalDate) -> bool;
}

/// `EmployeeJobStatusChecker`.
pub struct EmployeeJobStatusChecker<'a> {
    active: &'a dyn EmployeeActiveOnDatePort,
}

impl<'a> EmployeeJobStatusChecker<'a> {
    pub fn new(active: &'a dyn EmployeeActiveOnDatePort) -> Self {
        Self { active }
    }

    /// `canEmployeeWorkJobOnDate(Employee, Assignment, LocalDate)`. `job_id` is `None` where Java
    /// passes a `null` job — the check is skipped, matching `job != null && ...`.
    pub fn can_employee_work_job_on_date(
        &self,
        employee: &Employee,
        job_id: Option<i32>,
        date: LocalDate,
    ) -> bool {
        if !self.active.is_active_on_date(employee, date) {
            return false;
        }

        if let Some(job_id) = job_id
            && employee.employee_job_status(job_id, date).is_none()
        {
            return false;
        }

        true
    }
}
