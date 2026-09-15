//! Threads one shift's work through the whole flowed pipeline.
//!
//! The order here is the engine. Each stage consumes the last one's output, and
//! the same operations recur at different points against different inputs — so
//! a step in the wrong place produces plausible numbers that are quietly wrong.
//! The sequence is:
//!
//! 1. collect every non-staffing source and add them together
//! 2. work out the breaks that work earns, and add those in
//! 3. convert to bodies, then hold them inside the staffing bounds
//! 4. cost the staffing work, which needs the bodies from step 3
//! 5. combine **the un-broken non-staffing minutes** with the staffing minutes
//! 6. recompute breaks over that combined total, add them in
//! 7. convert to bodies again and re-apply the bounds — these are final
//! 8. write the work content and the shifts that will cover it
//!
//! Step 5 is the one to be careful of: it goes back to the minutes *before*
//! breaks were added at step 2. Breaks are worked out afresh over the combined
//! work, not carried forward, because the combined day earns a different break
//! entitlement than the non-staffing half did alone.

use crate::workcontent::generators::advanced::break_calculator::handle_breaks;
use crate::workcontent::generators::advanced::distribution_aggregator::{
    DistributionAggregator, DistributionAggregatorImpl,
};
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::kbi_related_results::AdvancedResults;
use crate::workcontent::generators::advanced::min_max_adjuster::{
    MinMaxAdjuster, MinMaxAdjusterImpl,
};
use crate::workcontent::generators::advanced::minutes_to_bodies::{
    MinutesToBodiesConverter, MinutesToBodiesConverterImpl,
};
use crate::workcontent::generators::advanced::non_staff_standards::{self, NonStaffResults};
use crate::workcontent::generators::advanced::planned_shift_record_creator;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::staff_standards;
use crate::workcontent::generators::advanced::work_content_record_creator;
use crate::workcontent::generators::error::GenerationError;

/// Steps 1 to 3: every non-staffing source, through breaks and bounds.
///
/// The minutes carried forward in the result are the ones from step 1 — that
/// is, **without** breaks. Step 5 needs them in that state.
pub fn process_non_staff_standards(
    params: &GeneratorParameters,
    providers: &Providers,
) -> Result<NonStaffResults, GenerationError> {
    let aggregator = DistributionAggregatorImpl::new();

    let sources = non_staff_standards::process(params, providers)?;
    let borrowed: Vec<&[DistributionItem]> =
        sources.iter().map(|source| source.as_slice()).collect();

    let work_minutes = aggregator.aggregate(params, &borrowed);
    let breaks = handle_breaks(params, &work_minutes);
    let with_breaks = aggregator.aggregate(params, &[&work_minutes, &breaks]);

    let bodies = MinutesToBodiesConverterImpl::new().convert(params, &with_breaks);
    let within_bounds =
        MinMaxAdjusterImpl::new().apply_min_max_values(&bodies, params, providers.environments);

    Ok(NonStaffResults::new(work_minutes, within_bounds))
}

/// Steps 5 to 7: combine, re-break, and settle on the final head count.
///
/// `non_staff_work_minutes` must be the pre-break minutes from
/// [`process_non_staff_standards`]; passing the with-break total instead would
/// count every break twice.
pub fn calculate_final_bodies(
    params: &GeneratorParameters,
    providers: &Providers,
    non_staff_work_minutes: &[DistributionItem],
    staff_work_minutes: &[DistributionItem],
) -> Vec<DistributionItem> {
    let with_breaks = combine_with_breaks(params, non_staff_work_minutes, staff_work_minutes);
    let bodies = MinutesToBodiesConverterImpl::new().convert(params, &with_breaks);

    // The authoritative pass: this is the head count the day is planned from.
    MinMaxAdjusterImpl::new().apply_min_max_values(&bodies, params, providers.environments)
}

/// Steps 5 and 6: the day's whole work, breaks included.
///
/// Split out from [`calculate_final_bodies`] because it is where
/// double-counted breaks would show, and minutes show it plainly where a
/// rounded head count can hide it.
pub fn combine_with_breaks(
    params: &GeneratorParameters,
    non_staff_work_minutes: &[DistributionItem],
    staff_work_minutes: &[DistributionItem],
) -> Vec<DistributionItem> {
    let aggregator = DistributionAggregatorImpl::new();

    let combined = aggregator.aggregate(params, &[non_staff_work_minutes, staff_work_minutes]);
    let breaks = handle_breaks(params, &combined);

    aggregator.aggregate(params, &[&combined, &breaks])
}

