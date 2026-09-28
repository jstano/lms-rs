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
use crate::entity::employee_regular_period::EmployeeRegularPeriod;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_type::EmployeeType;
use crate::entity::planned_shift::PlannedShift;
use crate::entity::pre_schedule::PreSchedule;
use crate::entity::pre_schedule_jobclass::PreScheduleJobclass;
use crate::entity::property::Property;
use crate::entity::property_data_key::PropertyDataKeyTag;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use date_range_rs::DateRange;
use joda_rs::{LocalDate, LocalDateTime};
use std::collections::{HashMap, HashSet};

/// `EmployeeShiftDAO`, narrowed to the methods `SchedulePreparationService` (step 5),
/// `SaveSchedulesService`, and `SaveScheduleSnapshotService` (step 10) call. `evict` is a no-op
/// boundary for this crate (there's no session cache to evict from without a persistence layer),
/// kept as a port rather than dropped so the call site still records that Java does it; the rest
/// are real reads/writes a future persistence layer implements for real.
pub trait EmployeeShiftPort {
    /// `evict(EmployeeShift)`.
    fn evict(&self, employee_shift_id: i32);

    /// `bulkDeleteByShiftID(Collection<Integer>)`. `SaveSchedulesService.deletePriorSchedules`'s
    /// dependency.
    fn bulk_delete_by_shift_id(&self, shift_ids: &HashSet<i32>);

    /// `save(EmployeeShift)`. `SaveSchedulesService.saveNewSchedules`/
    /// `SaveScheduleSnapshotService.copyScheduledShiftsToGeneratedShifts`'s dependency.
    fn save(&self, employee_shift: &EmployeeShift);

    /// `bulkDeleteEmployeeShiftsForJobsInCurrentProperty(Collection<Integer>, DateRange,
    /// ShiftType)` — always `ShiftType.GENERATED` here, the only value
    /// `SaveScheduleSnapshotService` ever passes (same "fixed constant, not modeled" treatment as
    /// `entity::planned_shift::PlannedShift`'s undocumented constant fields, applied to an enum
    /// argument instead of a field).
    fn bulk_delete_employee_shifts_for_jobs_in_current_property(
        &self,
        job_ids: &HashSet<i32>,
        date_range: &DateRange,
    );

    /// `getEmployeeScheduleShiftsForPeriod(Collection<Integer>, DateRange)`.
    /// `SaveScheduleSnapshotService.loadScheduledShifts`'s dependency.
    fn employee_schedule_shifts_for_period(
        &self,
        employee_ids: &HashSet<i32>,
        date_range: &DateRange,
    ) -> Vec<EmployeeShift>;
}

/// `PlannedShiftDAO`'s write-side surface (`save`, `bulkDeletePlannedShiftsForCurrentProperty`) —
/// kept as its own trait rather than folded into `PlannedShiftQueryPort` (the read side
/// `ForecastPlannedShiftLoader`/`OriginalProjectedHoursLoader` use), since step 10 is this DAO's
/// first writer and the two surfaces have no callers in common.
pub trait PlannedShiftPort {
    /// `save(PlannedShift)`.
    fn save(&self, planned_shift: &PlannedShift);

    /// `bulkDeletePlannedShiftsForCurrentProperty(Collection<Integer>, DateRange, PlanType)` —
    /// always `PlanType.GENERATED` here, same fixed-constant treatment as
    /// `EmployeeShiftPort::bulk_delete_employee_shifts_for_jobs_in_current_property`.
    fn bulk_delete_planned_shifts_for_current_property(
        &self,
        job_ids: &HashSet<i32>,
        date_range: &DateRange,
    );
}

/// `EmployeeShiftCloner.cloneEmployeeShiftAsGenerated(EmployeeShift)` — a deferred external
/// subsystem, same "narrow entity slice" treatment as `PARITY_AUDIT.md` findings 2/8. Java's real
/// clone copies dozens of fields (adjustments, errors, punches, pay rates/dollars) this crate's
/// `EmployeeShift`/`PlannedShift` stubs never modeled, since nothing else ported reads any of
/// them back. Growing both entities for this one write-only caller would be pure churn.
pub trait EmployeeShiftClonerPort {
    /// `cloneEmployeeShiftAsGenerated(EmployeeShift)`.
    fn clone_employee_shift_as_generated(&self, employee_shift: &EmployeeShift) -> EmployeeShift;
}

