//! Port of `com.unifocus.watson.server.scheduler.engine.process.PermanentScheduleProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! PermanentScheduleProcess.java`. `ScheduleEngine`'s pipeline step 7 — `PLAN_SCHEDULER.md`. Same
//! "`scheduleModel.getProgress().setMessage(...)` isn't ported" treatment as `PreScheduleProcess`
//! (`ScheduleModel::progress` isn't modeled yet — see that type's doc).
//!
//! This class and `RegularScheduleProcess` are identical apart from the `EmployeeType` they load
//! (`PERMANENT`/`REGULAR`) — both entry points, plus the whole `process/regularschedules/`
//! machinery they share, landed together in one wave; see `PARITY_AUDIT.md`'s merged step 7-8
//! entry.

use crate::engine::io::regular_schedule_loader::RegularScheduleLoader;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
use crate::engine::process::regularschedules::regular_schedule_generator::RegularScheduleGenerator;
use crate::entity::employee_type::EmployeeType;

/// `PermanentScheduleProcess`.
pub struct PermanentScheduleProcess<'a> {
    regular_schedule_loader: &'a RegularScheduleLoader<'a>,
    regular_schedule_generator: &'a RegularScheduleGenerator<'a>,
}

impl<'a> PermanentScheduleProcess<'a> {
    pub fn new(
        regular_schedule_loader: &'a RegularScheduleLoader<'a>,
        regular_schedule_generator: &'a RegularScheduleGenerator<'a>,
    ) -> Self {
        Self {
            regular_schedule_loader,
            regular_schedule_generator,
        }
    }

    /// `schedulePermanentEmployees(ScheduleModel)`.
    pub fn schedule_permanent_employees(
        &self,
        schedule_model: &mut ScheduleModel,
        active_on_date: &dyn EmployeeActiveOnDatePort,
    ) {
        let regular_schedules = self
            .regular_schedule_loader
            .load_regular_schedules(schedule_model, EmployeeType::Permanent);

        self.regular_schedule_generator.schedule_employees(
            schedule_model,
            &regular_schedules,
            active_on_date,
        );
    }
}
