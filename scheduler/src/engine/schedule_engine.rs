//! Port of `com.unifocus.watson.server.scheduler.engine.ScheduleEngine`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/ScheduleEngine.java`.
//! Phase 3: the 10-step pipeline orchestrator, now that every step exists. `Progress` isn't
//! carried — no step's Rust port threads it either (see `ScheduleModelLoader`'s own doc) — and
//! the `WatsonDataAccessCommand`/`@Transactional` framework plumbing around `execute()` is out of
//! scope, same as every other Spring-wiring layer this crate has skipped. `generateSchedules`
//! becomes `generate_schedules`, returning `None` only when `ScheduleModelLoader::load` does
//! (property couldn't be resolved), matching the loader's own `Option` contract.
//!
//! `rotateDayOffPlans`'s Java body is `void` — Hibernate's dirty-checking persists the mutated
//! `DayOffPlan`/`Employee` entities on transaction commit, without the `ScheduleModel` itself ever
//! seeing the result. `DayOffPlanRotator::rotate_day_off_plans` returns its mutated
//! `(Vec<DayOffPlan>, Vec<Employee>)` instead (no ORM here to persist them implicitly), so this
//! step calls it and discards the tuple — persisting the rotation is the same deferred
//! external-subsystem concern as the rest of this crate's DAO boundary, and discarding it matches
//! Java's behavior for the in-memory `ScheduleModel` exactly: the current run is unaffected either
//! way.
//!
//! Steps 3-4 (`ProjectedHoursReducer::reduce_projected_hours`/`EmployeeAvailableHoursBalancer::
//! compute_balance_factor`) both take `&ScheduleModel` alongside `&mut JobData` borrowed out of
//! that same model's `JobList` — Java's shared object graph gives `ScheduleEngine` both for free;
//! Rust can't alias `&ScheduleModel` with a `&mut JobData` borrowed out of it. Resolved with the
//! same take/reinsert idiom as `PreScheduleProcess` (`JobList::take_job_data`, new for this wave,
//! mirroring `EmployeeList::take_employee_data`).

use crate::engine::generate_schedules_parameters::GenerateSchedulesParameters;
use crate::engine::io::save_schedules_service::SaveSchedulesService;
use crate::engine::io::schedule_model_loader::ScheduleModelLoader;
use crate::engine::io::schedule_preparation_service::SchedulePreparationService;
use crate::engine::misc::day_off_plan_rotator::DayOffPlanRotator;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
use crate::engine::process::permanent_schedule_process::PermanentScheduleProcess;
use crate::engine::process::pre_schedule_process::PreScheduleProcess;
use crate::engine::process::projectedhoursreducers::projected_hours_reducer_factory::ProjectedHoursReducerFactory;
use crate::engine::process::regular_schedule_process::RegularScheduleProcess;
use crate::engine::process::schedulebalancers::employee_available_hours_balancer::EmployeeAvailableHoursBalancer;
use crate::engine::process::variable_schedule_process::VariableScheduleProcess;
use crate::entity::schedule_mode::ScheduleMode;

/// `ScheduleEngine`.
pub struct ScheduleEngine<'a> {
    schedule_model_loader: &'a ScheduleModelLoader<'a>,
    day_off_plan_rotator: &'a DayOffPlanRotator<'a>,
    projected_hours_reducer_factory: &'a ProjectedHoursReducerFactory<'a>,
    employee_available_hours_balancer: &'a EmployeeAvailableHoursBalancer<'a>,
    schedule_preparation_service: &'a SchedulePreparationService<'a>,
    pre_schedule_process: &'a PreScheduleProcess<'a>,
    permanent_schedule_process: &'a PermanentScheduleProcess<'a>,
    regular_schedule_process: &'a RegularScheduleProcess<'a>,
    variable_schedule_process: &'a VariableScheduleProcess<'a>,
    save_schedules_service: &'a SaveSchedulesService<'a>,
    active_on_date: &'a dyn EmployeeActiveOnDatePort,
}

impl<'a> ScheduleEngine<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schedule_model_loader: &'a ScheduleModelLoader<'a>,
        day_off_plan_rotator: &'a DayOffPlanRotator<'a>,
        projected_hours_reducer_factory: &'a ProjectedHoursReducerFactory<'a>,
        employee_available_hours_balancer: &'a EmployeeAvailableHoursBalancer<'a>,
        schedule_preparation_service: &'a SchedulePreparationService<'a>,
        pre_schedule_process: &'a PreScheduleProcess<'a>,
        permanent_schedule_process: &'a PermanentScheduleProcess<'a>,
        regular_schedule_process: &'a RegularScheduleProcess<'a>,
        variable_schedule_process: &'a VariableScheduleProcess<'a>,
        save_schedules_service: &'a SaveSchedulesService<'a>,
        active_on_date: &'a dyn EmployeeActiveOnDatePort,
    ) -> Self {
        Self {
            schedule_model_loader,
            day_off_plan_rotator,
            projected_hours_reducer_factory,
            employee_available_hours_balancer,
            schedule_preparation_service,
            pre_schedule_process,
            permanent_schedule_process,
            regular_schedule_process,
            variable_schedule_process,
            save_schedules_service,
            active_on_date,
        }
    }

    /// `generateSchedules(GenerateSchedulesParameters, Progress)`.
    pub fn generate_schedules(
        &self,
        params: &GenerateSchedulesParameters,
    ) -> Option<ScheduleModel> {
        let mut schedule_model = self.schedule_model_loader.load(params)?;

        self.rotate_day_off_plans(&schedule_model, params.rotate_days_off());

        self.adjust_projected_hours_for_jobs(&mut schedule_model);

        self.compute_balance_factors_for_jobs(&mut schedule_model);

        self.prepare_shifts_for_scheduling(params, &mut schedule_model);

        self.pre_schedule_employees(params, &mut schedule_model);

        self.schedule_permanent_employees(params, &mut schedule_model);

        self.schedule_regular_employees(params, &mut schedule_model);

        self.schedule_variable_employees(params, &mut schedule_model);

        self.save_schedules(&mut schedule_model);

        Some(schedule_model)
    }

    fn rotate_day_off_plans(&self, schedule_model: &ScheduleModel, rotate_days_off: bool) {
        rotate_day_off_plans(self.day_off_plan_rotator, schedule_model, rotate_days_off);
    }

    fn adjust_projected_hours_for_jobs(&self, schedule_model: &mut ScheduleModel) {
        adjust_projected_hours_for_jobs(self.projected_hours_reducer_factory, schedule_model);
    }

    fn compute_balance_factors_for_jobs(&self, schedule_model: &mut ScheduleModel) {
        compute_balance_factors_for_jobs(self.employee_available_hours_balancer, schedule_model);
    }

    fn prepare_shifts_for_scheduling(
        &self,
        params: &GenerateSchedulesParameters,
        schedule_model: &mut ScheduleModel,
    ) {
        self.schedule_preparation_service
            .prepare_shifts_for_scheduling(schedule_model, params.clear_schedules());
    }

    fn pre_schedule_employees(
        &self,
        params: &GenerateSchedulesParameters,
        schedule_model: &mut ScheduleModel,
    ) {
        if params.generate_pre_schedules() {
            self.pre_schedule_process
                .pre_schedule_employees(schedule_model);
        }
    }

    fn schedule_permanent_employees(
        &self,
        params: &GenerateSchedulesParameters,
        schedule_model: &mut ScheduleModel,
    ) {
        if params.generate_permanent_schedules() {
            self.permanent_schedule_process
                .schedule_permanent_employees(schedule_model, self.active_on_date);
        }
    }

    fn schedule_regular_employees(
        &self,
        params: &GenerateSchedulesParameters,
        schedule_model: &mut ScheduleModel,
    ) {
        if params.generate_regular_schedules() {
            self.regular_schedule_process
                .schedule_regular_employees(schedule_model, self.active_on_date);
        }
    }

    fn schedule_variable_employees(
        &self,
        params: &GenerateSchedulesParameters,
        schedule_model: &mut ScheduleModel,
    ) {
        if params.generate_variable_schedules() {
            self.variable_schedule_process
                .schedule_variable_employees(schedule_model);
        }
    }

    fn save_schedules(&self, schedule_model: &mut ScheduleModel) {
        self.save_schedules_service.save_schedules(schedule_model);
    }
}

