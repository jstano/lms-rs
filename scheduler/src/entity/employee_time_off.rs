//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeTimeOff`.
//!
//! Ground truth not re-read in full for this wave — field slice from Phase 0's read of
//! `engine/model/{WeeklyAvailableHours,Schedules}.java`: `toDateRange()` (`WeeklyAvailableHours`,
//! `Schedules.hasTimeOffOnDate`) and the raw start/end date-times (`Schedules.getLastTimeOffDate`).

use date_range_rs::{DateRange, DateTimeRange};
use joda_rs::LocalDateTime;

/// An approved time-off request. `EmployeeTimeOff`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeTimeOff {
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
}

impl EmployeeTimeOff {
    pub fn new(start_date_time: LocalDateTime, end_date_time: LocalDateTime) -> Self {
        Self {
            start_date_time,
            end_date_time,
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

    /// `toDateRange()`.
    pub fn to_date_range(&self) -> DateRange {
        DateRange::new(
            self.start_date_time.to_local_date(),
            self.end_date_time.to_local_date(),
        )
    }

    /// `toDateTimeRange()`.
    pub fn to_date_time_range(&self) -> DateTimeRange {
        DateTimeRange::of(self.start_date_time, self.end_date_time)
    }
}
