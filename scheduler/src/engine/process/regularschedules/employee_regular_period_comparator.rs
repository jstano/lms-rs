//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! EmployeeRegularPeriodComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/EmployeeRegularPeriodComparator.java`. A one-line delegation to the
//! already-ported `EmployeeSeniorityComparator` — Java's `Comparator<RegularSchedule>` reads each
//! side's `EmployeeData` off the `RegularSchedule` it holds directly; this port takes both
//! `EmployeeData`s as parameters instead, since `RegularSchedule` here only carries an
//! `employee_id` (see that type's doc) — the caller (`RegularSchedules::
//! regular_schedules_for_job_and_date`) already has to look each one up in `ScheduleModel` to sort
//! by it.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::job_data::JobData;
use crate::engine::process::comparators::employee_seniority_comparator::EmployeeSeniorityComparator;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `EmployeeRegularPeriodComparator`.
pub struct EmployeeRegularPeriodComparator<'a> {
    employee_seniority_comparator: EmployeeSeniorityComparator<'a>,
}

impl<'a> EmployeeRegularPeriodComparator<'a> {
    pub fn new(job_data: &'a JobData, shift_date: LocalDate) -> Self {
        Self {
            employee_seniority_comparator: EmployeeSeniorityComparator::new(job_data, shift_date),
        }
    }

    /// `compare(RegularSchedule, RegularSchedule)`.
    pub fn compare(
        &self,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        self.employee_seniority_comparator
            .compare(employee_data1, employee_data2)
    }
}
