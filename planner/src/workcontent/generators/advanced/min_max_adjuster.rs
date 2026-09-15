//! Holds the planned head count within its configured floors and ceilings.
//!
//! The minimum is applied first and the maximum second, so where the two
//! conflict the ceiling wins. Both work in whole bodies: the fractional values
//! have already been rounded by the time they arrive here.

use crate::workcontent::domain::job_min_max_coverage::MinMaxStaffing;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::min_max_coverage::{
    min_max_coverage, populate_min_max_distribution,
};
use crate::workcontent::generators::advanced::providers::EnvironmentResolver;

pub trait MinMaxAdjuster {
    /// The head count per period, held within its configured bounds.
    fn apply_min_max_values(
        &self,
        bodies: &[DistributionItem],
        params: &GeneratorParameters,
        environments: &dyn EnvironmentResolver,
    ) -> Vec<DistributionItem>;
}

pub struct MinMaxAdjusterImpl;

impl MinMaxAdjusterImpl {
    pub fn new() -> Self {
        Self
    }
}

impl MinMaxAdjuster for MinMaxAdjusterImpl {
    fn apply_min_max_values(
        &self,
        bodies: &[DistributionItem],
        params: &GeneratorParameters,
        environments: &dyn EnvironmentResolver,
    ) -> Vec<DistributionItem> {
        let staff_values: Vec<MinMaxStaffing> = min_max_coverage(params, environments);
        let (minimums, maximums) = populate_min_max_distribution(&staff_values, params);

        let floored = apply_minimum_values(bodies, &minimums, params);

        apply_maximum_values(&floored, &maximums, params)
    }
}

/// Raises any period staffed below its floor up to it.
///
/// Only applied when the shift has work somewhere in it: a shift planned empty
/// stays empty rather than being staffed up to its minimum for no reason.
pub fn apply_minimum_values(
    bodies: &[DistributionItem],
    minimums: &[DistributionItem],
    params: &GeneratorParameters,
) -> Vec<DistributionItem> {
    let mut floored = bodies.to_vec();

    let Some(shift_periods) = params.shift_date_range().and_then(|r| r.index_range()) else {
        return floored;
    };

    if !has_work(bodies, params) {
        return floored;
    }

    for index in shift_periods {
        let index = index as usize;
        let (Some(body), Some(minimum)) = (bodies.get(index), minimums.get(index)) else {
            continue;
        };

        let staffed = whole_bodies(body).max(whole_bodies(minimum));

        floored[index].set_value_per_period(staffed as f64);
    }

    floored
}

/// Holds every period to its ceiling, carrying what will not fit forward.
///
/// Overflow normally moves into the next period, and on past the end of the
/// shift if it has to; when the settings say to truncate instead, anything over
/// the ceiling is simply dropped. A ceiling of zero means none was configured,
/// so such a period is left uncapped and absorbs any carried overflow.
pub fn apply_maximum_values(
    bodies: &[DistributionItem],
    maximums: &[DistributionItem],
    params: &GeneratorParameters,
) -> Vec<DistributionItem> {
    let mut capped = bodies.to_vec();

    let Some(shift_periods) = params.shift_date_range().and_then(|r| r.index_range()) else {
        return capped;
    };

    let delay_overflow = !params.planner_settings().truncate_max_coverage;
    let last_shift_period = *shift_periods.end();

    let overflow = apply_max(&mut capped, bodies, maximums, shift_periods, delay_overflow);

    if delay_overflow && overflow > 0 {
        distribute_overflow(&mut capped, maximums, last_shift_period + 1, overflow);
    }

    capped
}

/// Caps each period of the shift, returning what is still left over at the end.
fn apply_max(
    capped: &mut [DistributionItem],
    bodies: &[DistributionItem],
    maximums: &[DistributionItem],
    shift_periods: std::ops::RangeInclusive<i32>,
    delay_overflow: bool,
) -> i32 {
    let mut remainder = 0;

    for index in shift_periods {
        let index = index as usize;
        let (Some(body), Some(maximum)) = (bodies.get(index), maximums.get(index)) else {
            continue;
        };

        let ceiling = whole_bodies(maximum);
        let wanted = whole_bodies(body) + remainder;

        // A ceiling of zero is "uncapped", not "nobody".
        if ceiling == 0 || wanted <= ceiling {
            capped[index].set_value_per_period(wanted as f64);
            remainder = 0;
        } else {
            capped[index].set_value_per_period(ceiling as f64);
            // Without delayed overflow the excess is discarded rather than
            // carried, so the remainder stays at zero throughout.
            remainder = if delay_overflow { wanted - ceiling } else { 0 };
        }
    }

    remainder
}

