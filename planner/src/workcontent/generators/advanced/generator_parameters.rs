//! The context every stage of the flowed pipeline is evaluated against.
//!
//! The flowed call graph is deep — standards fan out into distributors, into
//! spreaders, into adjusters — and nearly every level needs the same handful of
//! facts about which job, shift and date is being planned. Threading one
//! borrowed bundle keeps those signatures from growing.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition};
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::planner_settings::PlannerSettings;
use crate::workcontent::domain::standard_set::StandardSetId;
use date_range_rs::DateTimeRange;
use joda_rs::LocalDate;
use joda_rs::constants::{HOURS_PER_DAY, MINUTES_PER_HOUR};

pub struct GeneratorParameters<'a> {
    planner_model: &'a PlannerModel,
    job: &'a Job,
    planner_settings: &'a PlannerSettings,
    shift_date: LocalDate,
    shift: &'a JobShift,
    shift_detail: &'a JobShiftDefinition,
}

impl<'a> GeneratorParameters<'a> {
    pub fn new(
        planner_model: &'a PlannerModel,
        job: &'a Job,
        planner_settings: &'a PlannerSettings,
        shift_date: LocalDate,
        shift: &'a JobShift,
        shift_detail: &'a JobShiftDefinition,
    ) -> Self {
        Self {
            planner_model,
            job,
            planner_settings,
            shift_date,
            shift,
            shift_detail,
        }
    }

