//! Whole-plan scenarios, rebuilt from Java's integration tests.
//!
//! Everything else in this port checks one stage against one Spock table. These
//! check that a job's standards go in and the right shifts come out — which is
//! the only thing that catches a stage being correct in isolation and wrong in
//! sequence.
//!
//! Java's originals drive a Spring service against a database and assert on
//! what the DAOs hand back. Their *mechanics* are dropped; their concrete
//! expectations — the exact shift times — are kept, because those are the part
//! that carries information.

use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
use crate::workcontent::domain::distribution_method::DistributionMethod;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job::{Job, JobId};
use crate::workcontent::domain::job_shift::{JobShift, JobShiftDefinition};
use crate::workcontent::domain::location::LocationId;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::non_meal_break::NonMealBreak;
use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::planner_mode::PlannerMode;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::planner_settings::PlannerSettings;
use crate::workcontent::domain::shift_standard::{ShiftStandard, ShiftStandardRange};
use crate::workcontent::domain::standard_set::StandardSetId;
use crate::workcontent::domain::units::Units;
use crate::workcontent::domain::work_type::WorkType;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::providers::{EnvironmentResolver, Providers};
use crate::workcontent::generators::advanced::standards_processor;
use date_range_rs::DateRange;
use joda_rs::{DayOfWeek, LocalDate, LocalTime};
use std::collections::HashMap;

/// Java's scenarios all plan Monday 4 November 2013.
fn date() -> LocalDate {
    LocalDate::new(2013, 11, 4)
}

fn time(hour: i32, minute: i32) -> LocalTime {
    LocalTime::new(hour, minute, 0)
}

/// The day-of-week environment Java's fixtures price their standards in.
struct MondayEnvironment(EnvironmentId);

impl EnvironmentResolver for MondayEnvironment {
    fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
        Some(self.0)
    }
}

/// One job, one shift, and the standards on it — the shape Java's fixtures
/// build through their DAOs.
struct Scenario {
    planner_model: PlannerModel,
    job: Job,
    environments: MondayEnvironment,
}

impl Scenario {
    /// A job whose single Monday shift runs `shift_start` to `shift_end`, with
    /// `standards` built against the shift.
    ///
    /// `planner_settings` is the assignment's own, matching the
    /// `AssignmentPlannerSettings` each Java scenario attaches.
    fn new(
        shift_start: LocalTime,
        shift_end: LocalTime,
        planner_settings: PlannerSettings,
        business_driver_value: i32,
        build_standards: impl Fn(JobId, StandardSetId, &JobShift) -> Vec<ShiftStandard>,
    ) -> Self {
        let job_id = JobId::new();
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();

        let shift = JobShift::new(
            job_id,
            standard_set_id,
            "Day".to_string(),
            1,
            vec![JobShiftDefinition::new(
                DayOfWeek::Monday,
                shift_start,
                shift_end,
                0.0,
                0.0,
                1,
            )],
        );

        let standards = build_standards(job_id, standard_set_id, &shift);

        Self {
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
                        HashMap::from([(date(), business_driver_value)]),
                    ),
                )]),
            ),
            job: Job::new(
                LocationId::new(),
                planner_settings,
                vec![shift],
                Vec::new(),
                standards,
            ),
            environments: MondayEnvironment(EnvironmentId::new()),
        }
    }

    /// The shifts this job plans, as `(start, end)` local times.
    fn planned_shifts(&self) -> Vec<(LocalTime, LocalTime)> {
        let providers = Providers {
            environments: &self.environments,
            ..Providers::none()
        };

        let results = standards_processor::process(
            &self.planner_model,
            &self.job,
            &providers,
            &mut ExistingPlannedShiftMatcher::empty(),
        )
        .expect("all supported");

        let (planned_shifts, _) = results.into_parts();

        planned_shifts.iter().map(times_of).collect()
    }
}

fn times_of(planned_shift: &PlannedShift) -> (LocalTime, LocalTime) {
    (
        planned_shift
            .start_date_time()
            .expect("a start time")
            .to_local_time(),
        planned_shift
            .end_date_time()
            .expect("an end time")
            .to_local_time(),
    )
}

