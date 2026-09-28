//! Port of `com.unifocus.watson.server.scheduler.engine.model.ScheduleModel`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! ScheduleModel.java`. The pipeline's root aggregate; see `DATA_MODEL.md` §4.
//!
//! `progress` (`com.unifocus.tbx.core.progress.Progress`) is not carried as a field — nothing in
//! `scheduler` reads or writes it yet, and it's likely a callback/trait rather than data (see
//! `DATA_MODEL.md` §7); add it when a ported step needs to report progress.
//!
//! `job_schedule_log_map` is keyed on the job id, not an owned `JobData` — same entity-identity-
//! keyed-map treatment as `cleared_employee_shifts_map` (finding 5). Ported as of Phase 2 step 9,
//! unblocked by `process/variable/filters::EmployeeFilterKey` (was blocked, `DATA_MODEL.md` §5).
//!
//! `cleared_employee_shifts_map` carries `EmployeeShift`'s id, not an owned `EmployeeShift` —
//! same entity-identity-keyed-map treatment as `PARITY_AUDIT.md` finding 5.

use crate::engine::model::employee_list::EmployeeList;
use crate::engine::model::job_list::JobList;
use crate::engine::model::logging::job_schedule_log::JobScheduleLog;
use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
use crate::engine::model::shift_list::ShiftList;
use crate::entity::property::Property;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// `ScheduleModel`. No longer `Clone`/`PartialEq` as of Phase 2 step 9 — `job_schedule_log_map`'s
/// values hold `Box<dyn EmployeeFilter>`, which is neither. Nothing ported clones or compares a
/// whole `ScheduleModel` (call sites compare individual fields instead), so this cost nothing.
#[derive(Debug)]
pub struct ScheduleModel {
    property: Property,
    date_range: DateRange,
    new_shift_list: ShiftList,
    old_shift_list: ShiftList,
    cleared_employee_shifts: HashMap<i32, i32>,
    job_list: Option<JobList>,
    employee_list: Option<EmployeeList>,
    current_planned_shift_log: Option<PlannedShiftLog>,
    job_schedule_logs: HashMap<i32, JobScheduleLog>,
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
            job_schedule_logs: HashMap::new(),
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

    /// `getJobScheduleLog(JobData)` — takes the job id directly rather than `&JobData`, so
    /// callers that already hold a `&JobData` borrowed out of `self.job_list` don't hit a second
    /// borrow of `self` here (`JobScheduleLog` only ever needs the id, see its own doc).
    /// `getJobScheduleLog(Assignment)` is not ported — nothing ported resolves an arbitrary
    /// assignment id back to a full `Assignment` (see `PARITY_AUDIT.md` finding 40).
    pub fn job_schedule_log(&mut self, job_id: i32) -> &mut JobScheduleLog {
        self.job_schedule_logs
            .entry(job_id)
            .or_insert_with(|| JobScheduleLog::new(job_id))
    }

    /// `getJobScheduleLogs()` — sorted by the job's full name, case-insensitive (Java's
    /// `compareToIgnoreCase`), same ordering `JobList::all_jobs` already uses.
    /// `SaveScheduleLogService` (step 10), the first real caller.
    pub fn job_schedule_logs(&self) -> Vec<&JobScheduleLog> {
        let mut logs: Vec<&JobScheduleLog> = self.job_schedule_logs.values().collect();
        logs.sort_by_key(|log| {
            self.job_list
                .as_ref()
                .and_then(|jobs| jobs.job_data(log.job_id()))
                .map(|job_data| job_data.job().full_name().to_lowercase())
                .unwrap_or_default()
        });
        logs
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
