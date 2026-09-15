//! Drives the flowed pipeline for one job, across dates and shifts.
//!
//! Ported from Java's `KbiRelatedStandardsProcessor` and
//! `KbiRelatedStandardsProcessorForDate`, which are two classes doing one
//! thing: for each effective date, and each shift on the standard set that
//! actually runs that day, hand one set of parameters to the eight-step
//! sequence and collect what comes back.
//!
//! The matcher is threaded through the whole run rather than rebuilt per shift,
//! because a shift already on the schedule accounts for exactly one newly
//! planned shift — wherever in the run that one turns up.

use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::JobShift;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::kbi_related_results::AdvancedResults;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::standards_processor_for_shift;
use crate::workcontent::generators::error::GenerationError;
use joda_rs::LocalDate;

/// Everything the job's standards plan, over every date its settings are
/// effective on.
pub fn process(
    planner_model: &PlannerModel,
    job: &Job,
    providers: &Providers,
    matcher: &mut ExistingPlannedShiftMatcher,
) -> Result<AdvancedResults, GenerationError> {
    let mut results = AdvancedResults::new();

    for date in job.planner_settings().dates(planner_model) {
        process_standards_for_date(
            planner_model,
            job,
            date,
            providers,
            matcher,
            &mut results,
        )?;
    }

    Ok(results)
}

/// The same for a single date, for incremental regeneration.
///
/// Java resolves the standard set for that specific date here, where the
/// full run uses the plan's own; this port holds one standard set per plan, so
/// both paths read the same one.
pub fn process_for_date(
    planner_model: &PlannerModel,
    job: &Job,
    date: LocalDate,
    providers: &Providers,
    matcher: &mut ExistingPlannedShiftMatcher,
) -> Result<AdvancedResults, GenerationError> {
    let mut results = AdvancedResults::new();

    process_standards_for_date(planner_model, job, date, providers, matcher, &mut results)?;

    Ok(results)
}

/// One date: every shift on the standard set that runs that day.
pub fn process_standards_for_date(
    planner_model: &PlannerModel,
    job: &Job,
    date: LocalDate,
    providers: &Providers,
    matcher: &mut ExistingPlannedShiftMatcher,
    results: &mut AdvancedResults,
) -> Result<(), GenerationError> {
    for shift in job.shifts_for_standard_set(planner_model.standard_set_id()) {
        process_shift_for_date(
            planner_model,
            job,
            shift,
            date,
            providers,
            matcher,
            results,
        )?;
    }

    Ok(())
}

