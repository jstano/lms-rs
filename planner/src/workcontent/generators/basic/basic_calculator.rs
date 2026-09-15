//! Splits a day's required work into whole shifts plus a possible remainder.
//!
//! Ported from Java's `SimpleNonFlowedCalculator`; the business rules are
//! written out in `java/engine/workcontent/simplenonflowed/REQUIREMENTS.md`.

use crate::workcontent::common::break_length_calculator;
use crate::workcontent::common::numbers;
use crate::workcontent::domain::job_shift::{JobShiftDefinition, JobShiftId};
use crate::workcontent::domain::planner_settings::PlannerSettings;
use joda_rs::LocalDate;
use joda_rs::constants::MINUTES_PER_HOUR;

/// How a day's work divides into scheduled blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicCalculationResult {
    /// Blocks of the full chosen shift length, each including its paid breaks.
    number_of_full_time_shifts: i32,
    /// The leftover block, including its own paid breaks; zero when there is none.
    remaining_work_hours: f64,
    /// Paid break hours across all the blocks.
    work_hours_to_cover_breaks: f64,
    shift_date: LocalDate,
    job_shift_id: JobShiftId,
}

impl BasicCalculationResult {
    pub fn number_of_full_time_shifts(&self) -> i32 {
        self.number_of_full_time_shifts
    }

    pub fn remaining_work_hours(&self) -> f64 {
        self.remaining_work_hours
    }

    pub fn work_hours_to_cover_breaks(&self) -> f64 {
        self.work_hours_to_cover_breaks
    }

    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }

    pub fn job_shift_id(&self) -> JobShiftId {
        self.job_shift_id
    }

    pub fn has_remaining_work(&self) -> bool {
        self.remaining_work_hours != 0.0
    }

    /// A result with chosen values, for testing the stages downstream of this
    /// one against a known division of work.
    #[cfg(test)]
    pub fn with_values(
        number_of_full_time_shifts: i32,
        remaining_work_hours: f64,
        work_hours_to_cover_breaks: f64,
        shift_date: LocalDate,
        job_shift_id: JobShiftId,
    ) -> Self {
        Self {
            number_of_full_time_shifts,
            remaining_work_hours,
            work_hours_to_cover_breaks,
            shift_date,
            job_shift_id,
        }
    }
}

pub trait BasicCalculator {
    fn calculate(
        &self,
        planner_settings: &PlannerSettings,
        shift_detail: &JobShiftDefinition,
        job_shift_id: JobShiftId,
        shift_date: LocalDate,
        total_work_minutes: i32,
    ) -> BasicCalculationResult;
}

pub struct BasicCalculatorImpl;

impl BasicCalculatorImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl BasicCalculator for BasicCalculatorImpl {
    fn calculate(
        &self,
        planner_settings: &PlannerSettings,
        shift_detail: &JobShiftDefinition,
        job_shift_id: JobShiftId,
        shift_date: LocalDate,
        total_work_minutes: i32,
    ) -> BasicCalculationResult {
        let shift_length = shift_length(planner_settings, shift_detail);
        let productive_full_shift_hours =
            break_length_calculator::calculate_productive_time_from_total_duration(
                shift_length,
                planner_settings.meal_break.as_ref(),
                planner_settings.non_meal_break.as_ref(),
            );
        let break_hours_per_full_shift = shift_length - productive_full_shift_hours;

        // A shift with no productive time has nowhere to put work, and dividing
        // by it below would report an unbounded number of full shifts.
        if productive_full_shift_hours <= 0.0 {
            return BasicCalculationResult {
                number_of_full_time_shifts: 0,
                remaining_work_hours: 0.0,
                work_hours_to_cover_breaks: 0.0,
                shift_date,
                job_shift_id,
            };
        }

        let total_work_hours =
            numbers::round_hours(total_work_minutes as f64 / MINUTES_PER_HOUR as f64);
        let full_time_shifts = numbers::truncate(total_work_hours / productive_full_shift_hours);
        let paid_breaks_for_full_time_shifts =
            full_time_shifts as f64 * break_hours_per_full_shift;

        let remaining_work_hours = remaining_work_hours(
            planner_settings,
            shift_detail,
            productive_full_shift_hours,
            total_work_hours,
            full_time_shifts,
        );
        let paid_breaks_for_remaining_work = if remaining_work_hours == 0.0 {
            0.0
        } else {
            break_length_calculator::calculate_break_in_fractional_hours(
                remaining_work_hours,
                planner_settings.meal_break.as_ref(),
                planner_settings.non_meal_break.as_ref(),
            )
        };

        BasicCalculationResult {
            number_of_full_time_shifts: full_time_shifts,
            remaining_work_hours: remaining_work_hours + paid_breaks_for_remaining_work,
            work_hours_to_cover_breaks: paid_breaks_for_full_time_shifts
                + paid_breaks_for_remaining_work,
            shift_date,
            job_shift_id,
        }
    }
}

