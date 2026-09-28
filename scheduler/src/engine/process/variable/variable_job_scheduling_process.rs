//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.
//! VariableJobSchedulingProcess`.
//!
//! Ground truth: `taps/.../process/variable/VariableJobSchedulingProcess.java`. Java's lazily-
//! built `schedulingMethodMap` is a `match` over the three fixed `SchedulingMethod` variants
//! instead — same treatment as `ProjectedHoursReducerFactory` (`PARITY_AUDIT.md` finding 26): a
//! three-entry map needs no mutable-state-on-first-call cache.
//!
//! `scheduleModel.getProgress().setMessage(...)` is not ported — `Progress` isn't modeled in
//! this crate at all (see `ScheduleModel`'s own doc), so this is dropped rather than stubbed.
//!
//! `job.getSchedulingMethod()` returning `null` (an unmapped job) would NPE in Java's
//! `getSchedulingMethodMap().get(...)`; this crate models `scheduling_method` as `Option`, so an
//! unset method is skipped instead of panicking — flagging the divergence rather than
//! translating the implicit NPE literally.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::generators::{
    VariableByEmployeeSetGenerator, VariableByJobScheduleOrderGenerator,
    VariableBySeniorityGenerator, VariableSchedulingGenerator,
};
use crate::entity::scheduling_method::SchedulingMethod;

/// `VariableJobSchedulingProcess`.
pub struct VariableJobSchedulingProcess<'a> {
    variable_by_employee_set_generator: &'a VariableByEmployeeSetGenerator<'a>,
    variable_by_job_schedule_order_generator: &'a VariableByJobScheduleOrderGenerator<'a>,
    variable_by_seniority_generator: &'a VariableBySeniorityGenerator<'a>,
}

impl<'a> VariableJobSchedulingProcess<'a> {
    pub fn new(
        variable_by_employee_set_generator: &'a VariableByEmployeeSetGenerator<'a>,
        variable_by_job_schedule_order_generator: &'a VariableByJobScheduleOrderGenerator<'a>,
        variable_by_seniority_generator: &'a VariableBySeniorityGenerator<'a>,
    ) -> Self {
        Self {
            variable_by_employee_set_generator,
            variable_by_job_schedule_order_generator,
            variable_by_seniority_generator,
        }
    }

    /// `generateSchedulesForJobs(ScheduleModel, List<JobData>)`.
    pub fn generate_schedules_for_jobs(&self, schedule_model: &mut ScheduleModel, job_ids: &[i32]) {
        for &job_id in job_ids {
            let scheduling_method = schedule_model
                .job_list()
                .and_then(|job_list| job_list.job_data(job_id))
                .and_then(|job_data| job_data.job().scheduling_method());

            let Some(scheduling_method) = scheduling_method else {
                continue;
            };

            let generator: &dyn VariableSchedulingGenerator = match scheduling_method {
                SchedulingMethod::ByEmployeeSet => self.variable_by_employee_set_generator,
                SchedulingMethod::ByJobScheduleOrder => {
                    self.variable_by_job_schedule_order_generator
                }
                SchedulingMethod::BySeniority => self.variable_by_seniority_generator,
            };

            generator.generate_schedules(schedule_model, job_id);
        }
    }
}
