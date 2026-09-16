//! Port of `DailyAccumulator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyAccumulator.java`.
//!
//! One day's running totals — hours, overtime and double time — plus the two
//! daily limits the rule was configured with. Eight of the nineteen rules build
//! one per day of the work week and feed distributions into it.
//!
//! Every `add*` rounds through `TDouble.roundHours`, so the totals are always at
//! hours precision rather than accumulating raw sums. The two `compute*`
//! methods round too, and then clamp at zero.
//!
//! # The limits swap roles on a consecutive-day-limit day
//!
//! Both `compute*` methods branch on
//! [`WeeklyAccumulator::has_reached_consecutive_day_limit`], and the branch is
//! not a simple on/off:
//!
//! | | under the limit | at or over it |
//! |---|---|---|
//! | overtime | `hours - dailyOTLimit - overtime` | `hours - overtime` |
//! | double time | `hours - dailyDTLimit - doubleTime` | `hours - dailyOTLimit - doubleTime` |
//!
//! So on a seventh consecutive day **every** hour is overtime from the first
//! one, and double time starts at the *overtime* limit rather than the double
//! time one. That is the California rule these were written for, not an
//! oversight: the daily double-time limit stops applying and the overtime limit
//! takes its place.

use crate::common::numbers::round_hours;
use crate::rules::algorithm::hoursdistribution::weekly_accumulator::WeeklyAccumulator;
use joda_rs::LocalDate;

/// One day's hours, overtime and double time. `DailyAccumulator`.
#[derive(Debug, Clone, PartialEq)]
pub struct DailyAccumulator {
    date: LocalDate,
    hours: f64,
    overtime: f64,
    double_time: f64,
    daily_ot_limit: f64,
    daily_dt_limit: f64,
}

impl DailyAccumulator {
    /// An empty accumulator for one date, with the rule's two daily limits.
    pub fn new(date: LocalDate, daily_ot_limit: f64, daily_dt_limit: f64) -> Self {
        Self {
            date,
            hours: 0.0,
            overtime: 0.0,
            double_time: 0.0,
            daily_ot_limit,
            daily_dt_limit,
        }
    }

    /// `getDate()`.
    pub fn date(&self) -> LocalDate {
        self.date
    }

    /// Hours accumulated so far.
    ///
    /// Java leaves the three totals package-private with no getter and the
    /// rules in the package read the fields directly; they are accessors here.
    pub fn hours(&self) -> f64 {
        self.hours
    }

    /// Overtime accumulated so far.
    pub fn overtime(&self) -> f64 {
        self.overtime
    }

    /// Double time accumulated so far.
    pub fn double_time(&self) -> f64 {
        self.double_time
    }

    /// The daily overtime threshold this rule was configured with.
    pub fn daily_ot_limit(&self) -> f64 {
        self.daily_ot_limit
    }

    /// The daily double-time threshold this rule was configured with.
    pub fn daily_dt_limit(&self) -> f64 {
        self.daily_dt_limit
    }

    /// `addHours()`.
    pub fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    /// `addOvertime()`.
    pub fn add_overtime(&mut self, add_overtime: f64) {
        self.overtime = round_hours(self.overtime + add_overtime);
    }

    /// `addDoubleTime()`.
    pub fn add_double_time(&mut self, add_double_time: f64) {
        self.double_time = round_hours(self.double_time + add_double_time);
    }

    /// The overtime owed for this day so far, never below zero.
    /// `computeDailyOvertime()`.
    pub fn compute_daily_overtime(&self, weekly: &WeeklyAccumulator) -> f64 {
        let daily_ot = if weekly.has_reached_consecutive_day_limit() {
            self.hours - self.overtime
        } else {
            self.hours - self.daily_ot_limit - self.overtime
        };
        round_hours(daily_ot).max(0.0)
    }

