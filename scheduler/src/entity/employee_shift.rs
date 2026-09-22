//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeShift`.
//!
//! Ground truth not re-read in full for this wave — the field slice matches `DATA_MODEL.md` §3,
//! carried over from Phase 0's read of `engine/model/{ShiftList,Schedules}.java`, plus
//! `end_date_time`/`net_hours` for `process/checkers/`. Employee/property back-refs, punches, and
//! distributions are still not modeled — every checker that would need them
//! (`ScheduleRestrictionRuleChecker`, `EmployeeCertificationsChecker`) reaches into a deferred
//! external subsystem anyway (see `PARITY_AUDIT.md` finding 10), so there's no ported call site
//! for them yet.
//!
//! `end_date_time`/`net_hours` are `with_*` builder fields defaulting to `start_date_time`/`0.0`
//! — see `entity::assignment::Assignment`'s doc for why. `planned_shift` was added for
//! `SchedulePreparationService` (Phase 2 step 5), its first read/write — carried as the full
//! `PlannedShift` value (it's a small `Copy` type already held by value elsewhere, e.g.
//! `JobData.planned_shifts`), not flattened to an id like `assignment_id`.
//!
//! `employee_id`/`flags`/`shift_category_id` were added for `EmployeeShiftCreator` (Phase 2 step
//! 6), its first writer — also `with_*` builder fields, defaulting to `0`/`0`/`None`.
//! `EmployeeShiftCreator.createShift` also sets punches (`addPunch`), `createdOnDT`, and
//! `shiftType`/`source` (fixed constants, same treatment as `PlannedShift`'s undocumented
//! constant fields) — none of them are modeled, since nothing ported reads any of them back yet.

use crate::entity::planned_shift::PlannedShift;
use date_range_rs::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime};

/// A scheduled or worked shift for an employee. `EmployeeShift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeShift {
    id: i32,
    shift_date: LocalDate,
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
    job_id: i32,
    assignment_id: Option<i32>,
    net_hours: f64,
    planned_shift: Option<PlannedShift>,
    employee_id: i32,
    flags: i32,
    shift_category_id: Option<i32>,
}

impl EmployeeShift {
    pub fn new(
        id: i32,
        shift_date: LocalDate,
        start_date_time: LocalDateTime,
        job_id: i32,
        assignment_id: Option<i32>,
    ) -> Self {
        Self {
            id,
            shift_date,
            start_date_time,
            end_date_time: start_date_time,
            job_id,
            assignment_id,
            net_hours: 0.0,
            planned_shift: None,
            employee_id: 0,
            flags: 0,
            shift_category_id: None,
        }
    }

    /// `setEndDateTime(LocalDateTime)`.
    #[must_use]
    pub fn with_end_date_time(mut self, end_date_time: LocalDateTime) -> Self {
        self.end_date_time = end_date_time;
        self
    }

    /// `setNetHours(double)`.
    #[must_use]
    pub fn with_net_hours(mut self, net_hours: f64) -> Self {
        self.net_hours = net_hours;
        self
    }

    /// `setPlannedShift(PlannedShift)`.
    #[must_use]
    pub fn with_planned_shift(mut self, planned_shift: PlannedShift) -> Self {
        self.planned_shift = Some(planned_shift);
        self
    }

    /// `setEmployee(Employee)` — the employee's id.
    #[must_use]
    pub fn with_employee_id(mut self, employee_id: i32) -> Self {
        self.employee_id = employee_id;
        self
    }

    /// `setFlags(int)`.
    #[must_use]
    pub fn with_flags(mut self, flags: i32) -> Self {
        self.flags = flags;
        self
    }

    /// `setShiftCategory(ShiftCategory)`.
    #[must_use]
    pub fn with_shift_category_id(mut self, shift_category_id: Option<i32>) -> Self {
        self.shift_category_id = shift_category_id;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getShiftDate()`.
    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> LocalDateTime {
        self.start_date_time
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getAssignment().getID()` — `getAssignment()` may be `null` in Java.
    pub fn assignment_id(&self) -> Option<i32> {
        self.assignment_id
    }

    /// `getEndDateTime()`.
    pub fn end_date_time(&self) -> LocalDateTime {
        self.end_date_time
    }

    /// `getNetHours()`.
    pub fn net_hours(&self) -> f64 {
        self.net_hours
    }

    /// `getPlannedShift()`.
    pub fn planned_shift(&self) -> Option<PlannedShift> {
        self.planned_shift
    }

    /// `getEmployee().getID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getFlags()`.
    pub fn flags(&self) -> i32 {
        self.flags
    }

    /// `getShiftCategory().getID()` — `null` when unset.
    pub fn shift_category_id(&self) -> Option<i32> {
        self.shift_category_id
    }

    /// `setPlannedShift(PlannedShift)`.
    pub fn set_planned_shift(&mut self, planned_shift: Option<PlannedShift>) {
        self.planned_shift = planned_shift;
    }

    /// `toDateTimeRange()`.
    pub fn to_date_time_range(&self) -> DateTimeRange {
        DateTimeRange::of(self.start_date_time, self.end_date_time)
    }
}
