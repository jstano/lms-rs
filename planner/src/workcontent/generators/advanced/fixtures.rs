//! A whole job, configured for the flowed generator, for tests above the
//! per-shift stage.
//!
//! The stages below this one build `GeneratorParameters` through
//! `generator_parameters::fixtures::Context`, which describes *one* shift on
//! *one* date. The wiring stages need a job with several shifts across several
//! dates, which is a different shape: the shifts, their standards and the
//! planner model all have to agree about ids, so they are built together here
//! rather than assembled by hand in each test.

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
use crate::workcontent::domain::planner_mode::PlannerMode;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::planner_settings::PlannerSettings;
use crate::workcontent::domain::shift_standard::{ShiftStandard, ShiftStandardRange};
use crate::workcontent::domain::standard_set::StandardSetId;
use crate::workcontent::domain::units::Units;
use crate::workcontent::domain::work_type::WorkType;
use crate::workcontent::generators::advanced::providers::{EnvironmentResolver, Providers};
use date_range_rs::DateRange;
use joda_rs::{DayOfWeek, LocalDate, LocalTime};
use std::collections::HashMap;
use std::sync::OnceLock;

/// One environment, everywhere. Standards are priced per environment, so a
/// fixture with no resolver would cost every standard at nothing.
pub struct OneEnvironment(EnvironmentId);

impl EnvironmentResolver for OneEnvironment {
    fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
        Some(self.0)
    }
}

/// The same, as a `'static`, for the places that need an owned provider bundle
/// (the generator holds `Providers<'static>`). The id is minted once and then
/// reused, so two calls agree — which matters wherever a standard *is*
/// environment-scoped.
pub struct AnyEnvironment(OnceLock<EnvironmentId>);

pub static ANY_ENVIRONMENT: AnyEnvironment = AnyEnvironment(OnceLock::new());

impl EnvironmentResolver for AnyEnvironment {
    fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
        Some(*self.0.get_or_init(EnvironmentId::new))
    }
}

/// How one shift of the job is configured: which day it runs, and its times if
/// it has any.
#[derive(Clone, Copy)]
struct ShiftSpec {
    day_of_week: DayOfWeek,
    times: Option<(LocalTime, LocalTime)>,
}

pub struct JobFixture {
    dates: DateRange,
    effective_dates: DateRange,
    planner_mode: PlannerMode,
    shift_specs: Vec<ShiftSpec>,
    /// Minutes of work each shift's standard requires.
    work_minutes: i32,
    business_driver_id: BusinessDriverId,
    standard_set_id: StandardSetId,
    environments: OneEnvironment,
    planner_model: PlannerModel,
    job: Job,
}

impl JobFixture {
    /// A job planning a single day — Tuesday 1 January 2013 — with no shifts
    /// and no work, for a test to add what it needs.
    pub fn new() -> Self {
        let date = LocalDate::new(2013, 1, 1);
        let standard_set_id = StandardSetId::new();

        let mut fixture = Self {
            dates: DateRange::new(date, date),
            effective_dates: DateRange::new(
                LocalDate::new(2013, 1, 1),
                LocalDate::new(2013, 12, 31),
            ),
            planner_mode: PlannerMode::Standard,
            shift_specs: Vec::new(),
            work_minutes: 0,
            business_driver_id: BusinessDriverId::new(),
            standard_set_id,
            environments: OneEnvironment(EnvironmentId::new()),
            // Replaced by the first rebuild; built once so the fields exist.
            planner_model: empty_planner_model(standard_set_id),
            job: Job::new(
                LocationId::new(),
                planner_settings(DateRange::new(date, date)),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        };

        fixture.rebuild();
        fixture
    }

    /// Plan every date from `start` to `end` inclusive.
    #[must_use]
    pub fn over(mut self, start: LocalDate, end: LocalDate) -> Self {
        self.dates = DateRange::new(start, end);
        self.rebuild();
        self
    }

    /// Narrow the window the job's settings are effective on.
    #[must_use]
    pub fn effective_from(mut self, start: LocalDate, end: LocalDate) -> Self {
        self.effective_dates = DateRange::new(start, end);
        self.rebuild();
        self
    }

    /// Write the forecast as well as the plan.
    #[must_use]
    pub fn projected(mut self) -> Self {
        self.planner_mode = PlannerMode::Projected;
        self.rebuild();
        self
    }

    /// Add a shift that runs on `day_of_week` between the given times.
    #[must_use]
    pub fn with_shift_running(
        mut self,
        day_of_week: DayOfWeek,
        start_time: LocalTime,
        end_time: LocalTime,
    ) -> Self {
        self.shift_specs.push(ShiftSpec {
            day_of_week,
            times: Some((start_time, end_time)),
        });
        self.rebuild();
        self
    }

    /// Add a shift configured on `day_of_week` but with no times set, which
    /// the generator has to skip.
    #[must_use]
    pub fn with_shift_without_times(mut self, day_of_week: DayOfWeek) -> Self {
        self.shift_specs.push(ShiftSpec {
            day_of_week,
            times: None,
        });
        self.rebuild();
        self
    }

    /// Give every shift a standard requiring `minutes` of work per date.
    #[must_use]
    pub fn with_work(mut self, minutes: i32) -> Self {
        self.work_minutes = minutes;
        self.rebuild();
        self
    }

    pub fn planner_model(&self) -> &PlannerModel {
        &self.planner_model
    }

    pub fn job(&self) -> &Job {
        &self.job
    }

    pub fn providers(&self) -> Providers<'_> {
        Providers {
            environments: &self.environments,
            ..Providers::none()
        }
    }

