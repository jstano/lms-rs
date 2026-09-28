//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeRegularPeriod`.
//!
//! Ground truth not read directly — only the fields `RegularSchedule`'s constructor reads.

use joda_rs::{DayOfWeek, LocalTime};

/// One weekly recurring work period for an employee. `EmployeeRegularPeriod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeRegularPeriod {
    employee_id: i32,
    job_id: Option<i32>,
    assignment_id: Option<i32>,
    day_of_week: DayOfWeek,
    start_time: LocalTime,
    end_time: LocalTime,
    duration: f64,
}

impl EmployeeRegularPeriod {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        employee_id: i32,
        job_id: Option<i32>,
        assignment_id: Option<i32>,
        day_of_week: DayOfWeek,
        start_time: LocalTime,
        end_time: LocalTime,
        duration: f64,
    ) -> Self {
        Self {
            employee_id,
            job_id,
            assignment_id,
            day_of_week,
            start_time,
            end_time,
            duration,
        }
    }

    /// `getEmployee().getID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getJob()` — `getID()`; `null` in Java when the period has no explicit job.
    pub fn job_id(&self) -> Option<i32> {
        self.job_id
    }

    /// `getAssignment().getID()` — `null` in Java when the period has no assignment.
    pub fn assignment_id(&self) -> Option<i32> {
        self.assignment_id
    }

    /// `getDayOfWeek()`.
    pub fn day_of_week(&self) -> DayOfWeek {
        self.day_of_week
    }

    /// `getStartTime()`.
    pub fn start_time(&self) -> LocalTime {
        self.start_time
    }

    /// `getEndTime()`.
    pub fn end_time(&self) -> LocalTime {
        self.end_time
    }

    /// `getDuration()`.
    pub fn duration(&self) -> f64 {
        self.duration
    }
}