/// `PropertyDataKeyService.enabledBooleanKeyForPropertyWithTag(int, PropertyDataKey)`.
/// `SaveSchedulesService`'s dependency, narrowed to the two tags it reads
/// (`entity::property_data_key::PropertyDataKeyTag`).
pub trait PropertyDataKeyPort {
    /// `enabledBooleanKeyForPropertyWithTag(int, PropertyDataKey)`.
    fn enabled_boolean_key_for_property_with_tag(
        &self,
        property_id: i32,
        tag: PropertyDataKeyTag,
    ) -> bool;
}

/// `EmployeeAlertAPIJava.refreshRuleAlerts(int)`. `SaveSchedulesService.recalculateDataSets`'s
/// dependency — an alerting subsystem entirely out of this crate's scope.
pub trait EmployeeAlertPort {
    /// `refreshRuleAlerts(int)`.
    fn refresh_rule_alerts(&self, employee_id: i32);
}

/// `SchedulingShiftAudit` — an opaque write-only marker, not a modeled entity. Every field on
/// Java's real row (property, message text from `ResourceMgr`, employee-shift snapshot, ...) is
/// built by the deferred `SchedulingShiftAuditDAO` itself; nothing ported ever reads a field back
/// off one, only passes it straight from `SchedulingShiftAuditPort::create_*` to `save_all`.
#[derive(Debug, Clone, Copy)]
pub struct SchedulingShiftAudit;

/// `SchedulingShiftAuditDAO`, narrowed to the three methods
/// `SaveSchedulesService.auditScheduleChanges` calls. `create_deleted_schedule_audit` returns
/// `Option` — Java's `createSchedulingShiftAuditForSystemTaskWithNullPlannedShift` can return
/// `null`, and its one caller explicitly null-checks it; `create_added_shift_audit` doesn't,
/// matching the literal code (Java adds its result to the list unconditionally, with no null
/// check).
pub trait SchedulingShiftAuditPort {
    /// `createSchedulingShiftAuditForSystemTaskWithNullPlannedShift(EmployeeShift, String)`.
    fn create_deleted_schedule_audit(
        &self,
        old_shift: &EmployeeShift,
    ) -> Option<SchedulingShiftAudit>;

    /// `createSchedulingShiftAuditForSystemTask(PlannedShift, EmployeeShift, String)`.
    fn create_added_shift_audit(&self, new_shift: &EmployeeShift) -> SchedulingShiftAudit;

    /// `saveAll(Collection<SchedulingShiftAudit>)`.
    fn save_all(&self, audits: &[SchedulingShiftAudit]);
}

/// `PlannedShiftAudit` — same opaque write-only treatment as `SchedulingShiftAudit`.
#[derive(Debug, Clone, Copy)]
pub struct PlannedShiftAudit;

/// `PlannedShiftAuditDAO`, narrowed to the methods
/// `SaveSchedulesService.auditPlannedShiftChanges` calls. `create_modify_audit_unscheduled`/
/// `create_modify_audit_scheduled` take ids, not full `PlannedShift`/`EmployeeShift` values, for
/// the "old" side — Java's `createModifyAudit` takes live Hibernate entities there
/// (`scheduleModel.getClearedEmployeeShiftsMap()`'s key/value), but this crate's
/// `ScheduleModel.cleared_employee_shifts` only ever stored the ids
/// (`PARITY_AUDIT.md` finding 5's entity-identity-keyed-map treatment, decided at step 5 — well
/// before this wave needed the full entities back), and there's no by-id lookup for a plain
/// `PlannedShift`/`EmployeeShift` value outside the shift lists that already produced those ids.
/// A real implementation resolves them from the DB by id, same as it would anyway.
pub trait PlannedShiftAuditPort {
    /// `saveAddAudit(Property, Collection<PlannedShift>)`.
    fn save_add_audit(&self, property: &Property, planned_shifts: &[Option<PlannedShift>]);

