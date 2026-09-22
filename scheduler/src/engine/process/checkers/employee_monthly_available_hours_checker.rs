//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeMonthlyAvailableHoursChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeMonthlyAvailableHoursChecker.java`. `getHoursAvailable` reaches into
//! `ContractCalculatorProvider`/`PropertyDataDAO`/`Employee.getFirstHomeEmployeeJobStatusForPeriod`
//! contract-hours machinery this crate hasn't ported — resolved through
//! [`MonthlyContractHoursAvailablePort`], same "stub the deferred half, port the rest for real"
//! treatment as `EmployeeOvertimeChecker`. `getScheduledHoursForWeek` (despite the name, a
//! per-`DateRange` sum — Java reuses this helper name across the monthly and weekly checkers for
//! different periods) **is** ported for real: `ScheduleCalcDataSet.shifts_for_date_range` plus
//! `EmployeeShift.net_hours` are both grounded now (`PARITY_AUDIT.md` finding 12).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::schedule_mode::ScheduleMode;
use date_range_rs::DateRange;

/// `getHoursAvailable(ScheduleModel, EmployeeData, EmployeeShift)`.
pub trait MonthlyContractHoursAvailablePort {
    fn hours_available(
        &self,
        schedule_model: &ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> f64;
}

/// `EmployeeMonthlyAvailableHoursChecker`.
pub struct EmployeeMonthlyAvailableHoursChecker<'a> {
    contract_hours: &'a dyn MonthlyContractHoursAvailablePort,
}

impl<'a> EmployeeMonthlyAvailableHoursChecker<'a> {
    pub fn new(contract_hours: &'a dyn MonthlyContractHoursAvailablePort) -> Self {
        Self { contract_hours }
    }

    fn scheduled_hours_for_week(employee_data: &mut EmployeeData, date_range: &DateRange) -> f64 {
        employee_data
            .data_set()
            .shifts_for_date_range(date_range)
            .iter()
            .map(EmployeeShift::net_hours)
            .sum()
    }
}

impl CanWorkChecker for EmployeeMonthlyAvailableHoursChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        if schedule_model.property().schedule_mode() != Some(ScheduleMode::Monthly) {
            return true;
        }

        let hours_available =
            self.contract_hours
                .hours_available(schedule_model, employee_data, employee_shift);
        let hours_scheduled =
            Self::scheduled_hours_for_week(employee_data, schedule_model.date_range());

        hours_scheduled <= hours_available
    }
}
