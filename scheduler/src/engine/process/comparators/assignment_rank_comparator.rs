//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! AssignmentRankComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! AssignmentRankComparator.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `AssignmentRankComparator`.
pub struct AssignmentRankComparator;

impl SeniorityComparator for AssignmentRankComparator {
    fn compare(
        &self,
        _job: &Assignment,
        assignment: Option<&Assignment>,
        _shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        let Some(assignment) = assignment else {
            return Ordering::Equal;
        };

        let rank = |employee_data: &EmployeeData| -> i32 {
            employee_data
                .employee()
                .assignment(assignment.id())
                .filter(|ea| ea.is_active())
                .map_or(i32::MAX, |ea| ea.rank())
        };

        rank(employee_data1).cmp(&rank(employee_data2))
    }
}
