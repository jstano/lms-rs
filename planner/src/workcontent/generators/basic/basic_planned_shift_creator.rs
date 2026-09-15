//! Mirrors each block of work with a shift on the schedule.
//!
//! Ported from `SimpleNonFlowedGenerator`'s `PlannedShiftCreator`. Java also
//! stamps the property's default shift category onto each shift; shift
//! categories are not modelled in this port.

use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::shift_source::ShiftSource;
use crate::workcontent::domain::work_content::WorkContent;

pub trait BasicPlannedShiftCreator {
    fn create_planned_shifts_from_work_contents(
        &self,
        work_contents: &[WorkContent],
    ) -> Vec<PlannedShift>;
}

pub struct BasicPlannedShiftCreatorImpl;

impl BasicPlannedShiftCreatorImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl BasicPlannedShiftCreator for BasicPlannedShiftCreatorImpl {
    fn create_planned_shifts_from_work_contents(
        &self,
        work_contents: &[WorkContent],
    ) -> Vec<PlannedShift> {
        work_contents.iter().map(create_planned_shift).collect()
    }
}

fn create_planned_shift(work_content: &WorkContent) -> PlannedShift {
    let mut planned_shift = PlannedShift::new();

    planned_shift.set_job_id(Some(work_content.job_id()));
    planned_shift.set_assignment_id(Some(work_content.job_id()));
    planned_shift.set_property_id(Some(work_content.property_id()));
    planned_shift.set_shift_type(Some(work_content.shift_type()));
    planned_shift.set_shift_date(Some(work_content.shift_date()));
    planned_shift.set_date_shift_generated_from(Some(work_content.shift_date()));
    planned_shift.set_start_date_time(Some(work_content.calculated_start_date_time()));
    planned_shift.set_end_date_time(Some(work_content.calculated_end_date_time()));
    planned_shift.set_duration(work_content.calculated_hours());
    planned_shift.set_source(Some(ShiftSource::Auto));

    planned_shift
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::plan_type::PlanType;
    use date_range_rs::DateTimeRange;
    use joda_rs::{LocalDate, LocalTime};

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    fn work_content(job_id: JobId, property_id: LocationId) -> WorkContent {
        WorkContent::fixed(
            job_id,
            property_id,
            PlanType::Forecast,
            date(),
            &DateTimeRange::of(
                date().at_time(LocalTime::of_hour_minute(9, 0)),
                date().at_time(LocalTime::of_hour_minute(17, 0)),
            ),
        )
    }

    #[test]
    fn a_planned_shift_mirrors_its_work_content() {
        let job_id = JobId::new();
        let property_id = LocationId::new();
        let work_content = work_content(job_id, property_id);

        let planned_shifts = BasicPlannedShiftCreatorImpl::new()
            .create_planned_shifts_from_work_contents(std::slice::from_ref(&work_content));

        assert_eq!(planned_shifts.len(), 1);
        let planned_shift = &planned_shifts[0];
        assert_eq!(planned_shift.job_id(), Some(job_id));
        assert_eq!(planned_shift.assignment_id(), Some(job_id));
        assert_eq!(planned_shift.property_id(), Some(property_id));
        assert_eq!(planned_shift.shift_type(), Some(PlanType::Forecast));
        assert_eq!(planned_shift.shift_date(), Some(date()));
        assert_eq!(planned_shift.date_shift_generated_from(), Some(date()));
        assert_eq!(
            planned_shift.start_date_time(),
            Some(work_content.calculated_start_date_time())
        );
        assert_eq!(
            planned_shift.end_date_time(),
            Some(work_content.calculated_end_date_time())
        );
        assert_eq!(planned_shift.duration(), 8.0);
    }

    #[test]
    fn generated_shifts_are_marked_as_automatic() {
        let planned_shifts = BasicPlannedShiftCreatorImpl::new()
            .create_planned_shifts_from_work_contents(&[work_content(
                JobId::new(),
                LocationId::new(),
            )]);

        assert_eq!(planned_shifts[0].source(), Some(ShiftSource::Auto));
    }

    #[test]
    fn one_shift_is_created_per_work_content() {
        let job_id = JobId::new();
        let property_id = LocationId::new();

        let planned_shifts = BasicPlannedShiftCreatorImpl::new()
            .create_planned_shifts_from_work_contents(&[
                work_content(job_id, property_id),
                work_content(job_id, property_id),
                work_content(job_id, property_id),
            ]);

        assert_eq!(planned_shifts.len(), 3);
    }

    #[test]
    fn no_work_content_produces_no_shifts() {
        assert!(
            BasicPlannedShiftCreatorImpl::new()
                .create_planned_shifts_from_work_contents(&[])
                .is_empty()
        );
    }
}

/// Cases ported verbatim from the Java engine's own test suite.
///
/// Every expectation below is from `PlannedShiftCreatorTest.groovy`,
/// transcribed unchanged.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::plan_type::PlanType;
    use date_range_rs::DateTimeRange;
    use joda_rs::{LocalDate, LocalTime};

    fn at(date: LocalDate, hour: i32, minute: i32) -> joda_rs::LocalDateTime {
        date.at_time(LocalTime::of_hour_minute(hour, minute))
    }

    #[test]
    fn no_work_content_produces_no_planned_shifts() {
        assert!(
            BasicPlannedShiftCreatorImpl::new()
                .create_planned_shifts_from_work_contents(&[])
                .is_empty()
        );
    }

    /// Two blocks belonging to different jobs, one forecast and one original,
    /// each becoming exactly one shift that mirrors it.
    #[test]
    fn each_work_content_becomes_one_shift_mirroring_it() {
        let date_one = LocalDate::new(2013, 1, 1);
        let date_two = LocalDate::new(2013, 1, 2);
        let (job_one, job_two) = (JobId::new(), JobId::new());

        let work_content_one = WorkContent::fixed(
            job_one,
            LocationId::new(),
            PlanType::Forecast,
            date_one,
            &DateTimeRange::of(at(date_one, 8, 0), at(date_one, 16, 0)),
        );
        let work_content_two = WorkContent::fixed(
            job_two,
            LocationId::new(),
            PlanType::Original,
            date_two,
            &DateTimeRange::of(at(date_one, 7, 0), at(date_one, 17, 0)),
        );

        let planned_shifts = BasicPlannedShiftCreatorImpl::new()
            .create_planned_shifts_from_work_contents(&[
                work_content_one.clone(),
                work_content_two.clone(),
            ]);

        assert_eq!(planned_shifts.len(), 2);

        for work_content in [&work_content_one, &work_content_two] {
            let shifts: Vec<_> = planned_shifts
                .iter()
                .filter(|shift| shift.job_id() == Some(work_content.job_id()))
                .collect();

            assert_eq!(shifts.len(), 1);

            let shift = shifts[0];
            assert_eq!(shift.shift_date(), Some(work_content.shift_date()));
            assert_eq!(
                shift.date_shift_generated_from(),
                Some(work_content.shift_date())
            );
            assert_eq!(
                shift.start_date_time(),
                Some(work_content.calculated_start_date_time())
            );
            assert_eq!(
                shift.end_date_time(),
                Some(work_content.calculated_end_date_time())
            );
            assert_eq!(shift.duration(), work_content.calculated_hours());
            // Java compares the assignment; this port has no separate
            // assignment tree, so the job stands in for it on both sides.
            assert_eq!(shift.assignment_id(), Some(work_content.job_id()));
        }
    }
}
