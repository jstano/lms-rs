//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeTimeOff`.
//!
//! Ground truth not re-read in full for this wave — field slice from Phase 0's read of
//! `engine/model/{WeeklyAvailableHours,Schedules}.java`: `toDateRange()` (`WeeklyAvailableHours`,
//! `Schedules.hasTimeOffOnDate`) and the raw start/end date-times (`Schedules.getLastTimeOffDate`).
//! `is_full_day` was added for `ScheduleChecker` (Phase 3's `autosched` wave, its first real
//! reader) — a `with_*` builder field defaulting to `false`, following this crate's convention for
//! fields most existing callers never set.

use date_range_rs::{DateRange, DateTimeRange};
use joda_rs::LocalDateTime;

/// An approved time-off request. `EmployeeTimeOff`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeTimeOff {
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
    is_full_day: bool,
}

impl EmployeeTimeOff {
    pub fn new(start_date_time: LocalDateTime, end_date_time: LocalDateTime) -> Self {
        Self {
            start_date_time,
            end_date_time,
            is_full_day: false,
        }
    }

    /// `setFullDay(boolean)`.
    #[must_use]
    pub fn with_full_day(mut self, is_full_day: bool) -> Self {
        self.is_full_day = is_full_day;
        self
    }

    /// `isFullDay()`.
    pub fn is_full_day(&self) -> bool {
        self.is_full_day
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
