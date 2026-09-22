//! Port of `com.unifocus.watson.server.scheduler.engine.model.HoursByDate`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/HoursByDate.java`.
//! A plain date→hours accumulator; see `DATA_MODEL.md` §4.

use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// `HoursByDate`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HoursByDate {
    hours_by_date: HashMap<LocalDate, f64>,
}

impl HoursByDate {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getHoursForDate(LocalDate)` — `0.0` for a date with no entry.
    pub fn hours_for_date(&self, date: LocalDate) -> f64 {
        self.hours_by_date.get(&date).copied().unwrap_or(0.0)
    }

    /// `setHoursForDate(LocalDate, double)`.
    pub fn set_hours_for_date(&mut self, date: LocalDate, hours: f64) {
        self.hours_by_date.insert(date, hours);
    }

    /// `addHoursToDate(LocalDate, double)`.
    pub fn add_hours_to_date(&mut self, date: LocalDate, hours: f64) {
        let current = self.hours_by_date.get(&date).copied();
        self.hours_by_date
            .insert(date, current.unwrap_or(0.0) + hours);
    }

    /// `subtractHoursFromDate(LocalDate, double)`.
    pub fn subtract_hours_from_date(&mut self, date: LocalDate, hours: f64) {
        match self.hours_by_date.get(&date).copied() {
            Some(current) => {
                self.hours_by_date.insert(date, current - hours);
            }
            None => {
                self.hours_by_date.insert(date, -hours);
            }
        }
    }

    /// `sumHoursForDateRange(DateRange)`.
    pub fn sum_hours_for_date_range(&self, date_range: &DateRange) -> f64 {
        date_range
            .dates()
            .into_iter()
            .map(|date| self.hours_by_date.get(&date).copied().unwrap_or(0.0))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_with_no_entry_has_zero_hours() {
        let hours = HoursByDate::new();
        assert_eq!(hours.hours_for_date(LocalDate::of(2024, 1, 1)), 0.0);
    }

    #[test]
    fn add_hours_accumulates() {
        let mut hours = HoursByDate::new();
        let date = LocalDate::of(2024, 1, 1);
        hours.add_hours_to_date(date, 4.0);
        hours.add_hours_to_date(date, 3.5);
        assert_eq!(hours.hours_for_date(date), 7.5);
    }

    #[test]
    fn subtract_hours_from_an_empty_date_goes_negative() {
        let mut hours = HoursByDate::new();
        let date = LocalDate::of(2024, 1, 1);
        hours.subtract_hours_from_date(date, 2.0);
        assert_eq!(hours.hours_for_date(date), -2.0);
    }

    #[test]
    fn sum_over_a_range_treats_missing_dates_as_zero() {
        let mut hours = HoursByDate::new();
        hours.set_hours_for_date(LocalDate::of(2024, 1, 1), 4.0);
        hours.set_hours_for_date(LocalDate::of(2024, 1, 3), 6.0);
        // 1/2 has no entry.

        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 3));
        assert_eq!(hours.sum_hours_for_date_range(&range), 10.0);
    }
}
