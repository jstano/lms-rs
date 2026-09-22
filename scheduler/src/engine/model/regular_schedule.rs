//! Port of `com.unifocus.watson.server.scheduler.engine.model.RegularSchedule`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! RegularSchedule.java`. One weekly recurring work period for an employee; see `DATA_MODEL.md`
//! §4.
//!
//! `employee_data`/`job(shift_date)` take their `Employee`/id lookups as parameters rather than
//! an owned/live reference, the same divergence as `WeeklyAvailableHours` — see that module's doc
//! for why.

use crate::entity::employee::Employee;
use crate::entity::employee_regular_period::EmployeeRegularPeriod;
use date_range_rs::DateTimeRange;
use joda_rs::{DayOfWeek, LocalDate};

/// `RegularSchedule`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegularSchedule {
    employee_id: i32,
    job_id: Option<i32>,
    assignment_id: i32,
    day_of_week: DayOfWeek,
    start_time: joda_rs::LocalTime,
    end_time: joda_rs::LocalTime,
    duration: f64,
}

impl RegularSchedule {
    pub fn new(employee_id: i32, employee_regular_period: &EmployeeRegularPeriod) -> Self {
        Self {
            employee_id,
            job_id: employee_regular_period.job_id(),
            assignment_id: employee_regular_period.assignment_id(),
            day_of_week: employee_regular_period.day_of_week(),
            start_time: employee_regular_period.start_time(),
            end_time: employee_regular_period.end_time(),
            duration: employee_regular_period.duration(),
        }
    }

    /// `getEmployeeData()` / `getEmployee()` — id only, see module doc.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getJob(LocalDate)` — falls back to the employee's home job status on `shift_date` when
    /// the period itself has no explicit job.
    pub fn job(&self, shift_date: LocalDate, employee: &Employee) -> Option<i32> {
        self.job_id.or_else(|| {
            employee
                .home_employee_job_status(shift_date)
                .map(|s| s.job_id())
        })
    }

    /// `getAssignment()`.
    pub fn assignment_id(&self) -> i32 {
        self.assignment_id
    }

    /// `getDayOfWeek()`.
    pub fn day_of_week(&self) -> DayOfWeek {
        self.day_of_week
    }

    /// `getDateTimeRange(LocalDate)`.
    pub fn date_time_range(&self, shift_date: LocalDate) -> DateTimeRange {
        DateTimeRange::from_time_range_on_date(self.start_time, self.end_time, shift_date)
    }

    /// `getDuration()`.
    pub fn duration(&self) -> f64 {
        self.duration
    }
}
