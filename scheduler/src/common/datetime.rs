//! Port of the one `com.unifocus.tbx.core.DTUtil` method `scheduler` calls so far:
//! `durationInFractionalHours(LocalDateTime, LocalDateTime)`, used by
//! `EmployeeMinHoursOffChecker` to measure the gap between two shifts.

use joda_rs::LocalDateTime;

/// `DTUtil.durationInFractionalHours(LocalDateTime, LocalDateTime)`.
pub fn duration_in_fractional_hours(from: LocalDateTime, to: LocalDateTime) -> f64 {
    (to - from).fractional_hours()
}
