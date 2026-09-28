//! Port of `com.unifocus.watson.server.scheduler.engine.misc.CalculateDataSet`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! CalculateDataSet.java`. `distributeHoursForDataSet`/`adjustShiftForLunch` reach into
//! `SchedulesTimeCardCalculator`/`ScheduleLunchRunner`, both deferred external subsystems (see
//! `engine::misc::ports`'s doc). `distributeHoursForShift` sets `autoOTHours`/`autoDTHours` to
//! `0` on the shift — not modeled, since nothing ported reads either field back (same treatment
//! as `entity::employee_shift::EmployeeShift`'s other unmodeled constant-valued fields); nothing
//! in this crate calls `distributeHoursForShift` yet either (`PreScheduleProcess`, its first
//! caller wave, doesn't reach it — only `storeOvertimeAddShiftAndCalculate`/
//! `removeShiftAndCalculate`), so it's left unported until a real caller appears.
//!
//! `storeOvertimeAddShiftAndCalculate` needs an extra dependency Java's `CalculateDataSet` class
//! doesn't explicitly have: `employeeData.storePreScheduleCheckOvertime(dateRange)` calls
//! `dataSet.getOvertimeForDateRange` directly on the entity in Java, with no DI boundary visible
//! from `CalculateDataSet` itself. This crate models that calculation as an external port
//! (`OvertimeForDateRangePort`, `PARITY_AUDIT.md` finding 16) instead of inline entity logic, so
//! `CalculateDataSet` has to be handed the port explicitly to pass through.
//!
//! **Reordering trap, same shape as finding 29**: Java's `storeOvertimeAddShiftAndCalculate` adds
//! `employeeShift` to the data set *before* `adjustShiftForLunch` mutates it — harmless there
//! because `addEmployeeShift`/`adjustShiftForLunch` share the same object reference, so the
//! stored copy sees the later mutation too. `EmployeeShift` is a `Copy` value type here, so the
//! add has to happen *after* the lunch adjustment (taking `&mut EmployeeShift` so the caller's
//! own copy — later passed to `EmployeeTimeOffChecker`/`ScheduleSaver`/`removeShiftAndCalculate`
//! — reflects it too), not translated in Java's literal statement order.

use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::ports::OvertimeForDateRangePort;
use crate::entity::employee_shift::EmployeeShift;

/// `CalculateDataSet`.
pub struct CalculateDataSet<'a> {
    time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
    lunch_runner: &'a dyn ScheduleLunchRunnerPort,
    overtime: &'a dyn OvertimeForDateRangePort,
}

impl<'a> CalculateDataSet<'a> {
    pub fn new(
        time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
        lunch_runner: &'a dyn ScheduleLunchRunnerPort,
        overtime: &'a dyn OvertimeForDateRangePort,
    ) -> Self {
        Self {
            time_card_calculator,
            lunch_runner,
            overtime,
        }
    }

    /// `distributeHoursForDataSet(EmployeeData)`.
    pub fn distribute_hours_for_data_set(&self, employee_data: &mut EmployeeData) {
        self.time_card_calculator
            .calculate_overtime_for_schedule_calc_data_set(employee_data.data_set_mut());
    }

    /// `adjustShiftForLunch(EmployeeData, EmployeeShift)`.
    pub fn adjust_shift_for_lunch(
        &self,
        employee_data: &mut EmployeeData,
        employee_shift: &mut EmployeeShift,
    ) {
        self.lunch_runner
            .run_rules(employee_data.data_set_mut(), employee_shift);
    }

    /// `storeOvertimeAddShiftAndCalculate(ScheduleModel, EmployeeData, EmployeeShift)` — see the
    /// module doc's reordering note for why the lunch adjustment happens before the add.
    pub fn store_overtime_add_shift_and_calculate(
        &self,
        schedule_model: &ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &mut EmployeeShift,
    ) {
        employee_data.store_pre_schedule_check_overtime(schedule_model.date_range(), self.overtime);

        self.adjust_shift_for_lunch(employee_data, employee_shift);
        employee_data.add_employee_shift(employee_shift.clone());

        self.distribute_hours_for_data_set(employee_data);
    }

    /// `removeShiftAndCalculate(EmployeeData, EmployeeShift)`.
    pub fn remove_shift_and_calculate(
        &self,
        employee_data: &mut EmployeeData,
        employee_shift: EmployeeShift,
    ) {
        employee_data.remove_employee_shift(employee_shift);
        self.distribute_hours_for_data_set(employee_data);
    }
}
