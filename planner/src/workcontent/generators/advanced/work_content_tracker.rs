//! Tracks one block of work as it is peeled off a distribution array.
//!
//! Output creation works by repeatedly finding a run of non-zero periods,
//! recording how deep it goes, and subtracting that depth from the array — one
//! layer per pass. This carries the current layer's extent and depth, and
//! translates its times back into array indexes.

use crate::workcontent::common::dates;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use date_range_rs::DateTimeRange;
use joda_rs::LocalDateTime;
use joda_rs::constants::MINUTES_PER_HOUR;

pub struct WorkContentTrackerBean {
    /// When the shift this block came from began — the anchor every index is
    /// measured against, which is not necessarily when the block itself starts.
    shift_start_date_time: LocalDateTime,
    period_length: i32,
    periods_per_day: i32,
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
    /// How many bodies deep this layer runs.
    min_value: i32,
}

impl WorkContentTrackerBean {
    pub fn new(params: &GeneratorParameters) -> Self {
        let shift_start_date_time = params
            .shift_detail()
            .start_time()
            .map(|start_time| params.shift_date().at_time(start_time))
            .unwrap_or_else(|| params.shift_date().at_start_of_day());

        Self {
            shift_start_date_time,
            period_length: params.period_length(),
            periods_per_day: params.periods_per_day(),
            start_date_time: shift_start_date_time,
            end_date_time: shift_start_date_time,
            min_value: 0,
        }
    }

    pub fn shift_start_date_time(&self) -> LocalDateTime {
        self.shift_start_date_time
    }

    pub fn start_date_time(&self) -> LocalDateTime {
        self.start_date_time
    }

    pub fn set_start_date_time(&mut self, start_date_time: LocalDateTime) {
        self.start_date_time = start_date_time;
    }

    pub fn end_date_time(&self) -> LocalDateTime {
        self.end_date_time
    }

    pub fn set_end_date_time(&mut self, end_date_time: LocalDateTime) {
        self.end_date_time = end_date_time;
    }

    pub fn min_value(&self) -> i32 {
        self.min_value
    }

    pub fn set_min_value(&mut self, min_value: i32) {
        self.min_value = min_value;
    }

    pub fn date_time_range(&self) -> DateTimeRange {
        DateTimeRange::of(self.start_date_time, self.end_date_time)
    }

    /// Where this block starts in the distribution array.
    ///
    /// A block that begins after midnight of the shift's own date is a
    /// continuation of it, so it counts on into the array's second day rather
    /// than wrapping back to zero.
    pub fn starting_index(&self) -> i32 {
        let index = self.index_of_time(self.start_date_time);

        if self.start_date_time.to_local_date() > self.shift_start_date_time.to_local_date() {
            index + self.periods_per_day
        } else {
            index
        }
    }

    /// Where this block ends in the distribution array, counting on through any
    /// days between the shift's start and the block's end.
    pub fn ending_index(&self) -> i32 {
        let days_past_shift_start = dates::days_between(
            self.shift_start_date_time.to_local_date(),
            self.end_date_time.to_local_date(),
        ) as i32;

        self.index_of_time(self.end_date_time) + days_past_shift_start * self.periods_per_day
    }

    /// The period a time falls in, rounding down.
    fn index_of_time(&self, date_time: LocalDateTime) -> i32 {
        let minutes_from_midnight =
            date_time.hour() * MINUTES_PER_HOUR as i32 + date_time.minute();

        minutes_from_midnight / self.period_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// The Java suite pins these against a half-hour period length, where a day
    /// is 48 periods.
    fn context(start_time: LocalTime, end_time: LocalTime) -> Context {
        Context::new(LocalDate::new(2014, 1, 1), start_time, end_time, 30)
    }

    fn bean_for(
        context: &Context,
        start_date_time: LocalDateTime,
        end_date_time: LocalDateTime,
    ) -> WorkContentTrackerBean {
        let params = context.params();
        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(start_date_time);
        bean.set_end_date_time(end_date_time);
        bean
    }

    #[test]
    fn it_carries_the_block_extent_and_depth() {
        let context = context(LocalTime::new(6, 30, 0), LocalTime::new(22, 30, 0));
        let start = LocalDateTime::new(2014, 1, 1, 6, 30, 0);
        let end = LocalDateTime::new(2014, 1, 1, 22, 30, 0);

        let mut bean = bean_for(&context, start, end);
        bean.set_min_value(100);

        assert_eq!(bean.start_date_time(), start);
        assert_eq!(bean.end_date_time(), end);
        assert_eq!(bean.min_value(), 100);
        assert_eq!(bean.date_time_range().start(), start);
        assert_eq!(bean.date_time_range().end(), end);
    }

    #[rstest]
    // Blocks contained within the shift's own day.
    #[case(
        LocalDateTime::new(2014, 1, 1, 0, 0, 0),
        LocalDateTime::new(2014, 1, 1, 8, 0, 0),
        0,
        16
    )]
    #[case(
        LocalDateTime::new(2014, 1, 1, 8, 0, 0),
        LocalDateTime::new(2014, 1, 1, 16, 0, 0),
        16,
        32
    )]
    // Blocks running past midnight keep counting into the second day rather
    // than wrapping.
    #[case(
        LocalDateTime::new(2014, 1, 1, 23, 30, 0),
        LocalDateTime::new(2014, 1, 2, 7, 30, 0),
        47,
        63
    )]
    #[case(
        LocalDateTime::new(2014, 1, 1, 22, 0, 0),
        LocalDateTime::new(2014, 1, 2, 6, 0, 0),
        44,
        60
    )]
    fn indexes_are_measured_from_the_shift_start(
        #[case] start_date_time: LocalDateTime,
        #[case] end_date_time: LocalDateTime,
        #[case] expected_start_index: i32,
        #[case] expected_end_index: i32,
    ) {
        let context = context(
            start_date_time.to_local_time(),
            end_date_time.to_local_time(),
        );
        let bean = bean_for(&context, start_date_time, end_date_time);

        assert_eq!(bean.starting_index(), expected_start_index);
        assert_eq!(bean.ending_index(), expected_end_index);
    }

    #[test]
    fn a_block_starting_the_day_after_the_shift_counts_into_the_second_day() {
        // The shift starts at 22:00 on the 1st; a block starting at 02:00 on
        // the 2nd belongs to it, so it indexes at 48 + 4 rather than 4.
        let context = context(LocalTime::new(22, 0, 0), LocalTime::new(6, 0, 0));
        let bean = bean_for(
            &context,
            LocalDateTime::new(2014, 1, 2, 2, 0, 0),
            LocalDateTime::new(2014, 1, 2, 6, 0, 0),
        );

        assert_eq!(bean.starting_index(), 52);
        assert_eq!(bean.ending_index(), 60);
    }

    #[test]
    fn a_new_bean_starts_empty_at_the_shift_start() {
        let context = context(LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0));
        let params = context.params();
        let bean = WorkContentTrackerBean::new(&params);

        assert_eq!(
            bean.shift_start_date_time(),
            LocalDateTime::new(2014, 1, 1, 8, 0, 0)
        );
        assert_eq!(bean.min_value(), 0);
        assert_eq!(bean.starting_index(), 16);
    }
}
