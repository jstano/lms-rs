//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeAvailability`.
//!
//! One weekly availability row: on this weekday, between these times, the
//! employee is available, must be off, would prefer off or is rotated off.

use crate::common::enums::employee_avail_type::EmployeeAvailType;
use joda_rs::LocalTime;

/// An availability row. `EmployeeAvailability`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeAvailability {
    iso_day_of_week: i32,
    start_time: LocalTime,
    end_time: LocalTime,
    request_type: EmployeeAvailType,
}

impl EmployeeAvailability {
    /// Build a row. `iso_day_of_week` is 1 = Monday .. 7 = Sunday
    /// (`DayOfWeek.getISODayNo()`).
    pub fn new(
        iso_day_of_week: i32,
        start_time: LocalTime,
        end_time: LocalTime,
        request_type: EmployeeAvailType,
    ) -> Self {
        Self {
            iso_day_of_week,
            start_time,
            end_time,
            request_type,
        }
    }

    /// `getDayOfWeek().getISODayNo()`.
    pub fn iso_day_of_week(&self) -> i32 {
        self.iso_day_of_week
    }

    /// `getRequestType()`.
    pub fn request_type(&self) -> EmployeeAvailType {
        self.request_type
    }

    /// Midnight to midnight. `isAllDay()`.
    pub fn is_all_day(&self) -> bool {
        let midnight = LocalTime::of(0, 0, 0);
        self.start_time == midnight && self.end_time == midnight
    }
}
