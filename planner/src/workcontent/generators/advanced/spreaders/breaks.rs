//! Places paid break time around the work already planned.
//!
//! Breaks are unlike other work: they should land where a shift is already
//! running but not yet full, rather than creating fresh coverage. So this
//! spreader first looks for room at the tail of the shift — periods where fewer
//! people are working than at the busiest point — and fills backward-detected
//! gaps there. Only what will not fit falls through to the ordinary shape from
//! the planner settings.

use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::common::numbers;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::beginning::BeginningWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::ending::EndingWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::even::EvenWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::middle::MiddleWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::{WorkSpreader, spread_bounds};

pub struct BreakSpreader;

impl BreakSpreader {
    pub fn new() -> Self {
        Self
    }
}

impl WorkSpreader for BreakSpreader {
    /// With nothing already planned there is no tail to fill, so this is just
    /// the fallback shape.
    fn populate_array_with_work_minutes(
        &self,
        total_work_minutes: f64,
        params: &GeneratorParameters,
        range: &DateTimeRangeWithPeriodLength,
        target: &mut [DistributionItem],
    ) {
        self.populate_against_existing_work(total_work_minutes, params, range, target, &[]);
    }

    fn populate_against_existing_work(
        &self,
        total_work_minutes: f64,
        params: &GeneratorParameters,
        range: &DateTimeRangeWithPeriodLength,
        target: &mut [DistributionItem],
        existing_work: &[DistributionItem],
    ) {
        let remaining = if contains_work(existing_work) {
            fill_work_into_end_of_shift(total_work_minutes, params, range, target, existing_work)
        } else {
            total_work_minutes
        };

        // Java compares against zero with a one-ULP tolerance, which for a
        // value that reached exactly zero by subtraction is the same as an
        // equality test.
        if remaining != 0.0 {
            spread_remaining_work(remaining, params, range, target);
        }
    }
}

fn contains_work(existing_work: &[DistributionItem]) -> bool {
    existing_work
        .iter()
        .any(|item| item.value_per_period() > 0.0)
}

/// Fill break minutes into the under-covered tail of the shift.
///
/// Returns what could not be placed there.
fn fill_work_into_end_of_shift(
    total_work_minutes: f64,
    params: &GeneratorParameters,
    range: &DateTimeRangeWithPeriodLength,
    target: &mut [DistributionItem],
    existing_work: &[DistributionItem],
) -> f64 {
    let Some((starting_index, ending_index)) = spread_bounds(params, range) else {
        return total_work_minutes;
    };
    if starting_index < 0 || ending_index < starting_index {
        return total_work_minutes;
    }

    let period_length = range.period_length_in_minutes();
    let busiest_shift_count = existing_work
        .iter()
        .map(|item| rounded_shift_count(item.value_per_period(), params))
        .max()
        .unwrap_or(1);

    let open_shifts = open_shifts_by_period(
        existing_work,
        params,
        busiest_shift_count,
        starting_index,
        ending_index,
    );

    // The tail must itself be open; a shift running at full strength right to
    // the end leaves nowhere for a break to go, whatever room sits earlier.
    if open_shifts[ending_index as usize] == 0 {
        return total_work_minutes;
    }

    let mut remaining = total_work_minutes;

    for index in starting_index..=ending_index {
        if remaining <= 0.0 {
            break;
        }

        let open = open_shifts[index as usize];
        if open == 0 {
            continue;
        }

        let max_allocation = (period_length * open) as f64;
        let allocation = max_allocation.min(remaining);

        let Some(item) = target.get_mut(index as usize) else {
            break;
        };

        item.add_to_period_value(allocation);
        remaining -= allocation;
    }

    remaining
}

/// How many shifts are running but not full, per period, walking back from the
/// end of the shift.
///
/// The walk stops at the first fully-covered period, so only an unbroken run of
/// open periods at the tail is usable. Each period is also capped by the run so
/// far: room cannot reappear once it has narrowed. Note that a period with no
/// work at all counts as fully *open* rather than as a barrier, so a gap in the
/// middle of the tail does not end the run.
fn open_shifts_by_period(
    existing_work: &[DistributionItem],
    params: &GeneratorParameters,
    busiest_shift_count: i32,
    starting_index: i32,
    ending_index: i32,
) -> Vec<i32> {
    let mut open_shifts = vec![0; (ending_index + 1) as usize];
    let mut previous_open = busiest_shift_count;

    // Stops *above* the starting index: the first period of the shift is never
    // treated as open, matching Java's loop bound.
    let mut index = ending_index;
    while index > starting_index {
        let worked = existing_work
            .get(index as usize)
            .map(|item| rounded_shift_count(item.value_per_period(), params))
            .unwrap_or(0);
        let open_in_period = busiest_shift_count - worked;

        if open_in_period == 0 {
            break;
        }

        open_shifts[index as usize] = open_in_period.min(previous_open);
        previous_open = open_shifts[index as usize];
        index -= 1;
    }

    open_shifts
}