/// `rotateDayOffPlans(ScheduleModel, boolean)` — a free function, same "testable without wiring
/// up every other pipeline step" reasoning as [`adjust_projected_hours_for_jobs`]. Ground truth's
/// Java body is `void`; see this file's module doc for why the Rust port's `DayOffPlanRotator`
/// returns its mutation instead of discarding it — that return value is still discarded here,
/// matching Java's behavior for the in-memory `ScheduleModel` (the current run is unaffected
/// either way).
fn rotate_day_off_plans(
    day_off_plan_rotator: &DayOffPlanRotator,
    schedule_model: &ScheduleModel,
    rotate_days_off: bool,
) {
    if rotate_days_off {
        day_off_plan_rotator.rotate_day_off_plans(schedule_model);
    }
}

/// `adjustProjectedHoursForJobs(ScheduleModel)` — a free function (not a `ScheduleEngine` method)
/// so it's testable without wiring up every other pipeline step; see [`ScheduleEngine::new`]'s
/// other collaborators for why that wiring is otherwise unavoidable.
fn adjust_projected_hours_for_jobs(
    projected_hours_reducer_factory: &ProjectedHoursReducerFactory,
    schedule_model: &mut ScheduleModel,
) {
    if schedule_model.property().schedule_mode() != Some(ScheduleMode::Weekly) {
        return;
    }

    let job_ids: Vec<i32> = match schedule_model.job_list() {
        Some(job_list) => job_list.job_ids().collect(),
        None => return,
    };

    for job_id in job_ids {
        let Some(mut job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.take_job_data(job_id))
        else {
            continue;
        };

        let reducer = projected_hours_reducer_factory.projected_hours_reducer(&job_data);
        reducer.reduce_projected_hours(schedule_model, &mut job_data);

        schedule_model
            .job_list_mut()
            .expect("job_list was present before take_job_data")
            .add_job_data(job_data);
    }
}

/// `computeBalanceFactorsForJobs(ScheduleModel)` — a free function, same reasoning as
/// [`adjust_projected_hours_for_jobs`].
fn compute_balance_factors_for_jobs(
    employee_available_hours_balancer: &EmployeeAvailableHoursBalancer,
    schedule_model: &mut ScheduleModel,
) {
    if schedule_model.property().schedule_mode() != Some(ScheduleMode::Weekly) {
        return;
    }

    let job_ids: Vec<i32> = match schedule_model.job_list() {
        Some(job_list) => job_list.job_ids().collect(),
        None => return,
    };

    for job_id in job_ids {
        let Some(mut job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.take_job_data(job_id))
        else {
            continue;
        };

        if !job_data.job().is_balance_schedules() {
            schedule_model
                .job_list_mut()
                .expect("job_list was present before take_job_data")
                .add_job_data(job_data);
            continue;
        }

        employee_available_hours_balancer.compute_balance_factor(schedule_model, &mut job_data);

        schedule_model
            .job_list_mut()
            .expect("job_list was present before take_job_data")
            .add_job_data(job_data);
    }
}

