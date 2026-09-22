//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.SeniorityComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! SeniorityComparator.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// One tie-break rule in a job's seniority sort order. `SeniorityComparator`.
pub trait SeniorityComparator {
    /// `compare(Assignment, Assignment, LocalDate, EmployeeData, EmployeeData)`.
    fn compare(
        &self,
        job: &Assignment,
        assignment: Option<&Assignment>,
        shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering;
}
