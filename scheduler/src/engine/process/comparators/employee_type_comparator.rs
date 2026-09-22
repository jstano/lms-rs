//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! EmployeeTypeComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! EmployeeTypeComparator.java`. See `EmployeeType::sort_order`'s doc — the tie-break order
//! itself is a placeholder pending the real Java enum.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use crate::entity::employee_type::EmployeeType;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `EmployeeTypeComparator`.
pub struct EmployeeTypeComparator;

impl SeniorityComparator for EmployeeTypeComparator {
    fn compare(
        &self,
        _job: &Assignment,
        _assignment: Option<&Assignment>,
        _shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        let type1 = employee_data1.employee().employee_type();
        let type2 = employee_data2.employee().employee_type();

        if type1 == type2 {
            return Ordering::Equal;
        }

        let sort_order = EmployeeType::sort_order();
        let index_of = |t: EmployeeType| {
            sort_order
                .iter()
                .position(|&o| o == t)
                .unwrap_or(sort_order.len())
        };

        index_of(type1).cmp(&index_of(type2))
    }
}
