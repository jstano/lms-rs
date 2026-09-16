//! Port of `WeeklyAccumulator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/WeeklyAccumulator.java`.
//!
//! A work week's running hours and overtime, plus the consecutive-day counter
//! the daily accumulator branches on. Nine rules build one per `execute` and
//! carry it across the week's days.
//!
//! Like [`DailyAccumulator`](super::daily_accumulator::DailyAccumulator), both
//! `add*` methods round through `TDouble.roundHours`.
//! [`compute_weekly_ot`](WeeklyAccumulator::compute_weekly_ot) does **not** —
//! see below.
//!
//! # The consecutive-day modifier
//!
//! `consecDaysModifier = consecDayLimit + maxConsecDays - 1`, and
//! [`update_consecutive_days_for_shifts`](WeeklyAccumulator::update_consecutive_days_for_shifts)
//! wraps the counter around it:
//!
//! ```java
//! consecutiveDaysCounter++;
//! if (consecutiveDaysCounter % consecDaysModifier != 0) { //only reset the day after to 1
//!    consecutiveDaysCounter = consecutiveDaysCounter % consecDaysModifier;
//! }
//! ```
//!
//! The guard is what makes the limit day *stick*: landing exactly on the
//! modifier leaves the counter there rather than zeroing it, so that day is
//! still over the limit, and only the day after wraps to 1. With a limit of 7
//! and a max of 7 the modifier is 13, so the counter runs 1…13 and then
//! restarts — a fourteenth consecutive worked day is day 1 again.
//!
//! # Three `updateConsecutiveDays` overloads, three behaviours
//!
//! They are **not** interchangeable, and only the third does the wrap above:
//!
//! | Java | Here | Behaviour |
//! |---|---|---|
//! | `updateConsecutiveDays(int)` | [`update_consecutive_days`](WeeklyAccumulator::update_consecutive_days) | adds, no reset and no wrap — how a rule seeds the counter from the prior-days calculators |
//! | `updateConsecutiveDays(DailyData)` | [`update_consecutive_days_for_day`](WeeklyAccumulator::update_consecutive_days_for_day) | zero on a day with no shifts, else `+1` — no wrap |
//! | `updateConsecutiveDays(List<EmployeeShift>)` | [`update_consecutive_days_for_shifts`](WeeklyAccumulator::update_consecutive_days_for_shifts) | zero on an empty day, else `+1` then wrap |
//!
//! So a rule that feeds days through `DailyData` never wraps its counter and
//! one that feeds a shift list does. `CaliforniaOTHrsRuleImpl` takes the
//! middle row.
//!
//! # `computeWeeklyOT` is the odd one out
//!
//! ```java
//! boolean payOt = premiumHoursCountTowardsWeeklyOT && hours > weeklyLimit;
//! double ot = payOt ? Math.min(hours - weeklyLimit, originalHours)
//!                   : hours - weeklyLimit - weeklyOT;
//! return Math.max(0.0, ot);
//! ```
//!
//! `setBothConsecutiveAndWeeklyOt` is not ported: the field it writes is never
//! read, here or anywhere else. Exactly one rule calls the setter
//! (`CaliforniaExtendedOTHrsRuleImpl`); the other four rules that read the
//! `BOTH_CONSECUTIVE_AND_WEEKLY_OT` parameter keep it in a field of their own
//! and act on that, so nothing is lost by dropping the accumulator's copy.
//!
//! Two things to know. **The flag selects a formula, it does not filter
//! hours** — with it set, the answer is capped at `originalHours` and does not
//! subtract the overtime already accumulated; with it clear, the ordinary
//! running-total form applies. And **neither branch rounds**, alone among the
//! accumulator's arithmetic, so the result carries whatever precision the
//! subtraction produced and it is the caller's job to round it.

use crate::common::numbers::round_hours;

/// A work week's hours, overtime and consecutive-day counter.
/// `WeeklyAccumulator`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyAccumulator {
    hours: f64,
    weekly_ot: f64,
    consecutive_days_counter: i32,
    weekly_limit: f64,
    consec_day_limit: i32,
    consec_days_modifier: i32,
    premium_hours_count_towards_weekly_ot: bool,
    original_hours: f64,
}

impl WeeklyAccumulator {
    /// An empty accumulator, with the rule's weekly limit and the two
    /// consecutive-day numbers its modifier is derived from.
    pub fn new(weekly_limit: f64, consec_day_limit: i32, max_consec_days: i32) -> Self {
        Self {
            hours: 0.0,
            weekly_ot: 0.0,
            consecutive_days_counter: 0,
            weekly_limit,
            consec_day_limit,
            consec_days_modifier: consec_day_limit + max_consec_days - 1,
            premium_hours_count_towards_weekly_ot: false,
            original_hours: 0.0,
        }
    }

