//! Port of `com.unifocus.watson.server.scheduler.engine.misc.PlannedShiftMatcher`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! PlannedShiftMatcher.java`. A unit struct, not a struct with an `EmployeeAssignmentChecker`
//! field — `EmployeeAssignmentChecker::can_employee_work_assignment` is already a plain
//! associated function (no `&self`), same as `PlannedShiftCreator`'s own `workable_assignment`
//! helper reuses it.
//!
//! `times_match` needs no custom equality work: `date_range_rs::DateTimeRange` already implements
//! real value `PartialEq` (start/end field comparison), so `==` matches Java's
//! `DateTimeRange.equals()` exactly.

use crate::engine::model::job_data::JobData;
use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::process::checkers::employee_assignment_checker::EmployeeAssignmentChecker;
use crate::entity::employee::Employee;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `PlannedShiftMatcher`.
pub struct PlannedShiftMatcher;

impl PlannedShiftMatcher {
    /// `findMatchingPlannedShift(RegularSchedule, LocalDate, JobData)`.
    pub fn find_matching_planned_shift(
        &self,
        regular_schedule: &RegularSchedule,
        employee: &Employee,
        shift_date: LocalDate,
        job_data: &JobData,
    ) -> Option<PlannedShift> {
        let regular_schedule_assignment_id =
            Self::regular_schedule_assignment(regular_schedule, employee);

        for planned_shift in job_data.planned_shifts_for_date(shift_date) {
            if let Some(regular_schedule_assignment_id) = regular_schedule_assignment_id {
                if Self::assignments_match(regular_schedule_assignment_id, &planned_shift)
                    && Self::times_match(regular_schedule, &planned_shift, shift_date)
                {
                    return Some(planned_shift);
                }
            } else if Self::employee_can_work_planned_assignment_and_times_match(
                regular_schedule,
                employee,
                &planned_shift,
                shift_date,
            ) {
                return Some(planned_shift);
            }
        }

        None
    }

    /// `getRegularScheduleAssignment(RegularSchedule)`.
    fn regular_schedule_assignment(
        regular_schedule: &RegularSchedule,
        employee: &Employee,
    ) -> Option<i32> {
        let assignment_id = regular_schedule.assignment_id();

        if EmployeeAssignmentChecker::can_employee_work_assignment(employee, assignment_id) {
            assignment_id
        } else {
            None
        }
    }

    fn employee_can_work_planned_assignment_and_times_match(
        regular_schedule: &RegularSchedule,
        employee: &Employee,
        planned_shift: &PlannedShift,
        shift_date: LocalDate,
    ) -> bool {
        EmployeeAssignmentChecker::can_employee_work_assignment(
            employee,
            planned_shift.assignment_id(),
        ) && Self::times_match(regular_schedule, planned_shift, shift_date)
    }

    fn assignments_match(
        regular_schedule_assignment_id: i32,
        planned_shift: &PlannedShift,
    ) -> bool {
        planned_shift.assignment_id() == Some(regular_schedule_assignment_id)
    }

    fn times_match(
        regular_schedule: &RegularSchedule,
        planned_shift: &PlannedShift,
        shift_date: LocalDate,
    ) -> bool {
        regular_schedule.date_time_range(shift_date) == planned_shift.to_date_time_range()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee_assignment::EmployeeAssignment;
    use crate::entity::employee_regular_period::EmployeeRegularPeriod;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::work_class::WorkClass;
    use joda_rs::{DayOfWeek, LocalDateTime, LocalTime};

    fn shift_date() -> LocalDate {
        LocalDate::of(2024, 1, 1)
    }

    fn employee(assignments: Vec<EmployeeAssignment>) -> Employee {
        Employee::new(
            1,
            "Employee 1",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            assignments,
            Vec::new(),
        )
    }

    fn regular_schedule(assignment_id: Option<i32>) -> RegularSchedule {
        let period = EmployeeRegularPeriod::new(
            1,
            Some(1),
            assignment_id,
            DayOfWeek::Monday,
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
            8.0,
        );
        RegularSchedule::new(1, &period)
    }

    fn planned_shift(assignment_id: Option<i32>) -> PlannedShift {
        PlannedShift::new(
            1,
            1,
            shift_date(),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            8.0,
            None,
        )
        .with_end_date_time(LocalDateTime::of(2024, 1, 1, 17, 0, 0))
        .with_assignment_id(assignment_id)
    }

    #[test]
    fn matches_a_planned_shift_by_the_regular_schedules_own_workable_assignment_and_time_range() {
        let employee = employee(vec![EmployeeAssignment::new(10, 0, 0, true)]);
        let regular_schedule = regular_schedule(Some(10));
        let candidate = planned_shift(Some(10));

        let matched = PlannedShiftMatcher.find_matching_planned_shift(
            &regular_schedule,
            &employee,
            shift_date(),
            &job_data_with(candidate),
        );

        assert_eq!(matched, Some(candidate));
    }

    #[test]
    fn falls_back_to_the_planned_shifts_own_assignment_when_the_employee_cannot_work_the_regular_schedules_assignment()
     {
        // Employee has no standing on assignment 10 at all, so `EmployeeAssignmentChecker`
        // rejects it — `getRegularScheduleAssignment` returns null, falling through to the
        // "does the employee already work the planned shift's own assignment" branch.
        let employee = employee(vec![EmployeeAssignment::new(20, 0, 0, true)]);
        let regular_schedule = regular_schedule(Some(10));
        let candidate = planned_shift(Some(20));

        let matched = PlannedShiftMatcher.find_matching_planned_shift(
            &regular_schedule,
            &employee,
            shift_date(),
            &job_data_with(candidate),
        );

        assert_eq!(matched, Some(candidate));
    }

    #[test]
    fn no_match_when_time_ranges_differ_even_though_assignments_match() {
        let employee = employee(vec![EmployeeAssignment::new(10, 0, 0, true)]);
        let regular_schedule = regular_schedule(Some(10));
        let mismatched_time = PlannedShift::new(
            1,
            1,
            shift_date(),
            LocalDateTime::of(2024, 1, 1, 10, 0, 0),
            7.0,
            None,
        )
        .with_end_date_time(LocalDateTime::of(2024, 1, 1, 17, 0, 0))
        .with_assignment_id(Some(10));

        let matched = PlannedShiftMatcher.find_matching_planned_shift(
            &regular_schedule,
            &employee,
            shift_date(),
            &job_data_with(mismatched_time),
        );

        assert_eq!(matched, None);
    }

    #[test]
    fn no_match_when_the_candidates_assignment_is_none() {
        let employee = employee(vec![EmployeeAssignment::new(10, 0, 0, true)]);
        let regular_schedule = regular_schedule(Some(10));
        let candidate = planned_shift(None);

        let matched = PlannedShiftMatcher.find_matching_planned_shift(
            &regular_schedule,
            &employee,
            shift_date(),
            &job_data_with(candidate),
        );

        assert_eq!(matched, None);
    }

    fn job_data_with(planned_shift: PlannedShift) -> JobData {
        let mut job_data = JobData::new(crate::entity::assignment::Assignment::new(
            1,
            "Job 1",
            false,
            None,
            None,
            None,
            false,
            Vec::<crate::entity::assignment_sort_order::AssignmentSortOrder>::new(),
            Vec::new(),
            None,
        ));
        job_data.planned_shifts_mut().push(planned_shift);
        job_data
    }
}
