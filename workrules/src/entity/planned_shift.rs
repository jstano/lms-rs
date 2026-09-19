//! Port of `com.unifocus.watson.server.hibernate.entity.PlannedShift`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/PlannedShift.java`.
//!
//! The schedule an actual shift was built from. The Java entity is large —
//! job, property, shift type, work-content weighting — but `schedulelunch`'s
//! rules, the only readers so far, touch exactly two fields: how long the
//! shift was scheduled for, and when it was scheduled to start. Only those
//! come across.

use joda_rs::LocalDateTime;

/// The schedule an actual shift came from. `PlannedShift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedShift {
    start_date_time: LocalDateTime,
    duration: f64,
}

impl PlannedShift {
    /// Build a planned shift.
    pub fn new(start_date_time: LocalDateTime, duration: f64) -> Self {
        Self {
            start_date_time,
            duration,
        }
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> LocalDateTime {
        self.start_date_time
    }

    /// The scheduled length, in hours. `getDuration()`.
    pub fn duration(&self) -> f64 {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalDate;

    #[test]
    fn a_planned_shift_carries_its_start_and_duration() {
        let start = LocalDate::of(2014, 5, 6).at_time(joda_rs::LocalTime::of(0, 0, 0));
        let planned = PlannedShift::new(start, 8.0);

        assert_eq!(planned.start_date_time(), start);
        assert_eq!(planned.duration(), 8.0);
    }
}
