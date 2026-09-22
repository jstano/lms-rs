//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! SeniorityComparatorWithAssignment`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! SeniorityComparatorWithAssignment.java`. Same method signature as
//! [`SeniorityComparator`](super::seniority_comparator::SeniorityComparator) — no class in the
//! Phase 1 source implements this one instead of that one, so its purpose (versus the near-
//! identical `SeniorityComparator`) isn't confirmed. Ported for fidelity, not reused elsewhere.

use crate::engine::model::employee_data::EmployeeData;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `SeniorityComparatorWithAssignment`.
pub trait SeniorityComparatorWithAssignment {
    fn compare(
        &self,
        job: &Assignment,
        assignment: Option<&Assignment>,
        shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering;
}
