//! Places work that has to happen before the shift opens.
//!
//! Setting up runs *up to* the moment the operation starts, so the work is laid
//! backward from the end of its window. The window itself is usually only
//! half-configured — a standard says when the work must finish and lets the
//! engine work out when it has to start, or the other way round.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::distributors::Distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::spreaders::WorkSpreader;
use crate::workcontent::generators::advanced::spreaders::ending::EndingWorkSpreader;
use date_range_rs::DateTimeRange;
use joda_rs::LocalTime;

pub struct OpeningWorkDistributor {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl OpeningWorkDistributor {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl Distributor for OpeningWorkDistributor {
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
        let shift_start_time = shift_range.range().start().to_local_time();

        // Work finishes when the shift starts unless told otherwise.
        let latest_end_time = standard
            .and_then(|standard| standard.latest_work_end_time())
            .unwrap_or(shift_start_time);
        let earliest_start_time = standard
            .and_then(|standard| standard.earliest_work_start_time())
            .unwrap_or_else(|| {
                start_time_for_work(latest_end_time, work_minutes, params)
            });

        // Opening work that would begin after the shift has already opened is
        // not opening work; nothing is planned for it.
        if earliest_start_time.is_after(shift_start_time) {
            return empty;
        }

        let shift_date = params.shift_date();
        let work_start = shift_date.at_time(earliest_start_time);
        let work_end = shift_date.at_time(latest_end_time);

        // The window may reach back before the shift, but never starts later
        // than it.
        let start = if work_start.is_before(shift_range.range().start()) {
            work_start
        } else {
            shift_range.range().start()
        };

        let adjusted = DateTimeRangeWithPeriodLength::of(
            DateTimeRange::of(start, work_end),
            shift_range.period_length_in_minutes(),
        );

        let mut array = empty;
        EndingWorkSpreader::new().populate_array_with_work_minutes(
            work_minutes,
            params,
            &adjusted,
            &mut array,
        );

        array
    }
}

/// When the work has to start to finish by `latest_end_time`.
///
/// Rounded up to a whole period, and capped at the plan's ceiling for work
/// whose window is not fully pinned.
fn start_time_for_work(
    latest_end_time: LocalTime,
    work_minutes: f64,
    params: &GeneratorParameters,
) -> LocalTime {
    latest_end_time.minus_minutes(dynamic_work_minutes(work_minutes, params) as i64)
}

/// The span a block of work occupies, rounded up to a whole period and capped.
pub(super) fn dynamic_work_minutes(work_minutes: f64, params: &GeneratorParameters) -> i32 {
    let max_duration = params.planner_model().max_duration_minutes_for_dynamic_work();

    if work_minutes > max_duration as f64 {
        return max_duration;
    }

    let period_length = params.period_length();
    let whole_minutes = work_minutes as i32;

    if work_minutes % period_length as f64 == 0.0 {
        whole_minutes
    } else {
        (whole_minutes / period_length + 1) * period_length
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
    use joda_rs::LocalDate;
    use rstest::rstest;

    /// A shift on 2014-01-01 with half-hour periods.
    fn context(start: (i32, i32), end: (i32, i32), max_duration: i32) -> Context {
        Context::new(
            LocalDate::new(2014, 1, 1),
            LocalTime::new(start.0, start.1, 0),
            LocalTime::new(end.0, end.1, 0),
            30,
        )
        .with_max_duration_minutes_for_dynamic_work(max_duration)
    }

    /// A standard with the given work window.
    fn standard(
        earliest: Option<LocalTime>,
        latest: Option<LocalTime>,
    ) -> ShiftStandard {
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
        .distributed_by(DistributionMethod::Opening)
        .within_work_window(earliest, latest)
    }

    fn distribute(
        context: &Context,
        standard: &ShiftStandard,
        work_minutes: f64,
    ) -> Vec<DistributionItem> {
        OpeningWorkDistributor::new().distribute(
            work_minutes,
            Some(standard),
            &context.params(),
            &Providers::none(),
        )
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    /// Asserts the work occupies exactly `first..=last`, each holding `value`.
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
    fn work_that_would_start_after_the_shift_opens_is_not_planned() {
        // The shift opens at 08:00 but the standard says the work cannot begin
        // until 09:00 — that is not opening work.
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(
            Some(LocalTime::new(9, 0, 0)),
            Some(LocalTime::new(12, 0, 0)),
        );

        assert_eq!(total_of(&distribute(&context, &standard, 60.0)), 0.0);
    }

    #[test]
    fn a_fully_pinned_window_before_the_shift_is_used_as_given() {
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(Some(LocalTime::new(7, 0, 0)), Some(LocalTime::new(8, 0, 0)));

        let items = distribute(&context, &standard, 60.0);

        assert_eq!(total_of(&items), 60.0);
        assert_window(&items, 14, 15, 30.0);
    }

    #[test]
    fn a_window_starting_exactly_when_the_shift_does_is_still_valid() {
        let context = context((8, 0), (16, 0), 0);
        let standard = standard(Some(LocalTime::new(8, 0, 0)), Some(LocalTime::new(9, 0, 0)));

        let items = distribute(&context, &standard, 60.0);

        assert_eq!(total_of(&items), 60.0);
        assert_window(&items, 16, 17, 30.0);
    }

    #[rstest]
    // A 06:00-14:00 shift with a two-hour ceiling on dynamic work. With neither
    // end pinned the work ends when the shift opens and starts as late as it
    // can; with one end pinned the other is derived from it.
    #[case(None, None, 60.0, 10, 11, 30.0)]
    #[case(None, None, 240.0, 8, 11, 60.0)]
    #[case(Some((1, 0)), None, 240.0, 4, 11, 30.0)]
    #[case(None, Some((7, 0)), 240.0, 10, 13, 60.0)]
    #[case(Some((1, 0)), Some((7, 0)), 240.0, 6, 13, 30.0)]
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