/// Java's `AssignmentPlannerSettings`: half-hour periods, a four-hour minimum
/// and an eight-hour maximum shift, and **no breaks configured**, which is what
/// its `mealBreakAfter: null` means.
fn planner_settings() -> PlannerSettings {
    let mut planner_settings = PlannerSettings::default();

    planner_settings.period_length = 30;
    planner_settings.min_shift_length = 4.0;
    planner_settings.max_shift_length = 8.0;
    planner_settings.meal_break = None;
    planner_settings.non_meal_break = Some(NonMealBreak::with_no_break());
    planner_settings.rounding_threshold_below_one = 0.0;
    planner_settings.rounding_threshold_above_one = 0.0;
    planner_settings.effective_dates = DateRange::new(
        LocalDate::new(2013, 1, 1),
        LocalDate::new(2013, 12, 31),
    );

    planner_settings
}

/// A standard worth a flat number of hours a day, whatever the volume — Java's
/// `Units.HOURS` with `WorkType.DAILY`.
fn daily_hours_standard(
    job_id: JobId,
    standard_set_id: StandardSetId,
    shift: &JobShift,
    hours: f64,
) -> ShiftStandard {
    ShiftStandard::new(
        job_id,
        standard_set_id,
        shift.id(),
        BusinessDriverId::new(),
        WorkType::Daily,
        Units::Hours,
        0,
        vec![ShiftStandardRange::new(0, i32::MAX, hours)],
    )
}

/// **Java: `NonFlowedWithClosingWorkIntegrationTest`.**
///
/// Eight hours of work spread to the END of an 08:00–16:00 shift, plus an hour
/// of closing work pinned to 16:00–17:00. That is nine hours of single-body
/// coverage across a shift that allows eight, so it splits — and the hour left
/// over is stretched *backwards* to reach the four-hour minimum, because its
/// end is already at the end of the work.
#[test]
fn eight_hour_shift_with_one_hour_closing_generates_8_16_and_13_17() {
    let scenario = Scenario::new(
        time(8, 0),
        time(16, 0),
        planner_settings(),
        8,
        |job_id, standard_set_id, shift| {
            vec![
                daily_hours_standard(job_id, standard_set_id, shift, 8.0)
                    .distributed_by(DistributionMethod::NonFlowed)
                    .with_non_flowed_distribution_method(NonFlowedDistributionMethod::END),
                daily_hours_standard(job_id, standard_set_id, shift, 1.0)
                    .distributed_by(DistributionMethod::Closing)
                    .within_work_window(Some(time(16, 0)), Some(time(17, 0))),
            ]
        },
    );

    assert_eq!(
        scenario.planned_shifts(),
        vec![
            (time(8, 0), time(16, 0)),
            (time(13, 0), time(17, 0)),
        ]
    );
}

/// **Java: `NonFlowedWithOpeningWorkIntegrationTest`.**
///
/// The mirror image: work spread to the BEGINNING, and an hour of opening work
/// pinned to 07:00–08:00. The leftover hour sits at the *end* of the coverage
/// this time, so the same four-hour minimum stretches it backwards from 16:00.
#[test]
fn eight_hour_shift_with_one_hour_opening_generates_7_15_and_12_16() {
    let scenario = Scenario::new(
        time(8, 0),
        time(16, 0),
        planner_settings(),
        8,
        |job_id, standard_set_id, shift| {
            vec![
                daily_hours_standard(job_id, standard_set_id, shift, 8.0)
                    .distributed_by(DistributionMethod::NonFlowed)
                    .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING),
                daily_hours_standard(job_id, standard_set_id, shift, 1.0)
                    .distributed_by(DistributionMethod::Opening)
                    .within_work_window(Some(time(7, 0)), Some(time(8, 0))),
            ]
        },
    );

    assert_eq!(
        scenario.planned_shifts(),
        vec![
            (time(7, 0), time(15, 0)),
            (time(12, 0), time(16, 0)),
        ]
    );
}

/// **Java: `SecondNonFlowedWithOpeningWorkIntegrationTest`, `limitShift=true`.**
///
/// Fourteen hours of work, spread to the beginning of a round-the-clock shift,
/// with the spread itself capped at the maximum shift. The cap packs the work
/// into the first eight hours — so it stacks two deep over the first six —
/// and the two shifts that come out both start at midnight.
#[test]
fn fourteen_hours_on_a_midnight_shift_limited_to_the_max_shift() {
    let mut planner_settings = planner_settings();
    planner_settings.limit_shift_to_max_shift = true;
    planner_settings.generate_long_shifts = false;

    let scenario = Scenario::new(
        time(0, 0),
        time(0, 0),
        planner_settings,
        8,
        |job_id, standard_set_id, shift| {
            vec![
                daily_hours_standard(job_id, standard_set_id, shift, 14.0)
                    .distributed_by(DistributionMethod::NonFlowed)
                    .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING),
            ]
        },
    );

    assert_eq!(
        scenario.planned_shifts(),
        vec![
            (time(0, 0), time(8, 0)),
            (time(0, 0), time(6, 0)),
        ]
    );
}

