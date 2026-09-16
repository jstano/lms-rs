//! Port of `com.unifocus.watson.timeclock.common.PunchLogDTO`.
//!
//! A punch as the time clock reports it, before it has been matched to a shift.
//! The punch-validation rules take one of these and decide whether the clock
//! should accept it.
//!
//! Only what those rules read comes across: when the punch happened and what
//! kind it is.

use crate::common::enums::uftc_punch_type::UFTCPunchType;
use joda_rs::LocalDateTime;

/// A punch reported by a time clock. `PunchLogDTO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunchLog {
    punch_date_time: LocalDateTime,
    punch_type: UFTCPunchType,
}

impl PunchLog {
    /// Build a punch log entry.
    pub fn new(punch_date_time: LocalDateTime, punch_type: UFTCPunchType) -> Self {
        Self {
            punch_date_time,
            punch_type,
        }
    }

    /// When the punch happened. `getPunchDateTime()`.
    pub fn punch_date_time(&self) -> LocalDateTime {
        self.punch_date_time
    }

    /// What kind of punch it is. `getPunchType()`.
    pub fn punch_type(&self) -> UFTCPunchType {
        self.punch_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_punch_log_carries_its_time_and_type() {
        let log = PunchLog::new(LocalDateTime::of(2010, 1, 2, 12, 0, 0), UFTCPunchType::In);

        assert_eq!(
            log.punch_date_time(),
            LocalDateTime::of(2010, 1, 2, 12, 0, 0)
        );
        assert_eq!(log.punch_type(), UFTCPunchType::In);
    }
}
