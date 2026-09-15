//! Deducts work that another schedule already covers.
//!
//! Where two jobs share coverage, the work only needs planning once. This
//! distributor is the only one that produces *negative* values: it walks the
//! shift subtracting up to a period's worth wherever a sharing schedule is in
//! force, so the aggregated total comes out lower rather than higher.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::distributors::Distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::spreaders::minutes_for_next_period;

pub struct ShareWithDistributor {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl ShareWithDistributor {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl Distributor for ShareWithDistributor {
    fn distribute(
        &self,
        work_minutes: f64,
        _standard: Option<&ShiftStandard>,
        params: &GeneratorParameters,
        providers: &Providers,
    ) -> Vec<DistributionItem> {
        let mut array = self.list_creator.create_array_for(params);

        let Some(shift_range) = params.shift_date_range() else {
            return array;
        };
        let (Some(starting_index), Some(end_index)) =
            (shift_range.start_index(), shift_range.end_index())
        else {
            return array;
        };

        let period_length = shift_range.period_length_in_minutes();
        let ending_index = end_index - 1;
        let job_id = params.job().id();

        let mut remaining = work_minutes;
        let mut current_index = starting_index;

        while remaining > 0.0 && (current_index as usize) < array.len() {
            let item_date_time = array[current_index as usize].date_time();
            current_index += 1;

            let Some(schedule) = providers
                .share_with
                .first_matching_schedule(job_id, item_date_time)
            else {
                // No schedule here at all. Note this skips the wrap-around
                // below, so the scan keeps walking forward off the end of the
                // shift and eventually runs out of array — which is how Java
                // terminates when the sharing schedule does not cover the whole
                // shift. Preserve it: without this the loop would spin forever
                // on an uncovered shift.
                continue;
            };

            let minutes = minutes_for_next_period(remaining, period_length);

            // A period landing exactly on the schedule's end is matched above
            // but not deducted from here.
            if item_date_time >= schedule.start() && item_date_time < schedule.end() {
                // Subtracted through the *adding* path, so the value is
                // rounded on the way in — unlike `subtract_from_period_value`,
                // which this deliberately does not use.
                array[(current_index - 1) as usize]
                    .add_to_period_value(-numbers::round_raw_hours(minutes));
                remaining -= minutes;
            }

            if current_index > ending_index {
                current_index = starting_index;
            }
        }

        array
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use crate::workcontent::generators::advanced::providers::ShareWithScheduleProvider;
    use date_range_rs::DateTimeRange;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    /// Sharing windows on the shift date, matched inclusively of their end —
    /// the contract `ShareWithScheduleProvider` documents.
    struct Schedules {
        date: LocalDate,
        windows: Vec<((i32, i32), (i32, i32))>,
    }

    impl ShareWithScheduleProvider for Schedules {
        fn first_matching_schedule(
            &self,
            _job_id: JobId,
            date_time: LocalDateTime,
        ) -> Option<DateTimeRange> {
            self.windows
                .iter()
                .map(|(start, end)| {
                    DateTimeRange::of(
                        self.date.at_time(LocalTime::new(start.0, start.1, 0)),
                        self.date.at_time(LocalTime::new(end.0, end.1, 0)),
                    )
                })
                .find(|window| date_time >= window.start() && date_time <= window.end())
        }
    }

    /// The Java fixture: an 08:00-16:00 shift at half-hour periods, occupying
    /// indexes 16 through 31.
    fn context() -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        )
    }