    /// Rebuild the job and the model together.
    ///
    /// Standards are keyed on the shift they belong to, so the shifts and the
    /// standards have to be minted in one pass — which means every change
    /// regenerates both. Ids change as a result, which is harmless because
    /// nothing outside holds one across a change.
    fn rebuild(&mut self) {
        let job_id = JobId::new();

        let shifts: Vec<JobShift> = self
            .shift_specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let definition = match spec.times {
                    Some((start_time, end_time)) => JobShiftDefinition::new(
                        spec.day_of_week,
                        start_time,
                        end_time,
                        0.0,
                        0.0,
                        1,
                    ),
                    None => JobShiftDefinition::without_times(spec.day_of_week),
                };

                JobShift::new(
                    job_id,
                    self.standard_set_id,
                    format!("Shift {}", index + 1),
                    index as u32 + 1,
                    vec![definition],
                )
            })
            .collect();

        // One minute of work per unit of driver volume, so the driver value is
        // the shift's work in minutes directly. The value is left unscoped so
        // the fixture works with any environment resolver — what environment
        // resolves is exercised where the pricing is, not here.
        let standards: Vec<ShiftStandard> = shifts
            .iter()
            .map(|shift| {
                ShiftStandard::new(
                    job_id,
                    self.standard_set_id,
                    shift.id(),
                    self.business_driver_id,
                    WorkType::Variable,
                    Units::MinutesPerUnit,
                    0,
                    vec![ShiftStandardRange::new(0, 100_000, 1.0)],
                )
                .distributed_by(DistributionMethod::NonFlowed)
                .with_non_flowed_distribution_method(NonFlowedDistributionMethod::EVEN)
            })
            .collect();

        let driver_values: HashMap<LocalDate, i32> = self
            .dates
            .iter()
            .map(|date| (date, self.work_minutes))
            .collect();

        self.planner_model = PlannerModel::new(
            self.dates,
            self.planner_mode,
            LocationId::new(),
            self.standard_set_id,
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                self.business_driver_id,
                BusinessDriverValues::new(self.business_driver_id, driver_values),
            )]),
        );

        self.job = Job::new(
            LocationId::new(),
            planner_settings(self.effective_dates),
            shifts,
            Vec::new(),
            standards,
        );
    }
}

/// Half-hour periods, a half-hour meal break after five hours, and rounding
/// thresholds at zero — the same settings the per-shift fixtures use.
fn planner_settings(effective_dates: DateRange) -> PlannerSettings {
    let mut planner_settings = PlannerSettings::default();

    planner_settings.period_length = 30;
    planner_settings.meal_break = Some(MealBreak::new(5.0, 0.5));
    planner_settings.non_meal_break = Some(NonMealBreak::with_no_break());
    planner_settings.min_shift_length = 0.0;
    planner_settings.max_shift_length = 8.0;
    planner_settings.rounding_threshold_below_one = 0.0;
    planner_settings.rounding_threshold_above_one = 0.0;
    planner_settings.non_flowed_distribution_method = NonFlowedDistributionMethod::EVEN;
    planner_settings.effective_dates = effective_dates;

    planner_settings
}

fn empty_planner_model(standard_set_id: StandardSetId) -> PlannerModel {
    let date = LocalDate::new(2013, 1, 1);

    PlannerModel::new(
        DateRange::new(date, date),
        PlannerMode::Standard,
        LocationId::new(),
        standard_set_id,
        Vec::new(),
        Vec::new(),
        HashMap::new(),
    )
}