/// The whole sequence, steps 1 to 8: one shift's standards in, the work
/// content and shifts that cover them out.
///
/// The `matcher` carries what is already on the schedule, so shifts that would
/// duplicate an existing one are not written twice.
pub fn process_standards_for_shift(
    params: &GeneratorParameters,
    providers: &Providers,
    matcher: &mut ExistingPlannedShiftMatcher,
) -> Result<AdvancedResults, GenerationError> {
    let non_staff_results = process_non_staff_standards(params, providers)?;

    let staff_work = process_staff_standards(
        params,
        providers,
        non_staff_results.non_staff_bodies_after_breaks_applied(),
    )?;
    let staff_work_minutes = aggregate_staff_minutes(params, &staff_work);

    let final_bodies = calculate_final_bodies(
        params,
        providers,
        non_staff_results.non_staff_work_minutes_per_period(),
        &staff_work_minutes,
    );

    let mut results = AdvancedResults::new();
    results.add_work_contents(work_content_record_creator::create(&final_bodies, params));
    results.add_planned_shifts(planned_shift_record_creator::create_or_match_existing(
        &final_bodies,
        params,
        matcher,
    ));

    Ok(results)
}

/// Step 4: cost the staffing work against the shifts the non-staffing work
/// would need.
///
/// Those shifts are provisional — they exist only to be counted, and are thrown
/// away. The shifts that are actually written come from the final head count at
/// step 7, after the staffing work has been folded back in.
pub fn process_staff_standards(
    params: &GeneratorParameters,
    providers: &Providers,
    non_staff_bodies: &[DistributionItem],
) -> Result<Vec<Vec<DistributionItem>>, GenerationError> {
    let provisional_shifts =
        planned_shift_record_creator::create_do_not_attempt_to_match_existing(
            non_staff_bodies,
            params,
        );

    staff_standards::process(params, providers, &provisional_shifts)
}

