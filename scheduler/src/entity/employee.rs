//! Port of `com.unifocus.watson.server.hibernate.entity.Employee`
//! (and the `BasicEmployee` superclass its job-status lookups live on).
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/{Employee,
//! BasicEmployee}.java` for the methods below; only the fields Phase 1 (`process/comparators/`,
//! `engine/model/`) reads are modeled — see `entity` module docs and `DATA_MODEL.md` §3.
//!
//! `is_active_on_date`/`is_active_job_during_period` (needing `EmployeeStatus`/
//! `EmployeeStatusType`/`EmployeeJobStatus::overlaps_period`) were ported for `ScheduleChecker`/
//! `ScheduleHoursDistributionValidator` (Phase 3's `autosched` wave, their first real readers).
//! `status` is a `with_*` builder field defaulting to empty, following `day_off_plan`'s precedent
//! — most existing callers never set it. `day_off_plan`/`current_pattern_no` (`process/checkers/
//! EmployeeDayOffRotationPlanChecker`) are `with_*` builder fields, following
//! `entity::assignment::Assignment`'s pattern for the same reason.

use crate::entity::day_off_plan::DayOffPlan;
use crate::entity::employee_assignment::EmployeeAssignment;
use crate::entity::employee_job_status::EmployeeJobStatus;
use crate::entity::employee_status::EmployeeStatus;
use crate::entity::employee_status_type::EmployeeStatusType;
use crate::entity::employee_type::EmployeeType;
use crate::entity::work_class::WorkClass;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// `Employee`.
#[derive(Debug, Clone, PartialEq)]
pub struct Employee {
    id: i32,
    name: String,
    employee_type: EmployeeType,
    hours_available: Option<f64>,
    work_class: WorkClass,
    min_hours_off: Option<f64>,
    min_days_off: Option<i32>,
    hire_date: Option<LocalDate>,
    assignments: Vec<EmployeeAssignment>,
    employee_job_statuses: Vec<EmployeeJobStatus>,
    day_off_plan: Option<DayOffPlan>,
    current_pattern_no: i32,
    status: Vec<EmployeeStatus>,
}

impl Employee {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        name: impl Into<String>,
        employee_type: EmployeeType,
        hours_available: Option<f64>,
        work_class: WorkClass,
        min_hours_off: Option<f64>,
        min_days_off: Option<i32>,
        hire_date: Option<LocalDate>,
        assignments: Vec<EmployeeAssignment>,
        employee_job_statuses: Vec<EmployeeJobStatus>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            employee_type,
            hours_available,
            work_class,
            min_hours_off,
            min_days_off,
            hire_date,
            assignments,
            employee_job_statuses,
            day_off_plan: None,
            current_pattern_no: 0,
            status: Vec::new(),
        }
    }

    /// `setDayOffPlan(DayOffPlan)` + `setCurrentPatternNo(int)`.
    #[must_use]
    pub fn with_day_off_plan(mut self, day_off_plan: DayOffPlan, current_pattern_no: i32) -> Self {
        self.day_off_plan = Some(day_off_plan);
        self.current_pattern_no = current_pattern_no;
        self
    }

    /// `setStatus(List<EmployeeStatus>)`.
    #[must_use]
    pub fn with_status(mut self, status: Vec<EmployeeStatus>) -> Self {
        self.status = status;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getEmployeeType()`.
    pub fn employee_type(&self) -> EmployeeType {
        self.employee_type
    }

    /// `getHoursAvailable()` — `null` in Java falls back to `getWorkClass().getHoursAvailable()`.
    pub fn hours_available(&self) -> Option<f64> {
        self.hours_available
    }

    /// `getWorkClass()`.
    pub fn work_class(&self) -> WorkClass {
        self.work_class
    }

    /// `getMinHoursOff()` — nullable override, see `Assignment::min_hours_off`.
    pub fn min_hours_off(&self) -> Option<f64> {
        self.min_hours_off
    }

    /// `getMinDaysOff()`.
    pub fn min_days_off(&self) -> Option<i32> {
        self.min_days_off
    }

    /// `getHireDate()`.
    pub fn hire_date(&self) -> Option<LocalDate> {
        self.hire_date
    }

    /// `getAssignments()`.
    pub fn assignments(&self) -> &[EmployeeAssignment] {
        &self.assignments
    }

    /// `getAssignment(int)` — first (only, by construction) standing for that assignment.
    pub fn assignment(&self, assignment_id: i32) -> Option<&EmployeeAssignment> {
        self.assignments
            .iter()
            .find(|a| a.assignment_id() == assignment_id)
    }

    /// `getEmployeeJobStatuses()` — every status on file, regardless of date.
    pub fn employee_job_statuses(&self) -> &[EmployeeJobStatus] {
        &self.employee_job_statuses
    }

    /// `getEmployeeJobStatuses(LocalDate)` — every status whose date range contains `date`.
    pub fn employee_job_statuses_for_date(&self, date: LocalDate) -> Vec<EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .copied()
            .filter(|s| s.contains_date(date))
            .collect()
    }

    /// `getActiveEmployeeJobStatusesForDate(LocalDate)` (`BasicEmployee`) — despite the name,
    /// this is the exact same `containsDate` filter as [`employee_job_statuses_for_date`], not a
    /// further "is active" check; Java returns a `Set`, ported as a `Vec` since nothing here
    /// relies on set semantics.
    pub fn active_employee_job_statuses_for_date(&self, date: LocalDate) -> Vec<EmployeeJobStatus> {
        self.employee_job_statuses_for_date(date)
    }

    /// `getEmployeeJobStatus(int, LocalDate)` — first status for `job_id` whose range contains
    /// `date`.
    pub fn employee_job_status(&self, job_id: i32, date: LocalDate) -> Option<EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .copied()
            .find(|s| s.job_id() == job_id && s.contains_date(date))
    }

    /// `getHomeEmployeeJobStatus(LocalDate)` — first `isHome()` status whose range contains
    /// `date`.
    pub fn home_employee_job_status(&self, date: LocalDate) -> Option<EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .copied()
            .find(|s| s.is_home() && s.contains_date(date))
    }

    /// `getFirstHomeEmployeeJobStatusForPeriod(DateRange)` — the first date in `period` (in
    /// range order) that has a home job status.
    pub fn first_home_employee_job_status_for_period(
        &self,
        period: &DateRange,
    ) -> Option<EmployeeJobStatus> {
        period
            .dates()
            .into_iter()
            .find_map(|date| self.home_employee_job_status(date))
    }

    /// `getDayOffPlan()`.
    pub fn day_off_plan(&self) -> Option<&DayOffPlan> {
        self.day_off_plan.as_ref()
    }

    /// `getCurrentPatternNo()`.
    pub fn current_pattern_no(&self) -> i32 {
        self.current_pattern_no
    }

    /// `setCurrentPatternNo(int)`.
    pub fn set_current_pattern_no(&mut self, current_pattern_no: i32) {
        self.current_pattern_no = current_pattern_no;
    }

    /// `getStatus()`.
    pub fn status(&self) -> &[EmployeeStatus] {
        &self.status
    }

    /// `getStatusForDate(LocalDate)` — the first status whose range contains `date`.
    pub fn status_for_date(&self, date: LocalDate) -> Option<EmployeeStatus> {
        self.status.iter().copied().find(|s| s.contains_date(date))
    }

    /// `isActiveOnDate(LocalDate)`.
    pub fn is_active_on_date(&self, date: LocalDate) -> bool {
        let Some(status) = self.status_for_date(date) else {
            return false;
        };

        match status.status_type() {
            EmployeeStatusType::Active | EmployeeStatusType::Rehire => true,
            EmployeeStatusType::LeaveOfAbsence => false,
            EmployeeStatusType::Terminated => status.start_date() == date,
            EmployeeStatusType::Transferred => false,
        }
    }

    /// `isActiveJobDuringPeriod(int, DateRange)`.
    pub fn is_active_job_during_period(&self, job_id: i32, period: &DateRange) -> bool {
        self.employee_job_statuses
            .iter()
            .any(|status| status.overlaps_period(period) && status.job_id() == job_id)
    }

    /// `getActiveAssignmentIDs()`.
    pub fn active_assignment_ids(&self) -> Vec<i32> {
        self.assignments
            .iter()
            .filter(|a| a.is_active())
            .map(|a| a.assignment_id())
            .collect()
    }

    /// `getEffectiveAvailableHours()`.
    pub fn effective_available_hours(&self) -> f64 {
        self.hours_available
            .unwrap_or_else(|| self.work_class.hours_available())
    }
}