    /// Hours accumulated across the week so far.
    pub fn hours(&self) -> f64 {
        self.hours
    }

    /// Weekly overtime accumulated so far.
    pub fn weekly_ot(&self) -> f64 {
        self.weekly_ot
    }

    /// How many consecutive days have been worked up to and including today.
    pub fn consecutive_days_counter(&self) -> i32 {
        self.consecutive_days_counter
    }

    /// The weekly overtime threshold this rule was configured with.
    pub fn weekly_limit(&self) -> f64 {
        self.weekly_limit
    }

    /// `consecDayLimit + maxConsecDays - 1`, the value the counter wraps at.
    pub fn consec_days_modifier(&self) -> i32 {
        self.consec_days_modifier
    }

    /// Seed or advance the counter by a number of days.
    /// `updateConsecutiveDays(int)` — no reset, no wrap.
    pub fn update_consecutive_days(&mut self, days: i32) {
        self.consecutive_days_counter += days;
    }

    /// Advance the counter for one day, given whether that day was worked.
    /// `updateConsecutiveDays(DailyData)` — resets to zero on an unworked day
    /// and otherwise adds one, **without** the modifier wrap.
    ///
    /// Java takes a `DailyData` and asks only `getShifts().isEmpty()`, so this
    /// takes the answer to that question rather than the whole object; the
    /// caller has it either way and this keeps the two helpers independent.
    pub fn update_consecutive_days_for_day(&mut self, day_was_worked: bool) {
        self.consecutive_days_counter = if day_was_worked {
            self.consecutive_days_counter + 1
        } else {
            0
        };
    }

    /// The same, with the modifier wrap.
    /// `updateConsecutiveDays(List<EmployeeShift>)`.
    pub fn update_consecutive_days_for_shifts(&mut self, day_was_worked: bool) {
        if !day_was_worked {
            self.consecutive_days_counter = 0;
            return;
        }

        self.consecutive_days_counter += 1;
        // Landing exactly on the modifier is left alone, so the limit day
        // itself still counts as over the limit; only the day after wraps.
        if self.consecutive_days_counter % self.consec_days_modifier != 0 {
            self.consecutive_days_counter %= self.consec_days_modifier;
        }
    }

    /// `addHours()`.
    pub fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    /// `addWeeklyOT()`.
    pub fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// The weekly overtime owed so far, never below zero. `computeWeeklyOT()`
    /// — which does not round; see the module note.
    pub fn compute_weekly_ot(&self) -> f64 {
        let pay_ot = self.premium_hours_count_towards_weekly_ot && self.hours > self.weekly_limit;
        let ot = if pay_ot {
            (self.hours - self.weekly_limit).min(self.original_hours)
        } else {
            self.hours - self.weekly_limit - self.weekly_ot
        };
        ot.max(0.0)
    }

    /// Whether the counter has reached the rule's consecutive-day limit — the
    /// switch [`DailyAccumulator`](super::daily_accumulator::DailyAccumulator)
    /// branches on. `hasReachedConsecutiveDayLimit()`.
    pub fn has_reached_consecutive_day_limit(&self) -> bool {
        self.consecutive_days_counter >= self.consec_day_limit
    }

    /// `setPremiumHoursCountTowardsWeeklyOT()` — selects which formula
    /// [`compute_weekly_ot`](Self::compute_weekly_ot) uses.
    pub fn set_premium_hours_count_towards_weekly_ot(&mut self, value: bool) {
        self.premium_hours_count_towards_weekly_ot = value;
    }