    fn distribute(work_minutes: f64, windows: Vec<((i32, i32), (i32, i32))>) -> Vec<DistributionItem> {
        let context = context();
        let schedules = Schedules {
            date: LocalDate::new(2013, 7, 24),
            windows,
        };
        let providers = Providers {
            share_with: &schedules,
            ..Providers::none()
        };

        ShareWithDistributor::new().distribute(
            work_minutes,
            None,
            &context.params(),
            &providers,
        )
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    fn assert_window(items: &[DistributionItem], first: usize, last: usize, value: f64) {
        for (index, item) in items.iter().enumerate().take(last + 1).skip(first) {
            assert_eq!(item.value_per_period(), value, "period {index}");
        }
    }

    #[test]
    fn no_work_deducts_nothing() {
        let items = distribute(0.0, vec![((8, 0), (16, 0))]);

        assert_eq!(total_of(&items), 0.0);
    }

    #[test]
    fn no_sharing_schedule_deducts_nothing() {
        let items = distribute(480.0, Vec::new());

        assert_eq!(total_of(&items), 0.0);
    }

    #[test]
    fn a_schedule_covering_the_whole_shift_absorbs_all_the_work() {
        let items = distribute(480.0, vec![((8, 0), (16, 0))]);

        assert_eq!(total_of(&items), -480.0);
        assert_window(&items, 16, 31, -30.0);
    }

    #[test]
    fn more_work_than_the_shift_holds_wraps_and_deducts_again() {
        let items = distribute(960.0, vec![((8, 0), (16, 0))]);

        assert_eq!(total_of(&items), -960.0);
        assert_window(&items, 16, 31, -60.0);
    }

    #[test]
    fn a_second_identical_schedule_changes_nothing() {
        // Only the first match is ever used.
        let items = distribute(960.0, vec![((8, 0), (16, 0)), ((8, 0), (16, 0))]);

        assert_eq!(total_of(&items), -960.0);
        assert_window(&items, 16, 31, -60.0);
    }

    /// Two adjacent windows do *not* quite behave as one: the period landing
    /// exactly on the boundary is claimed by the earlier window — which matches
    /// it inclusively — and then rejected by the exclusive check, so the later
    /// window is never consulted for it and it is skipped. The work that misses
    /// it wraps round and doubles up on the first period instead.
    ///
    /// **Diverges from the Java assertion, deliberately** — same cause as
    /// [`a_schedule_covering_half_the_shift_absorbs_only_that_half`]. That test
    /// expects a flat `-30` across the shift, which only holds because it has
    /// inherited a full-shift schedule from an earlier feature method; a full
    /// schedule covers 12:00 exclusively and so hides the boundary gap.
    #[test]
    fn two_adjacent_schedules_skip_the_period_on_their_boundary() {
        let items = distribute(480.0, vec![((8, 0), (12, 0)), ((12, 0), (16, 0))]);

        assert_eq!(total_of(&items), -480.0);
        // 12:00 belongs to neither window in practice.
        assert_window(&items, 24, 24, 0.0);
        // Its work wraps back to the front of the shift.
        assert_window(&items, 16, 16, -60.0);
        assert_window(&items, 17, 23, -30.0);
        assert_window(&items, 25, 31, -30.0);
    }

    /// A schedule covering only part of the shift can only absorb what fits
    /// inside it; the scan walks off the end rather than wrapping, so the rest
    /// of the work is simply not deducted.
    ///
    /// **Diverges from the Java assertion, deliberately.** That test expects
    /// `-480`, but its `@Shared` schedule list is never cleared between feature
    /// methods, so by the time it runs it has inherited two full-shift
    /// schedules from earlier tests and is not actually testing a half
    /// schedule at all. Run in isolation the engine deducts `-240`, which is
    /// what this asserts.
    #[test]
    fn a_schedule_covering_half_the_shift_absorbs_only_that_half() {
        let items = distribute(480.0, vec![((8, 0), (12, 0))]);

        assert_eq!(total_of(&items), -240.0);
        assert_window(&items, 16, 23, -30.0);
        // 12:00 matches the schedule's end inclusively but is not deducted
        // from, and nothing past it is covered at all.
        assert_window(&items, 24, 31, 0.0);
    }

    /// Java's own suite marks this scenario `@Ignore` with "needs to be
    /// discussed with Ken" — the intended behaviour for two schedules
    /// overlapping the same half of the shift was never settled upstream, so
    /// there is nothing authoritative to port.
    #[test]
    #[ignore = "unresolved upstream: overlapping half-schedules, Java test is @Ignore'd"]
    fn overlapping_half_schedules_are_unresolved() {
        let items = distribute(480.0, vec![((8, 0), (12, 0)), ((8, 0), (12, 0))]);

        assert_eq!(total_of(&items), -240.0);
    }
}
