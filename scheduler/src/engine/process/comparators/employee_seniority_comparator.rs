//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! EmployeeSeniorityComparator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! EmployeeSeniorityComparator.java`. Java's `Comparator<EmployeeData>`; ported as a plain
//! struct with a `compare` method rather than `std::cmp::Ord`/a closure, since it needs the job
//! and shift date alongside the two employees — the same shape callers already get from
//! `SeniorityComparator`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::job_data::JobData;
use crate::engine::process::comparators::seniority_comparator_factory::SeniorityComparatorFactory;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `EmployeeSeniorityComparator`.
pub struct EmployeeSeniorityComparator<'a> {
    job_data: &'a JobData,
    shift_date: LocalDate,
}

impl<'a> EmployeeSeniorityComparator<'a> {
    pub fn new(job_data: &'a JobData, shift_date: LocalDate) -> Self {
        Self {
            job_data,
            shift_date,
        }
    }

    /// `compare(EmployeeData, EmployeeData)` — walks the job's sort-order chain, returning the
    /// first tie-breaker that doesn't call it a tie.
    pub fn compare(
        &self,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> Ordering {
        for sort_order in self.job_data.job().sort_order() {
            let comparator = SeniorityComparatorFactory::comparator(sort_order.sort_type());

            let result = comparator.compare(
                self.job_data.job(),
                None,
                self.shift_date,
                employee_data1,
                employee_data2,
            );

            if result != Ordering::Equal {
                return result;
            }
        }

        Ordering::Equal
    }
}
