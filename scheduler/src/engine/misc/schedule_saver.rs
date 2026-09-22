//! Port of `com.unifocus.watson.server.scheduler.engine.misc.ScheduleSaver`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! ScheduleSaver.java`. Java reads `employeeShift.getPlannedShift()` unconditionally and would
//! NPE if it were `null`; every call site ported so far always sets one
//! (`EmployeeShiftCreator::create_shift` always calls `with_planned_shift`), so this stays a safe
//! no-op on `None` instead — same "flag the NPE-shaped divergence rather than translate it
//! literally" treatment as `PARITY_AUDIT.md` finding 19.

use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee_shift::EmployeeShift;

/// `ScheduleSaver`.
pub struct ScheduleSaver;

impl ScheduleSaver {
    /// `saveSchedule(ScheduleModel, EmployeeShift)`.
    pub fn save_schedule(&self, schedule_model: &mut ScheduleModel, employee_shift: EmployeeShift) {
        let job_id = employee_shift.job_id();
        let shift_date = employee_shift.shift_date();
        let net_hours = employee_shift.net_hours();
        let planned_shift = employee_shift.planned_shift();

        if let Some(job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.job_data_mut(job_id))
        {
            job_data
                .scheduled_hours_mut()
                .add_hours_to_date(shift_date, net_hours);
        }

        schedule_model
            .new_shift_list_mut()
            .add_employee_shift(employee_shift);

        if let Some(planned_shift) = planned_shift {
            if planned_shift.id() == 0 {
                schedule_model
                    .new_shift_list_mut()
                    .add_planned_shift(Some(planned_shift));
            } else if let Some(job_data) = schedule_model
                .job_list_mut()
                .and_then(|job_list| job_list.job_data_mut(job_id))
            {
                job_data.remove_planned_shift(planned_shift);
            }
        }
    }
}
