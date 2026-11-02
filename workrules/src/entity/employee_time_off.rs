//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeTimeOff`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeTimeOff.java`.
//!
//! A request for time away. Only what the `schedulelabel` rules read comes
//! across: the status, the span, whether it is a full-day request, and the
//! type's id. The approval trail, the distributions and the notes do not.

use crate::common::enums::time_off_status::TimeOffStatus;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime};

/// A time-off request. `EmployeeTimeOff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeTimeOff {
    start: LocalDateTime,
    end: LocalDateTime,
    status: TimeOffStatus,
    time_off_type_id: i32,
    full_day: bool,
}

impl EmployeeTimeOff {
    /// Build a request (not full-day).
    pub fn new(
        start: LocalDateTime,
        end: LocalDateTime,
        status: TimeOffStatus,
        time_off_type_id: i32,
    ) -> Self {
        Self {
            start,
            end,
            status,
            time_off_type_id,
            full_day: false,
        }
    }

    /// Make this a full-day request: its range then runs to the **end of the
    /// last day**. `setFullDay()`.
    #[must_use]
    pub fn with_full_day(mut self, full_day: bool) -> Self {
        self.full_day = full_day;
        self
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> LocalDateTime {
        self.start
    }

    /// `getEndDateTime()`.
    pub fn end_date_time(&self) -> LocalDateTime {
        self.end
    }

    /// `getStatus()`.
    pub fn status(&self) -> TimeOffStatus {
        self.status
    }

    /// `getTimeOffType().getID()`.
    pub fn time_off_type_id(&self) -> i32 {
        self.time_off_type_id
    }

    /// Does the request's **calendar-date span** include `date`? The date parts
    /// of the start and end, both inclusive.
    pub fn covers_date(&self, date: LocalDate) -> bool {
        self.start.to_local_date() <= date && date <= self.end.to_local_date()
    }

    /// `toDateTimeRange()`: start to end, or for a full-day request start to
    /// the end of the last day (`end + 1 day`).
    pub fn to_date_time_range(&self) -> DateTimeRange {
        if self.full_day {
            DateTimeRange::of(self.start, self.end.plus_days(1))
        } else {
            DateTimeRange::of(self.start, self.end)
        }
    }
}
