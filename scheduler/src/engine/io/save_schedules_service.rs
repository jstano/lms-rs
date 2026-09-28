//! Port of `com.unifocus.watson.server.scheduler.engine.io.SaveSchedulesService` — the last
//! pipeline step (step 10) before Phase 3's `ScheduleEngine` orchestrator.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! SaveSchedulesService.java`. `recalculateDataSets`'s Hibernate `SessionFactory`/`FlushMode`
//! try/finally has no Rust equivalent — this crate has no persistence session to hold flush mode
//! for (`PARITY_AUDIT.md`'s methodology note already says so); only the loop body (recalculate
//! each employee's data set, refresh their rule alerts) is ported, not the flush-mode mechanics
//! around it. `scheduleModel.getProgress().setMessage(...)` calls are dropped for the same reason
//! `Progress` is dropped everywhere else (`ScheduleModel`'s own doc). `deletePriorSchedules`'s
//! `systemScheduleAuditEnabled` parameter is genuinely unused in the real Java method body — not
//! carried over here, since it has no effect either way.
//!
//! `CancelShiftRequestsService`/`SaveScheduleLogService`/`SaveScheduleSnapshotService` are
//! injected as already-constructed collaborators rather than re-threading every one of their own
//! ports through this type's constructor — this crate has no DI container, so composing the
//! smaller services once at the call site (mirroring Java's `@Autowired` object graph) is simpler
//! than flattening every leaf dependency up to the top.

use crate::engine::io::cancel_shift_requests_service::CancelShiftRequestsService;
use crate::engine::io::ports::{
    EmployeeAlertPort, EmployeeShiftPort, PlannedShiftAudit, PlannedShiftAuditPort,
    PlannedShiftPort, PropertyDataKeyPort, SchedulingShiftAuditPort,
};
use crate::engine::io::save_schedule_log_service::SaveScheduleLogService;
use crate::engine::io::save_schedule_snapshot_service::SaveScheduleSnapshotService;
use crate::engine::misc::ports::SchedulesTimeCardCalculatorPort;
use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::property_data_key::PropertyDataKeyTag;
use std::collections::HashSet;

/// `SaveSchedulesService`.
pub struct SaveSchedulesService<'a> {
    planned_shifts: &'a dyn PlannedShiftPort,
    employee_shifts: &'a dyn EmployeeShiftPort,
    scheduling_shift_audits: &'a dyn SchedulingShiftAuditPort,
    planned_shift_audits: &'a dyn PlannedShiftAuditPort,
    property_data_keys: &'a dyn PropertyDataKeyPort,
    schedules_time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
    employee_alerts: &'a dyn EmployeeAlertPort,
    cancel_shift_requests_service: &'a CancelShiftRequestsService,
    save_schedule_log_service: &'a SaveScheduleLogService<'a>,
    save_schedule_snapshot_service: &'a SaveScheduleSnapshotService<'a>,
}

