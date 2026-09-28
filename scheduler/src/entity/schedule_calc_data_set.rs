//! Partial port of `com.unifocus.watson.server.labor.calcshift.ScheduleCalcDataSet`.
//!
//! **Still a growing stub, not the full class.** `getShifts`/`getTimeOffRequests`/
//! `getAvailability`/`addShift`/`removeShift`/`getShiftsForDateRange` are modeled —
//! `getShiftsForDateRange` is a plain filter over `getShifts()` by date, which is all it can be
//! for a dataset that (so far) has no backing query of its own. `getTotalAccrualHours` is a real
//! calculation (rolling accrual totals) this crate hasn't grounded yet — see `DATA_MODEL.md` §7 —
//! and is deliberately **not** added as a fake pass-through method.
//!
//! `employee`/`dataset_start_date`/`calculation_mode` were added for `EmployeeListLoader`
//! (Phase 2 step 1) — the first real caller of `getEmployee`/`setDatasetStartDate`/
//! `setCalculationMode`. `employee` stays `Option` rather than required at construction: Java's
//! no-arg constructor leaves it `null`, and nothing ported so far needs it set eagerly.
//!
//! `date_range`/`calculation_start_date`/`pending_time_off_requests` and the `TimeCard`-interface
//! default methods (`is_open_for_editing_for`, `has_overtime`, `total_premium_hours`,
//! `net_hours_for_work_week`, `overtime_for_date_range`, `distribution_hours_for_date_range`) were
//! added for `ScheduleChecker`/`ScheduleMatcher` (Phase 3's `autosched` wave, their first real
//! callers) — see `taps/.../timecard/TimeCard.java`'s default methods. Unlike Java, where these
//! recompute from `EmployeeShift.getHoursDistributions()` via a property-config-driven "is this
//! distribution type regular or premium" lookup (`TimeCard.getRegularHoursDistributionTypeIds()`,
//! a real external classification this crate doesn't model), every ported call here sums
//! `HoursDistribution::is_premium` directly — `is_premium` is itself the pre-resolved stand-in for
//! that lookup (see `entity::hours_distribution`'s doc). `calculation_start_date` is a genuinely
//! different field from `dataset_start_date` (`datasetStartDate` = "earliest date for data
//! loaded", `calculationStartDate` = "earliest date for data that will be modified" — two
//! independently-set Java fields, not a derived pair).

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
    pending_time_off_requests: Vec<EmployeeTimeOff>,
    availability: Availability,
    employee: Option<Employee>,
    dataset_start_date: Option<LocalDate>,
    calculation_start_date: Option<LocalDate>,
    calculation_mode: Option<EmployeeCalculationMode>,
    date_range: Option<DateRange>,
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
            pending_time_off_requests: Vec::new(),
            availability,
            employee: None,
            dataset_start_date: None,
            calculation_start_date: None,
            calculation_mode: None,
            date_range: None,
        }
    }

    /// `setPendingTimeOffRequests(List<EmployeeTimeOff>)`.
    #[must_use]
    pub fn with_pending_time_off_requests(
        mut self,
        pending_time_off_requests: Vec<EmployeeTimeOff>,
    ) -> Self {
        self.pending_time_off_requests = pending_time_off_requests;
        self
    }

    /// The real constructor's `dateRange` parameter, which also defaults
    /// `calculationStartDate` to `dateRange.getStartDate()` (only in the constructor — Java has
    /// no `setDateRange`). [`set_calculation_start_date`](Self::set_calculation_start_date)
    /// overrides that default afterward, same as `setCalculationStartDate` does in Java.
    #[must_use]
    pub fn with_date_range(mut self, date_range: DateRange) -> Self {
        self.calculation_start_date
            .get_or_insert(date_range.start_date());
        self.date_range = Some(date_range);
        self
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

    /// `getPendingTimeOffRequests()`.
    pub fn pending_time_off_requests(&self) -> &[EmployeeTimeOff] {
        &self.pending_time_off_requests
    }

    /// `getDateRange()`.
    pub fn date_range(&self) -> Option<&DateRange> {
        self.date_range.as_ref()
    }

    /// `getCalculationStartDate()`.
    pub fn calculation_start_date(&self) -> Option<LocalDate> {
        self.calculation_start_date
    }

    /// `setCalculationStartDate(LocalDate)`.
    pub fn set_calculation_start_date(&mut self, calculation_start_date: LocalDate) {
        self.calculation_start_date = Some(calculation_start_date);
    }

    /// `isOpenForEditingOn(LocalDate)` — `date.isOnOrAfter(calculationStartDate)`.
    pub fn is_open_for_editing_on(&self, date: LocalDate) -> bool {
        self.calculation_start_date
            .is_none_or(|start| date.is_on_or_after(start))
    }

    /// `isOpenForEditingFor(EmployeeShift)`.
    pub fn is_open_for_editing_for(&self, employee_shift: &EmployeeShift) -> bool {
        self.is_open_for_editing_on(employee_shift.shift_date())
    }

    /// `TimeCard.getHoursDistributionsStream(DateRange)`.
    fn hours_distributions_for_period(
        &self,
        period: DateRange,
    ) -> impl Iterator<Item = crate::entity::hours_distribution::HoursDistribution> + '_ {
        self.shifts
            .iter()
            .flat_map(|shift| shift.hours_distributions().iter().copied())
            .filter(move |distribution| period.contains_date(distribution.date()))
    }

    /// `getDistributionHoursForDateRange(DateRange)`.
    pub fn distribution_hours_for_date_range(&self, period: DateRange) -> f64 {
        self.hours_distributions_for_period(period)
            .map(|d| d.hours())
            .sum()
    }

    /// `getDistributionHoursForDateRange()` — the no-arg overload, over the dataset's own
    /// `dateRange`. Returns `0.0` if `date_range` was never set (Java would NPE).
    pub fn total_distribution_hours(&self) -> f64 {
        match self.date_range {
            Some(date_range) => self.distribution_hours_for_date_range(date_range),
            None => 0.0,
        }
    }

    /// `getNetHoursForWorkWeek(DateRange)` — `TimeCard.getNetHours(DateRange)` under the hood.
    pub fn net_hours_for_work_week(&self, work_week: DateRange) -> f64 {
        self.distribution_hours_for_date_range(work_week)
    }

    /// `hasOvertime()`.
    pub fn has_overtime(&self) -> bool {
        match self.date_range {
            Some(date_range) => self
                .hours_distributions_for_period(date_range)
                .any(|d| d.is_premium()),
            None => false,
        }
    }

    /// `getOvertimeForDateRange(DateRange)`.
    pub fn overtime_for_date_range(&self, period: DateRange) -> f64 {
        self.hours_distributions_for_period(period)
            .filter(|d| d.is_premium())
            .map(|d| d.hours())
            .sum()
    }

    /// `getTotalPremiumHours()` — the no-arg overload, over the dataset's own `dateRange`.
    pub fn total_premium_hours(&self) -> f64 {
        match self.date_range {
            Some(date_range) => self.overtime_for_date_range(date_range),
            None => 0.0,
        }
    }

    /// `getShiftsForDateRange(DateRange)`.
    pub fn shifts_for_date_range(&self, date_range: &DateRange) -> Vec<EmployeeShift> {
        self.shifts
            .iter()
            .filter(|s| date_range.contains_date(s.shift_date()))
            .cloned()
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
