//! Finds the shift already on the schedule that a newly planned one would
//! duplicate.
//!
//! Planning a date that has already been planned would otherwise stack a second
//! copy of every shift on top of the first. Each existing shift can only
//! account for one new one, so a shift that matches is **taken out of the pool**
//! — two identical new shifts against one existing shift match once, and the
//! second is genuinely new.
//!
//! Java reads the pools off `PlannerModel` and removes from them in place. The
//! model here is shared immutably, so the pools live in the matcher instead;
//! the consuming behaviour is the same.

use crate::workcontent::domain::planned_shift::PlannedShift;

pub struct ExistingPlannedShiftMatcher {
    /// Shifts already on the schedule for this job.
    scheduled: Vec<PlannedShift>,
    /// Shifts held against an unanswered employee request. Skipped entirely
    /// when the plan is clearing the schedule — those shifts are going away.
    with_pending_requests: Vec<PlannedShift>,
    clear_schedules: bool,
}

impl ExistingPlannedShiftMatcher {
    /// A matcher with nothing already on the schedule, so nothing matches.
    pub fn empty() -> Self {
        Self::new(Vec::new(), Vec::new(), false)
    }

    pub fn new(
        scheduled: Vec<PlannedShift>,
        with_pending_requests: Vec<PlannedShift>,
        clear_schedules: bool,
    ) -> Self {
        Self {
            scheduled,
            with_pending_requests,
            clear_schedules,
        }
    }

    /// The existing shift `planned_shift_to_match` duplicates, removed from the
    /// pool so it cannot match twice.
    ///
    /// Scheduled shifts are searched first; shifts awaiting a request decision
    /// only if the schedule is not being cleared.
    pub fn find_matching_planned_shift(
        &mut self,
        planned_shift_to_match: &PlannedShift,
    ) -> Option<PlannedShift> {
        if let Some(matched) = take_match(&mut self.scheduled, planned_shift_to_match) {
            return Some(matched);
        }

        if self.clear_schedules {
            return None;
        }

        take_match(&mut self.with_pending_requests, planned_shift_to_match)
    }

    pub fn scheduled(&self) -> &[PlannedShift] {
        &self.scheduled
    }

    pub fn with_pending_requests(&self) -> &[PlannedShift] {
        &self.with_pending_requests
    }
}

fn take_match(pool: &mut Vec<PlannedShift>, to_match: &PlannedShift) -> Option<PlannedShift> {
    let index = pool
        .iter()
        .position(|existing| planned_shifts_match(existing, to_match))?;

    Some(pool.remove(index))
}

