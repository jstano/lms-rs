//! Port of `com.unifocus.watson.server.scheduler.engine.model.ScheduleModel`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! ScheduleModel.java`. The pipeline's root aggregate; see `DATA_MODEL.md` §4.
//!
//! `progress` (`com.unifocus.tbx.core.progress.Progress`) is not carried as a field — nothing in
//! `scheduler` reads or writes it yet, and it's likely a callback/trait rather than data (see
//! `DATA_MODEL.md` §7); add it when a ported step needs to report progress.
//!
//! `job_schedule_log_map`/`getJobScheduleLog`/`getJobScheduleLogs` are **not ported** — keyed by
//! `JobScheduleLog`, which is itself deferred (blocked on `EmployeeFilter`, `DATA_MODEL.md` §5).
//!
//! `cleared_employee_shifts_map` carries `EmployeeShift`'s id, not an owned `EmployeeShift` —
//! same entity-identity-keyed-map treatment as `PARITY_AUDIT.md` finding 5.

use crate::engine::model::employee_list::EmployeeList;
use crate::engine::model::job_list::JobList;
use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
use crate::engine::model::shift_list::ShiftList;
use crate::entity::property::Property;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// `ScheduleModel`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleModel {
    property: Property,
    date_range: DateRange,
    new_shift_list: ShiftList,
    old_shift_list: ShiftList,
    cleared_employee_shifts: HashMap<i32, i32>,
    job_list: Option<JobList>,
    employee_list: Option<EmployeeList>,
    current_planned_shift_log: Option<PlannedShiftLog>,
}

impl ScheduleModel {
    pub fn new(property: Property, date_range: DateRange) -> Self {
        Self {
            property,
            date_range,
            new_shift_list: ShiftList::new(),
            old_shift_list: ShiftList::new(),
            cleared_employee_shifts: HashMap::new(),
            job_list: None,
            employee_list: None,
            current_planned_shift_log: None,
        }
    }

    /// `getProperty()`.
    pub fn property(&self) -> &Property {
        &self.property
    }

    /// `getDateRange()`.
    pub fn date_range(&self) -> &DateRange {
        &self.date_range
    }

    /// `getWeekForDate(LocalDate)`.
    pub fn week_for_date(&self, date: LocalDate) -> DateRange {
        self.property.week_for_date(date)
    }

    /// `getNewShiftList()`.
    pub fn new_shift_list(&self) -> &ShiftList {
        &self.new_shift_list
    }

    /// `getNewShiftList()` (mutable).
    pub fn new_shift_list_mut(&mut self) -> &mut ShiftList {
        &mut self.new_shift_list
    }

    /// `getOldShiftList()`.
    pub fn old_shift_list(&self) -> &ShiftList {
        &self.old_shift_list
    }

    /// `getOldShiftList()` (mutable).
    pub fn old_shift_list_mut(&mut self) -> &mut ShiftList {
        &mut self.old_shift_list
    }

    /// `getClearedEmployeeShiftsMap()`.
    pub fn cleared_employee_shifts(&self) -> &HashMap<i32, i32> {
        &self.cleared_employee_shifts
    }

    /// `getClearedEmployeeShiftsMap()` (mutable).
    pub fn cleared_employee_shifts_mut(&mut self) -> &mut HashMap<i32, i32> {
        &mut self.cleared_employee_shifts
    }

    /// `getJobList()`.
    pub fn job_list(&self) -> Option<&JobList> {
        self.job_list.as_ref()
    }

    /// `getJobList()` (mutable — Java returns the live, mutable `JobList`).
    pub fn job_list_mut(&mut self) -> Option<&mut JobList> {
        self.job_list.as_mut()
    }

    /// `setJobList(JobList)`.
    pub fn set_job_list(&mut self, job_list: JobList) {
        self.job_list = Some(job_list);
    }

    /// `getEmployeeList()`.
    pub fn employee_list(&self) -> Option<&EmployeeList> {
        self.employee_list.as_ref()
    }

    /// `getEmployeeList()` (mutable — Java returns the live, mutable `EmployeeList`).
    pub fn employee_list_mut(&mut self) -> Option<&mut EmployeeList> {
        self.employee_list.as_mut()
    }

    /// `setEmployeeList(EmployeeList)`.
    pub fn set_employee_list(&mut self, employee_list: EmployeeList) {
        self.employee_list = Some(employee_list);
    }

    /// `getCurrentPlannedShiftLog()`.
    pub fn current_planned_shift_log(&self) -> Option<&PlannedShiftLog> {
        self.current_planned_shift_log.as_ref()
    }

    /// `getCurrentPlannedShiftLog()` (mutable).
    pub fn current_planned_shift_log_mut(&mut self) -> Option<&mut PlannedShiftLog> {
        self.current_planned_shift_log.as_mut()
    }

    /// `setCurrentPlannedShiftLog(PlannedShiftLog)`.
    pub fn set_current_planned_shift_log(&mut self, log: Option<PlannedShiftLog>) {
        self.current_planned_shift_log = log;
    }

    /// Gives `SchedulePreparationService` (step 5) simultaneous mutable access to several
    /// independent fields it must touch together per shift — Java's single mutable object graph
    /// gives every method that for free; Rust's borrow checker needs an explicit split since
    /// these are separate private fields, each already behind its own single-field accessor.
    /// Not a general-purpose accessor — add a narrower one instead if a future caller only needs
    /// one of these pieces.
    pub fn shift_preparation_fields(
        &mut self,
    ) -> (
        DateRange,
        Option<&mut JobList>,
        Option<&mut EmployeeList>,
        &mut ShiftList,
        &mut HashMap<i32, i32>,
    ) {
        (
            self.date_range,
            self.job_list.as_mut(),
            self.employee_list.as_mut(),
            &mut self.old_shift_list,
            &mut self.cleared_employee_shifts,
        )
    }
}
