//! Port of `com.unifocus.watson.server.labor.punchloader.TempPunch`.
//!
//! A punch that has arrived from a clock but is not yet part of a shift. The
//! meal-punch rules read only who made it, in which job, and when; the rest of
//! the punch loader's bookkeeping does not come across.

use crate::entity::employee::Employee;
use joda_rs::LocalDateTime;

/// A punch awaiting placement. `TempPunch`.
#[derive(Debug, Clone, PartialEq)]
pub struct TempPunch {
    employee: Employee,
    job_id: i32,
    punch_date_time: LocalDateTime,
}

impl TempPunch {
    /// Build a punch.
    pub fn new(employee: Employee, job_id: i32, punch_date_time: LocalDateTime) -> Self {
        Self {
            employee,
            job_id,
            punch_date_time,
        }
    }

    /// `getEmployee()`.
    pub fn employee(&self) -> &Employee {
        &self.employee
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getPunchDateTime()`.
    pub fn punch_date_time(&self) -> LocalDateTime {
        self.punch_date_time
    }
}
