//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.JobRankComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! JobRankComparator.java`. Java reads `employee.getEmployeeJobStatus(job, shiftDate).getRank()`
//! directly in the non-departmental branch, which NPEs if the employee has no status for that
//! job on that date; ported the same way (`.expect`) rather than silently defaulting, since a
//! comparator being asked to rank an employee against a job they have no status for is itself
//! the bug to surface, not paper over.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use crate::entity::employee::Employee;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `JobRankComparator`.
pub struct JobRankComparator;

impl SeniorityComparator for JobRankComparator {
    fn compare(
        &self,
        job: &Assignment,
        _assignment: Option<&Assignment>,
        shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        let job_rank = |employee_data: &EmployeeData| -> i32 {
            if job.is_departmental_seniority() {
                Self::department_seniority_rank(employee_data.employee(), job, shift_date)
            } else {
                employee_data
                    .employee()
                    .employee_job_status(job.id(), shift_date)
                    .expect("employee has no job status for the job being ranked")
                    .rank()
            }
        };

        job_rank(employee_data1).cmp(&job_rank(employee_data2))
    }
}

impl JobRankComparator {
    /// Java compares `employeeJobStatus.getJob().getParentAssignment().getID() ==
    /// department.getID()`, which NPEs if `job` has no parent (`isDepartmentalSeniority()` jobs
    /// are assumed to have one). Comparing `Option<i32>` instead means a parentless `job` here
    /// would match a status whose job is also parentless, rather than panicking — narrower than
    /// Java's crash-on-misconfiguration, wider than intended. Flag if this path is ever hit with
    /// a top-level job.
    fn department_seniority_rank(
        employee: &Employee,
        job: &Assignment,
        shift_date: LocalDate,
    ) -> i32 {
        let department_id = job.parent_assignment_id();

        let mut best_rank = 0;

        for status in employee.active_employee_job_statuses_for_date(shift_date) {
            if status.job_parent_assignment_id() == department_id
                && (best_rank == 0 || status.rank() < best_rank)
            {
                best_rank = status.rank();
            }
        }

        best_rank
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;

    fn department_job(id: i32, parent_id: Option<i32>) -> Assignment {
        Assignment::new(
            id,
            "job",
            false,
            None,
            None,
            parent_id,
            true,
            vec![],
            vec![],
            None,
        )
    }

    fn employee_with_statuses(
        id: i32,
        statuses: Vec<crate::entity::employee_job_status::EmployeeJobStatus>,
    ) -> Employee {
        Employee::new(
            id,
            "Employee",
            crate::entity::employee_type::EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            statuses,
        )
    }

    fn status(
        job_id: i32,
        department_id: Option<i32>,
        rank: i32,
    ) -> crate::entity::employee_job_status::EmployeeJobStatus {
        crate::entity::employee_job_status::EmployeeJobStatus::new(
            job_id,
            department_id,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            true,
            rank,
            LocalDate::of(2020, 1, 1),
            0.0,
            false,
            1,
        )
    }

    #[test]
    fn departmental_seniority_picks_the_best_rank_in_the_department() {
        let job = department_job(500, Some(100));
        let shift_date = LocalDate::of(2024, 1, 1);

        // Employee 1 has two jobs in department 100, best (lowest) rank is 2.
        let employee1 = employee_with_statuses(
            1,
            vec![status(501, Some(100), 5), status(502, Some(100), 2)],
        );
        // Employee 2 has one job in department 100 at rank 3.
        let employee2 = employee_with_statuses(2, vec![status(503, Some(100), 3)]);

        let data1 = EmployeeData::new(employee1, ScheduleCalcDataSet::default(), 0);
        let data2 = EmployeeData::new(employee2, ScheduleCalcDataSet::default(), 0);

        let result = JobRankComparator.compare(&job, None, shift_date, &data1, &data2);

        // Employee 1's best rank (2) beats employee 2's (3): emp1 sorts first.
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn non_departmental_seniority_reads_the_job_specific_rank() {
        let job = Assignment::new(
            500,
            "job",
            false,
            None,
            None,
            None,
            false,
            vec![],
            vec![],
            None,
        );
        let shift_date = LocalDate::of(2024, 1, 1);

        let employee1 = employee_with_statuses(1, vec![status(500, None, 10)]);
        let employee2 = employee_with_statuses(2, vec![status(500, None, 4)]);

        let data1 = EmployeeData::new(employee1, ScheduleCalcDataSet::default(), 0);
        let data2 = EmployeeData::new(employee2, ScheduleCalcDataSet::default(), 0);

        let result = JobRankComparator.compare(&job, None, shift_date, &data1, &data2);

        assert_eq!(result, Ordering::Greater);
    }
}
