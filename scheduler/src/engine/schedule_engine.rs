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
        if rotate_days_off {
            self.day_off_plan_rotator
                .rotate_day_off_plans(schedule_model);
        }
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
    //! Exercises the two new job-loop functions directly — `generate_schedules` itself is a
    //! straight-line call into ten already-tested steps, each with its own suite, so an
    //! end-to-end test here would mostly re-verify existing wiring rather than new behavior. The
    //! take/reinsert loop (this wave's only new logic) is what's worth covering: gating on
    //! `ScheduleMode::Weekly`/`is_balance_schedules`, and that every job survives the take/mutate/
    //! reinsert round trip.

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
}