/// Spills leftover bodies into the periods after the shift.
///
/// Each period takes as many as its own ceiling allows. The first period with
/// no ceiling configured ends the spill: past there the engine has no guidance
/// on what the operation can absorb, so the rest is dropped.
fn distribute_overflow(
    capped: &mut [DistributionItem],
    maximums: &[DistributionItem],
    from_index: i32,
    overflow: i32,
) {
    let mut remaining = overflow;

    for (index, period) in capped.iter_mut().enumerate().skip(from_index as usize) {
        let Some(maximum) = maximums.get(index) else {
            break;
        };

        let ceiling = whole_bodies(maximum);
        if ceiling == 0 {
            break;
        }

        if remaining > ceiling {
            period.set_value_per_period(ceiling as f64);
            remaining -= ceiling;
        } else {
            period.set_value_per_period(remaining as f64);
            break;
        }
    }
}

/// Whether any period of the shift has work planned in it.
fn has_work(bodies: &[DistributionItem], params: &GeneratorParameters) -> bool {
    let Some(shift_periods) = params.shift_date_range().and_then(|r| r.index_range()) else {
        return false;
    };

    shift_periods.into_iter().any(|index| {
        bodies
            .get(index as usize)
            .is_some_and(|body| body.value_per_period() != 0.0)
    })
}

