//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeCertificationsChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeCertificationsChecker.java`. Every line of Java's `canEmployeeWorkShift` depends on
//! `EmployeeCertificationValidator` (`watson/server/labor/employee`) and the shift's property —
//! neither modeled (`EmployeeShift` has no property back-ref yet, see that entity's doc). Ported
//! as a thin wrapper over [`EmployeeCertificationPort`] rather than skipped outright, following
//! the same "stub the deferred subsystem, keep the checker's shape" treatment as
//! `ScheduleRestrictionRuleChecker` (`PARITY_AUDIT.md` finding 10).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;

/// `new EmployeeCertificationValidator(employee).isNotCertified(shift.getProperty(),
/// shiftDateRange)`, negated to match this checker's "can work" sense.
pub trait EmployeeCertificationPort {
    fn is_certified(&self, employee_data: &EmployeeData, employee_shift: &EmployeeShift) -> bool;
}

/// `EmployeeCertificationsChecker`.
pub struct EmployeeCertificationsChecker<'a> {
    certifications: &'a dyn EmployeeCertificationPort,
}

impl<'a> EmployeeCertificationsChecker<'a> {
    pub fn new(certifications: &'a dyn EmployeeCertificationPort) -> Self {
        Self { certifications }
    }
}

impl CanWorkChecker for EmployeeCertificationsChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let can_work = self
            .certifications
            .is_certified(employee_data, employee_shift);

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "Employee is not certified",
            );
        }

        can_work
    }
}