    /// The double time owed for this day so far, never below zero.
    /// `computeDailyDoubleTime()` — note it measures against the **overtime**
    /// limit once the consecutive-day limit is reached.
    pub fn compute_daily_double_time(&self, weekly: &WeeklyAccumulator) -> f64 {
        let daily_dt = if weekly.has_reached_consecutive_day_limit() {
            self.hours - self.daily_ot_limit - self.double_time
        } else {
            self.hours - self.daily_dt_limit - self.double_time
        };
        round_hours(daily_dt).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    /// California's defaults: overtime past 8, double time past 12.
    fn accumulator() -> DailyAccumulator {
        DailyAccumulator::new(date(), 8.0, 12.0)
    }

    /// A weekly accumulator that has not reached its consecutive-day limit.
    fn within_week() -> WeeklyAccumulator {
        WeeklyAccumulator::new(40.0, 7, 7)
    }

    /// The same, wound forward to the seventh consecutive day.
    fn seventh_day() -> WeeklyAccumulator {
        let mut weekly = WeeklyAccumulator::new(40.0, 7, 7);
        weekly.update_consecutive_days(7);
        weekly
    }

    #[test]
    fn a_new_accumulator_holds_nothing() {
        let accumulator = accumulator();

        assert_eq!(accumulator.date(), date());
        assert_eq!(accumulator.hours(), 0.0);
        assert_eq!(accumulator.overtime(), 0.0);
        assert_eq!(accumulator.double_time(), 0.0);
    }

    #[test]
    fn every_add_rounds_to_hours_precision() {
        let mut accumulator = accumulator();

        for _ in 0..3 {
            accumulator.add_hours(0.3333);
        }

        // Rounded on the way in, not at the end: 0.33, then 0.66, then 0.99.
        // Summing first and rounding once would give 1.0.
        assert_eq!(accumulator.hours(), 0.99);
        assert_ne!(accumulator.hours(), round_hours(0.3333 * 3.0));
    }

    #[test]
    fn overtime_starts_at_the_daily_limit() {
        let mut accumulator = accumulator();
        accumulator.add_hours(8.0);

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 0.0);

        accumulator.add_hours(2.0);

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 2.0);
    }

    #[test]
    fn overtime_already_paid_is_not_paid_twice() {
        let mut accumulator = accumulator();
        accumulator.add_hours(10.0);
        accumulator.add_overtime(2.0);

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 0.0);

        accumulator.add_hours(1.0);

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 1.0);
    }

    #[test]
    fn double_time_starts_at_the_double_time_limit() {
        let mut accumulator = accumulator();
        accumulator.add_hours(12.0);

        assert_eq!(accumulator.compute_daily_double_time(&within_week()), 0.0);

        accumulator.add_hours(1.5);

        assert_eq!(accumulator.compute_daily_double_time(&within_week()), 1.5);
    }

    #[test]
    fn on_a_consecutive_day_limit_day_every_hour_is_overtime() {
        let mut accumulator = accumulator();
        accumulator.add_hours(4.0);

        assert_eq!(
            accumulator.compute_daily_overtime(&within_week()),
            0.0,
            "four hours is under the eight hour daily limit"
        );
        assert_eq!(
            accumulator.compute_daily_overtime(&seventh_day()),
            4.0,
            "but the limit does not apply on the seventh consecutive day"
        );
    }

    #[test]
    fn on_a_consecutive_day_limit_day_double_time_starts_at_the_overtime_limit() {
        let mut accumulator = accumulator();
        accumulator.add_hours(10.0);

        assert_eq!(
            accumulator.compute_daily_double_time(&within_week()),
            0.0,
            "ten hours is under the twelve hour double time limit"
        );
        assert_eq!(
            accumulator.compute_daily_double_time(&seventh_day()),
            2.0,
            "on the seventh day double time starts at the eight hour OT limit"
        );
    }

    #[test]
    fn neither_computation_can_go_negative() {
        let accumulator = accumulator();

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 0.0);
        assert_eq!(accumulator.compute_daily_double_time(&within_week()), 0.0);
        assert_eq!(accumulator.compute_daily_overtime(&seventh_day()), 0.0);
        assert_eq!(accumulator.compute_daily_double_time(&seventh_day()), 0.0);
    }

    #[test]
    fn computing_does_not_consume_the_accumulator() {
        let mut accumulator = accumulator();
        accumulator.add_hours(10.0);

        assert_eq!(accumulator.compute_daily_overtime(&within_week()), 2.0);
        assert_eq!(
            accumulator.compute_daily_overtime(&within_week()),
            2.0,
            "asking twice gives the same answer; only add_overtime moves it"
        );
    }
}
