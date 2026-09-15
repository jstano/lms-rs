//! Where on the day's shift each block of work is placed.
//!
//! Ported from Java's `WorkContentCalculator`. A block that is as long as the
//! shift template occupies the whole of it; a shorter one is anchored to
//! either end of the template so that blocks fill the day from the outside in.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::job_shift::JobShiftDefinition;
use crate::workcontent::domain::planner_settings::PlannerSettings;
use crate::workcontent::generators::basic::basic_calculator::BasicCalculationResult;
use date_range_rs::DateTimeRange;
use joda_rs::LocalDate;
use joda_rs::constants::MINUTES_PER_HOUR;

pub struct WorkContentCalculator<'a> {
    result: &'a BasicCalculationResult,
    planner_settings: &'a PlannerSettings,
    shift_detail: &'a JobShiftDefinition,
    shift_date: LocalDate,
}

impl<'a> WorkContentCalculator<'a> {
    pub fn new(
        result: &'a BasicCalculationResult,
        planner_settings: &'a PlannerSettings,
        shift_detail: &'a JobShiftDefinition,
        shift_date: LocalDate,
    ) -> Self {
        Self {
            result,
            planner_settings,
            shift_detail,
            shift_date,
        }
    }

    pub fn number_of_full_shifts(&self) -> i32 {
        self.result.number_of_full_time_shifts()
    }

    pub fn remaining_work_hours(&self) -> f64 {
        self.result.remaining_work_hours()
    }

    /// A full-shift block placed at the start of the day.
    pub fn full_shift_range_for_start_of_shift(&self) -> Option<DateTimeRange> {
        let shift_range = self.shift_range()?;

        if self.shift_fits_within_the_maximum() {
            return Some(shift_range);
        }

        let start_date_time = shift_range.start();

        Some(DateTimeRange::of(
            start_date_time,
            start_date_time.plus_minutes(self.max_shift_minutes()),
        ))
    }

    /// A full-shift block placed at the end of the day.
    pub fn full_shift_range_for_end_of_shift(&self) -> Option<DateTimeRange> {
        let shift_range = self.shift_range()?;

        if self.shift_fits_within_the_maximum() {
            return Some(shift_range);
        }

        let end_date_time = shift_range.end();

        Some(DateTimeRange::of(
            end_date_time.minus_minutes(self.max_shift_minutes()),
            end_date_time,
        ))
    }

    /// The leftover block, which always starts when the shift does.
    pub fn remaining_work_range(&self) -> Option<DateTimeRange> {
        let start_date_time = self.shift_range()?.start();
        let minutes =
            numbers::round(self.result.remaining_work_hours() * MINUTES_PER_HOUR as f64) as i64;

        Some(DateTimeRange::of(
            start_date_time,
            start_date_time.plus_minutes(minutes),
        ))
    }

    fn shift_range(&self) -> Option<DateTimeRange> {
        self.shift_detail.to_date_time_range(self.shift_date)
    }

    fn shift_fits_within_the_maximum(&self) -> bool {
        self.shift_detail.shift_length() <= self.planner_settings.max_shift_length
    }

