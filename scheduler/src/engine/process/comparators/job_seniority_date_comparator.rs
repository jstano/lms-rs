//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! JobSeniorityDateComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! JobSeniorityDateComparator.java`. Same shape as `JobRankComparator` — see its doc for the
//! `.expect()`/`Option`-equality notes, which apply identically here.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use crate::entity::employee::Employee;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `JobSeniorityDateComparator`.
pub struct JobSeniorityDateComparator;

impl SeniorityComparator for JobSeniorityDateComparator {
    fn compare(
        &self,
        job: &Assignment,
        _assignment: Option<&Assignment>,
        shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        let job_seniority_date = |employee_data: &EmployeeData| -> LocalDate {
            if job.is_departmental_seniority() {
                Self::department_seniority_date(employee_data.employee(), job, shift_date)
                    .expect("employee has no job status in the job's department")
            } else {
                employee_data
                    .employee()
                    .employee_job_status(job.id(), shift_date)
                    .expect("employee has no job status for the job being ranked")
                    .seniority_date()
            }
        };

        job_seniority_date(employee_data1).cmp(&job_seniority_date(employee_data2))
    }
}

impl JobSeniorityDateComparator {
    fn department_seniority_date(
        employee: &Employee,
        job: &Assignment,
        shift_date: LocalDate,
    ) -> Option<LocalDate> {
        let department_id = job.parent_assignment_id();

        let mut best_seniority_date: Option<LocalDate> = None;

        for status in employee.active_employee_job_statuses_for_date(shift_date) {
            if status.job_parent_assignment_id() == department_id
                && best_seniority_date.is_none_or(|best| status.seniority_date().is_before(best))
            {
                best_seniority_date = Some(status.seniority_date());
            }
        }

        best_seniority_date
    }
}
