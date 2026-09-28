//! Port of `com.unifocus.watson.server.scheduler.autosched.ScheduleHoursDistributionValidator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/autosched/
//! ScheduleHoursDistributionValidator.java`. A pure function over `Employee`/`EmployeeShift`, both
//! already modeled — no deferred subsystem here. `validate(EmployeeShift)` becomes
//! `validate(shift, employee)`: Java calls `shift.getEmployee()` for a live back-reference this
//! crate's `EmployeeShift` doesn't carry (it stores `employee_id` only), so the caller passes the
//! `Employee` it already has instead.
//!
//! Returns `ShiftErrorType`s directly rather than looked-up `ResourceMgr` strings — see
//! `entity::shift_error_type::ShiftErrorType`'s doc.

use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::shift_error_type::ShiftErrorType;
use joda_rs::LocalDate;
use std::collections::HashSet;

/// `ScheduleHoursDistributionValidator.validate(EmployeeShift)`.
pub fn validate(shift: &EmployeeShift, employee: &Employee) -> HashSet<ShiftErrorType> {
    shift
        .hours_distribution_dates()
        .flat_map(|date| validate_hours_distribution(shift, employee, date))
        .collect()
}

fn validate_hours_distribution(
    shift: &EmployeeShift,
    employee: &Employee,
    date: LocalDate,
) -> Vec<ShiftErrorType> {
    let mut errors = Vec::new();

    if employee.home_employee_job_status(date).is_none() {
        errors.push(ShiftErrorType::NoHomeJob);
    }

    if employee.employee_job_status(shift.job_id(), date).is_none() {
        errors.push(ShiftErrorType::JobNotActive);
    }

    if !employee.is_active_on_date(date) {
        errors.push(ShiftErrorType::NotActive);
    }

    errors
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `ScheduleHoursDistributionValidatorTest` — ground truth not read directly
    //! (no test file was found under `taps/.../autosched/`); cases below cover the three checks
    //! `validateHoursDistribution` runs, independently derived from the production method.

    use super::*;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::employee_status_type::EmployeeStatusType;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::work_class::WorkClass;
    use joda_rs::LocalTime;

    fn job_status(job_id: i32, is_home: bool, date: LocalDate) -> EmployeeJobStatus {
        EmployeeJobStatus::new(job_id, None, date, date, is_home, 0, date, 0.0, false, 0)
    }

    fn active_employee(date: LocalDate, job_statuses: Vec<EmployeeJobStatus>) -> Employee {
        Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            job_statuses,
        )
        .with_status(vec![EmployeeStatus::new(
            date,
            date,
            EmployeeStatusType::Active,
        )])
    }

    fn shift_on(date: LocalDate, job_id: i32) -> EmployeeShift {
        EmployeeShift::new(1, date, date.at_time(LocalTime::of(9, 0, 0)), job_id, None)
            .with_hours_distributions(vec![HoursDistribution::new(date, 8.0, false)])
    }

    #[test]
    fn no_errors_when_employee_is_active_with_a_home_job_status_for_the_shifts_job() {
        let date = LocalDate::of(2024, 1, 1);
        let employee = active_employee(date, vec![job_status(1, true, date)]);
        let shift = shift_on(date, 1);

        assert!(validate(&shift, &employee).is_empty());
    }

    #[test]
    fn no_home_job_when_no_job_status_is_home() {
        let date = LocalDate::of(2024, 1, 1);
        let employee = active_employee(date, vec![job_status(1, false, date)]);
        let shift = shift_on(date, 1);

        let errors = validate(&shift, &employee);
        assert!(errors.contains(&ShiftErrorType::NoHomeJob));
    }

    #[test]
    fn job_not_active_when_no_job_status_matches_the_shifts_job() {
        let date = LocalDate::of(2024, 1, 1);
        let employee = active_employee(date, vec![job_status(1, true, date)]);
        let shift = shift_on(date, 2);

        let errors = validate(&shift, &employee);
        assert!(errors.contains(&ShiftErrorType::JobNotActive));
        assert!(!errors.contains(&ShiftErrorType::NoHomeJob));
    }

    #[test]
    fn not_active_when_employee_has_no_status_covering_the_date() {
        let date = LocalDate::of(2024, 1, 1);
        let other_date = LocalDate::of(2024, 2, 1);
        let mut employee = active_employee(date, vec![job_status(1, true, date)]);
        employee = employee.with_status(vec![EmployeeStatus::new(
            other_date,
            other_date,
            EmployeeStatusType::Active,
        )]);
        let shift = shift_on(date, 1);

        let errors = validate(&shift, &employee);
        assert!(errors.contains(&ShiftErrorType::NotActive));
    }
}
