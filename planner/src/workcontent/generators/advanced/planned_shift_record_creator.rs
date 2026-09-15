//! Turns a day's head-count array into the shifts that will staff it.
//!
//! The array is peeled one layer at a time: find where coverage starts, run it
//! out to a shift's worth, stretch it to the minimum shift, take the depth that
//! runs the whole way, and subtract it. Each layer becomes that many shifts.
//!
//! The array is copied first — the caller's copy is left as it was, because the
//! same array is also peeled by the work-content creator, which must see the
//! original.

use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_tracker;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::planned_shift_creator;

/// Create the shifts, dropping any that duplicate what is already scheduled.
pub fn create_or_match_existing(
    bodies_per_period: &[DistributionItem],
    params: &GeneratorParameters,
    matcher: &mut ExistingPlannedShiftMatcher,
) -> Vec<PlannedShift> {
    create(bodies_per_period, params, Some(matcher))
}

/// Create the shifts unconditionally, without consulting the schedule.
pub fn create_do_not_attempt_to_match_existing(
    bodies_per_period: &[DistributionItem],
    params: &GeneratorParameters,
) -> Vec<PlannedShift> {
    create(bodies_per_period, params, None)
}

fn create(
    bodies_per_period: &[DistributionItem],
    params: &GeneratorParameters,
    matcher: Option<&mut ExistingPlannedShiftMatcher>,
) -> Vec<PlannedShift> {
    // A plan that allows no shift length at all can staff nothing. Java also
    // puts a message on the run's progress here; progress is not modelled.
    if params.planner_settings().max_shift_length == 0.0 {
        return Vec::new();
    }

    create_records(&mut bodies_per_period.to_vec(), params, matcher)
}

fn create_records(
    bodies_per_period: &mut [DistributionItem],
    params: &GeneratorParameters,
    matcher: Option<&mut ExistingPlannedShiftMatcher>,
) -> Vec<PlannedShift> {
    let mut matcher = matcher;
    let mut records = Vec::new();

    while let Some(mut bean) =
        distribution_item_tracker::find_first_non_zero(bodies_per_period, params)
    {
        if params.planner_settings().generate_long_shifts {
            distribution_item_tracker::find_planned_long_shift_endpoint(
                &mut bean,
                bodies_per_period,
                params,
            );
        } else {
            distribution_item_tracker::find_planned_shift_endpoint(
                &mut bean,
                bodies_per_period,
                params,
            );
        }

        // Without shift times there is no window to stretch the block toward,
        // so it keeps the length its coverage gives it.
        if let Some(shift_range) = params.shift_date_range() {
            distribution_item_tracker::adjust_for_min_shift(&mut bean, shift_range.range().end(), params);
        }

        distribution_item_tracker::find_planned_shift_minimum_value_in_range(
            &mut bean,
            bodies_per_period,
        );
        distribution_item_tracker::deduct_minimum_value_from_list_items(bodies_per_period, &bean);

        // A layer that reaches the end of the array is dropped rather than
        // staffed: it has been stretched into the space past the plan and is no
        // longer a shift anyone works. The deduction above still stands, so the
        // loop terminates.
        //
        // The work-content creator deliberately has no such guard — it records
        // the work regardless of where the array ends. Do not harmonise them.
        if is_at_end_of_periods(&bean, bodies_per_period) {
            break;
        }

        records.extend(planned_shift_creator::create_planned_shift_records(
            &bean,
            params,
            matcher.as_deref_mut(),
        ));
    }

    records
}

