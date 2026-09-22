//! Port of `com.unifocus.watson.server.scheduler.autosched.AvailPeriod`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/autosched/AvailPeriod.java`.
//! Only the fields `EmployeeData.getEffectiveAvailableHoursForDate` and
//! `EmployeeAvailabilityChecker` read are modeled.

use date_range_rs::DateTimeRange;
use joda_rs::LocalDateTime;

/// A block of time an employee is (or isn't) available to work. `AvailPeriod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvailPeriod {
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
    duration: f64,
}

impl AvailPeriod {
    pub fn new(
        start_date_time: LocalDateTime,
        end_date_time: LocalDateTime,
        duration: f64,
    ) -> Self {
        Self {
            start_date_time,
            end_date_time,
            duration,
        }
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> LocalDateTime {
        self.start_date_time
    }

    /// `getEndDateTime()`.
    pub fn end_date_time(&self) -> LocalDateTime {
        self.end_date_time
    }

    /// `getDuration()`.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// `toDateTimeRange()`.
    pub fn to_date_time_range(&self) -> DateTimeRange {
        DateTimeRange::of(self.start_date_time, self.end_date_time)
    }
}
