//! Turns a day's split into actual blocks of work on the calendar.
//!
//! Ported from Java's `SimpleNonFlowedWorkContentCreator` and
//! `WorkContentCreatorUtility`. Full shifts are laid down in pairs, one at each
//! end of the template, so the day fills from the outside in; an odd shift goes
//! at the start, and the remainder last.

use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::JobShiftDefinition;
use crate::workcontent::domain::plan_type::PlanType;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::work_content::WorkContent;
use crate::workcontent::generators::basic::basic_calculator::BasicCalculationResult;
use crate::workcontent::generators::basic::work_content_calculator::WorkContentCalculator;
use date_range_rs::DateTimeRange;
use joda_rs::LocalDate;

pub trait BasicWorkContentCreator {
    fn create_work_content(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift_detail: &JobShiftDefinition,
        result: &BasicCalculationResult,
        shift_date: LocalDate,
    ) -> Vec<WorkContent>;
}

pub struct BasicWorkContentCreatorImpl;

impl BasicWorkContentCreatorImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl BasicWorkContentCreator for BasicWorkContentCreatorImpl {
    fn create_work_content(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift_detail: &JobShiftDefinition,
        result: &BasicCalculationResult,
        shift_date: LocalDate,
    ) -> Vec<WorkContent> {
        let calculator = WorkContentCalculator::new(
            result,
            job.planner_settings(),
            shift_detail,
            shift_date,
        );
        let plan_types = planner_model.plan_types();

        let mut work_contents = Vec::new();
        let mut add_block = |range: Option<DateTimeRange>| {
            if let Some(range) = range {
                for plan_type in &plan_types {
                    work_contents.push(fixed_work_content(
                        job,
                        *plan_type,
                        shift_date,
                        &range,
                    ));
                }
            }
        };

        let paired_full_shifts = calculator.number_of_full_shifts() / 2;

        for _ in 0..paired_full_shifts {
            add_block(calculator.full_shift_range_for_start_of_shift());
            add_block(calculator.full_shift_range_for_end_of_shift());
        }

        if calculator.number_of_full_shifts() % 2 != 0 {
            add_block(calculator.full_shift_range_for_start_of_shift());
        }

        if result.has_remaining_work() {
            add_block(calculator.remaining_work_range());
        }

        work_contents
    }
}

