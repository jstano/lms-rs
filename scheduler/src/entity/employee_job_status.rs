//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeJobStatus`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeJobStatus.java`
//! for `containsDate`/`overlapsPeriod`; the rest inferred from call sites (`JobRankComparator`,
//! `JobSeniorityDateComparator`, `EmployeeData.hasJobAtAnyLevel`/`hasJobAtLevel`,
//! `Employee`/`BasicEmployee`'s lookup methods).
//!
//! `job` is *not* a nested [`Assignment`](crate::entity::assignment::Assignment) here — see the
//! `entity` module doc for why: the engine only ever reads the job's own id and its parent's id
//! off `EmployeeJobStatus.getJob()`, both carried flat as `job_id`/`job_parent_assignment_id`.

use crate::entity::employee_pay_type::EmployeePayType;
use joda_rs::LocalDate;

/// One employee's standing on one job for a date range. `EmployeeJobStatus`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmployeeJobStatus {
    job_id: i32,
    job_parent_assignment_id: Option<i32>,
    start_date: LocalDate,
    end_date: LocalDate,
    is_home: bool,
    rank: i32,
    seniority_date: LocalDate,
    contract_hours: f64,
    is_sub_only: bool,
    schedule_order: i32,
    pay_type: Option<EmployeePayType>,
}

impl EmployeeJobStatus {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job_id: i32,
        job_parent_assignment_id: Option<i32>,
        start_date: LocalDate,
        end_date: LocalDate,
        is_home: bool,
        rank: i32,
        seniority_date: LocalDate,
        contract_hours: f64,
        is_sub_only: bool,
        schedule_order: i32,
    ) -> Self {
        Self {
            job_id,
            job_parent_assignment_id,
            start_date,
            end_date,
            is_home,
            rank,
            seniority_date,
            contract_hours,
            is_sub_only,
            schedule_order,
            pay_type: None,
        }
    }

    /// `setPayType(EmployeePayType)`, as a builder — `process/variable/filters/`'s first reader.
    pub fn with_pay_type(mut self, pay_type: EmployeePayType) -> Self {
        self.pay_type = Some(pay_type);
        self
    }

    /// `getPayType()`.
    pub fn pay_type(&self) -> Option<EmployeePayType> {
        self.pay_type
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getJob().getParentAssignment().getID()` — the department, for the
    /// `isDepartmentalSeniority()` comparator path.
    pub fn job_parent_assignment_id(&self) -> Option<i32> {
        self.job_parent_assignment_id
    }

    /// `getStartDate()`.
    pub fn start_date(&self) -> LocalDate {
        self.start_date
    }

    /// `getEndDate()`.
    pub fn end_date(&self) -> LocalDate {
        self.end_date
    }

    /// `containsDate(LocalDate)` — `startDate <= date && endDate >= date`.
    pub fn contains_date(&self, date: LocalDate) -> bool {
        self.start_date.is_on_or_before(date) && self.end_date.is_on_or_after(date)
    }

    /// `overlapsPeriod(DateRange)` — `Employee::is_active_job_during_period`'s dependency
    /// (Phase 3's `autosched` wave, its first real caller).
    pub fn overlaps_period(&self, period: &date_range_rs::DateRange) -> bool {
        self.start_date.is_on_or_before(period.end_date())
            && self.end_date.is_on_or_after(period.start_date())
    }

    /// `isHome()`.
    pub fn is_home(&self) -> bool {
        self.is_home
    }

    /// `getRank()`.
    pub fn rank(&self) -> i32 {
        self.rank
    }

    /// `getSeniorityDate()`.
    pub fn seniority_date(&self) -> LocalDate {
        self.seniority_date
    }

    /// `getContractHours()`.
    pub fn contract_hours(&self) -> f64 {
        self.contract_hours
    }

    /// `isSubOnly()`.
    pub fn is_sub_only(&self) -> bool {
        self.is_sub_only
    }

    /// `getScheduleOrder()`.
    pub fn schedule_order(&self) -> i32 {
        self.schedule_order
    }
}