impl<'a> SaveSchedulesService<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        planned_shifts: &'a dyn PlannedShiftPort,
        employee_shifts: &'a dyn EmployeeShiftPort,
        scheduling_shift_audits: &'a dyn SchedulingShiftAuditPort,
        planned_shift_audits: &'a dyn PlannedShiftAuditPort,
        property_data_keys: &'a dyn PropertyDataKeyPort,
        schedules_time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
        employee_alerts: &'a dyn EmployeeAlertPort,
        cancel_shift_requests_service: &'a CancelShiftRequestsService,
        save_schedule_log_service: &'a SaveScheduleLogService<'a>,
        save_schedule_snapshot_service: &'a SaveScheduleSnapshotService<'a>,
    ) -> Self {
        Self {
            planned_shifts,
            employee_shifts,
            scheduling_shift_audits,
            planned_shift_audits,
            property_data_keys,
            schedules_time_card_calculator,
            employee_alerts,
            cancel_shift_requests_service,
            save_schedule_log_service,
            save_schedule_snapshot_service,
        }
    }

    /// `saveSchedules(ScheduleModel)`.
    pub fn save_schedules(&self, schedule_model: &mut ScheduleModel) {
        let property_id = schedule_model.property().id();
        let system_schedule_audit_enabled = self
            .property_data_keys
            .enabled_boolean_key_for_property_with_tag(
                property_id,
                PropertyDataKeyTag::SystemScheduleAudit,
            );
        let system_planned_shift_audit_enabled = self
            .property_data_keys
            .enabled_boolean_key_for_property_with_tag(
                property_id,
                PropertyDataKeyTag::SystemPlannedShiftAudit,
            );

        self.delete_prior_schedules(schedule_model);

        self.save_new_schedules(schedule_model);

        self.audit_changes(
            schedule_model,
            system_schedule_audit_enabled,
            system_planned_shift_audit_enabled,
        );
        self.clear_old_shift_list(schedule_model);

        self.recalculate_data_sets(schedule_model);

        self.cancel_shift_requests_service
            .cancel_shift_requests(schedule_model);

        self.save_schedule_log_service
            .save_schedule_logs(schedule_model);

        self.save_schedule_snapshot_service
            .save_schedule_snapshot(schedule_model);
    }

    /// `recalculateDataSets(ScheduleModel)` — flush-mode mechanics dropped, see module doc.
    fn recalculate_data_sets(&self, schedule_model: &mut ScheduleModel) {
        let Some(employee_list) = schedule_model.employee_list_mut() else {
            return;
        };

        let mut employee_data_list: Vec<&mut EmployeeData> =
            employee_list.employee_data_list_mut().collect();
        employee_data_list
            .sort_by_key(|employee_data| employee_data.employee().name().to_lowercase());

        for employee_data in employee_data_list {
            let employee_id = employee_data.employee().id();

            self.schedules_time_card_calculator
                .calculate_schedule_calc_data_set(employee_data.data_set_mut());
            self.employee_alerts.refresh_rule_alerts(employee_id);
        }
    }

    /// `deletePriorSchedules(ScheduleModel, boolean)` — the boolean parameter is unused in the
    /// real Java method body; not carried over here (see module doc).
    fn delete_prior_schedules(&self, schedule_model: &ScheduleModel) {
        let shift_ids = schedule_model.old_shift_list().employee_shift_ids();

        self.employee_shifts.bulk_delete_by_shift_id(&shift_ids);
    }

    /// `clearOldShiftList(ScheduleModel)`.
    fn clear_old_shift_list(&self, schedule_model: &mut ScheduleModel) {
        schedule_model.old_shift_list_mut().clear_employee_shifts();
    }

    /// `auditChanges(ScheduleModel, boolean, boolean)`.
    fn audit_changes(
        &self,
        schedule_model: &ScheduleModel,
        system_schedule_audit_enabled: bool,
        system_planned_shift_audit_enabled: bool,
    ) {
        if system_schedule_audit_enabled {
            self.audit_schedule_changes(schedule_model);
        }
        if system_planned_shift_audit_enabled {
            self.audit_planned_shift_changes(schedule_model);
        }
    }

    /// `auditScheduleChanges(ScheduleModel)`.
    fn audit_schedule_changes(&self, schedule_model: &ScheduleModel) {
        let old_shifts = schedule_model.old_shift_list().employee_shifts();
        let new_shifts = schedule_model.new_shift_list().employee_shifts();

        let mut audits_to_save = Vec::new();
        let mut duplicated_new_shift_ids = HashSet::new();

        for old_shift in old_shifts {
            let matching_shift = new_shifts
                .iter()
                .find(|new_shift| Self::shifts_match(new_shift, old_shift));

            match matching_shift {
                None => {
                    if let Some(delete_audit) = self
                        .scheduling_shift_audits
                        .create_deleted_schedule_audit(old_shift)
                    {
                        audits_to_save.push(delete_audit);
                    }
                }
                Some(matching_shift) => {
                    duplicated_new_shift_ids.insert(matching_shift.id());
                }
            }
        }

        for new_shift in new_shifts {
            if !duplicated_new_shift_ids.contains(&new_shift.id()) {
                audits_to_save.push(
                    self.scheduling_shift_audits
                        .create_added_shift_audit(new_shift),
                );
            }
        }

        if !audits_to_save.is_empty() {
            self.scheduling_shift_audits.save_all(&audits_to_save);
        }
    }

    /// The `Optional<EmployeeShift> matchingShift = ... .filter(...)` predicate.
    fn shifts_match(new_shift: &EmployeeShift, old_shift: &EmployeeShift) -> bool {
        new_shift.start_date_time() == old_shift.start_date_time()
            && new_shift.end_date_time() == old_shift.end_date_time()
            && new_shift.employee_id() == old_shift.employee_id()
            && new_shift.job_id() == old_shift.job_id()
            && new_shift.shift_date() == old_shift.shift_date()
    }

    /// `auditPlannedShiftChanges(ScheduleModel)`.
    fn audit_planned_shift_changes(&self, schedule_model: &ScheduleModel) {
        self.planned_shift_audits.save_add_audit(
            schedule_model.property(),
            schedule_model.new_shift_list().planned_shifts(),
        );

        let unscheduled_audits = self.unscheduled_planned_shift_audits(schedule_model);
        self.planned_shift_audits.save_all(&unscheduled_audits);

        let scheduled_audits = self.scheduled_planned_shift_audits(schedule_model);
        self.planned_shift_audits.save_all(&scheduled_audits);
    }

    /// `getUnscheduledPlannedShiftAudits(ScheduleModel)`.
    fn unscheduled_planned_shift_audits(
        &self,
        schedule_model: &ScheduleModel,
    ) -> Vec<PlannedShiftAudit> {
        let scheduled_planned_shift_ids: HashSet<i32> = schedule_model
            .new_shift_list()
            .employee_shifts()
            .iter()
            .filter_map(EmployeeShift::planned_shift)
            .map(|planned_shift| planned_shift.id())
            .collect();

        schedule_model
            .cleared_employee_shifts()
            .iter()
            .filter(|(planned_shift_id, _)| !scheduled_planned_shift_ids.contains(planned_shift_id))
            .map(|(&planned_shift_id, &old_employee_shift_id)| {
                self.planned_shift_audits.create_modify_audit_unscheduled(
                    schedule_model.property(),
                    planned_shift_id,
                    old_employee_shift_id,
                )
            })
            .collect()
    }

    /// `getScheduledPlannedShiftAudits(ScheduleModel)`. Java's `.map(it -> ...)` reads
    /// `it.getPlannedShift().getID()` unconditionally and would NPE on a `null` planned shift;
    /// every real `new_shift_list` entry always has one (`ScheduleSaver`'s only caller,
    /// `EmployeeShiftCreator`, always sets it — `PARITY_AUDIT.md` finding 30), but this is a
    /// reporting/audit path, not core scheduling logic, so a `filter_map` (skip, don't panic) is
    /// used instead of a literal `.expect()` — a deliberate, documented divergence rather than an
    /// unnecessary panic risk in an audit trail.
    fn scheduled_planned_shift_audits(
        &self,
        schedule_model: &ScheduleModel,
    ) -> Vec<PlannedShiftAudit> {
        let unscheduled_planned_shift_map = schedule_model.cleared_employee_shifts();

        schedule_model
            .new_shift_list()
            .employee_shifts()
            .iter()
            .filter_map(|new_employee_shift| {
                let planned_shift = new_employee_shift.planned_shift()?;
                let old_employee_shift_id = unscheduled_planned_shift_map
                    .get(&planned_shift.id())
                    .copied();

                Some(self.planned_shift_audits.create_modify_audit_scheduled(
                    schedule_model.property(),
                    &planned_shift,
                    old_employee_shift_id,
                    new_employee_shift,
                ))
            })
            .collect()
    }

    /// `saveNewSchedules(ScheduleModel)`.
    fn save_new_schedules(&self, schedule_model: &ScheduleModel) {
        for planned_shift in schedule_model
            .new_shift_list()
            .planned_shifts()
            .iter()
            .flatten()
        {
            self.planned_shifts.save(planned_shift);
        }

        for employee_shift in schedule_model.new_shift_list().employee_shifts() {
            self.employee_shifts.save(employee_shift);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn shift_at(id: i32, employee_id: i32, job_id: i32, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(
            id,
            date,
            date.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            job_id,
            None,
        )
        .with_employee_id(employee_id)
    }

    #[derive(Default)]
    struct FakePorts {
        saved_planned_shifts: RefCell<Vec<PlannedShift>>,
        saved_employee_shifts: RefCell<Vec<EmployeeShift>>,
        deleted_shift_ids: RefCell<Option<HashSet<i32>>>,
        property_data_keys: HashMap<PropertyDataKeyTag, bool>,
        deleted_audits: RefCell<Vec<i32>>,
        added_audits: RefCell<Vec<i32>>,
        saved_scheduling_audits: RefCell<usize>,
        refreshed_employee_alerts: RefCell<Vec<i32>>,
    }

    impl PlannedShiftPort for FakePorts {
        fn save(&self, planned_shift: &PlannedShift) {
            self.saved_planned_shifts.borrow_mut().push(*planned_shift);
        }

        fn bulk_delete_planned_shifts_for_current_property(
            &self,
            _job_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) {
        }
    }

    impl EmployeeShiftPort for FakePorts {
        fn evict(&self, _employee_shift_id: i32) {}

        fn bulk_delete_by_shift_id(&self, shift_ids: &HashSet<i32>) {
            *self.deleted_shift_ids.borrow_mut() = Some(shift_ids.clone());
        }

        fn save(&self, employee_shift: &EmployeeShift) {
            self.saved_employee_shifts
                .borrow_mut()
                .push(employee_shift.clone());
        }

        fn bulk_delete_employee_shifts_for_jobs_in_current_property(
            &self,
            _job_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) {
        }

        fn employee_schedule_shifts_for_period(
            &self,
            _employee_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) -> Vec<EmployeeShift> {
            Vec::new()
        }
    }

    impl SchedulingShiftAuditPort for FakePorts {
        fn create_deleted_schedule_audit(
            &self,
            old_shift: &EmployeeShift,
        ) -> Option<crate::engine::io::ports::SchedulingShiftAudit> {
            self.deleted_audits.borrow_mut().push(old_shift.id());
            Some(crate::engine::io::ports::SchedulingShiftAudit)
        }

        fn create_added_shift_audit(
            &self,
            new_shift: &EmployeeShift,
        ) -> crate::engine::io::ports::SchedulingShiftAudit {
            self.added_audits.borrow_mut().push(new_shift.id());
            crate::engine::io::ports::SchedulingShiftAudit
        }

        fn save_all(&self, audits: &[crate::engine::io::ports::SchedulingShiftAudit]) {
            *self.saved_scheduling_audits.borrow_mut() = audits.len();
        }
    }

    impl PlannedShiftAuditPort for FakePorts {
        fn save_add_audit(&self, _property: &Property, _planned_shifts: &[Option<PlannedShift>]) {}

        fn create_modify_audit_unscheduled(
            &self,
            _property: &Property,
            _planned_shift_id: i32,
            _old_employee_shift_id: i32,
        ) -> PlannedShiftAudit {
            PlannedShiftAudit
        }

        fn create_modify_audit_scheduled(
            &self,
            _property: &Property,
            _planned_shift: &PlannedShift,
            _old_employee_shift_id: Option<i32>,
            _new_employee_shift: &EmployeeShift,
        ) -> PlannedShiftAudit {
            PlannedShiftAudit
        }

        fn save_all(&self, _audits: &[PlannedShiftAudit]) {}
    }

    impl PropertyDataKeyPort for FakePorts {
        fn enabled_boolean_key_for_property_with_tag(
            &self,
            _property_id: i32,
            tag: PropertyDataKeyTag,
        ) -> bool {
            self.property_data_keys.get(&tag).copied().unwrap_or(false)
        }
    }

    impl SchedulesTimeCardCalculatorPort for FakePorts {
        fn calculate_overtime_for_schedule_calc_data_set(
            &self,
            _data_set: &mut crate::entity::schedule_calc_data_set::ScheduleCalcDataSet,
        ) {
        }

        fn calculate_schedule_calc_data_set(
            &self,
            _data_set: &mut crate::entity::schedule_calc_data_set::ScheduleCalcDataSet,
        ) {
        }
    }

    impl EmployeeAlertPort for FakePorts {
        fn refresh_rule_alerts(&self, employee_id: i32) {
            self.refreshed_employee_alerts
                .borrow_mut()
                .push(employee_id);
        }
    }

    impl crate::engine::io::ports::EmployeeShiftClonerPort for FakePorts {
        fn clone_employee_shift_as_generated(
            &self,
            employee_shift: &EmployeeShift,
        ) -> EmployeeShift {
            employee_shift.clone()
        }
    }

    #[test]
    fn deletes_old_shifts_by_id_and_saves_every_new_shift() {
        let date_range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);

        schedule_model
            .old_shift_list_mut()
            .add_employee_shift(shift_at(1, 100, 1, LocalDate::of(2024, 1, 1)));

        let planned_shift = PlannedShift::new(
            10,
            1,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 1).at_time(joda_rs::LocalTime::of(9, 0, 0)),
            8.0,
            None,
        );
        schedule_model.new_shift_list_mut().add_employee_shift(
            shift_at(2, 100, 1, LocalDate::of(2024, 1, 1)).with_planned_shift(planned_shift),
        );
        schedule_model
            .new_shift_list_mut()
            .add_planned_shift(Some(planned_shift));

        let ports = FakePorts::default();
        let cancel = CancelShiftRequestsService;

        // Build the two smaller collaborators with the same fake ports.
        struct NoOpReportLibrary;
        impl crate::engine::io::ports::ReportLibraryPort for NoOpReportLibrary {
            fn save(&self, _report_library: &crate::engine::io::ports::ReportLibrary) {}
        }
        struct NoOpScheduleLogWriter;
        impl crate::engine::io::save_schedule_log_service::ScheduleLogWriterPort for NoOpScheduleLogWriter {
            fn output_header(&self, _buf: &mut Vec<u8>) {}
            fn output_schedule_log(
                &self,
                _schedule_log: &crate::engine::model::logging::schedule_log::ScheduleLog,
                _buf: &mut Vec<u8>,
            ) {
            }
            fn output_footer(&self, _buf: &mut Vec<u8>) {}
        }

        let report_library = NoOpReportLibrary;
        let schedule_log_writer = NoOpScheduleLogWriter;
        let log_service = SaveScheduleLogService::new(&report_library, &schedule_log_writer);
        let snapshot_service = SaveScheduleSnapshotService::new(&ports, &ports, &ports);

        let service = SaveSchedulesService::new(
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &cancel,
            &log_service,
            &snapshot_service,
        );

        service.save_schedules(&mut schedule_model);

        assert_eq!(*ports.deleted_shift_ids.borrow(), Some(HashSet::from([1])));
        assert_eq!(ports.saved_planned_shifts.borrow().len(), 1);
        assert_eq!(ports.saved_employee_shifts.borrow().len(), 1);
        assert!(schedule_model.old_shift_list().employee_shifts().is_empty());
    }

    #[test]
    fn audits_deleted_and_added_shifts_when_the_audit_flag_is_enabled() {
        let date_range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);

        schedule_model
            .old_shift_list_mut()
            .add_employee_shift(shift_at(1, 100, 1, LocalDate::of(2024, 1, 1)));
        schedule_model
            .new_shift_list_mut()
            .add_employee_shift(shift_at(2, 200, 2, LocalDate::of(2024, 1, 2)));

        let mut property_data_keys = HashMap::new();
        property_data_keys.insert(PropertyDataKeyTag::SystemScheduleAudit, true);
        let ports = FakePorts {
            property_data_keys,
            ..Default::default()
        };

        let cancel = CancelShiftRequestsService;
        struct NoOpReportLibrary;
        impl crate::engine::io::ports::ReportLibraryPort for NoOpReportLibrary {
            fn save(&self, _report_library: &crate::engine::io::ports::ReportLibrary) {}
        }
        struct NoOpScheduleLogWriter;
        impl crate::engine::io::save_schedule_log_service::ScheduleLogWriterPort for NoOpScheduleLogWriter {
            fn output_header(&self, _buf: &mut Vec<u8>) {}
            fn output_schedule_log(
                &self,
                _schedule_log: &crate::engine::model::logging::schedule_log::ScheduleLog,
                _buf: &mut Vec<u8>,
            ) {
            }
            fn output_footer(&self, _buf: &mut Vec<u8>) {}
        }
        let report_library = NoOpReportLibrary;
        let schedule_log_writer = NoOpScheduleLogWriter;
        let log_service = SaveScheduleLogService::new(&report_library, &schedule_log_writer);
        let snapshot_service = SaveScheduleSnapshotService::new(&ports, &ports, &ports);

        let service = SaveSchedulesService::new(
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &ports,
            &cancel,
            &log_service,
            &snapshot_service,
        );

        service.save_schedules(&mut schedule_model);

        assert_eq!(*ports.deleted_audits.borrow(), vec![1]);
        assert_eq!(*ports.added_audits.borrow(), vec![2]);
        assert_eq!(*ports.saved_scheduling_audits.borrow(), 2);
    }
}