/// **Java: the same scenario with `limitShift=false`.**
///
/// The same fourteen hours, but the spread may run the whole shift, so it lays
/// down one body from midnight to 14:00. The peel then takes a full shift off
/// the front and the remaining six hours follow it, rather than overlapping it.
/// The contrast with the case above is the whole point of the pair.
#[test]
fn fourteen_hours_on_a_midnight_shift_not_limited_to_the_max_shift() {
    let mut planner_settings = planner_settings();
    planner_settings.limit_shift_to_max_shift = false;
    planner_settings.generate_long_shifts = false;

    let scenario = Scenario::new(
        time(0, 0),
        time(0, 0),
        planner_settings,
        8,
        |job_id, standard_set_id, shift| {
            vec![
                daily_hours_standard(job_id, standard_set_id, shift, 14.0)
                    .distributed_by(DistributionMethod::NonFlowed)
                    .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING),
            ]
        },
    );

    assert_eq!(
        scenario.planned_shifts(),
        vec![
            (time(0, 0), time(8, 0)),
            (time(8, 0), time(14, 0)),
        ]
    );
}

// ---------------------------------------------------------------------------
// The flowed matrix
// ---------------------------------------------------------------------------
//
// **Java: `FlowedIntegrationTest`.** This one cannot be ported as a port, and
// the reasons are worth stating because they decide what the Rust version is
// allowed to claim:
//
// - the whole feature method carries `@Ignore`, so **not one row runs
//   upstream** — including the single row that is not commented out;
// - 31 of its 32 `where:` rows are commented out;
// - its two live methods assert audit-trail row counts, which are out of scope;
// - every assertion goes through a DAO round-trip.
//
// So the expectations below come from the file's `verifyTest*` methods, which
// are concrete and specific, rather than from a passing upstream run. They are
// evidence of what the engine was believed to do, not proof that it does.
//
// Two columns of that matrix turn out to be **inert**, which is why its 32 rows
// collapse to the eight distinct cases below. Both claims are tested rather
// than assumed:
//
// 1. `distributionOption` (BEGINNING / MIDDLE / END) cannot matter, because the
//    non-staff standard is FLOWED and follows the curve — the non-flowed shape
//    is never consulted. See
//    [`the_non_flowed_shape_is_inert_for_a_flowed_standard`].
// 2. `staffValue` (4 / 8) cannot matter either, and this one is a **defect in
//    the Java fixture**: the staff standard is built and configured, and then
//    never added to the assignment. It is dead in every row. See
//    [`the_java_fixture_never_attaches_its_staff_standard`].
//
// Together those explain the shape of the matrix: the rows differ only by
// `nonStaffValue` and the shift bounds, and every group of rows sharing those
// shares a verifier.

/// Java's flow pattern: 48 half-hour periods, 5% of the day's work in each.
///
/// Over a ten-hour shift that is twenty periods at 5%, which comes to exactly
/// the whole of the work — the fixture is built so the arithmetic lands evenly.
struct FlatFivePercentCurve;

impl crate::workcontent::generators::advanced::providers::DistributionPatternProvider
    for FlatFivePercentCurve
{
    fn pattern_data(
        &self,
        _standard: &ShiftStandard,
        _date: LocalDate,
        range: &crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength,
    ) -> Option<Vec<crate::workcontent::generators::advanced::distribution_item::DistributionItem>>
    {
        let period_length = range.period_length_in_minutes() as i64;
        let start = date().at_start_of_day();

        Some(
            (0..96)
                .map(|period| {
                    crate::workcontent::generators::advanced::distribution_item::DistributionItem::new(
                        period,
                        start.plus_minutes(period as i64 * period_length),
                        5.0,
                    )
                })
                .collect(),
        )
    }
}

/// What the flowed scenario produced: the shifts, and the work content.
struct FlowedResults {
    planned_shifts: Vec<(LocalTime, LocalTime)>,
    work_content: Vec<(LocalTime, LocalTime, f64)>,
}

