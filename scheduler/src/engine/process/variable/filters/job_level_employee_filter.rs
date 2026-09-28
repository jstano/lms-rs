//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.filters.JobLevelEmployeeFilter`.
//!
//! Ground truth: `taps/.../process/variable/filters/JobLevelEmployeeFilter.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_checker::EmployeeJobChecker;
use crate::engine::process::variable::filters::employee_filter::{
    EmployeeFilter, EmployeeFilterKey,
};
use crate::entity::planned_shift::PlannedShift;

/// `JobLevelEmployeeFilter`.
#[derive(Debug, Clone, Copy)]
pub struct JobLevelEmployeeFilter {
    job_level: i32,
}

impl JobLevelEmployeeFilter {
    /// `JobLevelEmployeeFilter(int)`.
    pub fn new(job_level: i32) -> Self {
        Self { job_level }
    }
}

impl EmployeeFilter for JobLevelEmployeeFilter {
    fn name(&self) -> &str {
        "Default"
    }

    fn key(&self) -> EmployeeFilterKey {
        EmployeeFilterKey::JobLevel(self.job_level)
    }

    fn include_employee(
        &self,
        _schedule_model: &ScheduleModel,
        employee_data: &EmployeeData,
        planned_shift: &PlannedShift,
    ) -> bool {
        EmployeeJobChecker::employee_has_job(
            employee_data,
            planned_shift.job_id(),
            *planned_shift,
            self.job_level,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn shift() -> PlannedShift {
        PlannedShift::new(
            1,
            500,
            LocalDate::of(2024, 1, 1),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            8.0,
            None,
        )
    }

    fn model() -> ScheduleModel {
        let range =
            date_range_rs::DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 1));
        let property = crate::entity::property::Property::new(1, range);
        ScheduleModel::new(property, range)
    }

    #[test]
    fn includes_employee_with_job_at_any_level_when_level_is_negative() {
        let status = EmployeeJobStatus::new(
            500,
            None,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            0.0,
            false,
            0,
        );
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            vec![status],
        );
        let data = EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0);

        assert!(JobLevelEmployeeFilter::new(-1).include_employee(&model(), &data, &shift()));
    }

    #[test]
    fn excludes_employee_without_the_job() {
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            vec![],
        );
        let data = EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0);

        assert!(!JobLevelEmployeeFilter::new(-1).include_employee(&model(), &data, &shift()));
    }
}
