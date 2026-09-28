//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators.
//! AbstractVariableSchedulingGenerator`.
//!
//! Ground truth: `taps/.../process/variable/generators/AbstractVariableSchedulingGenerator.java`.
//! Java's abstract base class becomes two free functions instead of a shared struct/trait
//! default method — this crate has no inheritance, and both helpers only need `&ScheduleModel`/
//! `&mut ScheduleModel` plus a `job_id`, not any state the concrete generators themselves own.
//! `VariableByJobScheduleOrderGenerator`/`VariableBySeniorityGenerator` call these directly;
//! `VariableByEmployeeSetGenerator` doesn't (it has its own `resetBalanceLevel`/
//! `incrementBalanceLevel` shape, no `performCleanupPasses` call).

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::variable_job_scheduler::VariableJobScheduler;

/// `getNumberJobLevels(ScheduleModel)`.
pub fn number_job_levels(schedule_model: &ScheduleModel) -> i32 {
    let property = schedule_model.property();

    if property.max_scheduler_passes() > 0 {
        property.max_scheduler_passes()
    } else {
        property.max_employee_skills()
    }
}

/// `performCleanupPasses(ScheduleModel, JobData, int)`. Takes `job_id: i32` rather than
/// `&JobData`/`&mut JobData` — every read/write re-fetches the job fresh from `schedule_model`
/// around `variable_job_scheduler.schedule_job`'s own mutating call, the same idiom
/// `RegularScheduleSingleDate` established for the identical `&mut ScheduleModel`-vs-`&JobData`
/// conflict (`PARITY_AUDIT.md` finding 33).
pub fn perform_cleanup_passes(
    variable_job_scheduler: &VariableJobScheduler,
    schedule_model: &mut ScheduleModel,
    job_id: i32,
    job_level: i32,
) {
    let starts_at_level_zero = schedule_model
        .job_list()
        .and_then(|job_list| job_list.job_data(job_id))
        .is_some_and(|job_data| job_data.balance_level() == 0);

    if !starts_at_level_zero {
        return;
    }

    let max_balance_levels = schedule_model.property().max_balance_levels();

    loop {
        let balance_level = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(job_id))
            .map(|job_data| job_data.balance_level());

        let Some(balance_level) = balance_level else {
            break;
        };

        if balance_level >= max_balance_levels {
            break;
        }

        variable_job_scheduler.schedule_job(schedule_model, job_id, job_level);

        if let Some(job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.job_data_mut(job_id))
        {
            job_data.increment_balance_level();
        }
    }

    if let Some(job_data) = schedule_model
        .job_list_mut()
        .and_then(|job_list| job_list.job_data_mut(job_id))
    {
        job_data.reset_balance_level();
    }
}