/// Step 4's aggregation: the staffing arrays added together.
pub fn aggregate_staff_minutes(
    params: &GeneratorParameters,
    staff_work: &[Vec<DistributionItem>],
) -> Vec<DistributionItem> {
    let borrowed: Vec<&[DistributionItem]> =
        staff_work.iter().map(|source| source.as_slice()).collect();

    DistributionAggregatorImpl::new().aggregate(params, &borrowed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::distribution_method::DistributionMethod;
    use crate::workcontent::domain::environment::EnvironmentId;
    use crate::workcontent::domain::job::{Job, JobId};
    use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::meal_break::MealBreak;
    use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
    use crate::workcontent::domain::non_meal_break::NonMealBreak;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::domain::planned_shift::PlannedShift;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_model::PlannerModel;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::shift_standard::{
        ShiftStandard, ShiftStandardRange, ShiftStandardValue,
    };
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::units::Units;
    use crate::workcontent::domain::work_type::WorkType;
    use crate::workcontent::generators::advanced::providers::EnvironmentResolver;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDate, LocalTime};
    use std::collections::HashMap;

    fn date() -> LocalDate {
        LocalDate::new(2013, 7, 24)
    }

    struct OneEnvironment(EnvironmentId);

    impl EnvironmentResolver for OneEnvironment {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            Some(self.0)
        }
    }

    struct Fixture {
        planner_model: PlannerModel,
        job: Job,
        planner_settings: PlannerSettings,
        shift: JobShift,
        shift_detail: JobShiftDefinition,
    }

    impl Fixture {
        fn params(&self) -> GeneratorParameters<'_> {
            GeneratorParameters::new(
                &self.planner_model,
                &self.job,
                &self.planner_settings,
                date(),
                &self.shift,
                &self.shift_detail,
            )
        }
    }

    /// An 08:00-16:00 shift with a half-hour meal break, carrying one
    /// non-flowed standard worth `minutes_per_cover` per cover.
    fn fixture(
        environment_id: EnvironmentId,
        business_driver_id: BusinessDriverId,
        covers: i32,
        minutes_per_cover: f64,
    ) -> Fixture {
        let standard_set_id = StandardSetId::new();
        let detail = JobShiftDefinition::new(
            DayOfWeek::Wednesday,
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            0.0,
            0.0,
            1,
        );
        let shift = JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            vec![detail],
        );

        let standards = vec![
            ShiftStandard::new(
                JobId::new(),
                standard_set_id,
                shift.id(),
                business_driver_id,
                WorkType::Variable,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::with_values(
                    0,
                    100_000,
                    vec![ShiftStandardValue::new(environment_id, minutes_per_cover)],
                )],
            )
            .distributed_by(DistributionMethod::NonFlowed)
            .with_non_flowed_distribution_method(NonFlowedDistributionMethod::EVEN),
        ];

        let mut planner_settings = PlannerSettings::default();
        planner_settings.period_length = 30;
        planner_settings.meal_break = Some(MealBreak::new(5.0, 0.5));
        planner_settings.non_meal_break = Some(NonMealBreak::with_no_break());
        planner_settings.min_shift_length = 0.0;
        planner_settings.max_shift_length = 8.0;
        planner_settings.rounding_threshold_below_one = 0.0;
        planner_settings.rounding_threshold_above_one = 0.0;
        planner_settings.non_flowed_distribution_method = NonFlowedDistributionMethod::EVEN;

        Fixture {
            planner_model: PlannerModel::new(
                DateRange::new(date(), date()),
                PlannerMode::Standard,
                LocationId::new(),
                standard_set_id,
                Vec::new(),
                Vec::new(),
                HashMap::from([(
                    business_driver_id,
                    BusinessDriverValues::new(
                        business_driver_id,
                        HashMap::from([(date(), covers)]),
                    ),
                )]),
            ),
            job: Job::new(
                LocationId::new(),
                PlannerSettings::default(),
                Vec::new(),
                Vec::new(),
                standards,
            ),
            planner_settings,
            shift_detail: detail,
            shift,
        }
    }

    fn count_of(planned_shifts: &[PlannedShift], plan_type: PlanType) -> usize {
        planned_shifts
            .iter()
            .filter(|shift| shift.shift_type() == Some(plan_type))
            .count()
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    fn providers_for(environments: &OneEnvironment) -> Providers<'_> {
        Providers {
            environments,
            ..Providers::none()
        }
    }

    #[test]
    fn a_shift_with_no_standards_reaches_the_end_with_nothing() {
        let environment_id = EnvironmentId::new();
        let fixture = fixture(environment_id, BusinessDriverId::new(), 0, 0.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);

        let results =
            process_non_staff_standards(&fixture.params(), &providers).expect("all supported");

        assert_eq!(total_of(results.non_staff_work_minutes_per_period()), 0.0);
        assert_eq!(total_of(results.non_staff_bodies_after_breaks_applied()), 0.0);
    }

    #[test]
    fn the_minutes_carried_forward_are_the_ones_before_breaks() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        // Four hundred and eighty minutes of work: one full shift's worth.
        let fixture = fixture(environment_id, business_driver_id, 480, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);

        let results =
            process_non_staff_standards(&fixture.params(), &providers).expect("all supported");

        // Exactly the standard's own minutes, with no break time folded in —
        // step 5 depends on this.
        assert_eq!(total_of(results.non_staff_work_minutes_per_period()), 480.0);
    }

    #[test]
    fn the_work_becomes_a_head_count_within_the_staffing_bounds() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        // Two full shifts' worth of work across sixteen half-hour periods.
        let fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);

        let results =
            process_non_staff_standards(&fixture.params(), &providers).expect("all supported");
        let bodies = results.non_staff_bodies_after_breaks_applied();

        // Sixty minutes a period is two bodies, and the break time spread
        // over the shift pushes each period a little past that — which, with
        // the rounding thresholds at zero, rounds up to three.
        assert_eq!(bodies[16].value_per_period(), 3.0);
        assert_eq!(bodies[31].value_per_period(), 3.0);
    }

    /// Breaks are recomputed over the combined day, not carried forward — so
    /// running the combining stage on a day whose staffing work is zero still
    /// gives the same answer as the non-staffing stage did alone.
    #[test]
    fn combining_with_no_staffing_work_reproduces_the_non_staffing_bodies() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let non_staff = process_non_staff_standards(&params, &providers).expect("all supported");
        let no_staff_work = aggregate_staff_minutes(&params, &[]);

        let final_bodies = calculate_final_bodies(
            &params,
            &providers,
            non_staff.non_staff_work_minutes_per_period(),
            &no_staff_work,
        );

        assert_eq!(
            total_of(&final_bodies),
            total_of(non_staff.non_staff_bodies_after_breaks_applied())
        );
    }

    /// The guard against the mistake this stage is most prone to: handing the
    /// combining step the *with-break* minutes counts the day's breaks twice.
    ///
    /// Asserted on the minutes rather than the head count — a rounded body
    /// count can absorb the difference and show nothing wrong.
    #[test]
    fn feeding_the_combining_step_broken_minutes_would_double_the_breaks() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let non_staff = process_non_staff_standards(&params, &providers).expect("all supported");
        let no_staff_work = aggregate_staff_minutes(&params, &[]);

        let work_minutes = total_of(non_staff.non_staff_work_minutes_per_period());
        let correct = combine_with_breaks(
            &params,
            non_staff.non_staff_work_minutes_per_period(),
            &no_staff_work,
        );

        // Simulate the mistake: hand it the minutes with breaks already in.
        let breaks = handle_breaks(&params, non_staff.non_staff_work_minutes_per_period());
        let already_broken = DistributionAggregatorImpl::new().aggregate(
            &params,
            &[non_staff.non_staff_work_minutes_per_period(), &breaks],
        );
        let doubled = combine_with_breaks(&params, &already_broken, &no_staff_work);

        // The day's work plus one break entitlement...
        assert_eq!(total_of(&correct), work_minutes + total_of(&breaks));
        // ...against the same work with two.
        assert!(
            total_of(&doubled) > total_of(&correct),
            "double-counted breaks should inflate the day's minutes"
        );
    }

    /// The whole sequence end to end: a day's standards become work content
    /// and the shifts to cover it.
    #[test]
    fn the_full_sequence_produces_work_content_and_the_shifts_to_cover_it() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        // Two full shifts' worth of work.
        let fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let results = process_standards_for_shift(
            &params,
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(!results.work_contents().is_empty());
        assert!(!results.planned_shifts().is_empty());
        // Every shift falls on the day being planned.
        assert!(
            results
                .planned_shifts()
                .iter()
                .all(|shift| shift.shift_date() == Some(date()))
        );
    }

    /// Steps 4 and 8 both peel the array, and they must not agree by accident:
    /// step 4's shifts come from the non-staffing bodies and are thrown away,
    /// while step 8's come from the final head count.
    #[test]
    fn the_provisional_shifts_counted_at_step_4_are_not_the_shifts_written() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let non_staff = process_non_staff_standards(&params, &providers).expect("all supported");
        let provisional =
            planned_shift_record_creator::create_do_not_attempt_to_match_existing(
                non_staff.non_staff_bodies_after_breaks_applied(),
                &params,
            );

        let results = process_standards_for_shift(
            &params,
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        // With no staffing standards configured the two happen to agree in
        // count, but they are separately derived arrays — none of the
        // provisional shifts is one of the written ones.
        assert!(!provisional.is_empty());
        assert!(
            provisional
                .iter()
                .all(|shift| !results
                    .planned_shifts()
                    .iter()
                    .any(|written| written.id() == shift.id()))
        );
    }

    /// Forecast shifts already on the schedule are not written a second time.
    ///
    /// Only the forecast copy is ever checked — the plan's own copies are
    /// written every run regardless of what is scheduled, so replanning a day
    /// leaves the forecast alone and refreshes the plan.
    #[test]
    fn forecast_shifts_already_on_the_schedule_are_matched_rather_than_duplicated() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let mut fixture = fixture(environment_id, business_driver_id, 960, 1.0);
        fixture.planner_model = fixture.planner_model.with_planner_mode(PlannerMode::Projected);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let first = process_standards_for_shift(
            &params,
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        let forecast_count = count_of(first.planned_shifts(), PlanType::Forecast);
        let original_count = count_of(first.planned_shifts(), PlanType::Original);

        // Plan the same day again with the first run's shifts on the schedule.
        let (already_scheduled, _) = first.into_parts();
        let mut matcher =
            ExistingPlannedShiftMatcher::new(already_scheduled, Vec::new(), false);

        let second = process_standards_for_shift(&params, &providers, &mut matcher)
            .expect("all supported");

        assert!(forecast_count > 0);
        assert_eq!(count_of(second.planned_shifts(), PlanType::Forecast), 0);
        assert_eq!(
            count_of(second.planned_shifts(), PlanType::Original),
            original_count
        );
    }

    #[test]
    fn staffing_minutes_are_added_to_the_combined_total() {
        let environment_id = EnvironmentId::new();
        let business_driver_id = BusinessDriverId::new();
        let fixture = fixture(environment_id, business_driver_id, 480, 1.0);
        let environments = OneEnvironment(environment_id);
        let providers = providers_for(&environments);
        let params = fixture.params();

        let non_staff = process_non_staff_standards(&params, &providers).expect("all supported");

        let without_staff = calculate_final_bodies(
            &params,
            &providers,
            non_staff.non_staff_work_minutes_per_period(),
            &aggregate_staff_minutes(&params, &[]),
        );

        // A staffing array adding an hour to every period of the shift.
        let mut staff = aggregate_staff_minutes(&params, &[]);
        for item in staff.iter_mut().take(32).skip(16) {
            item.set_value_per_period(60.0);
        }
        let with_staff = calculate_final_bodies(
            &params,
            &providers,
            non_staff.non_staff_work_minutes_per_period(),
            &staff,
        );

        assert!(total_of(&with_staff) > total_of(&without_staff));
    }
}
