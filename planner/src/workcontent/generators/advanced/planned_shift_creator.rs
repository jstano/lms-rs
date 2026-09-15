//! Turns one peeled layer of work into the shifts that will cover it.
//!
//! A layer that runs `min_value` bodies deep needs that many people on it, so
//! it produces that many shifts — all with the same times, one per body, and
//! one set per plan type the run writes against.
//!
//! Two dates are recorded and they are not the same date: `shift_date` is when
//! the work actually starts, and `date_shift_generated_from` is the shift
//! template it came from. A night shift that crosses midnight has work starting
//! on the day after the template it was generated from.

use crate::workcontent::domain::plan_type::PlanType;
use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::shift_source::ShiftSource;
use crate::workcontent::generators::advanced::existing_planned_shift_matcher::ExistingPlannedShiftMatcher;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::work_content_tracker::WorkContentTrackerBean;
use date_range_rs::DateTimeRange;

/// The shifts covering `bean`'s layer.
///
/// Passing a `matcher` checks each forecast shift against what is already on
/// the schedule and drops the ones that would duplicate an existing shift;
/// passing `None` creates every shift unconditionally. Only the forecast copy
/// is checked — the plan's own copies are always written.
pub fn create_planned_shift_records(
    bean: &WorkContentTrackerBean,
    params: &GeneratorParameters,
    matcher: Option<&mut ExistingPlannedShiftMatcher>,
) -> Vec<PlannedShift> {
    let plan_types = params.planner_model().plan_types();
    let mut matcher = matcher;
    let mut planned_shifts = Vec::new();

    for _ in 0..bean.min_value() {
        for plan_type in &plan_types {
            let planned_shift = create_new_planned_shift(bean, params, *plan_type);

            let is_duplicate = *plan_type == PlanType::Forecast
                && matcher
                    .as_mut()
                    .is_some_and(|matcher| {
                        matcher.find_matching_planned_shift(&planned_shift).is_some()
                    });

            if !is_duplicate {
                planned_shifts.push(planned_shift);
            }
        }
    }

    planned_shifts
}