fn process_shift_for_date(
    planner_model: &PlannerModel,
    job: &Job,
    shift: &JobShift,
    date: LocalDate,
    providers: &Providers,
    matcher: &mut ExistingPlannedShiftMatcher,
    results: &mut AdvancedResults,
) -> Result<(), GenerationError> {
    // A shift that does not run on this date, or runs without times set, has
    // nowhere to put work — the same guard the non-flowed path applies.
    let shift_detail = match shift.shift_detail_for_date(date) {
        Some(shift_detail) if shift_detail.has_times() => shift_detail,
        _ => return Ok(()),
    };

    let params = GeneratorParameters::new(
        planner_model,
        job,
        job.planner_settings(),
        date,
        shift,
        shift_detail,
    );

    let (planned_shifts, work_contents) =
        standards_processor_for_shift::process_standards_for_shift(&params, providers, matcher)?
            .into_parts();

    results.add_planned_shifts(planned_shifts);
    results.add_work_contents(work_contents);

    Ok(())
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `KbiRelatedStandardsProcessorForDateTest.groovy` and
    //! `KbiRelatedStandardsProcessorTest.groovy`. Both Java tests mock the
    //! stage below them and assert on the calls it receives; the Rust ones run
    //! the real stage and assert on what it produces, which says the same
    //! thing about which shifts and dates were reached.

    use super::*;
    use crate::workcontent::generators::advanced::fixtures::JobFixture;
    use joda_rs::{DayOfWeek, LocalTime};

    /// Java's three shifts on one standard set: one with usable times, one
    /// that does not run that day, and one with no times at all. Only the
    /// first is processed.
    #[test]
    fn the_per_shift_sequence_is_only_used_with_valid_shift_details() {
        let fixture = JobFixture::new()
            .with_shift_running(DayOfWeek::Tuesday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0))
            // Runs on a different day, so no detail resolves for this date.
            .with_shift_running(DayOfWeek::Thursday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0))
            .with_shift_without_times(DayOfWeek::Tuesday)
            .with_work(480);
        let providers = fixture.providers();

        let results = process_for_date(
            fixture.planner_model(),
            fixture.job(),
            // A Tuesday.
            LocalDate::new(2013, 1, 1),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        // The same job with only the usable shift plans exactly the same
        // thing — which is what "the other two were skipped" means here.
        let only_valid = JobFixture::new()
            .with_shift_running(
                DayOfWeek::Tuesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_work(480);
        let only_valid_providers = only_valid.providers();
        let expected = process_for_date(
            only_valid.planner_model(),
            only_valid.job(),
            LocalDate::new(2013, 1, 1),
            &only_valid_providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(!results.planned_shifts().is_empty());
        assert_eq!(
            results.planned_shifts().len(),
            expected.planned_shifts().len()
        );
        assert_eq!(
            results.work_contents().len(),
            expected.work_contents().len()
        );
    }

    #[test]
    fn a_date_with_no_usable_shift_produces_nothing() {
        let fixture = JobFixture::new()
            .with_shift_without_times(DayOfWeek::Tuesday)
            .with_work(480);
        let providers = fixture.providers();

        let results = process_for_date(
            fixture.planner_model(),
            fixture.job(),
            LocalDate::new(2013, 1, 1),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(results.planned_shifts().is_empty());
        assert!(results.work_contents().is_empty());
    }

    /// Java's two-date run: both effective dates are processed and their
    /// output accumulates into one set of results.
    #[test]
    fn the_processor_processes_all_effective_dates() {
        let start_date = LocalDate::new(2013, 1, 1);
        let end_date = LocalDate::new(2013, 1, 2);
        let fixture = JobFixture::new()
            .over(start_date, end_date)
            .with_shift_running(DayOfWeek::Tuesday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0))
            .with_shift_running(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_work(480);
        let providers = fixture.providers();

        let results = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        let dates: Vec<_> = results
            .planned_shifts()
            .iter()
            .filter_map(|shift| shift.shift_date())
            .collect();

        assert!(dates.contains(&start_date));
        assert!(dates.contains(&end_date));
    }

    /// The single-date entry point reaches only that date.
    #[test]
    fn the_processor_processes_a_single_date() {
        let start_date = LocalDate::new(2013, 1, 1);
        let end_date = LocalDate::new(2013, 1, 2);
        let fixture = JobFixture::new()
            .over(start_date, end_date)
            .with_shift_running(DayOfWeek::Tuesday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0))
            .with_shift_running(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_work(480);
        let providers = fixture.providers();

        let results = process_for_date(
            fixture.planner_model(),
            fixture.job(),
            end_date,
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(!results.planned_shifts().is_empty());
        assert!(
            results
                .planned_shifts()
                .iter()
                .all(|shift| shift.shift_date() == Some(end_date))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::generators::advanced::fixtures::JobFixture;
    use joda_rs::{DayOfWeek, LocalTime};

    fn weekday_fixture() -> JobFixture {
        JobFixture::new()
            .over(LocalDate::new(2013, 1, 1), LocalDate::new(2013, 1, 2))
            .with_shift_running(DayOfWeek::Tuesday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0))
            .with_shift_running(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_work(480)
    }

    #[test]
    fn a_job_with_no_standards_plans_nothing() {
        let fixture = JobFixture::new()
            .with_shift_running(DayOfWeek::Tuesday, LocalTime::new(8, 0, 0), LocalTime::new(16, 0, 0));
        let providers = fixture.providers();

        let results = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(results.planned_shifts().is_empty());
        assert!(results.work_contents().is_empty());
    }

    /// Dates outside the settings' effective range are not planned at all.
    #[test]
    fn only_effective_dates_are_planned() {
        let fixture = weekday_fixture()
            // Effective from the second day onward.
            .effective_from(LocalDate::new(2013, 1, 2), LocalDate::new(2013, 12, 31));
        let providers = fixture.providers();

        let results = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(
            results
                .planned_shifts()
                .iter()
                .all(|shift| shift.shift_date() == Some(LocalDate::new(2013, 1, 2)))
        );
        assert!(!results.planned_shifts().is_empty());
    }

    /// One existing shift accounts for one new shift across the whole run, not
    /// one per date — which is why the matcher is threaded through rather than
    /// rebuilt per shift.
    #[test]
    fn the_matcher_is_consumed_across_the_whole_run_not_per_date() {
        let fixture = weekday_fixture().projected();
        let providers = fixture.providers();

        let first = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        // Put only the first date's forecast shifts on the schedule.
        let first_date = LocalDate::new(2013, 1, 1);
        let (planned_shifts, _) = first.into_parts();
        let already_scheduled: Vec<_> = planned_shifts
            .into_iter()
            .filter(|shift| {
                shift.shift_date() == Some(first_date)
                    && shift.shift_type() == Some(PlanType::Forecast)
            })
            .collect();
        let scheduled_count = already_scheduled.len();
        let mut matcher = ExistingPlannedShiftMatcher::new(already_scheduled, Vec::new(), false);

        let second = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut matcher,
        )
        .expect("all supported");

        let forecast_on_first_date = second
            .planned_shifts()
            .iter()
            .filter(|shift| {
                shift.shift_date() == Some(first_date)
                    && shift.shift_type() == Some(PlanType::Forecast)
            })
            .count();
        let forecast_on_second_date = second
            .planned_shifts()
            .iter()
            .filter(|shift| {
                shift.shift_date() == Some(LocalDate::new(2013, 1, 2))
                    && shift.shift_type() == Some(PlanType::Forecast)
            })
            .count();

        assert!(scheduled_count > 0);
        // The first date matched away entirely...
        assert_eq!(forecast_on_first_date, 0);
        // ...and the second date, which nothing was scheduled for, is untouched.
        assert!(forecast_on_second_date > 0);
    }

    #[test]
    fn a_projected_run_writes_both_plan_types() {
        let fixture = weekday_fixture().projected();
        let providers = fixture.providers();

        let results = process(
            fixture.planner_model(),
            fixture.job(),
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        assert!(
            results
                .planned_shifts()
                .iter()
                .any(|shift| shift.shift_type() == Some(PlanType::Forecast))
        );
        assert!(
            results
                .planned_shifts()
                .iter()
                .any(|shift| shift.shift_type() == Some(PlanType::Original))
        );
        assert_eq!(fixture.planner_model().planner_mode(), PlannerMode::Projected);
    }
}