    /// `setOriginalHours()` — the cap the premium formula applies.
    pub fn set_original_hours(&mut self, original_hours: f64) {
        self.original_hours = original_hours;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A forty-hour week with California's seven-day numbers: modifier 13.
    fn accumulator() -> WeeklyAccumulator {
        WeeklyAccumulator::new(40.0, 7, 7)
    }

    #[test]
    fn the_modifier_is_the_limit_plus_the_max_less_one() {
        assert_eq!(accumulator().consec_days_modifier(), 13);
        assert_eq!(
            WeeklyAccumulator::new(40.0, 6, 7).consec_days_modifier(),
            12
        );
    }

    #[test]
    fn hours_and_overtime_round_as_they_accumulate() {
        let mut accumulator = accumulator();

        for _ in 0..3 {
            accumulator.add_hours(0.3333);
            accumulator.add_weekly_ot(0.3333);
        }

        // Each add rounds to two places before the next one, so three thirds
        // reach 0.99 rather than the 1.0 a single rounded sum would give.
        assert_eq!(accumulator.hours(), 0.99);
        assert_eq!(accumulator.weekly_ot(), 0.99);
    }

    #[test]
    fn weekly_overtime_starts_at_the_weekly_limit() {
        let mut accumulator = accumulator();
        accumulator.add_hours(40.0);

        assert_eq!(accumulator.compute_weekly_ot(), 0.0);

        accumulator.add_hours(5.0);

        assert_eq!(accumulator.compute_weekly_ot(), 5.0);
    }

    #[test]
    fn weekly_overtime_already_paid_is_not_paid_twice() {
        let mut accumulator = accumulator();
        accumulator.add_hours(45.0);
        accumulator.add_weekly_ot(5.0);

        assert_eq!(accumulator.compute_weekly_ot(), 0.0);
    }

    #[test]
    fn the_premium_flag_selects_a_different_formula() {
        // Same state, two answers: the premium form caps at original hours and
        // ignores the overtime already accumulated.
        let mut accumulator = accumulator();
        accumulator.add_hours(48.0);
        accumulator.add_weekly_ot(5.0);
        accumulator.set_original_hours(6.0);

        assert_eq!(accumulator.compute_weekly_ot(), 3.0, "48 - 40 - 5");

        accumulator.set_premium_hours_count_towards_weekly_ot(true);

        assert_eq!(accumulator.compute_weekly_ot(), 6.0, "min(48 - 40, 6)");
    }

    #[test]
    fn the_premium_formula_needs_the_hours_over_the_limit() {
        let mut accumulator = accumulator();
        accumulator.set_premium_hours_count_towards_weekly_ot(true);
        accumulator.set_original_hours(6.0);
        accumulator.add_hours(40.0);

        // Not strictly over the limit, so payOt is false and the ordinary
        // formula applies: 40 - 40 - 0.
        assert_eq!(accumulator.compute_weekly_ot(), 0.0);
    }

    #[test]
    fn weekly_overtime_never_goes_negative() {
        let mut accumulator = accumulator();
        accumulator.add_hours(10.0);

        assert_eq!(accumulator.compute_weekly_ot(), 0.0);
    }

    #[test]
    fn the_seeding_overload_neither_resets_nor_wraps() {
        let mut accumulator = accumulator();

        accumulator.update_consecutive_days(12);
        accumulator.update_consecutive_days(5);

        assert_eq!(
            accumulator.consecutive_days_counter(),
            17,
            "past the modifier, and left there"
        );
    }

    #[test]
    fn an_unworked_day_resets_the_counter() {
        let mut accumulator = accumulator();
        accumulator.update_consecutive_days(4);

        accumulator.update_consecutive_days_for_day(false);

        assert_eq!(accumulator.consecutive_days_counter(), 0);
    }

    #[test]
    fn the_daily_data_overload_does_not_wrap() {
        let mut accumulator = accumulator();

        for _ in 0..15 {
            accumulator.update_consecutive_days_for_day(true);
        }

        assert_eq!(accumulator.consecutive_days_counter(), 15);
    }

    #[test]
    fn the_shift_list_overload_wraps_at_the_modifier() {
        let mut accumulator = accumulator();
        let mut counts = Vec::new();

        for _ in 0..15 {
            accumulator.update_consecutive_days_for_shifts(true);
            counts.push(accumulator.consecutive_days_counter());
        }

        assert_eq!(
            counts,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 1, 2],
            "day 13 stays at 13; only the day after resets to 1"
        );
    }

    #[test]
    fn the_consecutive_day_limit_is_inclusive() {
        let mut accumulator = accumulator();

        accumulator.update_consecutive_days(6);
        assert!(!accumulator.has_reached_consecutive_day_limit());

        accumulator.update_consecutive_days(1);
        assert!(
            accumulator.has_reached_consecutive_day_limit(),
            "the seventh day is the limit, not the eighth"
        );
    }

    #[test]
    fn the_counter_stays_over_the_limit_on_the_limit_day() {
        // The point of the `% != 0` guard: the modifier day must still test as
        // over the limit, or the rule that pays double time on it would not.
        let mut accumulator = accumulator();

        for _ in 0..13 {
            accumulator.update_consecutive_days_for_shifts(true);
        }

        assert_eq!(accumulator.consecutive_days_counter(), 13);
        assert!(accumulator.has_reached_consecutive_day_limit());

        accumulator.update_consecutive_days_for_shifts(true);

        assert_eq!(accumulator.consecutive_days_counter(), 1);
        assert!(!accumulator.has_reached_consecutive_day_limit());
    }
}
