//! Turns a day's head-count array into the blocks of work to be covered.
//!
//! The same peeling as the planned-shift creator, with two differences that
//! matter: the block runs as long as its coverage does rather than being cut to
//! a shift's length, and a block is recorded wherever it lands — there is no
//! end-of-array guard here. Both differences are deliberate: work content
//! records what needs doing, while a planned shift records what someone will
//! actually work.

use crate::workcontent::domain::work_content::WorkContent;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_tracker;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::work_content_tracker::WorkContentTrackerBean;
use date_range_rs::DateTimeRange;

/// The work content covering `bodies_per_period`.
///
/// The array is copied first, leaving the caller's copy as it was.
pub fn create(
    bodies_per_period: &[DistributionItem],
    params: &GeneratorParameters,
) -> Vec<WorkContent> {
    let mut bodies_per_period = bodies_per_period.to_vec();
    let mut records = Vec::new();

    while let Some(mut bean) =
        distribution_item_tracker::find_first_non_zero(&bodies_per_period, params)
    {
        distribution_item_tracker::find_work_content_endpoint(&mut bean, &bodies_per_period, params);
        distribution_item_tracker::find_minimum_value_in_range(&mut bean, &bodies_per_period);
        distribution_item_tracker::deduct_minimum_value_from_list_items(
            &mut bodies_per_period,
            &bean,
        );

        records.extend(create_work_content_records(&bean, params));
    }

    records
}

/// One record per body in the layer, per plan type the run writes against.
pub fn create_work_content_records(
    bean: &WorkContentTrackerBean,
    params: &GeneratorParameters,
) -> Vec<WorkContent> {
    let work_range = DateTimeRange::of(bean.start_date_time(), bean.end_date_time());
    let plan_types = params.planner_model().plan_types();
    let mut work_contents = Vec::new();

    for _ in 0..bean.min_value() {
        for plan_type in &plan_types {
            work_contents.push(WorkContent::fixed(
                params.job().id(),
                params.job().property_id(),
                *plan_type,
                bean.start_date_time().to_local_date(),
                &work_range,
            ));
        }
    }

    work_contents
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `WorkContentRecordCreatorTest.groovy`. Its second table, on
    //! the fractional hours of each record, is `@Ignore`d upstream and is not
    //! revived here.

    use super::*;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    fn date() -> LocalDate {
        LocalDate::new(2013, 8, 26)
    }

    const ALL_ONES: [i32; 9] = [1; 9];
    const EXAMPLE_1: [i32; 9] = [1, 2, 1, 3, 4, 4, 3, 2, 1];
    const EXAMPLE_2: [i32; 9] = [0, 2, 1, 3, 4, 4, 3, 2, 1];
    const EXAMPLE_4: [i32; 48] = [
        1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3,
        3, 4, 4, 4, 4, 4, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1,
    ];

    /// Java's run writes both the forecast and the plan, so every body
    /// produces two records.
    fn context(start_time: LocalTime, end_time: LocalTime) -> Context {
        let mut context = Context::new(date(), start_time, end_time, 30);
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        context
    }

    fn bodies(context: &Context, values: &[i32]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[15 + offset].set_value_per_period(*value as f64);
        }

        array
    }

    #[rstest]
    #[case(2, ALL_ONES.to_vec(), LocalTime::new(7, 30, 0), LocalTime::new(12, 0, 0))]
    #[case(10, EXAMPLE_1.to_vec(), LocalTime::new(7, 30, 0), LocalTime::new(12, 0, 0))]
    #[case(10, EXAMPLE_2.to_vec(), LocalTime::new(7, 30, 0), LocalTime::new(12, 0, 0))]
    #[case(8, EXAMPLE_4.to_vec(), LocalTime::new(0, 0, 0), LocalTime::new(23, 59, 0))]
    fn the_number_of_records_created(
        #[case] record_count: usize,
        #[case] values: Vec<i32>,
        #[case] start_time: LocalTime,
        #[case] end_time: LocalTime,
    ) {
        let context = context(start_time, end_time);
        let params = context.params();
        let array = bodies(&context, &values);

        let work_content = create(&array, &params);

        assert_eq!(work_content.len(), record_count);
        assert_eq!(
            work_content
                .iter()
                .filter(|record| record.shift_type() == PlanType::Forecast)
                .count(),
            record_count / 2
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use crate::workcontent::generators::advanced::planned_shift_record_creator;
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
    fn an_empty_array_produces_no_work_content() {
        let context = context();
        let params = context.params();

        assert!(create(&bodies(&context, 16, &[]), &params).is_empty());
    }

    #[test]
    fn a_block_records_its_own_times_and_hours() {
        let context = context();
        let params = context.params();
        // Four hours of one body, from 08:00.
        let array = bodies(&context, 16, &[1; 8]);

        let work_content = create(&array, &params);

        assert_eq!(work_content.len(), 1);
        assert_eq!(
            work_content[0].calculated_start_date_time(),
            date().at_time(LocalTime::new(8, 0, 0))
        );
        assert_eq!(
            work_content[0].calculated_end_date_time(),
            date().at_time(LocalTime::new(12, 0, 0))
        );
        assert_eq!(work_content[0].calculated_hours(), 4.0);
        assert_eq!(work_content[0].shift_date(), date());
    }

    #[test]
    fn the_callers_array_is_left_as_it_was() {
        let context = context();
        let params = context.params();
        let array = bodies(&context, 16, &[2; 16]);

        create(&array, &params);

        assert!(
            array
                .iter()
                .skip(16)
                .take(16)
                .all(|item| item.value_per_period() == 2.0)
        );
    }

    /// The asymmetry the port must preserve: work at the very end of the array
    /// is still recorded as work content, though no shift is created to staff
    /// it. Both behaviours are confirmed by Java's own passing tests.
    #[test]
    fn work_at_the_end_of_the_array_is_recorded_even_though_no_shift_is() {
        let context = context();
        let params = context.params();
        let array = bodies(&context, 92, &[1, 1, 1, 1]);

        let work_content = create(&array, &params);
        let planned_shifts =
            planned_shift_record_creator::create_do_not_attempt_to_match_existing(&array, &params);

        assert_eq!(work_content.len(), 1);
        assert!(planned_shifts.is_empty());
    }
}