fn is_at_end_of_periods(
    bean: &crate::workcontent::generators::advanced::work_content_tracker::WorkContentTrackerBean,
    bodies_per_period: &[DistributionItem],
) -> bool {
    bean.ending_index() >= bodies_per_period.len() as i32 - 1
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `PlannedShiftRecordCreatorTest.groovy` — the strongest
    //! end-to-end fixtures in the engine, since every step of the peel shows up
    //! in the times and durations of the shifts that come out.

    use super::*;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2013, 8, 26)
    }

    fn time(hour: i32, minute: i32) -> LocalTime {
        LocalTime::new(hour, minute, 0)
    }

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        date().at_time(time(hour, minute))
    }

    /// The Java fixture arrays, all laid in at index 16 (08:00).
    const EXAMPLE_1: [i32; 16] = [1, 1, 2, 2, 4, 3, 2, 2, 1, 1, 1, 1, 2, 1, 1, 1];
    const EXAMPLE_3: [i32; 24] = [
        1, 1, 2, 2, 4, 3, 2, 2, 1, 1, 1, 1, 2, 1, 1, 2, 1, 1, 1, 1, 2, 2, 2, 1,
    ];
    const EXAMPLE_4: [i32; 16] = [1; 16];

    fn context(end_time: LocalTime, generate_long_shifts: bool) -> Context {
        let mut context = Context::new(date(), time(8, 0), end_time, 30);
        context.planner_settings.min_shift_length = 4.0;
        context.planner_settings.max_shift_length = 8.0;
        context.planner_settings.generate_long_shifts = generate_long_shifts;
        context
    }

    fn bodies(context: &Context, values: &[i32]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[16 + offset].set_value_per_period(*value as f64);
        }

        array
    }

    /// `(duration, start, end)` of each shift, which is what Java asserts.
    fn shape(planned_shifts: &[PlannedShift]) -> Vec<(f64, LocalTime, LocalTime)> {
        planned_shifts
            .iter()
            .map(|shift| {
                (
                    shift.duration(),
                    shift
                        .start_date_time()
                        .expect("a start time")
                        .to_local_time(),
                    shift.end_date_time().expect("an end time").to_local_time(),
                )
            })
            .collect()
    }

    #[test]
    fn max_shift_of_zero_returns_an_empty_list() {
        let mut context = context(time(16, 0), false);
        context.planner_settings.min_shift_length = 0.0;
        context.planner_settings.max_shift_length = 0.0;
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_1);

        let planned_shifts = create_or_match_existing(
            &array,
            &params,
            &mut ExistingPlannedShiftMatcher::empty(),
        );

        assert!(planned_shifts.is_empty());
    }

    #[test]
    fn shift_ends_at_4_pm_without_long_shift_logic() {
        let context = context(time(16, 0), false);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_1);

        let planned_shifts = create_or_match_existing(
            &array,
            &params,
            &mut ExistingPlannedShiftMatcher::empty(),
        );

        assert_eq!(
            shape(&planned_shifts),
            vec![
                (8.0, time(8, 0), time(16, 0)),
                (4.0, time(9, 0), time(13, 0)),
                (4.0, time(10, 0), time(14, 0)),
                (4.0, time(10, 0), time(14, 0)),
                (4.0, time(12, 0), time(16, 0)),
            ]
        );
        assert!(
            planned_shifts
                .iter()
                .all(|shift| shift.shift_date() == Some(date()))
        );
        assert_eq!(planned_shifts[0].start_date_time(), Some(at(8, 0)));
        assert_eq!(planned_shifts[0].end_date_time(), Some(at(16, 0)));
    }

    /// The same array with long shifts allowed: the second layer spans its gap
    /// instead of being cut at it, so five shifts become four.
    #[test]
    fn shift_ends_at_4_pm_and_generates_long_shifts() {
        let context = context(time(16, 0), true);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_1);

        let planned_shifts = create_or_match_existing(
            &array,
            &params,
            &mut ExistingPlannedShiftMatcher::empty(),
        );

        assert_eq!(
            shape(&planned_shifts),
            vec![
                (8.0, time(8, 0), time(16, 0)),
                (5.5, time(9, 0), time(14, 30)),
                (4.0, time(10, 0), time(14, 0)),
                (4.0, time(10, 0), time(14, 0)),
            ]
        );
    }

    #[test]
    fn shift_ends_at_8_pm_and_generates_long_shifts() {
        let context = context(time(20, 0), true);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_3);

        let planned_shifts = create_or_match_existing(
            &array,
            &params,
            &mut ExistingPlannedShiftMatcher::empty(),
        );

        assert_eq!(
            shape(&planned_shifts),
            vec![
                (8.0, time(8, 0), time(16, 0)),
                (8.0, time(9, 0), time(17, 0)),
                (8.0, time(10, 0), time(18, 0)),
                (4.0, time(10, 0), time(14, 0)),
                (4.0, time(16, 0), time(20, 0)),
                (4.0, time(16, 0), time(20, 0)),
            ]
        );
    }

    /// A flat layer across the whole shift, already on the schedule: the
    /// forecast copy is matched away and only the plan's copy is written.
    #[test]
    fn matching_shift_processing_works_for_a_scheduled_shift() {
        let mut context = context(time(16, 0), false);
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_4);
        let existing = create_do_not_attempt_to_match_existing(&array, &params)
            .into_iter()
            .find(|shift| shift.shift_type() == Some(PlanType::Forecast))
            .expect("a forecast shift");
        let mut matcher = ExistingPlannedShiftMatcher::new(vec![existing], Vec::new(), false);

        let planned_shifts = create_or_match_existing(&array, &params, &mut matcher);

        assert_eq!(planned_shifts.len(), 1);
        assert_eq!(planned_shifts[0].shift_type(), Some(PlanType::Original));
    }

    #[test]
    fn matching_shift_processing_works_for_a_shift_with_a_pending_request() {
        let mut context = context(time(16, 0), false);
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_4);
        let existing = create_do_not_attempt_to_match_existing(&array, &params)
            .into_iter()
            .find(|shift| shift.shift_type() == Some(PlanType::Forecast))
            .expect("a forecast shift");
        let mut matcher = ExistingPlannedShiftMatcher::new(Vec::new(), vec![existing], false);

        let planned_shifts = create_or_match_existing(&array, &params, &mut matcher);

        assert_eq!(planned_shifts.len(), 1);
        assert_eq!(planned_shifts[0].shift_type(), Some(PlanType::Original));
    }

    /// The same schedule, asked not to match: both copies are written.
    #[test]
    fn the_method_that_does_not_try_to_match_existing_shifts_works() {
        let mut context = context(time(16, 0), false);
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        let params = context.params();
        let array = bodies(&context, &EXAMPLE_4);

        let planned_shifts = create_do_not_attempt_to_match_existing(&array, &params);

        assert_eq!(planned_shifts.len(), 2);
        assert_eq!(planned_shifts[0].shift_type(), Some(PlanType::Forecast));
        assert_eq!(planned_shifts[1].shift_type(), Some(PlanType::Original));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2013, 8, 26)
    }

    fn context() -> Context {
        let mut context = Context::new(
            date(),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        );
        context.planner_settings.min_shift_length = 4.0;
        context.planner_settings.max_shift_length = 8.0;
        context
    }

    fn bodies(context: &Context, start_index: usize, values: &[i32]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value as f64);
        }

        array
    }

    #[test]
    fn an_empty_array_staffs_nothing() {
        let context = context();
        let params = context.params();
        let array = bodies(&context, 16, &[]);

        assert!(
            create_do_not_attempt_to_match_existing(&array, &params).is_empty()
        );
    }

    /// The caller's array is peeled from a copy, because the work-content
    /// creator peels the same array afterwards and must see it untouched.
    #[test]
    fn the_callers_array_is_left_as_it_was() {
        let context = context();
        let params = context.params();
        let array = bodies(&context, 16, &[2; 16]);

        create_do_not_attempt_to_match_existing(&array, &params);

        assert!(
            array
                .iter()
                .skip(16)
                .take(16)
                .all(|item| item.value_per_period() == 2.0)
        );
    }

    /// Work stretched into the very end of the array is dropped rather than
    /// staffed — the guard the work-content creator deliberately lacks.
    #[test]
    fn a_layer_reaching_the_end_of_the_array_is_not_staffed() {
        let context = context();
        let params = context.params();
        // Coverage in the array's final periods, past any real shift.
        let array = bodies(&context, 92, &[1, 1, 1, 1]);

        let planned_shifts = create_do_not_attempt_to_match_existing(&array, &params);

        assert!(planned_shifts.is_empty());
    }
}
