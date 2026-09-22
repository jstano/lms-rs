//! DAO/loader boundaries `engine::io`'s loaders reach into — plus any later step's use of the
//! *same* DAO (see `EmployeePort`), since a DAO gets one trait regardless of how many steps call
//! it.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/dao/{PropertyDAO,EmployeeDAO,
//! SchedulesTimeCardDAO,PlannedShiftDAO,PreScheduleJobclassDAO}.java` and
//! `watson/server/common/engine/JobLoader.java`. Same "narrow port trait per DAO method surface"
//! treatment as `engine::process::ports::AssignmentPort` and `workrules::rules::ports` (see that
//! module's doc: "good repository traits by method surface... only the surface comes across").
//!
//! This crate has no persistence layer of its own — per `PLAN_SCHEDULER.md`'s Verification
//! section, "loaders/save services will initially be stubs/traits pending real persistence" — so
//! every method here mirrors exactly the one DAO call its corresponding Java loader makes, not
//! the whole DAO. A DAO whose only callers live outside `engine::io` (e.g. `DayOffPlanDAO`, only
//! called from `engine::misc::DayOffPlanRotator`) gets its port trait defined next to that
//! caller instead — see `engine::misc::ports`.

use crate::engine::generate_schedules_parameters::GenerateSchedulesParameters;
use crate::entity::assignment::Assignment;
use crate::entity::employee::Employee;
use crate::entity::planned_shift::PlannedShift;
use crate::entity::pre_schedule::PreSchedule;
use crate::entity::pre_schedule_jobclass::PreScheduleJobclass;
use crate::entity::property::Property;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Removing a shift from the Hibernate session cache. `EmployeeShiftDAO`, narrowed to `evict` —
/// `SchedulePreparationService`'s dependency. A no-op boundary for this crate (there's no
/// session cache to evict from without a persistence layer), kept as a port rather than dropped
/// so the call site still records that Java does it.
pub trait EmployeeShiftPort {
    /// `evict(EmployeeShift)`.
    fn evict(&self, employee_shift_id: i32);
}

/// Properties by id. `PropertyDAO`, narrowed to `findByID`. `ScheduleModelCreator`'s dependency.
pub trait PropertyPort {
    /// `findByID(int)`.
    fn find_by_id(&self, id: i32) -> Option<Property>;
}

/// Jobs in scope for a generate-schedules run.
/// `com.unifocus.watson.server.common.engine.JobLoader` — not itself a DAO, but the same
/// "external subsystem behind a narrow port" treatment as `PARITY_AUDIT.md` findings 10/16: its
/// real implementation resolves labor-structure scope (property/division/department/job filters)
/// this crate doesn't model. `JobListLoader`'s dependency.
pub trait JobLoaderPort {
    /// `loadJobs(GenerateSchedulesParameters)`.
    fn load_jobs(&self, params: &GenerateSchedulesParameters) -> Vec<Assignment>;
}

/// Pre-schedule job-class configuration by property. `PreScheduleJobclassDAO`, narrowed to
/// `findAllForProperty`. `PreScheduleJobLoader`'s dependency.
pub trait PreScheduleJobclassPort {
    /// `findAllForProperty(Property)`.
    fn find_all_for_property(&self, property_id: i32) -> Vec<PreScheduleJobclass>;
}

/// Employees eligible for a scheduling run. `EmployeeDAO`, narrowed to
/// `getEmployeesActiveWithJobsDuringPeriod` (`EmployeeListLoader`, step 1) and
/// `findAllForProperty` (`DayOffPlanRotator`, step 2 — same DAO, a second call site).
pub trait EmployeePort {
    /// `getEmployeesActiveWithJobsDuringPeriod(DateRange, Collection<Integer>)`.
    fn employees_active_with_jobs_during_period(
        &self,
        date_range: &DateRange,
        job_ids: &[i32],
    ) -> Vec<Employee>;

    /// `findAllForProperty(Property)`.
    fn find_all_for_property(&self, property_id: i32) -> Vec<Employee>;
}

/// Loaded time-card/shift data per employee. `SchedulesTimeCardDAO`, narrowed to
/// `loadScheduleCalcDataset`. `EmployeeListLoader`'s other dependency.
pub trait SchedulesTimeCardPort {
    /// `loadScheduleCalcDataset(DateRange, Set<Integer>)`.
    fn load_schedule_calc_dataset(
        &self,
        date_range: &DateRange,
        employee_ids: &[i32],
    ) -> HashMap<i32, ScheduleCalcDataSet>;
}

/// `OriginalProjectedHoursLoader.Results` — a plain projection row (job id, shift date, summed
/// duration), not a Hibernate entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginalProjectedHoursResult {
    job_id: i32,
    shift_date: LocalDate,
    hours: f64,
}

impl OriginalProjectedHoursResult {
    pub fn new(job_id: i32, shift_date: LocalDate, hours: f64) -> Self {
        Self {
            job_id,
            shift_date,
            hours,
        }
    }

    /// `getJobID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getShiftDate()`.
    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }

    /// `getHours()`.
    pub fn hours(&self) -> f64 {
        self.hours
    }
}

/// Planned-shift queries. `PlannedShiftDAO`, narrowed to the two queries
/// `ForecastPlannedShiftLoader`/`OriginalProjectedHoursLoader` each run against it — kept as one
/// trait (rather than one per loader) since both wrap the same underlying DAO.
pub trait PlannedShiftQueryPort {
    /// `ForecastPlannedShiftLoader.loadForecastPlannedShiftsForJobsAndDates`.
    fn forecast_planned_shifts_for_jobs_and_dates(
        &self,
        property_id: i32,
        job_ids: &[i32],
        date_range: &DateRange,
    ) -> Vec<PlannedShift>;

    /// `OriginalProjectedHoursLoader.loadOriginalProjectedHoursByJobAndDate`.
    fn original_projected_hours_by_job_and_date(
        &self,
        property_id: i32,
        job_ids: &[i32],
        date_range: &DateRange,
    ) -> Vec<OriginalProjectedHoursResult>;
}

/// Pre-scheduled employee assignments for a set of jobs/date range. `PreScheduleDAO`, narrowed to
/// `loadPreSchedulesForJobsAndDateRange`. `PreScheduleLoader`'s dependency (Phase 2 step 6).
pub trait PreScheduleDAOPort {
    /// `loadPreSchedulesForJobsAndDateRange(Collection<Integer>, DateRange)`.
    fn load_pre_schedules_for_jobs_and_date_range(
        &self,
        job_ids: &[i32],
        date_range: &DateRange,
    ) -> Vec<PreSchedule>;
}