/// How many whole shifts a period's minutes represent.
///
/// A part-period counts as another shift once it reaches the configured
/// rounding threshold — the same "is this worth a body" question the
/// minutes-to-bodies conversion asks, but expressed in minutes.
fn rounded_shift_count(value_per_period: f64, params: &GeneratorParameters) -> i32 {
    let period_length = params.period_length();
    let threshold_minutes = rounding_threshold_minutes(value_per_period, params);
    let full_periods = numbers::truncate(value_per_period / period_length as f64);
    let partial_minutes = numbers::truncate(value_per_period % period_length as f64);

    if partial_minutes > 0 && partial_minutes >= threshold_minutes {
        full_periods + 1
    } else {
        full_periods
    }
}

/// The minutes a part-period must reach to count as another shift.
fn rounding_threshold_minutes(value_per_period: f64, params: &GeneratorParameters) -> i32 {
    let settings = params.planner_settings();
    let period_length = params.period_length();

    let threshold = if value_per_period < period_length as f64 {
        settings.rounding_threshold_below_one
    } else {
        settings.rounding_threshold_above_one
    };

    numbers::round(period_length as f64 * threshold)
}

/// Place whatever would not fit at the tail using the configured default shape.
///
/// `Varying` has no branch in Java's switch and so does nothing at all. That is
/// reachable configuration rather than a misconfiguration, so it is replicated
/// as a silent no-op rather than an error.
fn spread_remaining_work(
    total_work_minutes: f64,
    params: &GeneratorParameters,
    range: &DateTimeRangeWithPeriodLength,
    target: &mut [DistributionItem],
) {
    let spreader: Box<dyn WorkSpreader> = match params.planner_settings().non_flowed_distribution_method {
        NonFlowedDistributionMethod::BEGINNING => Box::new(BeginningWorkSpreader::new()),
        NonFlowedDistributionMethod::MIDDLE => Box::new(MiddleWorkSpreader::new()),
        NonFlowedDistributionMethod::END => Box::new(EndingWorkSpreader::new()),
        NonFlowedDistributionMethod::EVEN => Box::new(EvenWorkSpreader::new()),
        NonFlowedDistributionMethod::VARYING => return,
    };

    spreader.populate_array_with_work_minutes(total_work_minutes, params, range, target);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// The Java fixture: a 03:00 to 11:00 shift at quarter-hour periods, so the
    /// shift occupies 12 through 43 of a 192-period array.
    fn context(method: NonFlowedDistributionMethod) -> Context {
        let mut context = Context::new(
            LocalDate::new(2015, 12, 16),
            LocalTime::new(3, 0, 0),
            LocalTime::new(11, 0, 0),
            15,
        );
        context.planner_settings.max_shift_length = 8.0;
        context.planner_settings.limit_shift_to_max_shift = true;
        context.planner_settings.rounding_threshold_below_one = 0.0;
        context.planner_settings.rounding_threshold_above_one = 0.0;
        context.planner_settings.non_flowed_distribution_method = method;
        context
    }

    fn empty_array(context: &Context) -> Vec<DistributionItem> {
        DistributionItemListCreatorImpl::new().create_array_for(&context.params())
    }

    /// An existing-work array with `values` laid in at the given indexes.
    fn existing_with(context: &Context, values: &[(usize, f64)]) -> Vec<DistributionItem> {
        let mut array = empty_array(context);
        for (index, value) in values {
            array[*index].add_to_period_value(*value);
        }
        array
    }

    fn spread(
        context: &Context,
        total_work_minutes: f64,
        existing_work: &[DistributionItem],
    ) -> Vec<DistributionItem> {
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = empty_array(context);

        BreakSpreader::new().populate_against_existing_work(
            total_work_minutes,
            &params,
            &range,
            &mut array,
            existing_work,
        );

        array
    }

    /// Asserts exactly the given periods hold the given values and every other
    /// period is empty.
    fn assert_only(items: &[DistributionItem], expected: &[(usize, f64)]) {
        assert_eq!(items.len(), 192);

        for (index, item) in items.iter().enumerate() {
            let expected_value = expected
                .iter()
                .find(|(at, _)| *at == index)
                .map(|(_, value)| *value)
                .unwrap_or(0.0);

            assert_eq!(item.value_per_period(), expected_value, "period {index}");
        }
    }

    #[rstest]
    // With nothing already planned, the break falls straight through to the
    // configured shape. Five quarter-hours of break, placed four ways.
    #[case(NonFlowedDistributionMethod::BEGINNING, 75.0, 12, 16, 15.0)]
    #[case(NonFlowedDistributionMethod::END, 75.0, 39, 43, 15.0)]
    #[case(NonFlowedDistributionMethod::MIDDLE, 75.0, 25, 29, 15.0)]
    #[case(NonFlowedDistributionMethod::EVEN, 64.0, 12, 43, 2.0)]
    fn with_no_existing_work_the_default_shape_decides(
        #[case] method: NonFlowedDistributionMethod,
        #[case] break_minutes: f64,
        #[case] first: usize,
        #[case] last: usize,
        #[case] value: f64,
    ) {
        let context = context(method);
        let items = spread(&context, break_minutes, &[]);

        let expected: Vec<_> = (first..=last).map(|index| (index, value)).collect();
        assert_only(&items, &expected);
    }

    #[rstest]
    // The shift's last period is fully covered, so there is no open tail and
    // the whole break falls through to the default shape.
    #[case(NonFlowedDistributionMethod::BEGINNING, 75.0, 12, 16, 15.0)]
    #[case(NonFlowedDistributionMethod::END, 75.0, 39, 43, 15.0)]
    #[case(NonFlowedDistributionMethod::MIDDLE, 75.0, 25, 29, 15.0)]
    #[case(NonFlowedDistributionMethod::EVEN, 64.0, 12, 43, 2.0)]
    fn a_shift_full_to_the_last_period_leaves_no_room_for_a_break(
        #[case] method: NonFlowedDistributionMethod,
        #[case] break_minutes: f64,
        #[case] first: usize,
        #[case] last: usize,
        #[case] value: f64,
    ) {
        let context = context(method);
        let existing = existing_with(&context, &[(43, 28.0)]);

        let items = spread(&context, break_minutes, &existing);

        let expected: Vec<_> = (first..=last).map(|index| (index, value)).collect();
        assert_only(&items, &expected);
    }

    #[test]
    fn an_open_tail_absorbs_the_whole_break() {
        // One shift's worth of work ending at period 40 leaves 41, 42 and 43
        // open, which is enough for all forty minutes.
        let context = context(NonFlowedDistributionMethod::BEGINNING);
        let existing = existing_with(&context, &[(40, 2.4)]);

        let items = spread(&context, 40.0, &existing);

        assert_only(&items, &[(41, 15.0), (42, 15.0), (43, 10.0)]);
    }

    #[test]
    fn a_gap_in_the_middle_of_the_tail_still_counts_as_open() {
        // Work runs at three shifts to period 35, drops to two for 36 and 37,
        // stops entirely for 38 and 39, then resumes at one shift to the end.
        // The empty periods are the most open of all, so the run is unbroken
        // and the break fills 36 through 40.
        let context = context(NonFlowedDistributionMethod::BEGINNING);
        let existing = existing_with(
            &context,
            &[
                (30, 33.0),
                (31, 33.0),
                (32, 33.0),
                (33, 33.0),
                (34, 33.0),
                (35, 33.0),
                (36, 25.0),
                (37, 25.0),
                (40, 8.0),
                (41, 8.0),
                (42, 8.0),
                (43, 8.0),
            ],
        );

        let items = spread(&context, 100.0, &existing);

        assert_only(
            &items,
            &[
                (36, 15.0),
                (37, 15.0),
                (38, 30.0),
                (39, 30.0),
                (40, 10.0),
            ],
        );
    }

    #[rstest]
    // Whether a part-period counts as another shift — and so closes the tail —
    // depends on the rounding thresholds. Work sits at periods 41 and 42; the
    // break lands at 42 or 43 depending on how far back the open run reaches.
    #[case([15.0, 1.0], 0.0, 0.2, [0.0, 15.0])]
    #[case([15.0, 1.0], 0.1, 0.2, [15.0, 0.0])]
    #[case([15.0, 2.0], 0.1, 0.2, [0.0, 15.0])]
    #[case([30.0, 16.0], 0.1, 0.0, [0.0, 15.0])]
    #[case([30.0, 17.0], 0.1, 0.2, [15.0, 0.0])]
    #[case([30.0, 18.0], 0.1, 0.2, [0.0, 15.0])]
    fn the_rounding_thresholds_decide_whether_a_period_is_open(
        #[case] existing_values: [f64; 2],
        #[case] threshold_below_one: f64,
        #[case] threshold_above_one: f64,
        #[case] expected: [f64; 2],
    ) {
        let mut context = context(NonFlowedDistributionMethod::BEGINNING);
        context.planner_settings.rounding_threshold_below_one = threshold_below_one;
        context.planner_settings.rounding_threshold_above_one = threshold_above_one;

        let existing = existing_with(
            &context,
            &[(41, existing_values[0]), (42, existing_values[1])],
        );

        let items = spread(&context, 15.0, &existing);

        assert_only(&items, &[(42, expected[0]), (43, expected[1])]);
    }

    /// Java's switch has no `VARYING` branch, so a break that cannot fit at the
    /// tail is simply dropped under that setting. Reachable configuration, not
    /// a misconfiguration.
    #[test]
    fn a_varying_default_silently_places_nothing() {
        let context = context(NonFlowedDistributionMethod::VARYING);

        let items = spread(&context, 75.0, &[]);

        assert_only(&items, &[]);
    }
}