    /// `createModifyAudit(Property, PlannedShift, EmployeeShift, PlannedShift, null)` — the
    /// "unscheduled" case (`getUnscheduledPlannedShiftAudits`): new/old planned shift are the same
    /// id, new employee shift is absent.
    fn create_modify_audit_unscheduled(
        &self,
        property: &Property,
        planned_shift_id: i32,
        old_employee_shift_id: i32,
    ) -> PlannedShiftAudit;

    /// `createModifyAudit(Property, PlannedShift, EmployeeShift, PlannedShift, EmployeeShift)` —
    /// the "scheduled" case (`getScheduledPlannedShiftAudits`): new/old planned shift are the
    /// same value, old employee shift may be absent (no matching cleared shift for this id), new
    /// employee shift is always present.
    fn create_modify_audit_scheduled(
        &self,
        property: &Property,
        planned_shift: &PlannedShift,
        old_employee_shift_id: Option<i32>,
        new_employee_shift: &EmployeeShift,
    ) -> PlannedShiftAudit;

    /// `saveAll(Collection<PlannedShiftAudit>)`.
    fn save_all(&self, audits: &[PlannedShiftAudit]);
}

/// `ReportLibrary`, narrowed to the fields `SaveScheduleLogService.createReportLibraryEntry`
/// sets. Unlike `SchedulingShiftAudit`/`PlannedShiftAudit`, every field here is real — this crate
/// builds the whole row itself (from `ScheduleModel` plus `SaveScheduleLogService`'s own
/// `ScheduleLogWriterPort` output), rather than delegating construction to an external "create"
/// call.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportLibrary {
    property_id: i32,
    report_name: String,
    generated_date_time: LocalDateTime,
    report_type: String,
    report_data: Vec<u8>,
    html_report_data: Vec<u8>,
    start_date: LocalDate,
    end_date: LocalDate,
}

impl ReportLibrary {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        property_id: i32,
        report_name: impl Into<String>,
        generated_date_time: LocalDateTime,
        report_type: impl Into<String>,
        report_data: Vec<u8>,
        html_report_data: Vec<u8>,
        start_date: LocalDate,
        end_date: LocalDate,
    ) -> Self {
        Self {
            property_id,
            report_name: report_name.into(),
            generated_date_time,
            report_type: report_type.into(),
            report_data,
            html_report_data,
            start_date,
            end_date,
        }
    }

    /// `getProperty().getID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getReportName()`.
    pub fn report_name(&self) -> &str {
        &self.report_name
    }

    /// `getGeneratedDateTime()`.
    pub fn generated_date_time(&self) -> LocalDateTime {
        self.generated_date_time
    }

    /// `getReportType()`.
    pub fn report_type(&self) -> &str {
        &self.report_type
    }

    /// `getReportData()`.
    pub fn report_data(&self) -> &[u8] {
        &self.report_data
    }

    /// `getHtmlReportData()`.
    pub fn html_report_data(&self) -> &[u8] {
        &self.html_report_data
    }

    /// `getStartDate()`.
    pub fn start_date(&self) -> LocalDate {
        self.start_date
    }

    /// `getEndDate()`.
    pub fn end_date(&self) -> LocalDate {
        self.end_date
    }
}

/// `ReportLibraryDAO`, narrowed to `save`. `SaveScheduleLogService`'s dependency.
pub trait ReportLibraryPort {
    /// `save(ReportLibrary)`.
    fn save(&self, report_library: &ReportLibrary);
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

/// Weekly recurring work periods for a set of jobs/employee type. `EmployeeRegularPeriodDAO`,
/// narrowed to `findForJobsAndEmployeeType`. `RegularScheduleLoader`'s dependency (Phase 2 steps
/// 7-8).
pub trait EmployeeRegularPeriodDAOPort {
    /// `findForJobsAndEmployeeType(Collection<Integer>, EmployeeType)`.
    fn find_for_jobs_and_employee_type(
        &self,
        job_ids: &[i32],
        employee_type: EmployeeType,
    ) -> Vec<EmployeeRegularPeriod>;
}
