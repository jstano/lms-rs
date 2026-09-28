//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.filters.ContractFTSecondaryJobEmployeeFilter`.
//!
//! Ground truth: `taps/.../process/variable/filters/ContractFTSecondaryJobEmployeeFilter.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::filters::employee_filter::{
    EmployeeFilter, EmployeeFilterKey,
};
use crate::entity::employee_pay_type::EmployeePayType;
use crate::entity::planned_shift::PlannedShift;

const FULL_TIME_CONTRACT_HOURS_PER_WEEK: f64 = 39.0;

/// `ContractFTSecondaryJobEmployeeFilter`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContractFtSecondaryJobEmployeeFilter;

impl EmployeeFilter for ContractFtSecondaryJobEmployeeFilter {
    fn name(&self) -> &str {
        "Contract FT, Secondary Job"
    }

    fn key(&self) -> EmployeeFilterKey {
        EmployeeFilterKey::ContractFtSecondaryJob
    }

    fn include_employee(
        &self,
        _schedule_model: &ScheduleModel,
        employee_data: &EmployeeData,
        planned_shift: &PlannedShift,
    ) -> bool {
        let Some(status) = employee_data
            .employee()
            .employee_job_status(planned_shift.job_id(), planned_shift.shift_date())
        else {
            return false;
        };

        if status.is_sub_only() {
            return false;
        }

        if status.is_home() {
            return false;
        }

        status.pay_type() == Some(EmployeePayType::Contract)
            && status.contract_hours() >= FULL_TIME_CONTRACT_HOURS_PER_WEEK
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

    fn status(
        is_home: bool,
        is_sub_only: bool,
        pay_type: EmployeePayType,
        hours: f64,
    ) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            500,
            None,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            is_home,
            1,
            LocalDate::of(2020, 1, 1),
            hours,
            is_sub_only,
            0,
        )
        .with_pay_type(pay_type)
    }

    fn employee_data(status: EmployeeJobStatus) -> EmployeeData {
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
        EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0)
    }

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
    fn includes_secondary_full_time_contract_employee() {
        let data = employee_data(status(false, false, EmployeePayType::Contract, 40.0));

        assert!(ContractFtSecondaryJobEmployeeFilter.include_employee(&model(), &data, &shift()));
    }

    #[test]
    fn excludes_home() {
        let data = employee_data(status(true, false, EmployeePayType::Contract, 40.0));

        assert!(!ContractFtSecondaryJobEmployeeFilter.include_employee(&model(), &data, &shift()));
    }

    #[test]
    fn excludes_below_full_time_hours() {
        let data = employee_data(status(false, false, EmployeePayType::Contract, 30.0));

        assert!(!ContractFtSecondaryJobEmployeeFilter.include_employee(&model(), &data, &shift()));
    }
}
