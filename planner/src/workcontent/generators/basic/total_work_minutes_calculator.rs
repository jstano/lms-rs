//! How much work the standards require, in minutes, for one job/shift/date.
//!
//! Ported from Java's `TotalWorkMinutesForStandardsCalculator` and
//! `CalculateWorkMinutesPerUnit`. The audit trail those classes write
//! (`WorkContentLogDetail` and its formula strings) is not ported.

use crate::workcontent::common::numbers;
use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::job_shift::JobShift;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::domain::units::Units;
use crate::workcontent::generators::error::GenerationError;
use joda_rs::LocalDate;
use joda_rs::constants::MINUTES_PER_HOUR;

pub trait TotalWorkMinutesCalculator {
    fn total_work_minutes(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift: &JobShift,
        date: LocalDate,
        shift_length: f64,
    ) -> Result<i32, GenerationError>;
}

pub struct TotalWorkMinutesCalculatorImpl;

impl TotalWorkMinutesCalculatorImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl TotalWorkMinutesCalculator for TotalWorkMinutesCalculatorImpl {
    fn total_work_minutes(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
        shift: &JobShift,
        date: LocalDate,
        shift_length: f64,
    ) -> Result<i32, GenerationError> {
        // Java accumulates into an `int` with a compound assignment, so each
        // standard's contribution is truncated as it is added rather than the
        // total being rounded once at the end.
        let mut total_work = 0;

        for standard in job.non_staff_shift_standards_for_standard_set_and_shift(
            planner_model.standard_set_id(),
            shift.id(),
        ) {
            total_work +=
                work_minutes_for_standard(planner_model, standard, date, shift_length)? as i32;
        }

        Ok(total_work)
    }
}

fn work_minutes_for_standard(
    planner_model: &PlannerModel,
    standard: &ShiftStandard,
    date: LocalDate,
    shift_length: f64,
) -> Result<f64, GenerationError> {
    let driver_value = planner_model.business_driver_value(standard.business_driver_id(), date);

    // The band is chosen on the raw volume, but the work is calculated from the
    // volume left after the suppressed portion is taken off.
    let standard_value = match standard.value_for_volume(driver_value) {
        Some(standard_value) => standard_value,
        None => return Ok(0.0),
    };
    let suppressed_driver_value = standard.suppressed_driver_value(driver_value);

    calculate_work_minutes_per_unit(
        standard.units(),
        standard_value,
        suppressed_driver_value,
        shift_length,
    )
}

