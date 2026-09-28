//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.VariableCanWorkChecker`.
//!
//! Ground truth: `taps/.../process/variable/VariableCanWorkChecker.java`. Same
//! `&mut EmployeeData`-taken-directly / `JobData`-cloned divergences as
//! `RegularScheduleCanWorkChecker` (see its module doc) — same borrow-conflict shape, second
//! occurrence.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::variable::variable_can_work_checker_factory::VariableCanWorkCheckerFactory;
use crate::entity::employee_shift::EmployeeShift;

/// `VariableCanWorkChecker`.
pub struct VariableCanWorkChecker<'a> {
    factory: &'a VariableCanWorkCheckerFactory<'a>,
}

impl<'a> VariableCanWorkChecker<'a> {
    pub fn new(factory: &'a VariableCanWorkCheckerFactory<'a>) -> Self {
        Self { factory }
    }

    /// `canEmployeeWorkShift(ScheduleModel, EmployeeData, EmployeeShift)`.
    pub fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let job_data = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(employee_shift.job_id()))
            .cloned();

        let Some(job_data) = job_data else {
            return false;
        };

        for checker in self.factory.can_work_checkers(&job_data) {
            if !checker.can_employee_work_shift(schedule_model, employee_data, employee_shift) {
                return false;
            }
        }

        true
    }
}
