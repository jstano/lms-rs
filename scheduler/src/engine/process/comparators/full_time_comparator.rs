//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.FullTimeComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! FullTimeComparator.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `FullTimeComparator`. Full-time employees sort before part-time.
pub struct FullTimeComparator;

impl SeniorityComparator for FullTimeComparator {
    fn compare(
        &self,
        _job: &Assignment,
        _assignment: Option<&Assignment>,
        _shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        let full_time1 = employee_data1.employee().work_class().is_fulltime();
        let full_time2 = employee_data2.employee().work_class().is_fulltime();

        // Java: full_time1 sorts first (-1), full_time2-only sorts first (1) — i.e. descending
        // on "is full time".
        full_time2.cmp(&full_time1)
    }
}
