//! Port of `com.unifocus.watson.server.scheduler.engine.model.RegularSchedules`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! RegularSchedules.java`. Wraps every weekly recurring work period whose employee is in this
//! run's `EmployeeList`; see `DATA_MODEL.md` §4.
//!
//! `regular_schedule_job_matches` treats an unresolved job (`RegularSchedule::job` returning
//! `None`) as **not matching**, not a panic — Java's `regularScheduleJob.getID()` would NPE on a
//! null job here, a divergence worth flagging rather than translating literally (same treatment as
//! `PARITY_AUDIT.md` finding 19).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::job_data::JobData;
use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::{
    EmployeeActiveOnDatePort, EmployeeJobStatusChecker,
};
use crate::engine::process::regularschedules::employee_regular_period_comparator::EmployeeRegularPeriodComparator;
use crate::entity::employee_regular_period::EmployeeRegularPeriod;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `RegularSchedules`.
#[derive(Debug, Clone, PartialEq)]
pub struct RegularSchedules {
    regular_schedules: Vec<RegularSchedule>,
}

impl RegularSchedules {
    /// `RegularSchedules(ScheduleModel, List<EmployeeRegularPeriod>)` — skips periods whose
    /// employee has no `EmployeeData` in this run.
    pub fn new(
        schedule_model: &ScheduleModel,
        employee_regular_periods: Vec<EmployeeRegularPeriod>,
    ) -> Self {
        let mut regular_schedules = Vec::new();

        if let Some(employee_list) = schedule_model.employee_list() {
            for employee_regular_period in employee_regular_periods {
                if employee_list
                    .employee_data(employee_regular_period.employee_id())
                    .is_some()
                {
                    regular_schedules.push(RegularSchedule::new(
                        employee_regular_period.employee_id(),
                        &employee_regular_period,
                    ));
                }
            }
        }

        Self { regular_schedules }
    }

    /// `getRegularSchedulesForJobAndDate(JobData, LocalDate)`.
    pub fn regular_schedules_for_job_and_date(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
        shift_date: LocalDate,
        active_on_date: &dyn EmployeeActiveOnDatePort,
    ) -> Vec<RegularSchedule> {
        let job_status_checker = EmployeeJobStatusChecker::new(active_on_date);
        let job_id = job_data.job().id();

        let mut matching: Vec<RegularSchedule> = self
            .regular_schedules
            .iter()
            .copied()
            .filter(|regular_schedule| {
                Self::regular_schedule_matches(
                    schedule_model,
                    regular_schedule,
                    job_id,
                    shift_date,
                    &job_status_checker,
                )
            })
            .collect();

        let comparator = EmployeeRegularPeriodComparator::new(job_data, shift_date);
        matching.sort_by(|a, b| Self::compare(schedule_model, &comparator, a, b));

        matching
    }

    fn regular_schedule_matches(
        schedule_model: &ScheduleModel,
        regular_schedule: &RegularSchedule,
        job_id: i32,
        shift_date: LocalDate,
        job_status_checker: &EmployeeJobStatusChecker,
    ) -> bool {
        if regular_schedule.day_of_week() != shift_date.day_of_week() {
            return false;
        }

        let Some(employee) = schedule_model
            .employee_list()
            .and_then(|employee_list| employee_list.employee_data(regular_schedule.employee_id()))
            .map(EmployeeData::employee)
        else {
            return false;
        };

        if regular_schedule.job(shift_date, employee) != Some(job_id) {
            return false;
        }

        job_status_checker.can_employee_work_job_on_date(employee, Some(job_id), shift_date)
    }

    fn compare(
        schedule_model: &ScheduleModel,
        comparator: &EmployeeRegularPeriodComparator,
        a: &RegularSchedule,
        b: &RegularSchedule,
    ) -> Ordering {
        let Some(employee_list) = schedule_model.employee_list() else {
            return Ordering::Equal;
        };

        match (
            employee_list.employee_data(a.employee_id()),
            employee_list.employee_data(b.employee_id()),
        ) {
            (Some(employee_data1), Some(employee_data2)) => {
                comparator.compare(employee_data1, employee_data2)
            }
            _ => Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::jc_sort_order_type::JcSortOrderType;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalTime};

    struct AlwaysActive;
    impl EmployeeActiveOnDatePort for AlwaysActive {
        fn is_active_on_date(&self, _employee: &Employee, _date: LocalDate) -> bool {
            true
        }
    }

    fn job(id: i32, sort_order: Vec<AssignmentSortOrder>) -> Assignment {
        Assignment::new(
            id,
            format!("Job {id}"),
            false,
            None,
            None,
            None,
            false,
            sort_order,
            Vec::new(),
            None,
        )
    }

    fn job_status(job_id: i32) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            job_id,
            None,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 7),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            40.0,
            false,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn employee(id: i32, hire_date: LocalDate, job_statuses: Vec<EmployeeJobStatus>) -> Employee {
        Employee::new(
            id,
            format!("Employee {id}"),
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            Some(hire_date),
            Vec::new(),
            job_statuses,
        )
    }

