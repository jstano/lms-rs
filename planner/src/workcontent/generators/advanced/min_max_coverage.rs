//! Reads configured staffing floors and ceilings into distribution arrays.
//!
//! Coverage is configured per environment at five-minute granularity, so
//! producing arrays the rest of the pipeline can use means resolving which
//! environment each day falls in, then sampling down to the planner's period
//! length.

use crate::workcontent::domain::job_min_max_coverage::MinMaxStaffing;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::EnvironmentResolver;

/// Five-minute slots in a day.
const SLOTS_PER_DAY: usize = 288;
/// Five-minute slots in an hour, the finest granularity coverage is stored at.
const SLOTS_PER_HOUR: i32 = 12;
const COVERAGE_DAYS: i64 = 2;

/// The staffing bounds covering the shift date and the day after, at
/// five-minute granularity.
///
/// A day with no coverage configured — or one whose environment cannot be
/// resolved — contributes a full day of unconfigured slots rather than being
/// left out, so the returned array always lines up with the two-day grid.
pub fn min_max_coverage(
    params: &GeneratorParameters,
    environments: &dyn EnvironmentResolver,
) -> Vec<MinMaxStaffing> {
    let mut staff_values = Vec::with_capacity(SLOTS_PER_DAY * COVERAGE_DAYS as usize);

    for day_offset in 0..COVERAGE_DAYS {
        let date = params.shift_date().plus_days(day_offset);

        // Java's coverage filter uses the date-level environment only; the
        // driver-specific tier is for standard values, not staffing bounds.
        let configured = environments
            .environment_for_date(date)
            .and_then(|environment_id| {
                params
                    .job()
                    .min_max_coverage_for_standard_set_and_environment(
                        params.standard_set_id(),
                        environment_id,
                    )
            });

        match configured {
            Some(coverage) => staff_values.extend_from_slice(coverage.staff_values()),
            None => staff_values.extend(std::iter::repeat_n(
                MinMaxStaffing::unconfigured(),
                SLOTS_PER_DAY,
            )),
        }
    }

    staff_values
}