/// Java's flowed fixture: a 07:00–17:00 Monday shift, a FLOWED non-staff
/// standard worth `non_staff_hours`, and breaks configured.
///
/// `attach_staff_standard` reproduces the fixture faithfully when false — Java
/// builds a staff standard and never attaches it.
fn run_flowed_scenario(
    non_staff_hours: f64,
    staff_hours: f64,
    non_flowed_shape: NonFlowedDistributionMethod,
    min_shift: f64,
    max_shift: f64,
    attach_staff_standard: bool,
) -> FlowedResults {
    let mut planner_settings = planner_settings();
    planner_settings.min_shift_length = min_shift;
    planner_settings.max_shift_length = max_shift;
    planner_settings.meal_break = Some(crate::workcontent::domain::meal_break::MealBreak::new(5.0, 0.5));
    planner_settings.non_meal_break = Some(
        crate::workcontent::domain::non_meal_break::NonMealBreak::new(4.0, 0.25),
    );
    planner_settings.rounding_threshold_below_one = 1.0;
    planner_settings.rounding_threshold_above_one = 1.0;
    // Java's fixture sets a shape on the *standard* and leaves the plan-level
    // one unset, which matters more than it looks: the break spreader reads the
    // plan's shape, not the standard's, and its `Varying` branch — the unset
    // default — silently places nothing. Setting this to the standard's shape
    // instead puts an hour of break at the front of the shift, stacks a second
    // body over 07:00-08:00, and adds a spurious 07:00-11:00 shift to every row
    // below. The Java expectations only hold with it left alone.

    let scenario = Scenario::new(
        time(7, 0),
        time(17, 0),
        planner_settings,
        8,
        |job_id, standard_set_id, shift| {
            let mut standards = vec![
                daily_hours_standard(job_id, standard_set_id, shift, non_staff_hours)
                    .distributed_by(DistributionMethod::Flowed)
                    .with_non_flowed_distribution_method(non_flowed_shape),
            ];

            if attach_staff_standard {
                standards.push(
                    ShiftStandard::new(
                        job_id,
                        standard_set_id,
                        shift.id(),
                        BusinessDriverId::new(),
                        WorkType::Staff,
                        Units::Hours,
                        0,
                        vec![ShiftStandardRange::new(0, i32::MAX, staff_hours)],
                    )
                    .distributed_by(DistributionMethod::NonFlowed)
                    .with_non_flowed_distribution_method(non_flowed_shape),
                );
            }

            standards
        },
    );

    let providers = Providers {
        environments: &scenario.environments,
        patterns: &FlatFivePercentCurve,
        ..Providers::none()
    };

    let results = standards_processor::process(
        &scenario.planner_model,
        &scenario.job,
        &providers,
        &mut ExistingPlannedShiftMatcher::empty(),
    )
    .expect("all supported");

    let (planned_shifts, work_contents) = results.into_parts();

    FlowedResults {
        planned_shifts: planned_shifts.iter().map(times_of).collect(),
        work_content: work_contents
            .iter()
            .map(|content| {
                (
                    content.calculated_start_date_time().to_local_time(),
                    content.calculated_end_date_time().to_local_time(),
                    content.calculated_hours(),
                )
            })
            .collect(),
    }
}