/// Whether two planned shifts are the same shift for scheduling purposes.
///
/// Java compares the assignment by object identity; here it is compared by id,
/// which is equivalent in practice — the engine works from one assignment
/// instance per job.
pub fn planned_shifts_match(planned_shift_1: &PlannedShift, planned_shift_2: &PlannedShift) -> bool {
    planned_shift_2.job_id() == planned_shift_1.job_id()
        && planned_shift_2.assignment_id() == planned_shift_1.assignment_id()
        && planned_shift_2.start_date_time() == planned_shift_1.start_date_time()
        && planned_shift_2.end_date_time() == planned_shift_1.end_date_time()
        && planned_shift_2.shift_type() == planned_shift_1.shift_type()
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `ExistingPlannedShiftMatcherTest.groovy`.

    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::plan_type::PlanType;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    fn at(hour: i32) -> LocalDateTime {
        LocalDate::new(2014, 1, 1).at_time(LocalTime::new(hour, 0, 0))
    }

    /// The Java fixture: job 101, assignment 201, 08:00–15:00.
    fn shift(
        job_id: JobId,
        assignment_id: JobId,
        start_hour: i32,
        end_hour: i32,
        shift_type: PlanType,
    ) -> PlannedShift {
        let mut planned_shift = PlannedShift::new();
        planned_shift.set_job_id(Some(job_id));
        planned_shift.set_assignment_id(Some(assignment_id));
        planned_shift.set_start_date_time(Some(at(start_hour)));
        planned_shift.set_end_date_time(Some(at(end_hour)));
        planned_shift.set_shift_type(Some(shift_type));
        planned_shift
    }

    #[test]
    fn no_existing_planned_shifts_returns_null() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let mut matcher = ExistingPlannedShiftMatcher::empty();

        let to_match = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);

        assert!(matcher.find_matching_planned_shift(&to_match).is_none());
    }

    #[test]
    fn existing_planned_shifts_but_no_match_returns_null() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let existing = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let to_match = shift(job_id, assignment_id, 8, 15, PlanType::Generated);
        let mut matcher = ExistingPlannedShiftMatcher::new(vec![existing], Vec::new(), false);

        assert!(matcher.find_matching_planned_shift(&to_match).is_none());
    }

    #[test]
    fn an_existing_planned_shift_that_matches_is_returned_and_consumed() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let existing = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let existing_id = existing.id();
        let to_match = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let mut matcher = ExistingPlannedShiftMatcher::new(vec![existing], Vec::new(), false);

        let matched = matcher.find_matching_planned_shift(&to_match);

        assert_eq!(matched.map(|shift| shift.id()), Some(existing_id));
        // Java asserts this against a mock that hands back a fresh list each
        // call, so the assertion cannot fail there. Here the pool is real.
        assert!(matcher.scheduled().is_empty());
    }

    #[test]
    fn a_shift_with_a_pending_request_that_matches_is_returned_and_consumed() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let existing = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let existing_id = existing.id();
        let to_match = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let mut matcher = ExistingPlannedShiftMatcher::new(Vec::new(), vec![existing], false);

        let matched = matcher.find_matching_planned_shift(&to_match);

        assert_eq!(matched.map(|shift| shift.id()), Some(existing_id));
        assert!(matcher.with_pending_requests().is_empty());
    }

    #[test]
    fn job_mismatch() {
        let assignment_id = JobId::new();
        let planned_shift_1 = shift(JobId::new(), assignment_id, 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(JobId::new(), assignment_id, 8, 15, PlanType::Forecast);

        assert!(!planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }

    #[test]
    fn assignment_mismatch() {
        let job_id = JobId::new();
        let planned_shift_1 = shift(job_id, JobId::new(), 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(job_id, JobId::new(), 8, 15, PlanType::Forecast);

        assert!(!planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }

    #[test]
    fn start_date_time_mismatch() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let planned_shift_1 = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(job_id, assignment_id, 9, 15, PlanType::Forecast);

        assert!(!planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }

    #[test]
    fn end_date_time_mismatch() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let planned_shift_1 = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(job_id, assignment_id, 8, 16, PlanType::Forecast);

        assert!(!planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }

    #[test]
    fn shift_type_mismatch() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let planned_shift_1 = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(job_id, assignment_id, 8, 15, PlanType::Generated);

        assert!(!planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }

    #[test]
    fn planned_shifts_match_when_every_field_agrees() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let planned_shift_1 = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);
        let planned_shift_2 = shift(job_id, assignment_id, 8, 15, PlanType::Forecast);

        assert!(planned_shifts_match(&planned_shift_1, &planned_shift_2));
    }
}

#[cfg(test)]
mod tests {
    //! Behaviour the Java suite's mocked pools could not exercise.

    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::plan_type::PlanType;
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    fn at(hour: i32) -> LocalDateTime {
        LocalDate::new(2014, 1, 1).at_time(LocalTime::new(hour, 0, 0))
    }

    fn shift(job_id: JobId, assignment_id: JobId) -> PlannedShift {
        let mut planned_shift = PlannedShift::new();
        planned_shift.set_job_id(Some(job_id));
        planned_shift.set_assignment_id(Some(assignment_id));
        planned_shift.set_start_date_time(Some(at(8)));
        planned_shift.set_end_date_time(Some(at(15)));
        planned_shift.set_shift_type(Some(PlanType::Forecast));
        planned_shift
    }

    /// One existing shift accounts for one new shift, not for every new shift
    /// that looks like it.
    #[test]
    fn one_existing_shift_can_only_account_for_one_new_shift() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let mut matcher = ExistingPlannedShiftMatcher::new(
            vec![shift(job_id, assignment_id)],
            Vec::new(),
            false,
        );
        let to_match = shift(job_id, assignment_id);

        assert!(matcher.find_matching_planned_shift(&to_match).is_some());
        assert!(matcher.find_matching_planned_shift(&to_match).is_none());
    }

    /// Shifts awaiting a request decision are going away when the schedule is
    /// being cleared, so they are not offered as matches.
    #[test]
    fn clearing_the_schedule_ignores_shifts_with_pending_requests() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let mut matcher = ExistingPlannedShiftMatcher::new(
            Vec::new(),
            vec![shift(job_id, assignment_id)],
            true,
        );
        let to_match = shift(job_id, assignment_id);

        assert!(matcher.find_matching_planned_shift(&to_match).is_none());
        assert_eq!(matcher.with_pending_requests().len(), 1);
    }

    /// The scheduled pool is exhausted before pending requests are considered.
    #[test]
    fn scheduled_shifts_are_matched_before_shifts_with_pending_requests() {
        let job_id = JobId::new();
        let assignment_id = JobId::new();
        let scheduled = shift(job_id, assignment_id);
        let scheduled_id = scheduled.id();
        let mut matcher = ExistingPlannedShiftMatcher::new(
            vec![scheduled],
            vec![shift(job_id, assignment_id)],
            false,
        );
        let to_match = shift(job_id, assignment_id);

        let matched = matcher.find_matching_planned_shift(&to_match);

        assert_eq!(matched.map(|shift| shift.id()), Some(scheduled_id));
        assert_eq!(matcher.with_pending_requests().len(), 1);
    }
}
