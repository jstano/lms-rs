//! Port of `com.unifocus.watson.server.hibernate.entity.SalaryDist`.
//!
//! A salaried employee's weekly pattern of hours: how many hours each weekday of
//! the week a salary is spread over. Only the id and the day values the
//! reconcile rules read come across.

use crate::common::numbers::round_raw_hours;

/// A weekly hours pattern. `SalaryDist`.
#[derive(Debug, Clone, PartialEq)]
pub struct SalaryDist {
    id: i32,
    day_hours: [f64; 7],
}

impl SalaryDist {
    /// Build a distribution from Sunday..Saturday hours.
    pub fn new(id: i32, day_hours: [f64; 7]) -> Self {
        Self { id, day_hours }
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// The sum of the seven days, rounded to the raw-hours precision.
    /// `getHoursPerWeek()`.
    pub fn hours_per_week(&self) -> f64 {
        round_raw_hours(self.day_hours.iter().sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weekly_total_is_the_sum_of_the_days() {
        assert_eq!(
            SalaryDist::new(1, [0.0, 4.0, 4.0, 4.0, 4.0, 4.0, 0.0]).hours_per_week(),
            20.0
        );
        assert_eq!(
            SalaryDist::new(2, [0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0]).hours_per_week(),
            5.0
        );
    }
}