    fn max_shift_minutes(&self) -> i64 {
        numbers::round(self.planner_settings.max_shift_length * MINUTES_PER_HOUR as f64) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::generators::basic::basic_calculator::{
        BasicCalculator, BasicCalculatorImpl,
    };
    use joda_rs::{DayOfWeek, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    fn time(hour: i32, minute: i32) -> LocalTime {
        LocalTime::of_hour_minute(hour, minute)
    }

    /// A 12-hour template, 08:00 to 20:00.
    fn twelve_hour_shift() -> JobShiftDefinition {
        JobShiftDefinition::new(DayOfWeek::Monday, time(8, 0), time(20, 0), 0.0, 0.0, 1)
    }

    fn settings(max_shift_length: f64) -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.period_length = 15;
        settings.min_shift_length = 0.0;
        settings.max_shift_length = max_shift_length;
        settings.rounding_threshold_below_one = 0.0;
        settings.rounding_threshold_above_one = 0.0;
        settings
    }

    fn result(
        settings: &PlannerSettings,
        shift_detail: &JobShiftDefinition,
        total_work_minutes: i32,
    ) -> BasicCalculationResult {
        BasicCalculatorImpl::new().calculate(
            settings,
            shift_detail,
            JobShiftId::new(),
            date(),
            total_work_minutes,
        )
    }

    #[test]
    fn a_shift_within_the_maximum_occupies_the_whole_template() {
        // The 12-hour template is not capped, so both ends are the full shift.
        let settings = settings(12.0);
        let shift_detail = twelve_hour_shift();
        let result = result(&settings, &shift_detail, 12 * 60);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        let start = calculator.full_shift_range_for_start_of_shift().unwrap();
        let end = calculator.full_shift_range_for_end_of_shift().unwrap();

        assert_eq!(start.start(), date().at_time(time(8, 0)));
        assert_eq!(start.end(), date().at_time(time(20, 0)));
        assert_eq!(end, start);
    }

    #[test]
    fn a_capped_shift_is_anchored_to_the_start_of_the_template() {
        // An 8-hour cap on a 12-hour template: 08:00 to 16:00.
        let settings = settings(8.0);
        let shift_detail = twelve_hour_shift();
        let result = result(&settings, &shift_detail, 8 * 60);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        let range = calculator.full_shift_range_for_start_of_shift().unwrap();

        assert_eq!(range.start(), date().at_time(time(8, 0)));
        assert_eq!(range.end(), date().at_time(time(16, 0)));
    }

    #[test]
    fn a_capped_shift_is_anchored_to_the_end_of_the_template() {
        // The same cap from the other end: 12:00 to 20:00.
        let settings = settings(8.0);
        let shift_detail = twelve_hour_shift();
        let result = result(&settings, &shift_detail, 8 * 60);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        let range = calculator.full_shift_range_for_end_of_shift().unwrap();

        assert_eq!(range.start(), date().at_time(time(12, 0)));
        assert_eq!(range.end(), date().at_time(time(20, 0)));
    }

    #[test]
    fn the_remainder_starts_when_the_shift_does() {
        let settings = settings(8.0);
        let shift_detail = twelve_hour_shift();
        // 8 h full shift plus 2 h left over.
        let result = result(&settings, &shift_detail, 10 * 60);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        let range = calculator.remaining_work_range().unwrap();

        assert_eq!(calculator.remaining_work_hours(), 2.0);
        assert_eq!(range.start(), date().at_time(time(8, 0)));
        assert_eq!(range.end(), date().at_time(time(10, 0)));
    }

    #[test]
    fn an_overnight_shift_places_blocks_across_midnight() {
        let settings = settings(8.0);
        // 22:00 to 06:00.
        let shift_detail =
            JobShiftDefinition::new(DayOfWeek::Monday, time(22, 0), time(6, 0), 0.0, 0.0, 1);
        let result = result(&settings, &shift_detail, 8 * 60);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        let range = calculator.full_shift_range_for_start_of_shift().unwrap();

        assert_eq!(range.start(), date().at_time(time(22, 0)));
        assert_eq!(range.end(), date().plus_days(1).at_time(time(6, 0)));
    }

    #[test]
    fn a_shift_without_times_has_no_ranges() {
        let settings = settings(8.0);
        let shift_detail = JobShiftDefinition::without_times(DayOfWeek::Monday);
        let result = result(&settings, &shift_detail, 0);
        let calculator = WorkContentCalculator::new(&result, &settings, &shift_detail, date());

        assert!(calculator.full_shift_range_for_start_of_shift().is_none());
        assert!(calculator.full_shift_range_for_end_of_shift().is_none());
        assert!(calculator.remaining_work_range().is_none());
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// Every expectation below is from `WorkContentCalculatorTest.groovy`,
/// transcribed unchanged.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use joda_rs::{DayOfWeek, LocalDateTime, LocalTime};
    use rstest::rstest;

    /// The Java fixture: 22 full shifts and 1.25 hours left over, planned on
    /// Christmas Day 2013 at a maximum shift of eight hours.
    fn shift_date() -> LocalDate {
        LocalDate::new(2013, 12, 25)
    }

    fn settings() -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.max_shift_length = 8.0;
        settings.period_length = 15;
        settings
    }

    fn result() -> BasicCalculationResult {
        BasicCalculationResult::with_values(22, 1.25, 0.0, shift_date(), JobShiftId::new())
    }

    fn shift_from(start: (i32, i32), end: (i32, i32)) -> JobShiftDefinition {
        JobShiftDefinition::new(
            DayOfWeek::Wednesday,
            LocalTime::of_hour_minute(start.0, start.1),
            LocalTime::of_hour_minute(end.0, end.1),
            0.0,
            0.0,
            1,
        )
    }

    fn at(day_offset: i64, hour: i32, minute: i32) -> LocalDateTime {
        shift_date().plus_days(day_offset).at_time(LocalTime::of_hour_minute(hour, minute))
    }

    #[rstest]
    // An eight-hour shift is already within the maximum, so it is used whole.
    #[case((7, 0), (15, 0), at(0, 7, 0), at(0, 15, 0))]
    // The same, but running past midnight into the following day.
    #[case((20, 0), (2, 0), at(0, 20, 0), at(1, 2, 0))]
    // A ten-hour shift exceeds the eight-hour maximum, so the block is cut
    // short at its end — two hours before the template finishes.
    #[case((6, 0), (16, 0), at(0, 6, 0), at(0, 14, 0))]
    fn a_full_shift_at_the_start_of_the_day_matches_java(
        #[case] start: (i32, i32),
        #[case] end: (i32, i32),
        #[case] expected_start: LocalDateTime,
        #[case] expected_end: LocalDateTime,
    ) {
        let (settings, result, shift) = (settings(), result(), shift_from(start, end));
        let calculator = WorkContentCalculator::new(&result, &settings, &shift, shift_date());

        let range = calculator
            .full_shift_range_for_start_of_shift()
            .expect("the shift has times");

        assert_eq!(range.start(), expected_start);
        assert_eq!(range.end(), expected_end);
    }

    #[rstest]
    #[case((7, 0), (15, 0), at(0, 7, 0), at(0, 15, 0))]
    #[case((20, 0), (2, 0), at(0, 20, 0), at(1, 2, 0))]
    // The mirror image: a capped block anchored to the end starts two hours
    // after the template does.
    #[case((6, 0), (16, 0), at(0, 8, 0), at(0, 16, 0))]
    fn a_full_shift_at_the_end_of_the_day_matches_java(
        #[case] start: (i32, i32),
        #[case] end: (i32, i32),
        #[case] expected_start: LocalDateTime,
        #[case] expected_end: LocalDateTime,
    ) {
        let (settings, result, shift) = (settings(), result(), shift_from(start, end));
        let calculator = WorkContentCalculator::new(&result, &settings, &shift, shift_date());

        let range = calculator
            .full_shift_range_for_end_of_shift()
            .expect("the shift has times");

        assert_eq!(range.start(), expected_start);
        assert_eq!(range.end(), expected_end);
    }

    #[test]
    fn the_remainder_block_matches_java() {
        // The fixture's 07:00-17:00 template with 1.25 hours left over.
        let (settings, result, shift) = (settings(), result(), shift_from((7, 0), (17, 0)));
        let calculator = WorkContentCalculator::new(&result, &settings, &shift, shift_date());

        let range = calculator
            .remaining_work_range()
            .expect("the shift has times");

        assert_eq!(range.start(), at(0, 7, 0));
        assert_eq!(range.end(), at(0, 8, 15));
    }

    #[test]
    fn the_calculator_reports_the_division_it_was_given() {
        let (settings, result, shift) = (settings(), result(), shift_from((7, 0), (17, 0)));
        let calculator = WorkContentCalculator::new(&result, &settings, &shift, shift_date());

        assert_eq!(calculator.number_of_full_shifts(), 22);
        assert_eq!(calculator.remaining_work_hours(), 1.25);
    }
}
