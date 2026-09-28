//! Port of `com.unifocus.watson.server.scheduler.engine.process.RegularScheduleProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! RegularScheduleProcess.java`. `ScheduleEngine`'s pipeline step 8 — `PLAN_SCHEDULER.md`.
//! Identical to `PermanentScheduleProcess` apart from the `EmployeeType` loaded — see that file's
//! doc for why both landed in this one wave.

use crate::engine::io::regular_schedule_loader::RegularScheduleLoader;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
use crate::engine::process::regularschedules::regular_schedule_generator::RegularScheduleGenerator;
use crate::entity::employee_type::EmployeeType;

/// `RegularScheduleProcess`.
pub struct RegularScheduleProcess<'a> {
    regular_schedule_loader: &'a RegularScheduleLoader<'a>,
    regular_schedule_generator: &'a RegularScheduleGenerator<'a>,
}

impl<'a> RegularScheduleProcess<'a> {
    pub fn new(
        regular_schedule_loader: &'a RegularScheduleLoader<'a>,
        regular_schedule_generator: &'a RegularScheduleGenerator<'a>,
    ) -> Self {
        Self {
            regular_schedule_loader,
            regular_schedule_generator,
        }
    }

    /// `scheduleRegularEmployees(ScheduleModel)`.
    pub fn schedule_regular_employees(
        &self,
        schedule_model: &mut ScheduleModel,
        active_on_date: &dyn EmployeeActiveOnDatePort,
    ) {
        let regular_schedules = self
            .regular_schedule_loader
            .load_regular_schedules(schedule_model, EmployeeType::Regular);

        self.regular_schedule_generator.schedule_employees(
            schedule_model,
            &regular_schedules,
            active_on_date,
        );
    }
}