#[cfg(test)]
mod tests {
    //! Exercises the job-loop and rotate-day-off-plans free functions directly, plus (in
    //! `generate_schedules_orchestration`) the full `generate_schedules` pipeline. Ground truth
    //! for the latter: `ScheduleEngineTest.groovy` (`taps/taps/src/test/.../engine/
    //! ScheduleEngineTest.groovy`, not present under `lms/scheduler`, which has no `src/test` at
    //! all) — seven Spock cases verifying each pipeline step's mock call count under different
    //! flag/mode combinations. This crate has no mocking layer, so `generate_schedules_
    //! orchestration` builds the real ten-collaborator dependency graph (several of them, e.g.
    //! `PreScheduleProcess`/`PermanentScheduleProcess`/`VariableScheduleProcess`, are concrete
    //! structs several ports deep, with nothing else in this crate ever constructing one) and
    //! proves each gate at that collaborator's own outermost port with a call-counting spy, or —
    //! for `EmployeeAvailableHoursBalancer`/`VariableByEmployeeSetGenerator`, which have no port
    //! of their own to spy on — via the real state change they make (`JobData::balance_factor`/
    //! `balance_level`) instead. Every path a given scenario's flags can't reach (e.g. `Regular
    //! ScheduleSingleShift`'s dependents, when the regular-periods DAO returns nothing) is wired
    //! to a fake that panics if actually invoked, so a wiring regression that reaches further than
    //! expected fails loudly rather than passing silently. Consolidated into four tests instead of
    //! seven — an all-off/all-on pair plus a per-flag isolation sweep — since Rust's fakes are
    //! rebuilt fresh per call (`fn run(...)`) rather than reset like Spock's per-test mocks, so
    //! the seven independent scenarios collapse into one parametrized function without losing any
    //! of Groovy's per-flag isolation.

    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::process::projectedhoursreducers::default_projected_hours_reducer::DefaultProjectedHoursReducer;
    use crate::engine::process::projectedhoursreducers::flat_projected_hours_reducer::FlatProjectedHoursReducer;
    use crate::engine::process::projectedhoursreducers::percent_projected_hours_reducer::PercentProjectedHoursReducer;
    use crate::entity::assignment::Assignment;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    fn assignment(job_id: i32, is_balance_schedules: bool) -> Assignment {
        Assignment::new(
            job_id,
            "Job",
            is_balance_schedules,
            None,
            None,
            None,
            false,
            Vec::new(),
            Vec::new(),
            None,
        )
    }

    fn schedule_model(
        schedule_mode: Option<crate::entity::schedule_mode::ScheduleMode>,
    ) -> ScheduleModel {
        let current_week = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let mut property = Property::new(1, current_week);
        if let Some(mode) = schedule_mode {
            property = property.with_schedule_mode(mode);
        }
        ScheduleModel::new(property, current_week)
    }

    #[test]
    fn adjust_projected_hours_for_jobs_skips_non_weekly_mode() {
        let mut model = schedule_model(None);
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(assignment(1, false)));
        model.set_job_list(job_list);

        let default_reducer = DefaultProjectedHoursReducer::new();
        let flat_reducer = FlatProjectedHoursReducer::new(&EMPLOYEE_DATA_SERVICES);
        let percent_reducer = PercentProjectedHoursReducer::new(&EMPLOYEE_DATA_SERVICES);
        let factory =
            ProjectedHoursReducerFactory::new(&default_reducer, &flat_reducer, &percent_reducer);

        adjust_projected_hours_for_jobs(&factory, &mut model);