/// The shift length to plan against: the template's, capped by the configured
/// maximum when one is set and shorter.
fn shift_length(planner_settings: &PlannerSettings, shift_detail: &JobShiftDefinition) -> f64 {
    let template_length = shift_detail.shift_length();

    if planner_settings.max_shift_length > 0.0
        && planner_settings.max_shift_length < template_length
    {
        planner_settings.max_shift_length
    } else {
        template_length
    }
}

/// The productive hours left over after the full shifts, after the threshold,
/// minimum-shift and period-rounding rules have been applied.
fn remaining_work_hours(
    planner_settings: &PlannerSettings,
    shift_detail: &JobShiftDefinition,
    productive_full_shift_hours: f64,
    total_work_hours: f64,
    full_time_shifts: i32,
) -> f64 {
    let min_shift = planner_settings.min_shift_length;
    let mut remaining_work_hours =
        numbers::round_hours(total_work_hours % productive_full_shift_hours);

    if numbers::round(remaining_work_hours * MINUTES_PER_HOUR as f64)
        < rounding_threshold_minutes(planner_settings, full_time_shifts)
    {
        // Too small to be worth scheduling.
        remaining_work_hours = 0.0;
    } else if remaining_work_hours < min_shift {
        // Too short to schedule as-is, so pad it out — but never past the
        // length of the shift it sits in.
        let shift_length = shift_detail.shift_length();

        remaining_work_hours = if shift_length < min_shift {
            shift_length
        } else {
            min_shift
        };
    }

    round_remaining_hours_to_nearest_period(planner_settings, remaining_work_hours)
}

fn round_remaining_hours_to_nearest_period(
    planner_settings: &PlannerSettings,
    remaining_hours: f64,
) -> f64 {
    let remaining_work_minutes = numbers::round(remaining_hours * MINUTES_PER_HOUR as f64);
    let period_length = planner_settings.period_length as i32;

    // Anything worth scheduling is worth at least one period.
    if remaining_work_minutes > 0 && remaining_work_minutes < period_length {
        return numbers::round_hours(period_length as f64 / MINUTES_PER_HOUR as f64);
    }

    let full_periods = numbers::round(remaining_work_minutes as f64 / period_length as f64);

    numbers::round_hours(full_periods as f64 * period_length as f64 / MINUTES_PER_HOUR as f64)
}