/// Bodies are whole people; the fraction is dropped rather than rounded.
fn whole_bodies(item: &DistributionItem) -> i32 {
    crate::workcontent::common::numbers::truncate(item.value_per_period())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    const SHIFT_START_INDEX: usize = 16;

    /// An 08:00 to 20:00 shift at half-hour periods: indexes 16 through 39.
    fn context(truncate_max_coverage: bool) -> Context {
        let mut context = Context::new(
            LocalDate::new(2013, 9, 26),
            LocalTime::new(8, 0, 0),
            LocalTime::new(20, 0, 0),
            30,
        );
        context.planner_settings.truncate_max_coverage = truncate_max_coverage;
        context
    }

    fn array_from(context: &Context, start_index: usize, values: &[i32]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value as f64);
        }

        array
    }

    fn shift_values(items: &[DistributionItem]) -> Vec<i32> {
        items[SHIFT_START_INDEX..40]
            .iter()
            .map(|item| item.value_per_period() as i32)
            .collect()
    }

    /// The bodies fixture the Java suite uses, which overshoots its ceiling in
    /// several places and spikes hard near the end.
    fn bodies_per_period() -> Vec<i32> {
        vec![
            2, 2, 2, 2, 4, 3, 2, 2, 2, 2, 5, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 10, 4, 2, 3, 3,
        ]
    }

    #[test]
    fn overflow_is_carried_into_the_periods_that_follow() {
        let context = context(false);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &bodies_per_period());
        let maximums = array_from(&context, SHIFT_START_INDEX, &[3; 24]);

        let capped = apply_maximum_values(&bodies, &maximums, &params);

        assert_eq!(capped.len(), 96);
        assert_eq!(
            shift_values(&capped),
            vec![2, 2, 2, 2, 3, 3, 3, 2, 2, 2, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3]
        );
    }

    #[test]
    fn truncating_discards_the_overflow_instead_of_carrying_it() {
        let context = context(true);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &bodies_per_period());
        let maximums = array_from(&context, SHIFT_START_INDEX, &[3; 24]);

        let capped = apply_maximum_values(&bodies, &maximums, &params);

        // The periods after each overshoot fall straight back to their own
        // demand rather than inheriting the excess.
        assert_eq!(
            shift_values(&capped),
            vec![2, 2, 2, 2, 3, 3, 2, 2, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 2]
        );
    }

    #[test]
    fn a_ceiling_of_zero_is_uncapped_and_absorbs_what_was_carried() {
        let context = context(false);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &bodies_per_period());
        // Two periods in the middle have no ceiling configured.
        let mut ceilings = vec![3; 24];
        ceilings[11] = 0;
        ceilings[12] = 0;
        let maximums = array_from(&context, SHIFT_START_INDEX, &ceilings);

        let capped = apply_maximum_values(&bodies, &maximums, &params);

        // The uncapped period takes its own two bodies plus the two carried
        // into it, rather than being held to zero.
        assert_eq!(
            shift_values(&capped),
            vec![2, 2, 2, 2, 3, 3, 3, 2, 2, 2, 3, 4, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3]
        );
    }

    #[test]
    fn overflow_past_the_shift_end_fills_the_periods_that_allow_it() {
        let context = context(false);
        let params = context.params();
        // One huge spike right at the end of the shift.
        let mut bodies = array_from(&context, SHIFT_START_INDEX, &[1; 23]);
        bodies[39].set_value_per_period(10.0);
        // Ceilings run past the end of the shift, so the spill has somewhere to go.
        let maximums = array_from(&context, SHIFT_START_INDEX, &[2; 28]);

        let capped = apply_maximum_values(&bodies, &maximums, &params);

        // The shift's last period is held to two, and the eight left over spill
        // into the following periods two at a time.
        assert_eq!(capped[39].value_per_period(), 2.0);
        assert_eq!(capped[40].value_per_period(), 2.0);
        assert_eq!(capped[41].value_per_period(), 2.0);
        assert_eq!(capped[42].value_per_period(), 2.0);
        assert_eq!(capped[43].value_per_period(), 2.0);
        // Past the configured ceilings the spill stops.
        assert_eq!(capped[44].value_per_period(), 0.0);
    }

    #[test]
    fn periods_below_their_floor_are_raised_to_it() {
        let context = context(false);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &[1, 5, 2, 4]);
        let minimums = array_from(&context, SHIFT_START_INDEX, &[3; 24]);

        let floored = apply_minimum_values(&bodies, &minimums, &params);

        assert_eq!(
            shift_values(&floored),
            vec![3, 5, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3]
        );
    }

    #[test]
    fn an_empty_period_inside_a_working_shift_is_still_floored() {
        let context = context(false);
        let params = context.params();
        // A gap in the middle of otherwise busy work.
        let bodies = array_from(&context, SHIFT_START_INDEX, &[4, 0, 0, 4]);
        let minimums = array_from(&context, SHIFT_START_INDEX, &[2; 24]);

        let floored = apply_minimum_values(&bodies, &minimums, &params);

        assert_eq!(floored[SHIFT_START_INDEX].value_per_period(), 4.0);
        assert_eq!(floored[SHIFT_START_INDEX + 1].value_per_period(), 2.0);
        assert_eq!(floored[SHIFT_START_INDEX + 2].value_per_period(), 2.0);
        assert_eq!(floored[SHIFT_START_INDEX + 3].value_per_period(), 4.0);
    }

    #[test]
    fn a_shift_with_no_work_at_all_is_left_alone() {
        let context = context(false);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &[]);
        let minimums = array_from(&context, SHIFT_START_INDEX, &[3; 24]);

        let floored = apply_minimum_values(&bodies, &minimums, &params);

        assert!(floored.iter().all(|item| item.value_per_period() == 0.0));
    }

    #[test]
    fn periods_outside_the_shift_are_not_floored() {
        let context = context(false);
        let params = context.params();
        let bodies = array_from(&context, SHIFT_START_INDEX, &[4]);
        let minimums = array_from(&context, 0, &[3; 96]);

        let floored = apply_minimum_values(&bodies, &minimums, &params);

        // Before the shift starts and after it ends, the floor does not apply.
        assert_eq!(floored[SHIFT_START_INDEX - 1].value_per_period(), 0.0);
        assert_eq!(floored[40].value_per_period(), 0.0);
        assert_eq!(floored[SHIFT_START_INDEX].value_per_period(), 4.0);
    }

    #[test]
    fn the_ceiling_wins_where_the_floor_and_ceiling_conflict() {
        use crate::workcontent::domain::environment::EnvironmentId;
        use crate::workcontent::domain::job::Job;
        use crate::workcontent::domain::job_min_max_coverage::JobMinMaxCoverage;

        let mut context = context(false);
        let environment_id = EnvironmentId::new();
        let standard_set_id = context.planner_model.standard_set_id();
        // A floor of four and a ceiling of two, all day.
        context.job = Job::test().with_flowed_standards(
            Vec::new(),
            Vec::new(),
            vec![JobMinMaxCoverage::new(
                context.job.id(),
                standard_set_id,
                environment_id,
                vec![MinMaxStaffing::new(4, 2); 288],
            )],
        );

        struct Fixed(EnvironmentId);
        impl EnvironmentResolver for Fixed {
            fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
                Some(self.0)
            }
        }

        let bodies = array_from(&context, SHIFT_START_INDEX, &[1; 24]);
        let params = context.params();

        let adjusted =
            MinMaxAdjusterImpl::new().apply_min_max_values(&bodies, &params, &Fixed(environment_id));

        // Raised to the floor of four, then held down to the ceiling of two.
        assert_eq!(adjusted[SHIFT_START_INDEX].value_per_period(), 2.0);
    }
}
