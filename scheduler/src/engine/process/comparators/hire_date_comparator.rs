//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.HireDateComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! HireDateComparator.java`. `Employee::hire_date()` is `Option<LocalDate>` here — Java's
//! `getHireDate().compareTo(...)` would NPE on a null hire date; `Option`'s `Ord` (`None` sorts
//! first) is a safe default rather than a confirmed match for whatever Java does elsewhere with a
//! missing hire date.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `HireDateComparator`.
pub struct HireDateComparator;

impl SeniorityComparator for HireDateComparator {
    fn compare(
        &self,
        _job: &Assignment,
        _assignment: Option<&Assignment>,
        _shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        employee_data1
            .employee()
            .hire_date()
            .cmp(&employee_data2.employee().hire_date())
    }
}
