//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeAvailabilityChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeAvailabilityChecker.java`.
//!
//! Java resolves `jobData` via `scheduleModel.getJobList().getJobData(employeeShift.getJob())` —
//! this port takes `job_data` directly as a parameter instead, since `ScheduleModel::job_list`
//! only offers `&JobList`/`&mut` (not both at once) and the caller already has the `JobData` in
//! hand from whichever loop is driving the check.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeAvailabilityChecker`.
pub struct EmployeeAvailabilityChecker<'a> {
    job_data: &'a JobData,
}

impl<'a> EmployeeAvailabilityChecker<'a> {
    pub fn new(job_data: &'a JobData) -> Self {
        Self { job_data }
    }
}

impl CanWorkChecker for EmployeeAvailabilityChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let shift_date_time_range = employee_shift.to_date_time_range();
        let availability = employee_data.data_set().availability();

        let avail_periods = if self.job_data.balance_level() == 0 {
            availability.avail_periods_including_preferred()
        } else {
            availability.avail_periods_required_off_only()
        };

        for avail_period in avail_periods {
            if shift_date_time_range.overlaps_exclusive(&avail_period.to_date_time_range()) {
                set_notes_for_employee(
                    schedule_model,
                    employee_data.employee().id(),
                    "Planned shift overlaps with availability",
                );

                return false;
            }
        }

        true
    }
}
