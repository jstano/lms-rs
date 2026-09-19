//! Port of `com.unifocus.watson.server.hibernate.entity.AssignmentPayRate`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/AssignmentPayRate.java`.
//!
//! One dated rate on a job: `Assignment.getEffectiveHourlyPayRate` picks the
//! latest of these on or before a date, walking up the parent-assignment chain
//! when the job itself has none. The Java entity also carries a tip rate and a
//! minimum wage, effective the same way — neither is read by any ported rule
//! yet, so only the hourly rate comes across.

use joda_rs::LocalDate;

/// A job's hourly rate as of some date. `AssignmentPayRate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssignmentPayRate {
    effective_date: LocalDate,
    hourly_rate: f64,
}

impl AssignmentPayRate {
    /// Build a pay rate.
    pub fn new(effective_date: LocalDate, hourly_rate: f64) -> Self {
        Self {
            effective_date,
            hourly_rate,
        }
    }

    /// `getEffectiveDate()`.
    pub fn effective_date(&self) -> LocalDate {
        self.effective_date
    }

    /// `getHourlyRate()`.
    pub fn hourly_rate(&self) -> f64 {
        self.hourly_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pay_rate_carries_its_date_and_rate() {
        let rate = AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5);
        assert_eq!(rate.effective_date(), LocalDate::of(1999, 1, 1));
        assert_eq!(rate.hourly_rate(), 6.5);
    }
}