/// The variable column of Java's matrix. Every row reduces to one of these,
/// named after the `verifyTest*` method it asserts.
#[rstest::rstest]
// `verifyTestMaxHoursOfZero`: no shift length is allowed, so nothing is
// staffed — but the work is still recorded. This is the H4/H5 asymmetry
// showing up in a whole-plan result.
#[case::max_hours_of_zero(10.0, 0.0, 0.0, vec![], vec![(time(7, 0), time(17, 0), 10.0)])]
// `verifyTestZero`: no work, so nothing at all.
#[case::zero(0.0, 4.0, 10.0, vec![], vec![])]
// `verifyTestOne`: ten hours over a ten-hour ceiling is one shift.
#[case::one(
    10.0, 4.0, 10.0,
    vec![(time(7, 0), time(17, 0))],
    vec![(time(7, 0), time(17, 0), 10.0)]
)]
// `verifyTestTwo`: twice the work is twice the depth, so two of everything.
#[case::two(
    20.0, 4.0, 10.0,
    vec![(time(7, 0), time(17, 0)), (time(7, 0), time(17, 0))],
    vec![(time(7, 0), time(17, 0), 10.0), (time(7, 0), time(17, 0), 10.0)]
)]
// `verifyTestThree`: an eight-hour ceiling splits the same ten hours, and the
// two-hour remainder is stretched back to the four-hour minimum. The work
// content is untouched by either bound — it still records one ten-hour block.
#[case::three(
    10.0, 4.0, 8.0,
    vec![(time(7, 0), time(15, 0)), (time(13, 0), time(17, 0))],
    vec![(time(7, 0), time(17, 0), 10.0)]
)]
// `verifyTestFour`: the same split, two deep.
#[case::four(
    20.0, 4.0, 8.0,
    vec![
        (time(7, 0), time(15, 0)),
        (time(13, 0), time(17, 0)),
        (time(7, 0), time(15, 0)),
        (time(13, 0), time(17, 0)),
    ],
    vec![(time(7, 0), time(17, 0), 10.0), (time(7, 0), time(17, 0), 10.0)]
)]
// `verifyTestFive`: an eight-hour *minimum* stretches the remainder further —
// back to 09:00 rather than 13:00.
#[case::five(
    10.0, 8.0, 8.0,
    vec![(time(7, 0), time(15, 0)), (time(9, 0), time(17, 0))],
    vec![(time(7, 0), time(17, 0), 10.0)]
)]
// `verifyTestSix`: the same, two deep.
#[case::six(
    20.0, 8.0, 8.0,
    vec![
        (time(7, 0), time(15, 0)),
        (time(9, 0), time(17, 0)),
        (time(7, 0), time(15, 0)),
        (time(9, 0), time(17, 0)),
    ],
    vec![(time(7, 0), time(17, 0), 10.0), (time(7, 0), time(17, 0), 10.0)]
)]
fn the_flowed_matrix(
    #[case] non_staff_hours: f64,
    #[case] min_shift: f64,
    #[case] max_shift: f64,
    #[case] expected_shifts: Vec<(LocalTime, LocalTime)>,
    #[case] expected_work_content: Vec<(LocalTime, LocalTime, f64)>,
) {
    let results = run_flowed_scenario(
        non_staff_hours,
        4.0,
        NonFlowedDistributionMethod::BEGINNING,
        min_shift,
        max_shift,
        false,
    );

    assert_eq!(sorted(results.planned_shifts), sorted(expected_shifts));
    assert_eq!(
        results.work_content.len(),
        expected_work_content.len(),
        "work content count"
    );
    for expected in &expected_work_content {
        assert!(
            results.work_content.contains(expected),
            "expected work content {expected:?} in {:?}",
            results.work_content
        );
    }
}

fn sorted(mut times: Vec<(LocalTime, LocalTime)>) -> Vec<(LocalTime, LocalTime)> {
    times.sort_by_key(|(start, end)| (start.to_string(), end.to_string()));
    times
}

/// Java's matrix varies the non-flowed shape across three values and expects
/// the same answer from all three. It would: the standard is FLOWED, so it
/// follows the curve and the non-flowed shape is never read.
#[test]
fn the_non_flowed_shape_is_inert_for_a_flowed_standard() {
    let shapes = [
        NonFlowedDistributionMethod::BEGINNING,
        NonFlowedDistributionMethod::MIDDLE,
        NonFlowedDistributionMethod::END,
    ];

    let results: Vec<_> = shapes
        .iter()
        .map(|shape| run_flowed_scenario(10.0, 4.0, *shape, 4.0, 10.0, false).planned_shifts)
        .collect();

    assert_eq!(results[0], vec![(time(7, 0), time(17, 0))]);
    assert!(results.iter().all(|shifts| *shifts == results[0]));
}

/// Java's matrix also varies the staff value across two values and expects the
/// same answer from both — because its staff standard is **never attached to
/// the assignment**. Building it and leaving it detached makes the column dead.
///
/// Attaching it here shows the omission is not harmless: the staffing work is
/// real, and it changes the plan.
#[test]
fn the_java_fixture_never_attaches_its_staff_standard() {
    let without_staff = run_flowed_scenario(
        10.0,
        4.0,
        NonFlowedDistributionMethod::BEGINNING,
        4.0,
        10.0,
        false,
    );
    let with_staff = run_flowed_scenario(
        10.0,
        4.0,
        NonFlowedDistributionMethod::BEGINNING,
        4.0,
        10.0,
        true,
    );

    assert_eq!(without_staff.planned_shifts, vec![(time(7, 0), time(17, 0))]);
    assert_ne!(with_staff.planned_shifts, without_staff.planned_shifts);
}