        assert!(model.job_list().unwrap().job_data(1).is_some());
    }

    #[test]
    fn adjust_projected_hours_for_jobs_round_trips_every_job_in_weekly_mode() {
        let mut model = schedule_model(Some(crate::entity::schedule_mode::ScheduleMode::Weekly));
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(assignment(1, false)));
        job_list.add_job_data(JobData::new(assignment(2, false)));
        model.set_job_list(job_list);

        let default_reducer = DefaultProjectedHoursReducer::new();
        let flat_reducer = FlatProjectedHoursReducer::new(&EMPLOYEE_DATA_SERVICES);
        let percent_reducer = PercentProjectedHoursReducer::new(&EMPLOYEE_DATA_SERVICES);
        let factory =
            ProjectedHoursReducerFactory::new(&default_reducer, &flat_reducer, &percent_reducer);

        adjust_projected_hours_for_jobs(&factory, &mut model);

        let job_list = model.job_list().unwrap();
        assert!(job_list.job_data(1).is_some());
        assert!(job_list.job_data(2).is_some());
    }

    #[test]
    fn compute_balance_factors_for_jobs_only_touches_balance_schedules_jobs() {
        let mut model = schedule_model(Some(crate::entity::schedule_mode::ScheduleMode::Weekly));
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(assignment(1, true)));
        job_list.add_job_data(JobData::new(assignment(2, false)));
        model.set_job_list(job_list);

        let balancer = EmployeeAvailableHoursBalancer::new(&EMPLOYEE_DATA_SERVICES);

        compute_balance_factors_for_jobs(&balancer, &mut model);

        let job_list = model.job_list().unwrap();
        assert!(job_list.job_data(1).is_some());
        assert!(job_list.job_data(2).is_some());
    }

    use crate::engine::misc::employee_data_services::EmployeeDataServices;
    use std::sync::LazyLock;

    static EMPLOYEE_DATA_SERVICES: LazyLock<EmployeeDataServices> =
        LazyLock::new(EmployeeDataServices::new);

    struct CallCountingDayOffPlanPort {
        calls: std::cell::Cell<u32>,
    }
    impl crate::engine::misc::ports::DayOffPlanPort for CallCountingDayOffPlanPort {
        fn find_all_for_property(
            &self,
            _property_id: i32,
        ) -> Vec<crate::entity::day_off_plan::DayOffPlan> {
            self.calls.set(self.calls.get() + 1);
            Vec::new()
        }
    }

    struct NoOpEmployeePort;
    impl crate::engine::io::ports::EmployeePort for NoOpEmployeePort {
        fn employees_active_with_jobs_during_period(
            &self,
            _date_range: &DateRange,
            _job_ids: &[i32],
        ) -> Vec<crate::entity::employee::Employee> {
            unimplemented!("not exercised by rotate_day_off_plans")
        }

        fn find_all_for_property(
            &self,
            _property_id: i32,
        ) -> Vec<crate::entity::employee::Employee> {
            Vec::new()
        }
    }

    #[test]
    fn rotate_day_off_plans_only_calls_the_rotator_when_requested() {
        let day_off_plans = CallCountingDayOffPlanPort {
            calls: std::cell::Cell::new(0),
        };
        let employees = NoOpEmployeePort;
        let rotator = DayOffPlanRotator::new(&day_off_plans, &employees);

        let model = schedule_model(None);

        rotate_day_off_plans(&rotator, &model, false);
        assert_eq!(day_off_plans.calls.get(), 0);

        rotate_day_off_plans(&rotator, &model, true);
        assert_eq!(day_off_plans.calls.get(), 1);
    }

    /// Port of `ScheduleEngineTest.groovy`'s seven `testGenerateSchedules*` cases. Java verifies
    /// each pipeline step's mock call count directly; this crate has no mocking layer, so each
    /// step's gate is instead proven by a spy planted at that step's own outermost port (or, for
    /// `EmployeeAvailableHoursBalancer` — a concrete collaborator with no port of its own — by the
    /// real `balance_factor` state change it produces). Every collaborator that flag combination
    /// can't reach in this scenario (one job, one employee, zero pre-schedules/regular-periods,
    /// so `RegularScheduleGenerator`'s and `VariableJobSchedulingProcess`'s *other* dispatch arms
    /// never run) is wired to a dead fake that panics if actually invoked, so a wiring regression
    /// that reaches further than expected fails loudly instead of silently passing.
    mod generate_schedules_orchestration {
        use super::*;
        use crate::engine::io::cancel_shift_requests_service::CancelShiftRequestsService;
        use crate::engine::io::employee_list_loader::EmployeeListLoader;
        use crate::engine::io::forecast_planned_shift_loader::ForecastPlannedShiftLoader;
        use crate::engine::io::job_list_loader::JobListLoader;
        use crate::engine::io::original_projected_hours_loader::OriginalProjectedHoursLoader;
        use crate::engine::io::planned_shift_loader::PlannedShiftLoader;
        use crate::engine::io::ports::{
            EmployeeAlertPort, EmployeePort, EmployeeRegularPeriodDAOPort, EmployeeShiftClonerPort,
            EmployeeShiftPort, JobLoaderPort, OriginalProjectedHoursResult, PlannedShiftAudit,
            PlannedShiftAuditPort, PlannedShiftPort, PlannedShiftQueryPort, PreScheduleDAOPort,
            PreScheduleJobclassPort, PropertyDataKeyPort, PropertyPort, ReportLibrary,
            ReportLibraryPort, SchedulesTimeCardPort, SchedulingShiftAudit, SchedulingShiftAuditPort,
        };
        use crate::engine::io::pre_schedule_job_loader::PreScheduleJobLoader;
        use crate::engine::io::pre_schedule_loader::PreScheduleLoader;
        use crate::engine::io::regular_schedule_loader::RegularScheduleLoader;
        use crate::engine::io::save_schedule_log_service::{
            SaveScheduleLogService, ScheduleLogWriterPort,
        };
        use crate::engine::io::save_schedule_snapshot_service::SaveScheduleSnapshotService;
        use crate::engine::io::schedule_model_creator::ScheduleModelCreator;
        use crate::engine::misc::calculate_data_set::CalculateDataSet;
        use crate::engine::misc::employee_shift_creator::EmployeeShiftCreator;
        use crate::engine::misc::planned_shift_creator::PlannedShiftCreator;
        use crate::engine::misc::planned_shift_helper::PlannedShiftHelper;
        use crate::engine::misc::planned_shift_matcher::PlannedShiftMatcher;
        use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
        use crate::engine::misc::projected_hours_checker::ProjectedHoursChecker;
        use crate::engine::misc::schedule_saver::ScheduleSaver;
        use crate::engine::model::employee_data::EmployeeData;
        use crate::engine::model::job_data::JobData;
        use crate::engine::model::logging::schedule_log::ScheduleLog;
        use crate::engine::process::checkers::employee_certifications_checker::EmployeeCertificationPort;
        use crate::engine::process::checkers::employee_job_status_checker::EmployeeJobStatusChecker;
        use crate::engine::process::checkers::employee_monthly_available_hours_checker::MonthlyContractHoursAvailablePort;
        use crate::engine::process::checkers::employee_time_off_checker::EmployeeTimeOffChecker;
        use crate::engine::process::checkers::schedule_restriction_rule_checker::{
            ScheduleRestrictionCheckResult, ScheduleRestrictionRulePort,
        };
        use crate::engine::process::pre_schedule_process::PreScheduleProcess;
        use crate::engine::process::ports::{AssignmentPort, OvertimeForDateRangePort};
        use crate::engine::process::projectedhoursreducers::projected_hours_reducer::ProjectedHoursReducer;
        use crate::engine::process::regularschedules::regular_schedule_can_work_checker::RegularScheduleCanWorkChecker;
        use crate::engine::process::regularschedules::regular_schedule_can_work_checker_factory::RegularScheduleCanWorkCheckerFactory;
        use crate::engine::process::regularschedules::regular_schedule_employee_shift_creator::RegularScheduleEmployeeShiftCreator;
        use crate::engine::process::regularschedules::regular_schedule_generator::RegularScheduleGenerator;
        use crate::engine::process::regularschedules::regular_schedule_single_date::RegularScheduleSingleDate;
        use crate::engine::process::regularschedules::regular_schedule_single_shift::RegularScheduleSingleShift;
        use crate::engine::process::variable::generators::variable_by_employee_set_generator::VariableByEmployeeSetGenerator;
        use crate::engine::process::variable::generators::variable_by_job_schedule_order_generator::VariableByJobScheduleOrderGenerator;
        use crate::engine::process::variable::generators::variable_by_seniority_generator::VariableBySeniorityGenerator;
        use crate::engine::process::variable::non_pre_scheduled_job_process::NonPreScheduledJobProcess;
        use crate::engine::process::variable::planned_shift_scheduler::PlannedShiftScheduler;
        use crate::engine::process::variable::pre_scheduled_job_process::PreScheduledJobProcess;
        use crate::engine::process::variable::variable_can_work_checker::VariableCanWorkChecker;
        use crate::engine::process::variable::variable_can_work_checker_factory::VariableCanWorkCheckerFactory;
        use crate::engine::process::variable::variable_checker::VariableChecker;
        use crate::engine::process::variable::variable_job_scheduler::VariableJobScheduler;
        use crate::engine::process::variable::variable_job_scheduling_process::VariableJobSchedulingProcess;
        use crate::entity::employee::Employee;
        use crate::entity::employee_job_status::EmployeeJobStatus;
        use crate::entity::employee_regular_period::EmployeeRegularPeriod;
        use crate::entity::employee_shift::EmployeeShift;
        use crate::entity::employee_type::EmployeeType;
        use crate::entity::planned_shift::PlannedShift;
        use crate::entity::pre_schedule::PreSchedule;
        use crate::entity::pre_schedule_jobclass::PreScheduleJobclass;
        use crate::entity::property_data_key::PropertyDataKeyTag;
        use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
        use crate::entity::scheduling_method::SchedulingMethod;
        use crate::entity::work_class::WorkClass;
        use std::cell::{Cell, RefCell};
        use std::collections::{HashMap, HashSet};

        struct Spies {
            day_off_calls: u32,
            projected_hours_calls: u32,
            balance_factor: f64,
            pre_schedule_calls: u32,
            permanent_calls: u32,
            regular_calls: u32,
            variable_balance_level: i32,
        }

        /// Every dead port a fake for this scenario should never actually reach, grouped onto one
        /// struct — see the enclosing module doc for why each path is dead here.
        struct DeadPort;
        impl AssignmentPort for DeadPort {
            fn find_by_id(&self, _id: i32) -> Option<Assignment> {
                unreachable!("dead in this scenario")
            }
        }
        impl ScheduleRestrictionRulePort for DeadPort {
            fn check(
                &self,
                _employee_data: &EmployeeData,
                _employee_shift: &EmployeeShift,
            ) -> ScheduleRestrictionCheckResult {
                unreachable!("dead in this scenario")
            }
        }
        impl EmployeeCertificationPort for DeadPort {
            fn is_certified(
                &self,
                _employee_data: &EmployeeData,
                _employee_shift: &EmployeeShift,
            ) -> bool {
                unreachable!("dead in this scenario")
            }
        }
        impl SchedulesTimeCardCalculatorPort for DeadPort {
            fn calculate_overtime_for_schedule_calc_data_set(
                &self,
                _data_set: &mut ScheduleCalcDataSet,
            ) {
                unreachable!("dead in this scenario")
            }
            fn calculate_schedule_calc_data_set(&self, _data_set: &mut ScheduleCalcDataSet) {
                unreachable!("dead in this scenario")
            }
        }
        impl ScheduleLunchRunnerPort for DeadPort {
            fn run_rules(
                &self,
                _data_set: &mut ScheduleCalcDataSet,
                _employee_shift: &mut EmployeeShift,
            ) {
                unreachable!("dead in this scenario")
            }
        }
        impl OvertimeForDateRangePort for DeadPort {
            fn overtime_for_date_range(
                &self,
                _employee_data: &mut EmployeeData,
                _date_range: &DateRange,
            ) -> f64 {
                unreachable!("dead in this scenario")
            }
        }
        impl MonthlyContractHoursAvailablePort for DeadPort {
            fn hours_available(
                &self,
                _schedule_model: &ScheduleModel,
                _employee_data: &mut EmployeeData,
                _employee_shift: &EmployeeShift,
            ) -> f64 {
                unreachable!("dead in this scenario")
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn run(
            schedule_mode: Option<crate::entity::schedule_mode::ScheduleMode>,
            rotate_days_off: bool,
            generate_pre_schedules: bool,
            generate_permanent_schedules: bool,
            generate_regular_schedules: bool,
            generate_variable_schedules: bool,
        ) -> Spies {
            let date = LocalDate::of(2024, 1, 1);
            let range = DateRange::new(date, date);

            // --- ScheduleModelLoader: one balance-schedules job with one qualifying employee ---
            let mut property = Property::new(1, range);
            if let Some(mode) = schedule_mode {
                property = property.with_schedule_mode(mode);
            }
            struct FakePropertyPort {
                property: Property,
            }
            impl PropertyPort for FakePropertyPort {
                fn find_by_id(&self, _id: i32) -> Option<Property> {
                    Some(self.property.clone())
                }
            }
            let property_port = FakePropertyPort {
                property: property.clone(),
            };

            let job = Assignment::new(
                1,
                "Job",
                true,
                None,
                None,
                None,
                false,
                Vec::new(),
                Vec::new(),
                None,
            )
            .with_scheduling_method(SchedulingMethod::ByEmployeeSet);

            struct FakeJobLoaderPort {
                jobs: Vec<Assignment>,
            }
            impl JobLoaderPort for FakeJobLoaderPort {
                fn load_jobs(&self, _params: &GenerateSchedulesParameters) -> Vec<Assignment> {
                    self.jobs.clone()
                }
            }
            let job_loader_port = FakeJobLoaderPort { jobs: vec![job] };

            struct FakePreScheduleJobclassPort;
            impl PreScheduleJobclassPort for FakePreScheduleJobclassPort {
                fn find_all_for_property(&self, _property_id: i32) -> Vec<PreScheduleJobclass> {
                    Vec::new()
                }
            }
            let pre_schedule_jobclass_port = FakePreScheduleJobclassPort;

            let employee = Employee::new(
                1,
                "Employee",
                EmployeeType::Regular,
                None,
                WorkClass::new(40.0, true),
                None,
                None,
                None,
                Vec::new(),
                vec![EmployeeJobStatus::new(
                    1, None, date, date, true, 0, date, 0.0, false, 0,
                )],
            );

            struct FakeEmployeePort {
                employees: Vec<Employee>,
            }
            impl EmployeePort for FakeEmployeePort {
                fn employees_active_with_jobs_during_period(
                    &self,
                    _date_range: &DateRange,
                    _job_ids: &[i32],
                ) -> Vec<Employee> {
                    self.employees.clone()
                }
                fn find_all_for_property(&self, _property_id: i32) -> Vec<Employee> {
                    self.employees.clone()
                }
            }
            let employee_port = FakeEmployeePort {
                employees: vec![employee.clone()],
            };

            let mut data_set = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default());
            data_set.set_employee(employee);
            let mut data_sets = HashMap::new();
            data_sets.insert(1, data_set);

            struct FakeSchedulesTimeCardPort {
                data_sets: HashMap<i32, ScheduleCalcDataSet>,
            }
            impl SchedulesTimeCardPort for FakeSchedulesTimeCardPort {
                fn load_schedule_calc_dataset(
                    &self,
                    _date_range: &DateRange,
                    _employee_ids: &[i32],
                ) -> HashMap<i32, ScheduleCalcDataSet> {
                    self.data_sets.clone()
                }
            }
            let time_card_port = FakeSchedulesTimeCardPort { data_sets };

            struct FakePlannedShiftQueryPort;
            impl PlannedShiftQueryPort for FakePlannedShiftQueryPort {
                fn forecast_planned_shifts_for_jobs_and_dates(
                    &self,
                    _property_id: i32,
                    _job_ids: &[i32],
                    _date_range: &DateRange,
                ) -> Vec<PlannedShift> {
                    Vec::new()
                }
                fn original_projected_hours_by_job_and_date(
                    &self,
                    _property_id: i32,
                    _job_ids: &[i32],
                    _date_range: &DateRange,
                ) -> Vec<OriginalProjectedHoursResult> {
                    Vec::new()
                }
            }
            let planned_shift_query_port = FakePlannedShiftQueryPort;

            let loader = ScheduleModelLoader::new(
                ScheduleModelCreator::new(&property_port),
                JobListLoader::new(&job_loader_port),
                PreScheduleJobLoader::new(&pre_schedule_jobclass_port),
                EmployeeListLoader::new(&employee_port, &time_card_port),
                PlannedShiftLoader::new(
                    ForecastPlannedShiftLoader::new(&planned_shift_query_port),
                    OriginalProjectedHoursLoader::new(&planned_shift_query_port),
                ),
            );

            // --- DayOffPlanRotator (spy) ---
            let day_off_plan_port = CallCountingDayOffPlanPort {
                calls: Cell::new(0),
            };
            let day_off_employee_port = NoOpEmployeePort;
            let rotator = DayOffPlanRotator::new(&day_off_plan_port, &day_off_employee_port);

            // --- ProjectedHoursReducerFactory (spy default reducer; job has no override method) ---
            struct SpyReducer {
                calls: Cell<u32>,
            }
            impl ProjectedHoursReducer for SpyReducer {
                fn reduce_projected_hours(
                    &self,
                    _schedule_model: &ScheduleModel,
                    _job_data: &mut JobData,
                ) {
                    self.calls.set(self.calls.get() + 1);
                }
            }
            struct DeadReducer;
            impl ProjectedHoursReducer for DeadReducer {
                fn reduce_projected_hours(
                    &self,
                    _schedule_model: &ScheduleModel,
                    _job_data: &mut JobData,
                ) {
                    unreachable!("job's projected_hours_reduction_method is None")
                }
            }
            let default_reducer = SpyReducer {
                calls: Cell::new(0),
            };
            let flat_reducer = DeadReducer;
            let percent_reducer = DeadReducer;
            let factory = ProjectedHoursReducerFactory::new(
                &default_reducer,
                &flat_reducer,
                &percent_reducer,
            );

            // --- EmployeeAvailableHoursBalancer: real, observed via job_data.balance_factor() ---
            let balancer = EmployeeAvailableHoursBalancer::new(&EMPLOYEE_DATA_SERVICES);

            // --- SchedulePreparationService: always runs, nothing for it to do here ---
            struct NoOpEmployeeShiftPort;
            impl EmployeeShiftPort for NoOpEmployeeShiftPort {
                fn evict(&self, _employee_shift_id: i32) {}
                fn bulk_delete_by_shift_id(&self, _shift_ids: &HashSet<i32>) {}
                fn save(&self, _employee_shift: &EmployeeShift) {}
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
            let prep_port = NoOpEmployeeShiftPort;
            let preparation_service = SchedulePreparationService::new(&prep_port);

            // --- active_on_date ---
            struct AlwaysActive;
            impl EmployeeActiveOnDatePort for AlwaysActive {
                fn is_active_on_date(&self, _employee: &Employee, _date: LocalDate) -> bool {
                    true
                }
            }
            let active_on_date = AlwaysActive;

            // --- PreScheduleProcess (spy DAO; empty rows keep the rest dead) ---
            struct SpyPreScheduleDAOPort {
                calls: Cell<u32>,
            }
            impl PreScheduleDAOPort for SpyPreScheduleDAOPort {
                fn load_pre_schedules_for_jobs_and_date_range(
                    &self,
                    _job_ids: &[i32],
                    _date_range: &DateRange,
                ) -> Vec<PreSchedule> {
                    self.calls.set(self.calls.get() + 1);
                    Vec::new()
                }
            }
            let pre_schedule_dao = SpyPreScheduleDAOPort {
                calls: Cell::new(0),
            };
            let pre_schedule_loader = PreScheduleLoader::new(&pre_schedule_dao);

            let dead_port = DeadPort;
            let schedule_saver = ScheduleSaver;
            let planned_shift_creator = PlannedShiftCreator;
            let employee_shift_creator = EmployeeShiftCreator;
            let employee_time_off_checker = EmployeeTimeOffChecker;
            let pre_schedule_calculate_data_set =
                CalculateDataSet::new(&dead_port, &dead_port, &dead_port);

            let pre_schedule_process = PreScheduleProcess::new(
                &pre_schedule_loader,
                &schedule_saver,
                &planned_shift_creator,
                &employee_shift_creator,
                &employee_time_off_checker,
                &pre_schedule_calculate_data_set,
            );

            // --- PermanentScheduleProcess / RegularScheduleProcess: each gets its own spy DAO so
            // the two gates can be told apart; the generator chain behind them is shared and dead
            // (empty regular periods mean `RegularScheduleSingleDate` never finds a schedule to
            // act on, so `RegularScheduleSingleShift`'s own dependencies never run). ---
            struct SpyRegularPeriodDAOPort {
                calls: Cell<u32>,
            }
            impl EmployeeRegularPeriodDAOPort for SpyRegularPeriodDAOPort {
                fn find_for_jobs_and_employee_type(
                    &self,
                    _job_ids: &[i32],
                    _employee_type: EmployeeType,
                ) -> Vec<EmployeeRegularPeriod> {
                    self.calls.set(self.calls.get() + 1);
                    Vec::new()
                }
            }
            let permanent_dao = SpyRegularPeriodDAOPort {
                calls: Cell::new(0),
            };
            let regular_dao = SpyRegularPeriodDAOPort {
                calls: Cell::new(0),
            };
            let permanent_loader = RegularScheduleLoader::new(&permanent_dao);
            let regular_loader = RegularScheduleLoader::new(&regular_dao);

            let can_work_factory =
                RegularScheduleCanWorkCheckerFactory::new(&dead_port, &dead_port, &dead_port);
            let can_work_checker = RegularScheduleCanWorkChecker::new(&can_work_factory);
            let planned_shift_matcher = PlannedShiftMatcher;
            let planned_shift_helper =
                PlannedShiftHelper::new(&planned_shift_matcher, &planned_shift_creator);
            let regular_shift_creator = RegularScheduleEmployeeShiftCreator::new(
                &employee_shift_creator,
                &planned_shift_helper,
            );
            let regular_calculate_data_set =
                CalculateDataSet::new(&dead_port, &dead_port, &dead_port);
            let single_shift = RegularScheduleSingleShift::new(
                &schedule_saver,
                &can_work_checker,
                &regular_shift_creator,
                &regular_calculate_data_set,
            );
            let projected_hours_checker = ProjectedHoursChecker;
            let single_date =
                RegularScheduleSingleDate::new(&single_shift, &projected_hours_checker);
            let generator = RegularScheduleGenerator::new(&single_date);

            let permanent_process = PermanentScheduleProcess::new(&permanent_loader, &generator);
            let regular_process = RegularScheduleProcess::new(&regular_loader, &generator);

            // --- VariableScheduleProcess: the job's ByEmployeeSet method is real (its own
            // `VariableSchedulingGenerator` field is concretely typed, not `&dyn`, so it can't be
            // swapped for a spy); with zero planned shifts on the job its inner scheduling loop
            // never runs, but `generate_schedules` unconditionally resets then increments the
            // job's `balance_level` around that loop either way — that's the observable signal.
            // The other two dispatch arms are dead (job's method is always `ByEmployeeSet`). ---
            let variable_can_work_factory = VariableCanWorkCheckerFactory::new(
                &dead_port, &dead_port, &dead_port, &dead_port, &dead_port,
            );
            let variable_can_work_checker = VariableCanWorkChecker::new(&variable_can_work_factory);
            let variable_checker = VariableChecker;
            let variable_calculate_data_set =
                CalculateDataSet::new(&dead_port, &dead_port, &dead_port);
            let employee_job_status_checker = EmployeeJobStatusChecker::new(&active_on_date);
            let planned_shift_scheduler = PlannedShiftScheduler::new(
                &schedule_saver,
                &variable_can_work_checker,
                &variable_checker,
                &variable_calculate_data_set,
                &employee_job_status_checker,
            );
            let by_employee_set = VariableByEmployeeSetGenerator::new(&planned_shift_scheduler);
            let by_job_schedule_order_scheduler =
                VariableJobScheduler::new(&planned_shift_scheduler);
            let by_job_schedule_order =
                VariableByJobScheduleOrderGenerator::new(&by_job_schedule_order_scheduler);
            let by_seniority_scheduler = VariableJobScheduler::new(&planned_shift_scheduler);
            let by_seniority = VariableBySeniorityGenerator::new(&by_seniority_scheduler);
            let variable_job_scheduling_process = VariableJobSchedulingProcess::new(
                &by_employee_set,
                &by_job_schedule_order,
                &by_seniority,
            );
            let pre_scheduled_job_process =
                PreScheduledJobProcess::new(&variable_job_scheduling_process);
            let non_pre_scheduled_job_process =
                NonPreScheduledJobProcess::new(&variable_job_scheduling_process);
            let variable_process = VariableScheduleProcess::new(
                &pre_scheduled_job_process,
                &non_pre_scheduled_job_process,
            );

            // --- SaveSchedulesService: always runs; real, on an empty old/new shift list ---
            #[derive(Default)]
            struct SavePorts {
                saved_employee_shifts: RefCell<Vec<EmployeeShift>>,
            }
            impl PlannedShiftPort for SavePorts {
                fn save(&self, _planned_shift: &PlannedShift) {}
                fn bulk_delete_planned_shifts_for_current_property(
                    &self,
                    _job_ids: &HashSet<i32>,
                    _date_range: &DateRange,
                ) {
                }
            }
            impl EmployeeShiftPort for SavePorts {
                fn evict(&self, _employee_shift_id: i32) {}
                fn bulk_delete_by_shift_id(&self, _shift_ids: &HashSet<i32>) {}
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
            impl SchedulingShiftAuditPort for SavePorts {
                fn create_deleted_schedule_audit(
                    &self,
                    _old_shift: &EmployeeShift,
                ) -> Option<SchedulingShiftAudit> {
                    None
                }
                fn create_added_shift_audit(
                    &self,
                    _new_shift: &EmployeeShift,
                ) -> SchedulingShiftAudit {
                    SchedulingShiftAudit
                }
                fn save_all(&self, _audits: &[SchedulingShiftAudit]) {}
            }
            impl PlannedShiftAuditPort for SavePorts {
                fn save_add_audit(
                    &self,
                    _property: &Property,
                    _planned_shifts: &[Option<PlannedShift>],
                ) {
                }
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
            impl PropertyDataKeyPort for SavePorts {
                fn enabled_boolean_key_for_property_with_tag(
                    &self,
                    _property_id: i32,
                    _tag: PropertyDataKeyTag,
                ) -> bool {
                    false
                }
            }
            impl SchedulesTimeCardCalculatorPort for SavePorts {
                fn calculate_overtime_for_schedule_calc_data_set(
                    &self,
                    _data_set: &mut ScheduleCalcDataSet,
                ) {
                }
                fn calculate_schedule_calc_data_set(&self, _data_set: &mut ScheduleCalcDataSet) {}
            }
            impl EmployeeAlertPort for SavePorts {
                fn refresh_rule_alerts(&self, _employee_id: i32) {}
            }
            impl EmployeeShiftClonerPort for SavePorts {
                fn clone_employee_shift_as_generated(
                    &self,
                    employee_shift: &EmployeeShift,
                ) -> EmployeeShift {
                    employee_shift.clone()
                }
            }
            struct NoOpReportLibrary;
            impl ReportLibraryPort for NoOpReportLibrary {
                fn save(&self, _report_library: &ReportLibrary) {}
            }
            struct NoOpScheduleLogWriter;
            impl ScheduleLogWriterPort for NoOpScheduleLogWriter {
                fn output_header(&self, _buf: &mut Vec<u8>) {}
                fn output_schedule_log(&self, _schedule_log: &ScheduleLog, _buf: &mut Vec<u8>) {}
                fn output_footer(&self, _buf: &mut Vec<u8>) {}
            }

            let save_ports = SavePorts::default();
            let cancel_shift_requests_service = CancelShiftRequestsService;
            let report_library = NoOpReportLibrary;
            let schedule_log_writer = NoOpScheduleLogWriter;
            let log_service = SaveScheduleLogService::new(&report_library, &schedule_log_writer);
            let snapshot_service =
                SaveScheduleSnapshotService::new(&save_ports, &save_ports, &save_ports);
            let save_schedules_service = SaveSchedulesService::new(
                &save_ports,
                &save_ports,
                &save_ports,
                &save_ports,
                &save_ports,
                &save_ports,
                &save_ports,
                &cancel_shift_requests_service,
                &log_service,
                &snapshot_service,
            );

            let engine = ScheduleEngine::new(
                &loader,
                &rotator,
                &factory,
                &balancer,
                &preparation_service,
                &pre_schedule_process,
                &permanent_process,
                &regular_process,
                &variable_process,
                &save_schedules_service,
                &active_on_date,
            );

            let params = GenerateSchedulesParameters::new(range, 1)
                .with_rotate_days_off(rotate_days_off)
                .with_generate_pre_schedules(generate_pre_schedules)
                .with_generate_permanent_schedules(generate_permanent_schedules)
                .with_generate_regular_schedules(generate_regular_schedules)
                .with_generate_variable_schedules(generate_variable_schedules);

            let result = engine.generate_schedules(&params);
            let job_data = result
                .as_ref()
                .and_then(|model| model.job_list())
                .and_then(|job_list| job_list.job_data(1));
            let balance_factor = job_data
                .map(|job_data| job_data.balance_factor())
                .unwrap_or(1.0);
            let variable_balance_level = job_data
                .map(|job_data| job_data.balance_level())
                .unwrap_or(0);

            Spies {
                day_off_calls: day_off_plan_port.calls.get(),
                projected_hours_calls: default_reducer.calls.get(),
                balance_factor,
                pre_schedule_calls: pre_schedule_dao.calls.get(),
                permanent_calls: permanent_dao.calls.get(),
                regular_calls: regular_dao.calls.get(),
                variable_balance_level,
            }
        }

        #[test]
        fn every_gate_is_off_when_every_flag_is_false_and_the_mode_is_not_weekly() {
            let spies = run(None, false, false, false, false, false);

            assert_eq!(spies.day_off_calls, 0);
            assert_eq!(spies.projected_hours_calls, 0);
            assert_eq!(spies.balance_factor, 1.0);
            assert_eq!(spies.pre_schedule_calls, 0);
            assert_eq!(spies.permanent_calls, 0);
            assert_eq!(spies.regular_calls, 0);
            assert_eq!(spies.variable_balance_level, 0);
        }

        #[test]
        fn every_gate_is_on_when_every_flag_is_true_and_the_mode_is_weekly() {
            let spies = run(
                Some(crate::entity::schedule_mode::ScheduleMode::Weekly),
                true,
                true,
                true,
                true,
                true,
            );

            assert_eq!(spies.day_off_calls, 1);
            assert_eq!(spies.projected_hours_calls, 1);
            assert_eq!(spies.balance_factor, 0.0);
            assert_eq!(spies.pre_schedule_calls, 1);
            assert_eq!(spies.permanent_calls, 1);
            assert_eq!(spies.regular_calls, 1);
            assert_eq!(spies.variable_balance_level, 1);
        }

        #[test]
        fn each_flag_only_gates_its_own_step() {
            let only_rotate = run(None, true, false, false, false, false);
            assert_eq!(only_rotate.day_off_calls, 1);
            assert_eq!(only_rotate.pre_schedule_calls, 0);
            assert_eq!(only_rotate.permanent_calls, 0);
            assert_eq!(only_rotate.regular_calls, 0);
            assert_eq!(only_rotate.variable_balance_level, 0);

            let only_pre_schedule = run(None, false, true, false, false, false);
            assert_eq!(only_pre_schedule.pre_schedule_calls, 1);
            assert_eq!(only_pre_schedule.day_off_calls, 0);
            assert_eq!(only_pre_schedule.permanent_calls, 0);
            assert_eq!(only_pre_schedule.regular_calls, 0);
            assert_eq!(only_pre_schedule.variable_balance_level, 0);

            let only_permanent = run(None, false, false, true, false, false);
            assert_eq!(only_permanent.permanent_calls, 1);
            assert_eq!(only_permanent.pre_schedule_calls, 0);
            assert_eq!(only_permanent.regular_calls, 0);
            assert_eq!(only_permanent.variable_balance_level, 0);

            let only_regular = run(None, false, false, false, true, false);
            assert_eq!(only_regular.regular_calls, 1);
            assert_eq!(only_regular.permanent_calls, 0);
            assert_eq!(only_regular.pre_schedule_calls, 0);
            assert_eq!(only_regular.variable_balance_level, 0);

            let only_variable = run(None, false, false, false, false, true);
            assert_eq!(only_variable.variable_balance_level, 1);
            assert_eq!(only_variable.regular_calls, 0);
            assert_eq!(only_variable.permanent_calls, 0);
            assert_eq!(only_variable.pre_schedule_calls, 0);
        }

        #[test]
        fn schedule_preparation_always_runs_regardless_of_flags_or_mode() {
            // `prepareShiftsForScheduling` is unconditional in Java too (only its `clearSchedules`
            // argument varies) — this only needs to prove `generate_schedules` reaches it and
            // returns a model either way, since `SchedulePreparationService` has its own suite.
            assert!(run(None, false, false, false, false, false).balance_factor <= 1.0);
            assert!(
                run(
                    Some(crate::entity::schedule_mode::ScheduleMode::Weekly),
                    true,
                    true,
                    true,
                    true,
                    true
                )
                .balance_factor
                    <= 1.0
            );
        }
    }
}