/// The staffing bounds as two arrays at the planner's granularity.
///
/// Each period takes the bounds configured at the moment it begins, rather than
/// an average over it — the same sampling the spread values use. Periods before
/// the shift starts are left empty: bounds only apply once the shift is running.
pub fn populate_min_max_distribution(
    staff_values: &[MinMaxStaffing],
    params: &GeneratorParameters,
) -> (Vec<DistributionItem>, Vec<DistributionItem>) {
    let list_creator = DistributionItemListCreatorImpl::new();

    let Some(shift_range) = params.shift_date_range() else {
        let empty = list_creator.create_array_for(params);
        let minimums = empty.clone();

        return (minimums, empty);
    };

    let mut minimums = list_creator.create_array(&shift_range);
    let mut maximums = minimums.clone();

    let periods_per_hour = SLOTS_PER_HOUR / (shift_range.period_length_in_minutes() / 5);
    let slots_per_period = (SLOTS_PER_HOUR / periods_per_hour) as usize;
    let shift_start_index = shift_range.start_index().unwrap_or(0);

    for (period_index, slot) in (0..staff_values.len())
        .step_by(slots_per_period)
        .enumerate()
    {
        if (period_index as i32) < shift_start_index {
            continue;
        }
        let Some(staffing) = staff_values.get(slot) else {
            break;
        };
        let (Some(minimum), Some(maximum)) =
            (minimums.get_mut(period_index), maximums.get_mut(period_index))
        else {
            break;
        };

        minimum.add_to_period_value(staffing.minimum() as f64);
        maximum.add_to_period_value(staffing.maximum() as f64);
    }

    (minimums, maximums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::Job;
    use crate::workcontent::domain::job_min_max_coverage::JobMinMaxCoverage;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// Resolves every date to the same environment.
    struct FixedEnvironment(Option<EnvironmentId>);

    impl EnvironmentResolver for FixedEnvironment {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            self.0
        }
    }

    /// Resolves only the first day, leaving the second unresolved.
    struct FirstDayOnly {
        environment_id: EnvironmentId,
        first_day: LocalDate,
    }

    impl EnvironmentResolver for FirstDayOnly {
        fn environment_for_date(&self, date: LocalDate) -> Option<EnvironmentId> {
            (date == self.first_day).then_some(self.environment_id)
        }
    }

    fn context(start: LocalTime, end: LocalTime, period_length: u32) -> Context {
        Context::new(LocalDate::new(2013, 9, 26), start, end, period_length)
    }

    fn uniform_day(minimum: i32, maximum: i32) -> Vec<MinMaxStaffing> {
        vec![MinMaxStaffing::new(minimum, maximum); SLOTS_PER_DAY]
    }

    /// Half-hour blocks whose six five-minute slots each hold different values,
    /// so sampling can be told apart from averaging.
    fn varying_day() -> Vec<MinMaxStaffing> {
        (0..SLOTS_PER_DAY)
            .map(|slot| {
                let within_block = (slot % 6) as i32;
                MinMaxStaffing::new(3 + within_block, 4 + within_block)
            })
            .collect()
    }

    fn with_coverage(context: &mut Context, environment_id: EnvironmentId, day: Vec<MinMaxStaffing>) {
        let standard_set_id = context.planner_model.standard_set_id();
        context.job = Job::test().with_flowed_standards(
            Vec::new(),
            Vec::new(),
            vec![JobMinMaxCoverage::new(
                context.job.id(),
                standard_set_id,
                environment_id,
                day,
            )],
        );
    }

    #[test]
    fn coverage_spans_the_shift_date_and_the_day_after() {
        let mut context = context(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), 30);
        let environment_id = EnvironmentId::new();
        with_coverage(&mut context, environment_id, uniform_day(3, 4));

        let coverage = min_max_coverage(
            &context.params(),
            &FixedEnvironment(Some(environment_id)),
        );

        assert_eq!(coverage.len(), SLOTS_PER_DAY * 2);
        assert!(coverage.iter().all(|staffing| staffing.minimum() == 3));
    }

    #[test]
    fn a_day_with_no_coverage_falls_back_to_unconfigured_slots() {
        let mut context = context(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), 30);
        let environment_id = EnvironmentId::new();
        with_coverage(&mut context, environment_id, uniform_day(3, 4));

        let coverage = min_max_coverage(
            &context.params(),
            &FirstDayOnly {
                environment_id,
                first_day: LocalDate::new(2013, 9, 26),
            },
        );

        assert_eq!(coverage.len(), SLOTS_PER_DAY * 2);
        // The first day is configured, the second falls back.
        assert_eq!(coverage[0], MinMaxStaffing::new(3, 4));
        assert_eq!(coverage[SLOTS_PER_DAY], MinMaxStaffing::unconfigured());
        assert!(!coverage[SLOTS_PER_DAY].has_maximum());
    }

    #[test]
    fn coverage_is_unconfigured_when_no_environment_resolves() {
        let context = context(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), 30);

        let coverage = min_max_coverage(&context.params(), &FixedEnvironment(None));

        assert_eq!(coverage.len(), SLOTS_PER_DAY * 2);
        assert!(coverage.iter().all(|s| *s == MinMaxStaffing::unconfigured()));
    }

    #[rstest]
    // A daytime shift starting at 08:00, which is index 16 at half-hour periods.
    #[case(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), 30, 15, 16)]
    // An early shift starting at 04:00, index 8.
    #[case(LocalTime::new(4, 0, 0), LocalTime::new(12, 0, 0), 30, 7, 8)]
    // An overnight shift starting at 23:00, index 46.
    #[case(LocalTime::new(23, 0, 0), LocalTime::new(6, 0, 0), 30, 45, 46)]
    fn bounds_apply_from_the_shift_start_onwards(
        #[case] start: LocalTime,
        #[case] end: LocalTime,
        #[case] period_length: u32,
        #[case] last_empty_index: usize,
        #[case] first_covered_index: usize,
    ) {
        let context = context(start, end, period_length);
        let staff_values = [uniform_day(3, 4), uniform_day(3, 4)].concat();

        let (minimums, maximums) =
            populate_min_max_distribution(&staff_values, &context.params());

        assert_eq!(minimums.len(), maximums.len());
        assert_eq!(minimums.len(), 96);
        // Nothing applies before the shift begins.
        assert_eq!(minimums[last_empty_index].value_per_period(), 0.0);
        assert_eq!(maximums[last_empty_index].value_per_period(), 0.0);
        // From the shift start the configured bounds take effect.
        assert_eq!(minimums[first_covered_index].value_per_period(), 3.0);
        assert_eq!(maximums[first_covered_index].value_per_period(), 4.0);
        assert_eq!(minimums[first_covered_index + 1].value_per_period(), 3.0);
        assert_eq!(maximums[first_covered_index + 1].value_per_period(), 4.0);
    }

    #[rstest]
    #[case(30, 96, 16)]
    #[case(15, 192, 32)]
    #[case(10, 288, 48)]
    fn each_period_samples_the_bounds_it_starts_on(
        #[case] period_length: u32,
        #[case] expected_size: usize,
        #[case] shift_start_index: usize,
    ) {
        let context = context(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), period_length);
        let staff_values = [varying_day(), varying_day()].concat();

        let (minimums, maximums) =
            populate_min_max_distribution(&staff_values, &context.params());

        assert_eq!(minimums.len(), expected_size);
        assert_eq!(maximums.len(), expected_size);
        // Every slot in a half hour differs, but at half-hour periods only the
        // first is read, so the whole shift reads a constant 3 and 4.
        if period_length == 30 {
            assert!(
                minimums[shift_start_index..48]
                    .iter()
                    .all(|item| item.value_per_period() == 3.0)
            );
            assert!(
                maximums[shift_start_index..48]
                    .iter()
                    .all(|item| item.value_per_period() == 4.0)
            );
        } else {
            // At finer granularities the sampled values vary within the block
            // but are always real bounds.
            assert!(
                minimums[shift_start_index..]
                    .iter()
                    .take(10)
                    .all(|item| item.value_per_period() >= 3.0)
            );
        }
    }

    #[test]
    fn a_shift_without_times_produces_empty_arrays() {
        let mut context = context(LocalTime::new(8, 0, 0), LocalTime::new(20, 0, 0), 30);
        context.shift_detail =
            crate::workcontent::domain::job_shift::JobShiftDefinition::without_times(
                joda_rs::DayOfWeek::Monday,
            );

        let (minimums, maximums) =
            populate_min_max_distribution(&uniform_day(3, 4), &context.params());

        assert!(minimums.iter().all(|item| item.value_per_period() == 0.0));
        assert!(maximums.iter().all(|item| item.value_per_period() == 0.0));
    }
}
