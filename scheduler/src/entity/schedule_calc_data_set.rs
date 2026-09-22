//! Partial port of `com.unifocus.watson.server.labor.calcshift.ScheduleCalcDataSet`.
//!
//! **Still a growing stub, not the full class.** `getShifts`/`getTimeOffRequests`/
//! `getAvailability`/`addShift`/`removeShift`/`getShiftsForDateRange` are modeled —
//! `getShiftsForDateRange` is a plain filter over `getShifts()` by date, which is all it can be
//! for a dataset that (so far) has no backing query of its own. `getOvertimeForDateRange` and
//! `getTotalAccrualHours` are real calculations (rolling overtime, accrual totals) this crate
//! hasn't grounded yet — see `DATA_MODEL.md` §7 — and are deliberately **not** added as fake
//! pass-through methods; `EmployeeData.store_pre_schedule_check_overtime` and
//! `EmployeeMonthlyAvailableHoursChecker`/`EmployeeOvertimeChecker`'s hours-available half stay
//! unported until those land.
//!
//! `employee`/`dataset_start_date`/`calculation_mode` were added for `EmployeeListLoader`
//! (Phase 2 step 1) — the first real caller of `getEmployee`/`setDatasetStartDate`/
//! `setCalculationMode`. `employee` stays `Option` rather than required at construction: Java's
//! no-arg constructor leaves it `null`, and nothing ported so far needs it set eagerly.

use crate::entity::availability::Availability;
use crate::entity::employee::Employee;
use crate::entity::employee_calculation_mode::EmployeeCalculationMode;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_time_off::EmployeeTimeOff;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// `ScheduleCalcDataSet`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScheduleCalcDataSet {
    shifts: Vec<EmployeeShift>,
    time_off_requests: Vec<EmployeeTimeOff>,
    availability: Availability,
    employee: Option<Employee>,
    dataset_start_date: Option<LocalDate>,
    calculation_mode: Option<EmployeeCalculationMode>,
}

impl ScheduleCalcDataSet {
    pub fn new(
        shifts: Vec<EmployeeShift>,
        time_off_requests: Vec<EmployeeTimeOff>,
        availability: Availability,
    ) -> Self {
        Self {
            shifts,
            time_off_requests,
            availability,
            employee: None,
            dataset_start_date: None,
            calculation_mode: None,
        }
    }

    /// `getEmployee()`.
    pub fn employee(&self) -> Option<&Employee> {
        self.employee.as_ref()
    }

    /// `setEmployee(Employee)`.
    pub fn set_employee(&mut self, employee: Employee) {
        self.employee = Some(employee);
    }

    /// `getDatasetStartDate()`.
    pub fn dataset_start_date(&self) -> Option<LocalDate> {
        self.dataset_start_date
    }

    /// `setDatasetStartDate(LocalDate)`.
    pub fn set_dataset_start_date(&mut self, dataset_start_date: LocalDate) {
        self.dataset_start_date = Some(dataset_start_date);
    }

    /// `getCalculationMode()`.
    pub fn calculation_mode(&self) -> Option<EmployeeCalculationMode> {
        self.calculation_mode
    }

    /// `setCalculationMode(EmployeeCalculationMode)`.
    pub fn set_calculation_mode(&mut self, calculation_mode: EmployeeCalculationMode) {
        self.calculation_mode = Some(calculation_mode);
    }

    /// `getShifts()`.
    pub fn shifts(&self) -> &[EmployeeShift] {
        &self.shifts
    }

    /// `getShifts()` (mutable — `SchedulePreparationService` replaces the whole list after
    /// filtering it, rather than calling `add_shift`/`remove_shift` per element).
    pub fn shifts_mut(&mut self) -> &mut Vec<EmployeeShift> {
        &mut self.shifts
    }

    /// `getTimeOffRequests()`.
    pub fn time_off_requests(&self) -> &[EmployeeTimeOff] {
        &self.time_off_requests
    }

    /// `getShiftsForDateRange(DateRange)`.
    pub fn shifts_for_date_range(&self, date_range: &DateRange) -> Vec<EmployeeShift> {
        self.shifts
            .iter()
            .copied()
            .filter(|s| date_range.contains_date(s.shift_date()))
            .collect()
    }

    /// `getAvailability()`.
    pub fn availability(&self) -> &Availability {
        &self.availability
    }

    /// `addShift(EmployeeShift)`.
    pub fn add_shift(&mut self, shift: EmployeeShift) {
        self.shifts.push(shift);
    }

    /// `removeShift(EmployeeShift)`.
    pub fn remove_shift(&mut self, shift: EmployeeShift) {
        if let Some(index) = self.shifts.iter().position(|s| *s == shift) {
            self.shifts.remove(index);
        }
    }
}
