//! Works out the paid break time a day's work incurs, and where to put it.
//!
//! Breaks are not planned from standards — they fall out of how much work there
//! already is. So this asks the non-flowed calculator the same question it
//! answers for the simple path ("how many break hours does this much work
//! earn?") and then hands the answer to the break spreader, which slots it in
//! around the work rather than on top of it.

use crate::workcontent::common::numbers;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::spreaders::WorkSpreader;
use crate::workcontent::generators::advanced::spreaders::breaks::BreakSpreader;
use crate::workcontent::generators::basic::basic_calculator::{BasicCalculator, BasicCalculatorImpl};
use joda_rs::constants::MINUTES_PER_HOUR;

/// The paid break minutes `existing_work_minutes_per_period` earns, placed.
///
/// No work means no breaks: the calculator is not consulted at all, which
/// matters because it would otherwise report the break entitlement of a
/// zero-length day.
pub fn handle_breaks(
    params: &GeneratorParameters,
    existing_work_minutes_per_period: &[DistributionItem],
) -> Vec<DistributionItem> {
    let list_creator = DistributionItemListCreatorImpl::new();

    let Some(range) = params.shift_date_range() else {
        return list_creator.create_array_for(params);
    };

    let mut breaks = list_creator.create_array(&range);
    let total_existing_work = total_work_minutes(existing_work_minutes_per_period);

    if total_existing_work <= 0 {
        return breaks;
    }

    // The same calculation the non-flowed path uses; shared rather than
    // reimplemented.
    let result = BasicCalculatorImpl::new().calculate(
        params.planner_settings(),
        params.shift_detail(),
        params.shift().id(),
        params.shift_date(),
        total_existing_work,
    );

    let break_minutes =
        numbers::round(result.work_hours_to_cover_breaks() * MINUTES_PER_HOUR as f64);

    BreakSpreader::new().populate_against_existing_work(
        break_minutes as f64,
        params,
        &range,
        &mut breaks,
        existing_work_minutes_per_period,
    );

    breaks
}

/// The day's work, rounded to whole minutes.
fn total_work_minutes(minutes_per_period: &[DistributionItem]) -> i32 {
    numbers::round(
        minutes_per_period
            .iter()
            .map(|item| item.value_per_period())
            .sum(),
    )
}

#[cfg(test)]
mod tests {
    use crate::workcontent::domain::meal_break::MealBreak;
    use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
    use crate::workcontent::domain::non_meal_break::NonMealBreak;
    use crate::workcontent::generators::advanced::break_calculator::{
        handle_breaks, total_work_minutes,
    };
    use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    /// An 08:00-16:00 shift at half-hour periods, with a half-hour meal break
    /// after five hours.
    fn context() -> Context {
        let mut context = Context::new(
            date(),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        );
        context.planner_settings.meal_break = Some(MealBreak::new(5.0, 0.5));
        context.planner_settings.non_meal_break = Some(NonMealBreak::with_no_break());
        context.planner_settings.min_shift_length = 0.0;
        context.planner_settings.max_shift_length = 8.0;
        context.planner_settings.rounding_threshold_below_one = 0.0;
        context.planner_settings.rounding_threshold_above_one = 0.0;
        context.planner_settings.non_flowed_distribution_method =
            NonFlowedDistributionMethod::BEGINNING;
        context
    }

    /// Existing work laid into the shift's periods from `start_index`.
    fn work(context: &Context, start_index: usize, values: &[f64]) -> Vec<DistributionItem> {
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value);
        }

        array
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    /// The Java suite never covered this path — it mocks the calculator, so the
    /// short-circuit is invisible to it. Closed here: a day with no work must
    /// not be asked what breaks it earns.
    #[test]
    fn no_work_earns_no_breaks() {
        let context = context();
        let empty = work(&context, 16, &[]);

        let breaks = handle_breaks(&context.params(), &empty);

        assert_eq!(total_of(&breaks), 0.0);
    }

    #[test]
    fn a_full_shifts_work_earns_its_break() {
        let context = context();
        // Seven and a half productive hours across the shift — one full shift
        // once the half-hour break is accounted for.
        let existing = work(&context, 16, &[30.0; 15]);

        let breaks = handle_breaks(&context.params(), &existing);

        // Half an hour of paid break.
        assert_eq!(total_of(&breaks), 30.0);
    }

    #[test]
    fn the_break_lands_in_the_open_tail_of_the_shift() {
        let context = context();
        // Work fills the first fifteen periods and stops, leaving the last
        // period of the shift free.
        let existing = work(&context, 16, &[30.0; 15]);

        let breaks = handle_breaks(&context.params(), &existing);

        // Period 31 is the shift's last, and the only one with room.
        assert_eq!(breaks[31].value_per_period(), 30.0);
    }

    #[test]
    fn a_shift_with_no_room_left_falls_back_to_the_default_shape() {
        let context = context();
        // Every period of the shift already full.
        let existing = work(&context, 16, &[30.0; 16]);

        let breaks = handle_breaks(&context.params(), &existing);

        // Nowhere open at the tail, so the break goes to the front instead.
        assert!(total_of(&breaks) > 0.0);
        assert_eq!(breaks[16].value_per_period(), 30.0);
    }

    #[test]
    fn more_work_earns_more_break_time() {
        let context = context();
        let one_shift = handle_breaks(&context.params(), &work(&context, 16, &[30.0; 8]));
        let two_shifts = handle_breaks(&context.params(), &work(&context, 16, &[60.0; 16]));

        assert!(total_of(&two_shifts) > total_of(&one_shift));
    }

    #[test]
    fn the_days_work_is_summed_to_whole_minutes() {
        let context = context();
        let existing = work(&context, 16, &[10.4, 10.4, 10.4]);

        // 31.2 minutes rounds to 31.
        assert_eq!(total_work_minutes(&existing), 31);
    }

    #[test]
    fn a_shift_without_times_earns_no_breaks() {
        let mut context = context();
        context.shift_detail =
            crate::workcontent::domain::job_shift::JobShiftDefinition::without_times(
                joda_rs::DayOfWeek::Monday,
            );

        let breaks = handle_breaks(&context.params(), &[]);

        assert_eq!(total_of(&breaks), 0.0);
    }
}