    pub fn planner_model(&self) -> &'a PlannerModel {
        self.planner_model
    }

    pub fn job(&self) -> &'a Job {
        self.job
    }

    pub fn planner_settings(&self) -> &'a PlannerSettings {
        self.planner_settings
    }

    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }

    pub fn shift(&self) -> &'a JobShift {
        self.shift
    }

    pub fn shift_detail(&self) -> &'a JobShiftDefinition {
        self.shift_detail
    }

    /// The standard set being planned from.
    ///
    /// Java picks per date when the plan is in dynamic-standard-set mode; that
    /// mode is not supported here, so this is always the plan-wide set.
    pub fn standard_set_id(&self) -> StandardSetId {
        self.planner_model.standard_set_id()
    }

    pub fn period_length(&self) -> i32 {
        self.planner_settings.period_length as i32
    }

    /// The shift placed on its date, at the planner's granularity.
    ///
    /// `None` for a shift with no times set, which has nowhere to put work.
    pub fn shift_date_range(&self) -> Option<DateTimeRangeWithPeriodLength> {
        self.shift_detail
            .to_date_time_range(self.shift_date)
            .map(|range| DateTimeRangeWithPeriodLength::of(range, self.period_length()))
    }

    /// The canonical two-day array's range, anchored at midnight of the shift
    /// date. Every distribution array in the pipeline is built against this, so
    /// they all share one index space.
    pub fn distribution_array_range(&self) -> DateTimeRangeWithPeriodLength {
        let start = self.shift_date.at_start_of_day();

        DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(start, start.plus_days(2)),
            self.period_length(),
        )
    }

    pub fn periods_per_day(&self) -> i32 {
        self.periods_per_hour() * HOURS_PER_DAY as i32
    }

    /// The longest shift that may be planned, counted in periods.
    pub fn max_shift_length_in_periods(&self) -> i32 {
        (self.planner_settings.max_shift_length * self.periods_per_hour() as f64) as i32
    }

    /// The shortest shift that may be planned, counted in periods.
    pub fn min_shift_length_in_periods(&self) -> i32 {
        (self.planner_settings.min_shift_length * self.periods_per_hour() as f64) as i32
    }

    fn periods_per_hour(&self) -> i32 {
        MINUTES_PER_HOUR as i32 / self.period_length()
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use std::collections::HashMap;

    /// The pieces a `GeneratorParameters` borrows from, kept alive together so
    /// tests can build one without threading six separate bindings.
    pub struct Context {
        pub planner_model: PlannerModel,
        pub job: Job,
        pub planner_settings: PlannerSettings,
        pub shift: JobShift,
        pub shift_detail: JobShiftDefinition,
        pub shift_date: LocalDate,
    }

    impl Context {
        /// A shift on `shift_date` running `start_time` to `end_time`, planned
        /// at `period_length` minute granularity.
        pub fn new(
            shift_date: LocalDate,
            start_time: LocalTime,
            end_time: LocalTime,
            period_length: u32,
        ) -> Self {
            let mut planner_settings = PlannerSettings::default();
            planner_settings.period_length = period_length;

            let shift_detail =
                JobShiftDefinition::new(DayOfWeek::Monday, start_time, end_time, 0.0, 0.0, 1);

            Self {
                planner_model: PlannerModel::new(
                    DateRange::new(shift_date, shift_date),
                    PlannerMode::Standard,
                    LocationId::new(),
                    StandardSetId::new(),
                    Vec::new(),
                    Vec::new(),
                    HashMap::new(),
                ),
                job: Job::test(),
                planner_settings,
                shift: JobShift::test(),
                shift_detail,
                shift_date,
            }
        }

        /// Cap opening and closing work at `minutes`.
        #[must_use]
        pub fn with_max_duration_minutes_for_dynamic_work(mut self, minutes: i32) -> Self {
            self.planner_model = self
                .planner_model
                .with_max_duration_minutes_for_dynamic_work(minutes);
            self
        }

        /// Plan with the legacy whole-minute flow rounding.
        #[must_use]
        pub fn with_legacy_flow_pattern_rounding(mut self) -> Self {
            self.planner_model = self.planner_model.with_legacy_flow_pattern_rounding();
            self
        }

        pub fn params(&self) -> GeneratorParameters<'_> {
            GeneratorParameters::new(
                &self.planner_model,
                &self.job,
                &self.planner_settings,
                self.shift_date,
                &self.shift,
                &self.shift_detail,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::Context;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};
    use rstest::rstest;

    fn day_shift(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        )
    }

    #[rstest]
    #[case(10, 144)]
    #[case(15, 96)]
    #[case(30, 48)]
    #[case(60, 24)]
    fn periods_per_day_follows_the_period_length(
        #[case] period_length: u32,
        #[case] expected: i32,
    ) {
        assert_eq!(day_shift(period_length).params().periods_per_day(), expected);
    }

    #[test]
    fn shift_length_bounds_are_counted_in_periods() {
        let context = day_shift(30);
        let params = context.params();

        // The defaults are a 4 hour minimum and an 8 hour maximum, which at
        // half-hour periods are 8 and 16 periods.
        assert_eq!(params.max_shift_length_in_periods(), 16);
        assert_eq!(params.min_shift_length_in_periods(), 8);
    }

    #[test]
    fn the_shift_date_range_places_the_shift_on_its_date() {
        let context = day_shift(30);
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");

        assert_eq!(range.period_length_in_minutes(), 30);
        assert_eq!(range.range().start(), LocalDateTime::new(2013, 7, 24, 8, 0, 0));
        assert_eq!(range.range().end(), LocalDateTime::new(2013, 7, 24, 16, 0, 0));
        assert_eq!(range.start_index(), Some(16));
        assert_eq!(range.end_index(), Some(32));
    }

    #[test]
    fn an_overnight_shift_ends_on_the_following_day() {
        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(23, 0, 0),
            LocalTime::new(7, 0, 0),
            30,
        );
        let range = context.params().shift_date_range().expect("has times");

        assert_eq!(range.range().end(), LocalDateTime::new(2013, 7, 25, 7, 0, 0));
        assert_eq!(range.start_index(), Some(46));
        assert_eq!(range.end_index(), Some(62));
    }

    #[test]
    fn a_shift_without_times_has_no_range() {
        let mut context = day_shift(30);
        context.shift_detail = crate::workcontent::domain::job_shift::JobShiftDefinition::without_times(
            joda_rs::DayOfWeek::Monday,
        );

        assert!(context.params().shift_date_range().is_none());
    }

    #[test]
    fn the_distribution_array_range_spans_two_days_from_midnight() {
        let context = day_shift(30);
        let params = context.params();
        let range = params.distribution_array_range();

        assert_eq!(range.range().start(), LocalDateTime::new(2013, 7, 24, 0, 0, 0));
        assert_eq!(range.range().end(), LocalDateTime::new(2013, 7, 26, 0, 0, 0));
        assert_eq!(range.period_length_in_minutes(), 30);
    }
}