    fn period(
        employee_id: i32,
        job_id: Option<i32>,
        day_of_week: DayOfWeek,
    ) -> EmployeeRegularPeriod {
        EmployeeRegularPeriod::new(
            employee_id,
            job_id,
            None,
            day_of_week,
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
            8.0,
        )
    }

    fn model_with(employees: Vec<Employee>, job_data: JobData) -> ScheduleModel {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let mut model = ScheduleModel::new(property, range);

        let mut job_list = JobList::new();
        job_list.add_job_data(job_data);
        model.set_job_list(job_list);

        let mut employee_list = EmployeeList::new();
        for employee in employees {
            employee_list.add_employee_data(EmployeeData::new(
                employee,
                ScheduleCalcDataSet::default(),
                0,
            ));
        }
        model.set_employee_list(employee_list);

        model
    }

    #[test]
    fn excludes_a_regular_schedule_whose_day_of_week_does_not_match_the_shift_date() {
        let model = model_with(
            vec![employee(1, LocalDate::of(2020, 1, 1), vec![job_status(1)])],
            JobData::new(job(1, Vec::new())),
        );
        let regular_schedules =
            RegularSchedules::new(&model, vec![period(1, Some(1), DayOfWeek::Tuesday)]);

        // 2024-01-01 is a Monday; the period is Tuesday-only.
        let matches = regular_schedules.regular_schedules_for_job_and_date(
            &model,
            model.job_list().unwrap().job_data(1).unwrap(),
            LocalDate::of(2024, 1, 1),
            &AlwaysActive,
        );

        assert!(matches.is_empty());
    }

    #[test]
    fn excludes_a_regular_schedule_for_a_different_job_explicit_or_via_home_status_fallback() {
        let model = model_with(
            vec![
                employee(1, LocalDate::of(2020, 1, 1), vec![job_status(1)]),
                employee(2, LocalDate::of(2020, 1, 1), vec![job_status(2)]),
            ],
            JobData::new(job(1, Vec::new())),
        );
        let regular_schedules = RegularSchedules::new(
            &model,
            vec![
                period(1, Some(2), DayOfWeek::Monday), // explicit job 2, doesn't match job 1
                period(2, None, DayOfWeek::Monday),    // falls back to employee 2's home job (2)
            ],
        );

        let matches = regular_schedules.regular_schedules_for_job_and_date(
            &model,
            model.job_list().unwrap().job_data(1).unwrap(),
            LocalDate::of(2024, 1, 1),
            &AlwaysActive,
        );

        assert!(matches.is_empty());
    }

    #[test]
    fn excludes_a_regular_schedule_the_employee_job_status_checker_rejects() {
        // Employee has no EmployeeJobStatus for job 1 at all, so
        // `EmployeeJobStatusChecker::can_employee_work_job_on_date` returns false.
        let model = model_with(
            vec![employee(1, LocalDate::of(2020, 1, 1), Vec::new())],
            JobData::new(job(1, Vec::new())),
        );
        let regular_schedules =
            RegularSchedules::new(&model, vec![period(1, Some(1), DayOfWeek::Monday)]);

        let matches = regular_schedules.regular_schedules_for_job_and_date(
            &model,
            model.job_list().unwrap().job_data(1).unwrap(),
            LocalDate::of(2024, 1, 1),
            &AlwaysActive,
        );

        assert!(matches.is_empty());
    }

    #[test]
    fn sorts_matches_by_the_jobs_seniority_chain() {
        let sort_order = vec![AssignmentSortOrder::new(JcSortOrderType::HireDate)];
        let model = model_with(
            vec![
                employee(1, LocalDate::of(2022, 1, 1), vec![job_status(1)]),
                employee(2, LocalDate::of(2020, 1, 1), vec![job_status(1)]),
            ],
            JobData::new(job(1, sort_order)),
        );
        let regular_schedules = RegularSchedules::new(
            &model,
            vec![
                period(1, Some(1), DayOfWeek::Monday),
                period(2, Some(1), DayOfWeek::Monday),
            ],
        );

        let matches = regular_schedules.regular_schedules_for_job_and_date(
            &model,
            model.job_list().unwrap().job_data(1).unwrap(),
            LocalDate::of(2024, 1, 1),
            &AlwaysActive,
        );

        let ids: Vec<i32> = matches.iter().map(RegularSchedule::employee_id).collect();
        // Employee 2 hired earlier (2020) than employee 1 (2022) — more senior, sorts first.
        assert_eq!(ids, vec![2, 1]);
    }
}