/// Java also stamps the property's default shift category onto each shift;
/// shift categories are not modelled in this port, as in BASIC.
///
/// Java additionally leaves `assignment` unset for a job-level assignment and
/// sets it otherwise. That distinction lives on the labor level, which is not
/// modelled either, so the id is set unconditionally — the same choice the
/// shipped BASIC creator makes, and consistent with what the matcher compares.
fn create_new_planned_shift(
    bean: &WorkContentTrackerBean,
    params: &GeneratorParameters,
    plan_type: PlanType,
) -> PlannedShift {
    let work_range = DateTimeRange::of(bean.start_date_time(), bean.end_date_time());
    let mut planned_shift = PlannedShift::new();

    planned_shift.set_job_id(Some(params.job().id()));
    planned_shift.set_assignment_id(Some(params.job().id()));
    planned_shift.set_property_id(Some(params.job().property_id()));
    planned_shift.set_shift_type(Some(plan_type));
    planned_shift.set_shift_date(Some(bean.start_date_time().to_local_date()));
    planned_shift.set_date_shift_generated_from(Some(
        bean.shift_start_date_time().to_local_date(),
    ));
    planned_shift.set_source(Some(ShiftSource::Auto));
    planned_shift.set_start_date_time(Some(bean.start_date_time()));
    planned_shift.set_end_date_time(Some(bean.end_date_time()));
    planned_shift.set_duration(work_range.duration().fractional_hours());

    planned_shift
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `PlannedShiftCreatorTest.groovy`. Its shift-category
    //! assertions are dropped — categories are out of scope for this port.

    use super::*;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    #[test]
    fn an_empty_work_content_tracker_bean_generates_no_planned_shifts() {
        let mut context = Context::new(
            LocalDate::new(2014, 10, 10),
            LocalTime::new(10, 0, 0),
            LocalTime::new(15, 0, 0),
            30,
        );
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        let params = context.params();
        // A bean with a depth of zero covers nobody.
        let bean = WorkContentTrackerBean::new(&params);

        let results = create_planned_shift_records(
            &bean,
            &params,
            Some(&mut ExistingPlannedShiftMatcher::empty()),
        );

        assert!(results.is_empty());
    }

    /// Java's midnight-crossing case: a 16:00 template generates work that
    /// starts at midnight the next day. The work's date and the template's date
    /// must not collapse into one.
    #[test]
    fn date_shift_generated_from_is_the_template_date_not_the_work_start_date() {
        let shift_generation_date = LocalDate::new(2016, 8, 6);
        let mut context = Context::new(
            shift_generation_date,
            LocalTime::new(16, 0, 0),
            LocalTime::new(22, 0, 0),
            30,
        );
        context.planner_model = context.planner_model.with_planner_mode(PlannerMode::Projected);
        let params = context.params();

        let mut bean = WorkContentTrackerBean::new(&params);
        bean.set_start_date_time(shift_generation_date.plus_days(1).at_start_of_day());
        bean.set_end_date_time(
            shift_generation_date
                .plus_days(1)
                .at_time(LocalTime::new(6, 0, 0)),
        );
        bean.set_min_value(1);

        let results = create_planned_shift_records(&bean, &params, None);

        assert_eq!(results.len(), 2);

        for (planned_shift, shift_type) in results.iter().zip([PlanType::Forecast, PlanType::Original])
        {
            assert_eq!(planned_shift.job_id(), Some(params.job().id()));
            assert_eq!(planned_shift.shift_type(), Some(shift_type));
            assert_eq!(
                planned_shift.shift_date(),
                Some(shift_generation_date.plus_days(1))
            );
            assert_eq!(
                planned_shift.date_shift_generated_from(),
                Some(shift_generation_date)
            );
            assert_eq!(planned_shift.start_date_time(), Some(bean.start_date_time()));
            assert_eq!(planned_shift.end_date_time(), Some(bean.end_date_time()));
            assert_eq!(planned_shift.duration(), 6.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2014, 10, 10)
    }

    /// A five-hour layer on a 10:00 shift, `min_value` bodies deep.
    fn context(planner_mode: PlannerMode) -> Context {
        let mut context = Context::new(
            date(),
            LocalTime::new(10, 0, 0),
            LocalTime::new(15, 0, 0),
            30,
        );
        context.planner_model = context.planner_model.with_planner_mode(planner_mode);
        context
    }

    fn bean(params: &GeneratorParameters, min_value: i32) -> WorkContentTrackerBean {
        let mut bean = WorkContentTrackerBean::new(params);
        bean.set_start_date_time(date().at_time(LocalTime::new(10, 0, 0)));
        bean.set_end_date_time(date().at_time(LocalTime::new(15, 0, 0)));
        bean.set_min_value(min_value);
        bean
    }

    /// A layer three bodies deep needs three people on it.
    #[test]
    fn a_shift_is_created_for_every_body_in_the_layer() {
        let context = context(PlannerMode::Standard);
        let params = context.params();

        let results = create_planned_shift_records(&bean(&params, 3), &params, None);

        assert_eq!(results.len(), 3);
        assert!(
            results
                .iter()
                .all(|shift| shift.shift_type() == Some(PlanType::Standard))
        );
    }

    /// A projected run writes the forecast and the plan, so each body produces
    /// two shifts.
    #[test]
    fn a_projected_run_writes_one_shift_per_body_per_plan_type() {
        let context = context(PlannerMode::Projected);
        let params = context.params();

        let results = create_planned_shift_records(&bean(&params, 2), &params, None);

        assert_eq!(results.len(), 4);
        assert_eq!(
            results
                .iter()
                .filter(|shift| shift.shift_type() == Some(PlanType::Forecast))
                .count(),
            2
        );
        assert_eq!(
            results
                .iter()
                .filter(|shift| shift.shift_type() == Some(PlanType::Original))
                .count(),
            2
        );
    }

    /// The forecast copy of a shift that is already on the schedule is dropped;
    /// the plan's own copy is written regardless.
    #[test]
    fn a_forecast_shift_already_on_the_schedule_is_not_written_again() {
        let context = context(PlannerMode::Projected);
        let params = context.params();
        let existing = create_planned_shift_records(&bean(&params, 1), &params, None)
            .into_iter()
            .find(|shift| shift.shift_type() == Some(PlanType::Forecast))
            .expect("a forecast shift");
        let mut matcher = ExistingPlannedShiftMatcher::new(vec![existing], Vec::new(), false);

        let results = create_planned_shift_records(&bean(&params, 1), &params, Some(&mut matcher));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].shift_type(), Some(PlanType::Original));
    }

    /// One existing shift accounts for one body, not for the whole layer.
    #[test]
    fn a_deeper_layer_keeps_the_bodies_the_schedule_does_not_already_cover() {
        let context = context(PlannerMode::Projected);
        let params = context.params();
        let existing = create_planned_shift_records(&bean(&params, 1), &params, None)
            .into_iter()
            .find(|shift| shift.shift_type() == Some(PlanType::Forecast))
            .expect("a forecast shift");
        let mut matcher = ExistingPlannedShiftMatcher::new(vec![existing], Vec::new(), false);

        let results = create_planned_shift_records(&bean(&params, 3), &params, Some(&mut matcher));

        // Three bodies: two forecast shifts survive, and all three plan copies.
        assert_eq!(
            results
                .iter()
                .filter(|shift| shift.shift_type() == Some(PlanType::Forecast))
                .count(),
            2
        );
        assert_eq!(
            results
                .iter()
                .filter(|shift| shift.shift_type() == Some(PlanType::Original))
                .count(),
            3
        );
    }

    #[test]
    fn every_shift_is_stamped_as_automatically_generated() {
        let context = context(PlannerMode::Standard);
        let params = context.params();

        let results = create_planned_shift_records(&bean(&params, 1), &params, None);

        assert_eq!(results[0].source(), Some(ShiftSource::Auto));
        assert_eq!(results[0].property_id(), Some(params.job().property_id()));
        assert_eq!(results[0].duration(), 5.0);
    }
}