fn fixed_work_content(
    job: &Job,
    plan_type: PlanType,
    shift_date: LocalDate,
    range: &DateTimeRange,
) -> WorkContent {
    WorkContent::fixed(job.id(), job.property_id(), plan_type, shift_date, range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::generators::basic::basic_calculator::{
        BasicCalculator, BasicCalculatorImpl,
    };
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use std::collections::HashMap;

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    fn time(hour: i32, minute: i32) -> LocalTime {
        LocalTime::of_hour_minute(hour, minute)
    }

    /// A 24-hour template, so several 8-hour shifts fit inside it.
    fn all_day_shift() -> JobShiftDefinition {
        JobShiftDefinition::new(DayOfWeek::Monday, time(0, 0), time(0, 0), 0.0, 0.0, 1)
    }

    fn settings() -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.period_length = 15;
        settings.min_shift_length = 0.0;
        settings.max_shift_length = 8.0;
        settings.rounding_threshold_below_one = 0.0;
        settings.rounding_threshold_above_one = 0.0;
        settings
    }

    fn job() -> Job {
        Job::new(
            LocationId::new(),
            settings(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn model(planner_mode: PlannerMode) -> PlannerModel {
        PlannerModel::new(
            DateRange::new(date(), date()),
            planner_mode,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        )
    }

    fn work_content(
        planner_mode: PlannerMode,
        total_work_minutes: i32,
    ) -> (Vec<WorkContent>, BasicCalculationResult) {
        let job = job();
        let shift_detail = all_day_shift();
        let result = BasicCalculatorImpl::new().calculate(
            job.planner_settings(),
            &shift_detail,
            JobShiftId::new(),
            date(),
            total_work_minutes,
        );

        let work_contents = BasicWorkContentCreatorImpl::new().create_work_content(
            &model(planner_mode),
            &job,
            &shift_detail,
            &result,
            date(),
        );

        (work_contents, result)
    }

    fn blocks(total_work_minutes: i32) -> Vec<WorkContent> {
        // A single plan type means one work content per block.
        work_content(PlannerMode::Standard, total_work_minutes).0
    }

    #[test]
    fn no_work_produces_no_blocks() {
        assert!(blocks(0).is_empty());
    }

    #[test]
    fn one_full_shift_is_placed_at_the_start_of_the_day() {
        let blocks = blocks(8 * 60);

        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].calculated_start_date_time(),
            date().at_time(time(0, 0))
        );
        assert_eq!(
            blocks[0].calculated_end_date_time(),
            date().at_time(time(8, 0))
        );
    }

    #[test]
    fn two_full_shifts_are_placed_at_each_end_of_the_day() {
        let blocks = blocks(16 * 60);

        assert_eq!(blocks.len(), 2);
        // Start of day.
        assert_eq!(
            blocks[0].calculated_start_date_time(),
            date().at_time(time(0, 0))
        );
        // End of day: the last 8 hours before midnight.
        assert_eq!(
            blocks[1].calculated_start_date_time(),
            date().at_time(time(16, 0))
        );
        assert_eq!(
            blocks[1].calculated_end_date_time(),
            date().plus_days(1).at_time(time(0, 0))
        );
    }

    #[test]
    fn an_odd_third_full_shift_goes_at_the_start() {
        let blocks = blocks(24 * 60);

        assert_eq!(blocks.len(), 3);
        assert_eq!(
            blocks[2].calculated_start_date_time(),
            date().at_time(time(0, 0))
        );
        assert_eq!(
            blocks[2].calculated_end_date_time(),
            date().at_time(time(8, 0))
        );
    }

    #[test]
    fn a_remainder_adds_one_more_block() {
        // Two full 8-hour shifts plus 2 hours.
        let blocks = blocks(18 * 60);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[2].calculated_hours(), 2.0);
        assert_eq!(
            blocks[2].calculated_start_date_time(),
            date().at_time(time(0, 0))
        );
    }

    #[test]
    fn work_shorter_than_one_shift_is_a_single_remainder_block() {
        let blocks = blocks(3 * 60);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].calculated_hours(), 3.0);
    }

    #[test]
    fn every_block_is_created_once_per_plan_type() {
        // A projected run writes both the forecast and the original plan.
        let (work_contents, _) = work_content(PlannerMode::Projected, 18 * 60);

        assert_eq!(work_contents.len(), 6);
        assert_eq!(
            work_contents
                .iter()
                .filter(|wc| wc.shift_type() == PlanType::Forecast)
                .count(),
            3
        );
        assert_eq!(
            work_contents
                .iter()
                .filter(|wc| wc.shift_type() == PlanType::Original)
                .count(),
            3
        );
    }

    #[test]
    fn blocks_are_labelled_with_the_job_and_date() {
        let job = job();
        let shift_detail = all_day_shift();
        let result = BasicCalculatorImpl::new().calculate(
            job.planner_settings(),
            &shift_detail,
            JobShiftId::new(),
            date(),
            8 * 60,
        );

        let work_contents = BasicWorkContentCreatorImpl::new().create_work_content(
            &model(PlannerMode::Standard),
            &job,
            &shift_detail,
            &result,
            date(),
        );

        assert_eq!(work_contents[0].job_id(), job.id());
        assert_eq!(work_contents[0].property_id(), job.property_id());
        assert_eq!(work_contents[0].shift_date(), date());
    }

    #[test]
    fn a_shift_without_times_produces_no_blocks() {
        let job = job();
        let shift_detail = JobShiftDefinition::without_times(DayOfWeek::Monday);
        let result = BasicCalculatorImpl::new().calculate(
            job.planner_settings(),
            &shift_detail,
            JobShiftId::new(),
            date(),
            8 * 60,
        );

        let work_contents = BasicWorkContentCreatorImpl::new().create_work_content(
            &model(PlannerMode::Standard),
            &job,
            &shift_detail,
            &result,
            date(),
        );

        assert!(work_contents.is_empty());
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// Every expectation below is from `SimpleNonFlowedWorkContentCreatorTest.groovy`,
/// transcribed unchanged. The Java fixture plans in PROJECTED mode, which writes
/// both the forecast and the original plan, so every block appears twice.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDateTime, LocalTime};
    use rstest::rstest;
    use std::collections::HashMap;

    fn shift_date() -> LocalDate {
        LocalDate::new(2013, 12, 25)
    }

    /// The Java fixture's planner settings: a four-hour minimum, an eight-hour
    /// maximum, quarter-hour periods.
    fn job() -> Job {
        let mut settings = PlannerSettings::default();
        settings.min_shift_length = 4.0;
        settings.max_shift_length = 8.0;
        settings.period_length = 15;

        Job::new(
            LocationId::new(),
            settings,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn projected_model() -> PlannerModel {
        PlannerModel::new(
            DateRange::new(shift_date(), shift_date()),
            PlannerMode::Projected,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        )
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

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        shift_date().at_time(LocalTime::of_hour_minute(hour, minute))
    }

    /// The blocks, together with the job they were planned for — the identity
    /// checks below compare against that same job, not a fresh one.
    fn create(
        shift_detail: &JobShiftDefinition,
        full_shifts: i32,
        remaining_work_hours: f64,
    ) -> (Vec<WorkContent>, Job) {
        let job = job();
        let result = BasicCalculationResult::with_values(
            full_shifts,
            remaining_work_hours,
            0.0,
            shift_date(),
            JobShiftId::new(),
        );

        let work_contents = BasicWorkContentCreatorImpl::new().create_work_content(
            &projected_model(),
            &job,
            shift_detail,
            &result,
            shift_date(),
        );

        (work_contents, job)
    }

    /// How many blocks of `plan_type` start at `start` and end at `end`.
    fn count_blocks(
        work_contents: &[WorkContent],
        plan_type: PlanType,
        start: LocalDateTime,
        end: LocalDateTime,
    ) -> usize {
        work_contents
            .iter()
            .filter(|content| {
                content.shift_type() == plan_type
                    && content.preferred_start_date_time() == start
                    && content.latest_end_date_time() == end
            })
            .count()
    }

    /// The invariants the Java suite asserts over every record it produces.
    fn assert_common_conditions(work_contents: &[WorkContent], job: &Job) {
        for content in work_contents {
            assert_eq!(content.job_id(), job.id());
            assert_eq!(content.property_id(), job.property_id());
            assert_eq!(content.shift_date(), shift_date());
            assert_eq!(
                content.earliest_start_date_time(),
                content.preferred_start_date_time()
            );
            assert_eq!(
                content.earliest_start_date_time(),
                content.calculated_start_date_time()
            );

            let hours = DateTimeRange::of(
                content.earliest_start_date_time(),
                content.latest_end_date_time(),
            )
            .duration()
            .fractional_hours();
            assert_eq!(content.calculated_hours(), hours);
            assert_eq!(content.adjusted_hours(), hours);
            assert!(!content.is_locked());
        }
    }

    #[rstest]
    // A shift already within the maximum: every block occupies the whole
    // template, so they are all identical.
    #[case(28, (7, 0), (15, 0))]
    #[case(10, (9, 0), (14, 0))]
    fn a_shift_within_the_maximum_makes_every_block_the_same(
        #[case] full_shifts: i32,
        #[case] start: (i32, i32),
        #[case] end: (i32, i32),
    ) {
        let shift = shift_from(start, end);
        let (work_contents, job) = create(&shift, full_shifts, 0.0);

        assert_eq!(work_contents.len(), (full_shifts * 2) as usize);
        assert_common_conditions(&work_contents, &job);

        let (block_start, block_end) = (at(start.0, start.1), at(end.0, end.1));
        for plan_type in [PlanType::Original, PlanType::Forecast] {
            assert_eq!(
                count_blocks(&work_contents, plan_type, block_start, block_end),
                full_shifts as usize
            );
        }
    }

    /// A ten-hour template against an eight-hour maximum: half the blocks are
    /// anchored to the start of the template and half to the end.
    #[test]
    fn a_shift_longer_than_the_maximum_splits_blocks_between_both_ends() {
        let shift = shift_from((7, 0), (17, 0));
        let (work_contents, job) = create(&shift, 28, 0.0);

        assert_eq!(work_contents.len(), 56);
        assert_common_conditions(&work_contents, &job);

        for plan_type in [PlanType::Original, PlanType::Forecast] {
            assert_eq!(
                count_blocks(&work_contents, plan_type, at(7, 0), at(15, 0)),
                14
            );
            assert_eq!(
                count_blocks(&work_contents, plan_type, at(9, 0), at(17, 0)),
                14
            );
        }
    }

    /// A remainder block always starts when the shift does.
    #[test]
    fn a_remainder_block_starts_at_the_beginning_of_the_shift() {
        let shift = shift_from((7, 0), (17, 0));
        let (work_contents, job) = create(&shift, 0, 4.5);

        assert_eq!(work_contents.len(), 2);
        assert_common_conditions(&work_contents, &job);

        for plan_type in [PlanType::Original, PlanType::Forecast] {
            assert_eq!(
                count_blocks(&work_contents, plan_type, at(7, 0), at(11, 30)),
                1
            );
        }
    }
}
