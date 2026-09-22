//! Port of `com.unifocus.watson.server.scheduler.engine.model.JobData`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/JobData.java`.
//! Per-job scheduling state for one pipeline run; see `DATA_MODEL.md` §4.
//!
//! `average_shift_length` is computed on demand here rather than cached on a mutable field —
//! Java's `<= 0.0` sentinel on a lazily-populated field is a memoized pure function, not a real
//! default; see `PARITY_AUDIT.md` finding 4.

use crate::engine::model::hours_by_date::HoursByDate;
use crate::engine::model::pre_schedule_parameters::PreScheduleParameters;
use crate::entity::assignment::Assignment;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `JobData`.
#[derive(Debug, Clone, PartialEq)]
pub struct JobData {
    job: Assignment,
    projected_hours: HoursByDate,
    scheduled_hours: HoursByDate,
    planned_shifts: Vec<PlannedShift>,
    balance_factor: f64,
    balance_level: i32,
    pre_schedule_parameters: Option<PreScheduleParameters>,
}

impl JobData {
    pub fn new(job: Assignment) -> Self {
        Self {
            job,
            projected_hours: HoursByDate::new(),
            scheduled_hours: HoursByDate::new(),
            planned_shifts: Vec::new(),
            balance_factor: 1.0,
            balance_level: 0,
            pre_schedule_parameters: None,
        }
    }

    /// `getJob()`.
    pub fn job(&self) -> &Assignment {
        &self.job
    }

    /// `getProjectedHours()`.
    pub fn projected_hours(&self) -> &HoursByDate {
        &self.projected_hours
    }

    /// `getProjectedHours()` (mutable — Java returns the live, mutable `HoursByDate`).
    pub fn projected_hours_mut(&mut self) -> &mut HoursByDate {
        &mut self.projected_hours
    }

    /// `getScheduledHours()`.
    pub fn scheduled_hours(&self) -> &HoursByDate {
        &self.scheduled_hours
    }

    /// `getScheduledHours()` (mutable).
    pub fn scheduled_hours_mut(&mut self) -> &mut HoursByDate {
        &mut self.scheduled_hours
    }

    /// `getPlannedShifts()`.
    pub fn planned_shifts(&self) -> &[PlannedShift] {
        &self.planned_shifts
    }

    /// `getPlannedShifts()` (mutable — Java's list is mutated directly by callers).
    pub fn planned_shifts_mut(&mut self) -> &mut Vec<PlannedShift> {
        &mut self.planned_shifts
    }

    /// `getPlannedShiftsForDate(LocalDate)` — filtered and sorted by start time.
    pub fn planned_shifts_for_date(&self, shift_date: LocalDate) -> Vec<PlannedShift> {
        let mut shifts: Vec<PlannedShift> = self
            .planned_shifts
            .iter()
            .copied()
            .filter(|s| s.shift_date() == shift_date)
            .collect();
        shifts.sort_by_key(|s| s.start_date_time());
        shifts
    }

    /// `removePlannedShift(PlannedShift)`.
    pub fn remove_planned_shift(&mut self, planned_shift: PlannedShift) {
        if let Some(index) = self.planned_shifts.iter().position(|s| *s == planned_shift) {
            self.planned_shifts.remove(index);
        }
    }

    /// `willScheduledHoursExceedProjectedHours(LocalDate, double)`.
    pub fn will_scheduled_hours_exceed_projected_hours(
        &self,
        date: LocalDate,
        additional_hours: f64,
    ) -> bool {
        self.scheduled_hours.hours_for_date(date) + additional_hours
            > self.projected_hours.hours_for_date(date)
    }

    /// `hasExceededProjectedHours(LocalDate)`.
    pub fn has_exceeded_projected_hours(&self, shift_date: LocalDate) -> bool {
        self.scheduled_hours.hours_for_date(shift_date)
            >= self.projected_hours.hours_for_date(shift_date)
    }

    /// `getAverageShiftLength()` — Java memoizes this on first call; ported as a plain
    /// computation instead (see module doc).
    pub fn average_shift_length(&self) -> f64 {
        if self.planned_shifts.is_empty() {
            return 0.0;
        }
        let total: f64 = self.planned_shifts.iter().map(PlannedShift::duration).sum();
        total / self.planned_shifts.len() as f64
    }

    /// `getBalanceLevel()`.
    pub fn balance_level(&self) -> i32 {
        self.balance_level
    }

    /// `incrementBalanceLevel()`.
    pub fn increment_balance_level(&mut self) {
        self.balance_level += 1;
    }

    /// `resetBalanceLevel()`.
    pub fn reset_balance_level(&mut self) {
        self.balance_level = 0;
    }

    /// `getBalanceFactor()`.
    pub fn balance_factor(&self) -> f64 {
        self.balance_factor
    }

    /// `setBalanceFactor(double)`.
    pub fn set_balance_factor(&mut self, balance_factor: f64) {
        self.balance_factor = balance_factor;
    }

    /// `getPreScheduleParameters()`.
    pub fn pre_schedule_parameters(&self) -> Option<PreScheduleParameters> {
        self.pre_schedule_parameters
    }

    /// `setPreScheduleParameters(PreScheduleParameters)`.
    pub fn set_pre_schedule_parameters(
        &mut self,
        pre_schedule_parameters: Option<PreScheduleParameters>,
    ) {
        self.pre_schedule_parameters = pre_schedule_parameters;
    }
}