/// Convert a standard value and a volume into work minutes according to the
/// unit the standard is expressed in.
///
/// A standard value of zero short-circuits before the unit is consulted, which
/// is why an otherwise unsupported unit still yields no work rather than an
/// error when its value is zero — matching Java, where the guard precedes the
/// switch.
pub fn calculate_work_minutes_per_unit(
    units: Units,
    standard_value: f64,
    driver_value: i32,
    shift_length: f64,
) -> Result<f64, GenerationError> {
    if standard_value == 0.0 {
        return Ok(0.0);
    }

    let minutes_per_hour = MINUTES_PER_HOUR as f64;
    let driver_value = driver_value as f64;

    let minutes = match units {
        Units::HoursPerUnit => standard_value * driver_value * minutes_per_hour,
        Units::MinutesPerUnit => standard_value * driver_value,
        Units::UnitsPerHour => driver_value / standard_value * minutes_per_hour,
        Units::UnitsPerMinute => driver_value / standard_value,
        Units::Hours => standard_value * minutes_per_hour,
        Units::Minutes => standard_value,
        Units::UnitsPerShift => driver_value / standard_value * shift_length * minutes_per_hour,
        Units::UnitsPerPerson => return Err(GenerationError::InvalidUnits(units)),
    };

    Ok(numbers::round_raw_hours(minutes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::{JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::shift_standard::ShiftStandardRange;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::work_type::WorkType;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use rstest::rstest;
    use std::collections::HashMap;

    const DATE: (i32, i32, i32) = (2025, 10, 6);

    fn date() -> LocalDate {
        LocalDate::new(DATE.0, DATE.1, DATE.2)
    }

    // 2 units per unit-of-driver, one band covering every volume used here.
    #[rstest]
    // 0.5 hours per unit * 10 units * 60 = 300 minutes.
    #[case(Units::HoursPerUnit, 0.5, 10, 8.0, 300.0)]
    // 1.5 minutes per unit * 10 units.
    #[case(Units::MinutesPerUnit, 1.5, 10, 8.0, 15.0)]
    // 10 units at 4 units an hour = 2.5 hours = 150 minutes.
    #[case(Units::UnitsPerHour, 4.0, 10, 8.0, 150.0)]
    // 10 units at 4 units a minute = 2.5 minutes.
    #[case(Units::UnitsPerMinute, 4.0, 10, 8.0, 2.5)]
    // A flat 2 hours regardless of volume.
    #[case(Units::Hours, 2.0, 10, 8.0, 120.0)]
    // A flat 90 minutes regardless of volume.
    #[case(Units::Minutes, 90.0, 10, 8.0, 90.0)]
    // 10 units at 4 units a shift = 2.5 shifts of 8 hours = 1200 minutes.
    #[case(Units::UnitsPerShift, 4.0, 10, 8.0, 1200.0)]
    fn each_unit_converts_to_work_minutes(
        #[case] units: Units,
        #[case] standard_value: f64,
        #[case] driver_value: i32,
        #[case] shift_length: f64,
        #[case] expected: f64,
    ) {
        assert_eq!(
            calculate_work_minutes_per_unit(units, standard_value, driver_value, shift_length),
            Ok(expected)
        );
    }

    #[test]
    fn a_zero_standard_value_generates_no_work() {
        // Guarded explicitly, since the rate units would divide by it.
        assert_eq!(
            calculate_work_minutes_per_unit(Units::UnitsPerHour, 0.0, 10, 8.0),
            Ok(0.0)
        );
        assert_eq!(
            calculate_work_minutes_per_unit(Units::MinutesPerUnit, 0.0, 10, 8.0),
            Ok(0.0)
        );
    }

    #[test]
    fn volume_independent_units_ignore_the_driver_value() {
        assert_eq!(
            calculate_work_minutes_per_unit(Units::Hours, 2.0, 0, 8.0),
            Ok(120.0)
        );
        assert_eq!(
            calculate_work_minutes_per_unit(Units::Minutes, 90.0, 999, 8.0),
            Ok(90.0)
        );
    }

    fn standard(
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: BusinessDriverId,
        work_type: WorkType,
        units: Units,
        suppress_value: i32,
        ranges: Vec<ShiftStandardRange>,
    ) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            business_driver_id,
            work_type,
            units,
            suppress_value,
            ranges,
        )
    }

    fn shift(standard_set_id: StandardSetId) -> JobShift {
        JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            vec![JobShiftDefinition::new(
                DayOfWeek::Monday,
                LocalTime::of_hour_minute(9, 0),
                LocalTime::of_hour_minute(17, 0),
                0.0,
                0.0,
                1,
            )],
        )
    }

    fn model(
        standard_set_id: StandardSetId,
        business_driver_id: BusinessDriverId,
        volume: i32,
    ) -> PlannerModel {
        PlannerModel::new(
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
                    HashMap::from([(date(), volume)]),
                ),
            )]),
        )
    }

    fn job(shift_standards: Vec<ShiftStandard>) -> Job {
        Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            shift_standards,
        )
    }

    #[test]
    fn work_from_every_matching_standard_is_summed() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 100);
        // 1.5 min/unit * 100 = 150, plus a flat 30 minutes.
        let job = job(vec![
            standard(
                standard_set_id,
                shift.id(),
                business_driver_id,
                WorkType::Variable,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 1000, 1.5)],
            ),
            standard(
                standard_set_id,
                shift.id(),
                business_driver_id,
                WorkType::Daily,
                Units::Minutes,
                0,
                vec![ShiftStandardRange::new(0, 1000, 30.0)],
            ),
        ]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 180);
    }

    #[test]
    fn the_suppressed_volume_is_taken_off_before_the_standard_is_applied() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 100);
        // 2 min/unit against 100 - 40 = 60 units.
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            40,
            vec![ShiftStandardRange::new(0, 1000, 2.0)],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 120);
    }

    #[test]
    fn a_suppress_value_above_the_volume_floors_at_zero() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 10);
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            40,
            vec![ShiftStandardRange::new(0, 1000, 2.0)],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 0);
    }

    #[test]
    fn the_band_is_chosen_on_the_raw_volume_not_the_suppressed_one() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 100);
        // The raw 100 lands in the upper band at 3 min/unit; the suppressed 60
        // would have landed in the lower band at 1 min/unit.
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            40,
            vec![
                ShiftStandardRange::new(0, 99, 1.0),
                ShiftStandardRange::new(100, 1000, 3.0),
            ],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 180);
    }

    #[test]
    fn a_volume_outside_every_band_generates_no_work() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 5000);
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::new(0, 1000, 2.0)],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 0);
    }

    #[test]
    fn staff_standards_do_not_contribute() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 100);
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            WorkType::Staff,
            Units::MinutesPerUnit,
            0,
            vec![ShiftStandardRange::new(0, 1000, 2.0)],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 0);
    }

    #[test]
    fn a_job_with_no_standards_requires_no_work() {
        let standard_set_id = StandardSetId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, BusinessDriverId::new(), 100);

        let total = TotalWorkMinutesCalculatorImpl::new().total_work_minutes(
            &planner_model,
            &job(Vec::new()),
            &shift,
            date(),
            8.0,
        )
        .expect("the fixture is configured to plan");

        assert_eq!(total, 0);
    }

    #[test]
    fn each_standards_contribution_is_truncated_as_it_is_added() {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, 1);
        // Two standards of 10.5 minutes each truncate to 10 apiece, not 21.
        let half_minute_standard = || {
            standard(
                standard_set_id,
                shift.id(),
                business_driver_id,
                WorkType::Variable,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 1000, 10.5)],
            )
        };
        let job = job(vec![half_minute_standard(), half_minute_standard()]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, 20);
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// Every expectation below is a row from `CalculateWorkMinutesPerUnitTest.groovy`,
/// transcribed unchanged.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::{JobShiftDefinition, JobShiftId};
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::planner_settings::PlannerSettings;
    use crate::workcontent::domain::shift_standard::ShiftStandardRange;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use crate::workcontent::domain::work_type::WorkType;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};
    use rstest::rstest;
    use std::collections::HashMap;

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    /// A Monday shift, 09:00 to 17:00.
    fn shift(standard_set_id: StandardSetId) -> JobShift {
        JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            vec![JobShiftDefinition::new(
                DayOfWeek::Monday,
                LocalTime::of_hour_minute(9, 0),
                LocalTime::of_hour_minute(17, 0),
                0.0,
                0.0,
                1,
            )],
        )
    }

    fn model(
        standard_set_id: StandardSetId,
        business_driver_id: BusinessDriverId,
        volume: i32,
    ) -> PlannerModel {
        PlannerModel::new(
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
                    HashMap::from([(date(), volume)]),
                ),
            )]),
        )
    }

    fn job(shift_standards: Vec<ShiftStandard>) -> Job {
        Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            shift_standards,
        )
    }

    fn standard(
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: BusinessDriverId,
        units: Units,
        suppress_value: i32,
        ranges: Vec<ShiftStandardRange>,
    ) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            standard_set_id,
            job_shift_id,
            business_driver_id,
            WorkType::Variable,
            units,
            suppress_value,
            ranges,
        )
    }

    /// `CalculateWorkMinutesPerUnitTest.testCalculations` — all twenty-one rows.
    ///
    /// The `UNITS_PER_PERSON` row reaches the zero-standard-value guard before
    /// the unit is consulted, which is why it yields no work rather than the
    /// error a non-zero value would produce. See
    /// [`units_with_no_formula_are_an_error_once_past_the_zero_guard`].
    #[rstest]
    // A standard value of zero short-circuits to no work, whatever the unit.
    #[case(Units::HoursPerUnit, 0.0, 1, 1.0, 0.0)]
    #[case(Units::Hours, 0.0, 1, 1.0, 0.0)]
    #[case(Units::Minutes, 0.0, 1, 1.0, 0.0)]
    #[case(Units::MinutesPerUnit, 0.0, 1, 1.0, 0.0)]
    #[case(Units::UnitsPerHour, 0.0, 1, 1.0, 0.0)]
    #[case(Units::UnitsPerMinute, 0.0, 1, 1.0, 0.0)]
    #[case(Units::UnitsPerPerson, 0.0, 1, 1.0, 0.0)]
    #[case(Units::UnitsPerShift, 0.0, 1, 1.0, 0.0)]
    // Hours per unit scales with both the standard and the volume.
    #[case(Units::HoursPerUnit, 1.0, 1, 1.0, 60.0)]
    #[case(Units::HoursPerUnit, 2.0, 1, 1.0, 120.0)]
    #[case(Units::HoursPerUnit, 2.0, 2, 10.0, 240.0)]
    // Units per minute divides the volume by the rate.
    #[case(Units::UnitsPerMinute, 1.0, 1, 1.0, 1.0)]
    #[case(Units::UnitsPerMinute, 2.0, 1, 1.0, 0.5)]
    // Flat hours and minutes ignore the volume entirely.
    #[case(Units::Hours, 2.0, 1, 1.0, 120.0)]
    #[case(Units::Minutes, 2.0, 1, 1.0, 2.0)]
    // Units per shift brings the shift length into it; the others do not.
    #[case(Units::UnitsPerShift, 2.0, 1, 1.0, 30.0)]
    #[case(Units::UnitsPerShift, 2.0, 1, 10.0, 300.0)]
    #[case(Units::MinutesPerUnit, 2.0, 1, 1.0, 2.0)]
    #[case(Units::MinutesPerUnit, 2.0, 1, 10.0, 2.0)]
    // Units per hour divides the volume by the rate, then scales to an hour.
    #[case(Units::UnitsPerHour, 2.0, 2, 1.0, 60.0)]
    #[case(Units::UnitsPerHour, 2.0, 10, 1.0, 300.0)]
    fn matches_the_java_work_minutes_per_unit_table(
        #[case] units: Units,
        #[case] standard_value: f64,
        #[case] driver_value: i32,
        #[case] shift_length: f64,
        #[case] expected: f64,
    ) {
        assert_eq!(
            calculate_work_minutes_per_unit(units, standard_value, driver_value, shift_length),
            Ok(expected)
        );
    }

    /// Java reaches `default: throw IllegalArgumentException` for a unit with
    /// no formula. The Spock table only ever exercises it with a zero standard
    /// value, so it never gets there; this pins the branch Java would take.
    #[test]
    fn units_with_no_formula_are_an_error_once_past_the_zero_guard() {
        assert_eq!(
            calculate_work_minutes_per_unit(Units::UnitsPerPerson, 2.0, 10, 8.0),
            Err(GenerationError::InvalidUnits(Units::UnitsPerPerson))
        );
    }

    /// `TotalWorkMinutesForStandardsCalculatorTest."calculation uses suppress value"`.
    ///
    /// The suppressed volume is what the work is calculated from, so at one
    /// minute per unit the resulting minutes *are* the suppressed volume. Note
    /// that the raw volume still chooses the band; only the calculation is
    /// suppressed.
    #[rstest]
    // A suppression larger than the volume leaves nothing.
    #[case(100, 101, 0)]
    // Suppressing exactly the volume also leaves nothing.
    #[case(100, 100, 0)]
    #[case(101, 100, 1)]
    // No suppression configured leaves the volume untouched.
    #[case(100, 0, 100)]
    fn matches_the_java_suppress_value_table(
        #[case] driver_value: i32,
        #[case] suppress_value: i32,
        #[case] expected_suppressed_value: i32,
    ) {
        let standard_set_id = StandardSetId::new();
        let business_driver_id = BusinessDriverId::new();
        let shift = shift(standard_set_id);
        let planner_model = model(standard_set_id, business_driver_id, driver_value);
        let job = job(vec![standard(
            standard_set_id,
            shift.id(),
            business_driver_id,
            // One minute of work per unit, so the minutes read back as volume.
            Units::MinutesPerUnit,
            suppress_value,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        )]);

        let total = TotalWorkMinutesCalculatorImpl::new()
            .total_work_minutes(&planner_model, &job, &shift, date(), 8.0)
            .expect("the fixture is configured to plan");

        assert_eq!(total, expected_suppressed_value);
    }
}
