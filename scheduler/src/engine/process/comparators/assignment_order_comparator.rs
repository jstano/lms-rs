//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! AssignmentOrderComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! AssignmentOrderComparator.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `AssignmentOrderComparator`.
pub struct AssignmentOrderComparator;

impl SeniorityComparator for AssignmentOrderComparator {
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

        let order_no = |employee_data: &EmployeeData| -> i32 {
            employee_data
                .employee()
                .assignment(assignment.id())
                .filter(|ea| ea.is_active())
                .map_or(i32::MAX, |ea| ea.order_no())
        };

        order_no(employee_data1).cmp(&order_no(employee_data2))
    }
}
