//! Port of `com.unifocus.watson.server.scheduler.engine.misc.ProjectedHoursChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! ProjectedHoursChecker.java`. Both overloads are thin wrappers over the already-ported
//! `JobData::will_scheduled_hours_exceed_projected_hours` — only the `(JobData, LocalDate, f64)`
//! overload is reached by this wave's callers (`process/regularschedules/
//! RegularScheduleSingleDate`), but the `EmployeeShift` overload is ported too for 1:1 parity with
//! the Java class.

use crate::engine::model::job_data::JobData;
use crate::entity::employee_shift::EmployeeShift;
use joda_rs::LocalDate;

/// `ProjectedHoursChecker`.
pub struct ProjectedHoursChecker;

impl ProjectedHoursChecker {
    /// `willScheduledHoursExceedProjectedHours(EmployeeShift, JobData)`.
    pub fn will_scheduled_hours_exceed_projected_hours_for_shift(
        &self,
        employee_shift: &EmployeeShift,
        job_data: &JobData,
    ) -> bool {
        self.will_scheduled_hours_exceed_projected_hours(
            job_data,
            employee_shift.shift_date(),
            employee_shift.net_hours(),
        )
    }

    /// `willScheduledHoursExceedProjectedHours(JobData, LocalDate, double)`.
    pub fn will_scheduled_hours_exceed_projected_hours(
        &self,
        job_data: &JobData,
        shift_date: LocalDate,
        hours: f64,
    ) -> bool {
        job_data.will_scheduled_hours_exceed_projected_hours(shift_date, hours)
    }
}
