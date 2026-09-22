//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! DefaultSeniorityComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! DefaultSeniorityComparator.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::assignment::Assignment;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `DefaultSeniorityComparator`.
pub struct DefaultSeniorityComparator;

impl SeniorityComparator for DefaultSeniorityComparator {
    fn compare(
        &self,
        _job: &Assignment,
        _assignment: Option<&Assignment>,
        _shift_date: LocalDate,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        employee_data1
            .random()
            .cmp(&employee_data2.random())
            .then_with(|| employee_data1.weight().cmp(&employee_data2.weight()))
            .then_with(|| {
                employee_data1
                    .employee()
                    .name()
                    .to_lowercase()
                    .cmp(&employee_data2.employee().name().to_lowercase())
            })
    }
}