/// The number of minutes a remainder must reach to be scheduled at all.
fn rounding_threshold_minutes(planner_settings: &PlannerSettings, full_time_shifts: i32) -> i32 {
    let round_threshold = if full_time_shifts < 1 {
        planner_settings.rounding_threshold_below_one
    } else {
        planner_settings.rounding_threshold_above_one
    };

    numbers::truncate(planner_settings.period_length as f64 * round_threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::meal_break::MealBreak;
    use crate::workcontent::domain::non_meal_break::NonMealBreak;
    use joda_rs::{DayOfWeek, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    /// An 8-hour template, 09:00 to 17:00.
    fn eight_hour_shift() -> JobShiftDefinition {
        JobShiftDefinition::new(
            DayOfWeek::Monday,
            LocalTime::of_hour_minute(9, 0),
            LocalTime::of_hour_minute(17, 0),
            0.0,
            0.0,
            1,
        )
    }

    /// Settings giving an 8-hour shift 0.5 hours of paid break, as in the
    /// worked example in REQUIREMENTS.md.
    fn settings_with_half_hour_break() -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.period_length = 15;
        settings.min_shift_length = 0.0;
        settings.max_shift_length = 0.0;
        settings.rounding_threshold_below_one = 0.0;
        settings.rounding_threshold_above_one = 0.0;
        settings.meal_break = Some(MealBreak::new(5.0, 0.5));
        settings.non_meal_break = Some(NonMealBreak::with_no_break());
        settings
    }

    fn settings_without_breaks() -> PlannerSettings {
        let mut settings = settings_with_half_hour_break();
        settings.meal_break = None;
        settings.non_meal_break = None;
        settings
    }

    fn calculate(
        settings: &PlannerSettings,
        total_work_minutes: i32,
    ) -> BasicCalculationResult {
        BasicCalculatorImpl::new().calculate(
            settings,
            &eight_hour_shift(),
            JobShiftId::new(),
            date(),
            total_work_minutes,
        )
    }

    #[test]
    fn the_worked_example_from_the_requirements() {
        // 8.0 h template with 0.5 h paid break gives 7.5 productive hours.
        // 17.0 h of required work is 2 full shifts (15.0 h) plus 2.0 h over.
        let settings = settings_with_half_hour_break();

        let result = calculate(&settings, 17 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 2);
        // The 2.0 h remainder is under the 5 h meal-break threshold, so it
        // carries no break time of its own.
        assert_eq!(result.remaining_work_hours(), 2.0);
        // 2 full shifts * 0.5 h of break.
        assert_eq!(result.work_hours_to_cover_breaks(), 1.0);
    }

    #[test]
    fn work_that_fits_whole_shifts_exactly_leaves_no_remainder() {
        let settings = settings_with_half_hour_break();

        // 15.0 h is exactly two 7.5 h productive shifts.
        let result = calculate(&settings, 15 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 2);
        assert_eq!(result.remaining_work_hours(), 0.0);
        assert!(!result.has_remaining_work());
    }

    #[test]
    fn work_shorter_than_one_shift_is_all_remainder() {
        let settings = settings_with_half_hour_break();

        let result = calculate(&settings, 3 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 0);
        assert_eq!(result.remaining_work_hours(), 3.0);
        assert!(result.has_remaining_work());
    }

    #[test]
    fn no_required_work_plans_nothing() {
        let settings = settings_with_half_hour_break();

        let result = calculate(&settings, 0);

        assert_eq!(result.number_of_full_time_shifts(), 0);
        assert_eq!(result.remaining_work_hours(), 0.0);
        assert_eq!(result.work_hours_to_cover_breaks(), 0.0);
    }

    #[test]
    fn a_remainder_below_the_threshold_is_discarded() {
        // With one full shift planned, the above-one threshold applies: a whole
        // period (15 minutes) must be reached.
        let mut settings = settings_without_breaks();
        settings.rounding_threshold_above_one = 1.0;

        // 8 hours of full shift plus 10 minutes.
        let result = calculate(&settings, 8 * 60 + 10);

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 0.0);
    }

    #[test]
    fn a_remainder_at_the_threshold_is_kept() {
        let mut settings = settings_without_breaks();
        settings.rounding_threshold_above_one = 1.0;

        let result = calculate(&settings, 8 * 60 + 15);

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 0.25);
    }

    #[test]
    fn the_two_thresholds_apply_to_different_shift_counts() {
        let mut settings = settings_without_breaks();
        // Lenient below one full shift, strict above.
        settings.rounding_threshold_below_one = 0.0;
        settings.rounding_threshold_above_one = 4.0; // an hour

        // No full shift: the 10-minute remainder survives and rounds up to a period.
        let below = calculate(&settings, 10);
        assert_eq!(below.number_of_full_time_shifts(), 0);
        assert_eq!(below.remaining_work_hours(), 0.25);

        // One full shift: the same 10 minutes is now discarded.
        let above = calculate(&settings, 8 * 60 + 10);
        assert_eq!(above.number_of_full_time_shifts(), 1);
        assert_eq!(above.remaining_work_hours(), 0.0);
    }

    #[test]
    fn a_remainder_below_the_minimum_shift_is_raised_to_it() {
        let mut settings = settings_without_breaks();
        settings.min_shift_length = 4.0;

        // 8 h full shift plus 2 h, which is under the 4 h minimum.
        let result = calculate(&settings, 10 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 4.0);
    }

    #[test]
    fn a_minimum_shift_longer_than_the_shift_is_capped_at_the_shift() {
        let mut settings = settings_without_breaks();
        settings.min_shift_length = 12.0;

        // The 8-hour template caps the raise.
        let result = calculate(&settings, 10 * 60);

        assert_eq!(result.remaining_work_hours(), 8.0);
    }

    #[test]
    fn a_remainder_shorter_than_one_period_is_rounded_up_to_one() {
        let settings = settings_without_breaks();

        // 5 minutes, under the 15-minute period.
        let result = calculate(&settings, 5);

        assert_eq!(result.remaining_work_hours(), 0.25);
    }

    #[test]
    fn a_remainder_is_rounded_to_the_nearest_period() {
        let settings = settings_without_breaks();

        // 20 minutes rounds to 15, 23 minutes rounds to 30.
        assert_eq!(calculate(&settings, 20).remaining_work_hours(), 0.25);
        assert_eq!(calculate(&settings, 23).remaining_work_hours(), 0.5);
    }

    #[test]
    fn the_maximum_shift_length_caps_the_shift_when_it_is_shorter() {
        let mut settings = settings_without_breaks();
        settings.max_shift_length = 4.0;

        // Against 4-hour shifts, 10 hours is two full shifts plus 2 hours.
        let result = calculate(&settings, 10 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 2);
        assert_eq!(result.remaining_work_hours(), 2.0);
    }

    #[test]
    fn a_maximum_shift_length_longer_than_the_template_leaves_it_alone() {
        let mut settings = settings_without_breaks();
        settings.max_shift_length = 12.0;

        let result = calculate(&settings, 10 * 60);

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 2.0);
    }

    #[test]
    fn the_remainder_carries_its_own_paid_breaks() {
        let mut settings = settings_with_half_hour_break();
        settings.min_shift_length = 0.0;

        // 7.5 productive hours per shift; 13.5 h is one full shift plus 6.0 h.
        // A 6.0 h remainder passes the 5 h meal-break mark, so it gains 0.5 h.
        let result = calculate(&settings, 13 * 60 + 30);

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 6.5);
        // 0.5 h for the full shift plus 0.5 h for the remainder.
        assert_eq!(result.work_hours_to_cover_breaks(), 1.0);
    }

    #[test]
    fn a_shift_with_no_productive_time_plans_nothing() {
        // A definition without times has no length, so there is no shift to
        // divide the work into.
        let settings = settings_without_breaks();

        let result = BasicCalculatorImpl::new().calculate(
            &settings,
            &JobShiftDefinition::without_times(DayOfWeek::Monday),
            JobShiftId::new(),
            date(),
            8 * 60,
        );

        assert_eq!(result.number_of_full_time_shifts(), 0);
        assert_eq!(result.remaining_work_hours(), 0.0);
        assert_eq!(result.work_hours_to_cover_breaks(), 0.0);
    }

    #[test]
    fn the_result_carries_the_date_and_shift_it_was_calculated_for() {
        let job_shift_id = JobShiftId::new();
        let settings = settings_without_breaks();

        let result = BasicCalculatorImpl::new().calculate(
            &settings,
            &eight_hour_shift(),
            job_shift_id,
            date(),
            480,
        );

        assert_eq!(result.shift_date(), date());
        assert_eq!(result.job_shift_id(), job_shift_id);
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// These are not independently-written scenarios: every expectation below is a
/// row from `SimpleNonFlowedCalculatorTest.groovy`, transcribed unchanged, so a
/// disagreement here is a disagreement with the engine being ported.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::workcontent::domain::meal_break::MealBreak;
    use crate::workcontent::domain::non_meal_break::NonMealBreak;
    use joda_rs::{DayOfWeek, LocalTime};
    use rstest::rstest;

    /// Total minutes, written the way the Java table writes them.
    const fn hm(hours: i32, minutes: i32) -> i32 {
        hours * 60 + minutes
    }

    /// A shift template of `length` hours, starting at 09:00.
    fn shift_of(length: i32) -> JobShiftDefinition {
        JobShiftDefinition::new(
            DayOfWeek::Monday,
            LocalTime::of_hour_minute(9, 0),
            LocalTime::of_hour_minute(9 + length, 0),
            0.0,
            0.0,
            1,
        )
    }

    /// Java passes the no-break sentinel object rather than a null, and the
    /// break calculator distinguishes the two, so the sentinel is used here.
    #[allow(clippy::too_many_arguments)]
    fn calculate_with(
        shift_length: i32,
        period_length: u32,
        min_shift: f64,
        max_shift: f64,
        meal_break: MealBreak,
        non_meal_break: NonMealBreak,
        round_below_one: f64,
        round_above_one: f64,
        total_work_minutes: i32,
    ) -> BasicCalculationResult {
        let mut settings = PlannerSettings::default();
        settings.period_length = period_length;
        settings.min_shift_length = min_shift;
        settings.max_shift_length = max_shift;
        settings.meal_break = Some(meal_break);
        settings.non_meal_break = Some(non_meal_break);
        settings.rounding_threshold_below_one = round_below_one;
        settings.rounding_threshold_above_one = round_above_one;

        BasicCalculatorImpl::new().calculate(
            &settings,
            &shift_of(shift_length),
            JobShiftId::new(),
            LocalDate::new(2025, 10, 6),
            total_work_minutes,
        )
    }

    /// `SimpleNonFlowedCalculatorTest.testCalculations` — all seventeen rows.
    ///
    /// Every row runs against an eight-hour template with a maximum shift of
    /// eight hours. The first four take the same 220 hours of work through each
    /// combination of meal and rest breaks, which is where paid-break
    /// accumulation across many full shifts actually gets exercised.
    #[rstest]
    // 220 hours of work, with no breaks and then each break combination. More
    // break time per shift means more shifts are needed to cover the same work.
    #[case(0.0, 8.0, MealBreak::with_no_break(), NonMealBreak::with_no_break(), 0.0, 0.0, 30, hm(220, 0), 27, 4.0, 0.0)]
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::with_no_break(), 0.0, 0.0, 30, hm(220, 0), 29, 2.5, 14.5)]
    #[case(0.0, 8.0, MealBreak::with_no_break(), NonMealBreak::new(2.0, 0.25), 0.0, 0.0, 30, hm(220, 0), 30, 2.75, 22.75)]
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::new(2.0, 0.25), 0.0, 0.0, 30, hm(220, 0), 31, 3.25, 31.25)]
    // Two minutes of work: kept as a whole period when the below-one threshold
    // is zero, discarded once it is raised above it.
    #[case(0.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.0, 0.2, 30, hm(0, 2), 0, 0.5, 0.0)]
    #[case(0.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(0, 2), 0, 0.0, 0.0)]
    // Just over and just under a full shift's worth of productive time.
    #[case(0.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(7, 32), 1, 0.0, 0.5)]
    #[case(0.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(7, 36), 1, 0.5, 0.5)]
    // Part shifts with no minimum configured.
    #[case(0.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(2, 46), 0, 3.0, 0.0)]
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(4, 33), 0, 4.5, 0.0)]
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(12, 3), 1, 4.5, 0.5)]
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(12, 16), 1, 5.5, 1.0)]
    // The same work at a finer period length rounds the remainder differently.
    #[case(0.0, 8.0, MealBreak::new(5.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 15, hm(12, 16), 1, 4.75, 0.5)]
    // With a four-hour minimum, a surviving remainder is padded up to it.
    #[case(4.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.0, 0.2, 30, hm(0, 2), 0, 4.5, 0.5)]
    #[case(4.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(0, 2), 0, 0.0, 0.0)]
    #[case(4.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(7, 36), 1, 4.5, 1.0)]
    #[case(4.0, 8.0, MealBreak::new(4.0, 0.5), NonMealBreak::with_no_break(), 0.1, 0.2, 30, hm(2, 46), 0, 4.5, 0.5)]
    #[allow(clippy::too_many_arguments)]
    fn matches_the_java_calculation_table(
        #[case] min_shift: f64,
        #[case] max_shift: f64,
        #[case] meal_break: MealBreak,
        #[case] non_meal_break: NonMealBreak,
        #[case] round_below_one: f64,
        #[case] round_above_one: f64,
        #[case] period_length: u32,
        #[case] total_work_minutes: i32,
        #[case] expected_full_shifts: i32,
        #[case] expected_remaining_work: f64,
        #[case] expected_break_hours: f64,
    ) {
        let result = calculate_with(
            8,
            period_length,
            min_shift,
            max_shift,
            meal_break,
            non_meal_break,
            round_below_one,
            round_above_one,
            total_work_minutes,
        );

        assert_eq!(
            result.number_of_full_time_shifts(),
            expected_full_shifts,
            "full shifts"
        );
        assert_eq!(
            result.remaining_work_hours(),
            expected_remaining_work,
            "remaining work hours"
        );
        assert_eq!(
            result.work_hours_to_cover_breaks(),
            expected_break_hours,
            "work hours to cover breaks"
        );
    }

    /// `SimpleNonFlowedCalculatorTest` — "If shift length is less than Min
    /// shift then shift length takes precedence".
    ///
    /// A three-hour template cannot be padded out to a four-hour minimum, so
    /// the template wins.
    #[test]
    fn a_shift_shorter_than_the_minimum_caps_the_remainder_at_its_own_length() {
        let result = calculate_with(
            3,
            15,
            4.0,
            8.0,
            MealBreak::with_no_break(),
            NonMealBreak::with_no_break(),
            0.1,
            0.2,
            hm(4, 0),
        );

        assert_eq!(result.number_of_full_time_shifts(), 1);
        assert_eq!(result.remaining_work_hours(), 3.0);
        assert_eq!(result.work_hours_to_cover_breaks(), 0.0);
    }
}
