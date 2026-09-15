//! Places work that has to happen after the shift closes.
//!
//! The mirror of [`OpeningWorkDistributor`](super::opening::OpeningWorkDistributor):
//! closing down runs *from* the moment the operation stops, so the work is laid
//! forward from the start of its window, and the half-configured end that gets
//! worked out is the late one rather than the early one.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::distributors::Distributor;
use crate::workcontent::generators::advanced::distributors::opening::dynamic_work_minutes;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::spreaders::WorkSpreader;
use crate::workcontent::generators::advanced::spreaders::beginning::BeginningWorkSpreader;
use date_range_rs::DateTimeRange;

pub struct ClosingWorkDistributor {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl ClosingWorkDistributor {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl Distributor for ClosingWorkDistributor {
    fn distribute(
        &self,
        work_minutes: f64,
        standard: Option<&ShiftStandard>,
        params: &GeneratorParameters,
        _providers: &Providers,
    ) -> Vec<DistributionItem> {
        let empty = self.list_creator.create_array_for(params);

        let Some(shift_range) = params.shift_date_range() else {
            return empty;
        };
        let shift_end_time = shift_range.range().end().to_local_time();

        // Work begins when the shift ends unless told otherwise.
        let earliest_start_time = standard
            .and_then(|standard| standard.earliest_work_start_time())
            .unwrap_or(shift_end_time);
        let latest_end_time = standard
            .and_then(|standard| standard.latest_work_end_time())
            .unwrap_or_else(|| {
                earliest_start_time
                    .plus_minutes(dynamic_work_minutes(work_minutes, params) as i64)
            });

        // Closing work that would be finished before the shift has closed is
        // not closing work; nothing is planned for it.
        if latest_end_time.is_before(shift_end_time) {
            return empty;
        }

        let shift_date = params.shift_date();
        let work_start = shift_date.at_time(earliest_start_time);
        let work_end = shift_date.at_time(latest_end_time);

        // The window may run past the shift, but never ends earlier than it.
        let end = if work_end.is_after(shift_range.range().end()) {
            work_end
        } else {
            shift_range.range().end()
        };

        let adjusted = DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(work_start, end),
            shift_range.period_length_in_minutes(),
        );

        let mut array = empty;
        BeginningWorkSpreader::new().populate_array_with_work_minutes(
            work_minutes,
            params,
            &adjusted,
            &mut array,
        );

        array
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::distribution_method::DistributionMethod;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::shift_standard::ShiftStandardRange;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::domain::work_type::WorkType;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    fn context(start: (i32, i32), end: (i32, i32), max_duration: i32) -> Context {
        Context::new(
            LocalDate::new(2014, 1, 1),
            LocalTime::new(start.0, start.1, 0),
            LocalTime::new(end.0, end.1, 0),
            30,
        )
        .with_max_duration_minutes_for_dynamic_work(max_duration)
    }

    fn standard(earliest: Option<LocalTime>, latest: Option<LocalTime>) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::Minutes,
            0,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        )
        .distributed_by(DistributionMethod::Closing)
        .within_work_window(earliest, latest)
    }

    fn distribute(
        context: &Context,
        standard: &ShiftStandard,
        work_minutes: f64,
    ) -> Vec<DistributionItem> {
        ClosingWorkDistributor::new().distribute(
            work_minutes,
            Some(standard),
            &context.params(),
            &Providers::none(),
        )
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    fn assert_window(items: &[DistributionItem], first: usize, last: usize, value: f64) {
        for (index, item) in items.iter().enumerate() {
            let expected = if (first..=last).contains(&index) {
                value
            } else {
                0.0
            };
            assert_eq!(item.value_per_period(), expected, "period {index}");
        }
    }

    #[test]
    fn work_that_would_finish_before_the_shift_closes_is_not_planned() {
        // The shift closes at 16:00 but the standard says the work is done by
        // 12:00 — that is not closing work.
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(
            Some(LocalTime::new(9, 0, 0)),
            Some(LocalTime::new(12, 0, 0)),
        );

        assert_eq!(total_of(&distribute(&context, &standard, 60.0)), 0.0);
    }

    #[test]
    fn a_fully_pinned_window_after_the_shift_is_used_as_given() {
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(
            Some(LocalTime::new(16, 0, 0)),
            Some(LocalTime::new(17, 0, 0)),
        );

        let items = distribute(&context, &standard, 60.0);

        assert_eq!(total_of(&items), 60.0);
        assert_window(&items, 32, 33, 30.0);
    }

    #[test]
    fn a_window_ending_exactly_when_the_shift_does_is_still_valid() {
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(
            Some(LocalTime::new(15, 0, 0)),
            Some(LocalTime::new(16, 0, 0)),
        );

        let items = distribute(&context, &standard, 60.0);

        assert_eq!(total_of(&items), 60.0);
        assert_window(&items, 30, 31, 30.0);
    }

    #[rstest]
    // A 06:00-14:00 shift with a two-hour ceiling on dynamic work. With neither
    // end pinned the work starts when the shift closes and runs as long as it
    // needs; with one end pinned the other is derived from it.
    #[case(None, None, 60.0, 28, 29, 30.0)]
    #[case(None, None, 240.0, 28, 31, 60.0)]
    #[case(None, Some((23, 0)), 240.0, 28, 35, 30.0)]
    #[case(Some((13, 0)), None, 240.0, 26, 29, 60.0)]
    #[case(Some((13, 0)), Some((23, 0)), 240.0, 26, 33, 30.0)]
    fn a_half_open_window_has_its_other_end_worked_out(
        #[case] earliest: Option<(i32, i32)>,
        #[case] latest: Option<(i32, i32)>,
        #[case] work_minutes: f64,
        #[case] first: usize,
        #[case] last: usize,
        #[case] value: f64,
    ) {
        let context = context((6, 0), (14, 0), 120);
        let standard = standard(
            earliest.map(|(hour, minute)| LocalTime::new(hour, minute, 0)),
            latest.map(|(hour, minute)| LocalTime::new(hour, minute, 0)),
        );

        let items = distribute(&context, &standard, work_minutes);

        assert_eq!(total_of(&items), work_minutes);
        assert_window(&items, first, last, value);
    }
}
