use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::standards_processor;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::work_generators::{WorkGenerator, WorkResults};
use std::any::Any;

/// The generator for flowed (KBI-distributed) standards: work is spread across
/// the shift following a curve, rather than laid down in whole blocks.
pub struct AdvancedWorkGenerator {
    providers: Providers<'static>,
    /// Shifts already on the schedule for the jobs being planned.
    ///
    /// Java reads these off `PlannerModel`, which does not model them in this
    /// port, so they are supplied to the generator instead. Left empty, the
    /// generator behaves exactly as Java does against an empty schedule: every
    /// shift it plans is written.
    scheduled_planned_shifts: Vec<PlannedShift>,
    planned_shifts_with_pending_requests: Vec<PlannedShift>,
    clear_schedules: bool,
}

impl AdvancedWorkGenerator {
    /// A generator with none of the external data configured.
    ///
    /// The provider defaults match Java's behaviour when the corresponding
    /// structure is empty — with one consequence worth knowing: with no
    /// environment resolver, standards priced per environment cost nothing, so
    /// this generator plans no work until one is supplied.
    pub fn new() -> Self {
        Self {
            providers: Providers::none(),
            scheduled_planned_shifts: Vec::new(),
            planned_shifts_with_pending_requests: Vec::new(),
            clear_schedules: false,
        }
    }

    /// Supply the external data the flowed engine consults.
    #[must_use]
    pub fn with_providers(mut self, providers: Providers<'static>) -> Self {
        self.providers = providers;
        self
    }

    /// Supply what is already on the schedule, so replanning does not write a
    /// second copy of a shift that is already there.
    #[must_use]
    pub fn with_existing_planned_shifts(
        mut self,
        scheduled: Vec<PlannedShift>,
        with_pending_requests: Vec<PlannedShift>,
        clear_schedules: bool,
    ) -> Self {
        self.scheduled_planned_shifts = scheduled;
        self.planned_shifts_with_pending_requests = with_pending_requests;
        self.clear_schedules = clear_schedules;
        self
    }

    /// One matcher per run, so an existing shift accounts for one newly planned
    /// shift across the whole job rather than once per date.
    fn matcher(&self) -> ExistingPlannedShiftMatcher {
        ExistingPlannedShiftMatcher::new(
            self.scheduled_planned_shifts.clone(),
            self.planned_shifts_with_pending_requests.clone(),
            self.clear_schedules,
        )
    }
}

impl WorkGenerator for AdvancedWorkGenerator {
    fn generate_work(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<WorkResults, GenerationError> {
        let results = standards_processor::process(
            planner_model,
            job,
            &self.providers,
            &mut self.matcher(),
        )?;

        let (planned_shifts, work_contents) = results.into_parts();

        Ok(WorkResults::with_work_content(
            job.id(),
            work_contents,
            planned_shifts,
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::generators::advanced::fixtures::{ANY_ENVIRONMENT, JobFixture};
    use joda_rs::{DayOfWeek, LocalDate, LocalTime};

    fn planned_shifts_of(results: &WorkResults) -> &[PlannedShift] {
        results
            .planned_shifts()
            .map(Vec::as_slice)
            .expect("a work-content result")
    }

    fn work_content_of(
        results: &WorkResults,
    ) -> &[crate::workcontent::domain::work_content::WorkContent] {
        results
            .work_content()
            .map(Vec::as_slice)
            .expect("a work-content result")
    }

    fn fixture() -> JobFixture {
        JobFixture::new()
            .over(LocalDate::new(2013, 1, 1), LocalDate::new(2013, 1, 2))
            .with_shift_running(
                DayOfWeek::Tuesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_shift_running(
                DayOfWeek::Wednesday,
                LocalTime::new(8, 0, 0),
                LocalTime::new(16, 0, 0),
            )
            .with_work(480)
    }

    /// The generator no longer plans nothing. Everything below this point has
    /// been built and tested for several groups; this is the first test that
    /// the whole of it is actually reachable through the public entry point.
    #[test]
    fn a_flowed_job_produces_work_content_and_the_shifts_to_cover_it() {
        let fixture = fixture();
        let generator = AdvancedWorkGenerator::new().with_providers(Providers {
            environments: &ANY_ENVIRONMENT,
            ..Providers::none()
        });

        let results = generator
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        assert_eq!(results.job_id(), fixture.job().id());
        assert!(!work_content_of(&results).is_empty());
        assert!(!planned_shifts_of(&results).is_empty());
    }

    /// Every shift written falls on a date in the planning window.
    #[test]
    fn the_shifts_written_fall_on_the_dates_planned() {
        let fixture = fixture();
        let generator = AdvancedWorkGenerator::new().with_providers(Providers {
            environments: &ANY_ENVIRONMENT,
            ..Providers::none()
        });

        let results = generator
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        assert!(planned_shifts_of(&results).iter().all(|shift| {
            matches!(
                shift.shift_date(),
                Some(date)
                    if date == LocalDate::new(2013, 1, 1) || date == LocalDate::new(2013, 1, 2)
            )
        }));
    }

    /// Without an environment resolver, standards priced per environment cost
    /// nothing — the same as Java against an empty seasonal calendar. Stated as
    /// a test so the default is a decision rather than a surprise.
    #[test]
    fn a_generator_with_no_environment_data_plans_nothing() {
        let fixture = fixture();

        let results = AdvancedWorkGenerator::new()
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        assert!(work_content_of(&results).is_empty());
        assert!(planned_shifts_of(&results).is_empty());
    }

    /// Shifts already on the schedule are not written a second time.
    #[test]
    fn forecast_shifts_already_scheduled_are_not_written_again() {
        let fixture = fixture().projected();
        let providers = Providers {
            environments: &ANY_ENVIRONMENT,
            ..Providers::none()
        };

        let first = AdvancedWorkGenerator::new()
            .with_providers(providers)
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        let already_scheduled: Vec<_> = planned_shifts_of(&first)
            .iter()
            .filter(|shift| shift.shift_type() == Some(PlanType::Forecast))
            .cloned()
            .collect();

        let second = AdvancedWorkGenerator::new()
            .with_providers(Providers {
                environments: &ANY_ENVIRONMENT,
                ..Providers::none()
            })
            .with_existing_planned_shifts(already_scheduled.clone(), Vec::new(), false)
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        assert!(!already_scheduled.is_empty());
        assert!(
            planned_shifts_of(&second)
                .iter()
                .all(|shift| shift.shift_type() != Some(PlanType::Forecast))
        );
    }

    /// The generator holds its schedule, so planning twice from the same one
    /// gives the same answer both times.
    #[test]
    fn the_generator_can_be_run_more_than_once() {
        let fixture = fixture();
        let generator = AdvancedWorkGenerator::new().with_providers(Providers {
            environments: &ANY_ENVIRONMENT,
            ..Providers::none()
        });

        let first = generator
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");
        let second = generator
            .generate_work(fixture.planner_model(), fixture.job())
            .expect("all supported");

        assert_eq!(
            planned_shifts_of(&first).len(),
            planned_shifts_of(&second).len()
        );
        assert_eq!(
            work_content_of(&first).len(),
            work_content_of(&second).len()
        );
    }
}
