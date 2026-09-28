//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.VariableChecker`.
//!
//! Ground truth: `taps/.../process/variable/VariableChecker.java`.
//!
//! Java's `PlannedShift.getJob()` returns the live `Assignment` entity; this crate's
//! `PlannedShift` only carries `job_id` (see its module doc), so `job: &Assignment` is threaded
//! in as an explicit parameter instead (the caller, `PlannedShiftScheduler`, already has the
//! resolved `JobData`/`Assignment` on hand).
//!
//! `compareEmployees` always passes `None` for the `assignment` parameter `SeniorityComparator`s
//! take (Java passes `plannedShift.getAssignment()`) — `PlannedShift` here only carries
//! `assignment_id: Option<i32>`, and nothing in this crate resolves an arbitrary assignment id
//! back to a full `Assignment` (`JobList` only maps *job* ids to `Assignment`s).
//! `AssignmentOrderComparator`/`AssignmentRankComparator` already treat a `None` assignment as an
//! automatic tie (see their own files), so this only silently skips those two tie-breakers —
//! every other `JcSortOrderType` variant is unaffected. Flagging as a real divergence: revisit if
//! an assignment-id-to-`Assignment` lookup gets built for another caller.
//!
//! `compareJobScheduleOrders` panics (via `.expect`) if either employee has no
//! `EmployeeJobStatus` for the shift's job/date — same as Java's unchecked NPE. Every real call
//! site (`PlannedShiftScheduler`) only reaches this after `EmployeeJobStatusChecker` has already
//! confirmed both employees have one, so this is never hit in practice, matching Java.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::process::comparators::seniority_comparator_factory::SeniorityComparatorFactory;
use crate::entity::assignment::Assignment;
use crate::entity::planned_shift::PlannedShift;
use std::cmp::Ordering;

/// `VariableChecker`.
#[derive(Debug, Default, Clone, Copy)]
pub struct VariableChecker;

impl VariableChecker {
    /// `isEmployeeBetter(PlannedShift, EmployeeData, EmployeeData)`.
    pub fn is_employee_better(
        &self,
        job: &Assignment,
        planned_shift: &PlannedShift,
        employee_to_check: &EmployeeData,
        current_best_employee: &EmployeeData,
    ) -> bool {
        let result = self.compare_job_schedule_orders(
            planned_shift,
            employee_to_check,
            current_best_employee,
        );

        if result < 0 {
            return true;
        } else if result > 0 {
            return false;
        }

        self.compare_employees(job, planned_shift, employee_to_check, current_best_employee) < 0
    }

    fn compare_job_schedule_orders(
        &self,
        planned_shift: &PlannedShift,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> i32 {
        let status1 = employee_data1
            .employee()
            .employee_job_status(planned_shift.job_id(), planned_shift.shift_date())
            .expect("compareJobScheduleOrders requires an EmployeeJobStatus for employee 1");
        let status2 = employee_data2
            .employee()
            .employee_job_status(planned_shift.job_id(), planned_shift.shift_date())
            .expect("compareJobScheduleOrders requires an EmployeeJobStatus for employee 2");

        if status1.is_home() && !status2.is_home() {
            return -1;
        }

        if !status1.is_home() && status2.is_home() {
            return 1;
        }

        status1.schedule_order() - status2.schedule_order()
    }

    fn compare_employees(
        &self,
        job: &Assignment,
        _planned_shift: &PlannedShift,
        employee_data1: &EmployeeData,
        employee_data2: &EmployeeData,
    ) -> i32 {
        for sort_order in job.sort_order() {
            let comparator = SeniorityComparatorFactory::comparator(sort_order.sort_type());

            let result = comparator.compare(
                job,
                None,
                _planned_shift.shift_date(),
                employee_data1,
                employee_data2,
            );

            if result != Ordering::Equal {
                return match result {
                    Ordering::Less => -1,
                    Ordering::Greater => 1,
                    Ordering::Equal => 0,
                };
            }
        }

        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn job() -> Assignment {
        Assignment::new(
            500,
            "job",
            false,
            None,
            None,
            None,
            false,
            vec![],
            vec![],
            None,
        )
    }

    fn shift() -> PlannedShift {
        PlannedShift::new(
            1,
            500,
            LocalDate::of(2024, 1, 1),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            8.0,
            None,
        )
    }

    fn employee_data(is_home: bool, schedule_order: i32) -> EmployeeData {
        let status = EmployeeJobStatus::new(
            500,
            None,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            is_home,
            1,
            LocalDate::of(2020, 1, 1),
            0.0,
            false,
            schedule_order,
        );
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            vec![status],
        );
        EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0)
    }

    #[test]
    fn home_employee_beats_non_home_employee() {
        let home = employee_data(true, 5);
        let non_home = employee_data(false, 1);

        assert!(VariableChecker.is_employee_better(&job(), &shift(), &home, &non_home));
        assert!(!VariableChecker.is_employee_better(&job(), &shift(), &non_home, &home));
    }

    #[test]
    fn lower_schedule_order_wins_when_both_home() {
        let better = employee_data(true, 1);
        let worse = employee_data(true, 5);

        assert!(VariableChecker.is_employee_better(&job(), &shift(), &better, &worse));
        assert!(!VariableChecker.is_employee_better(&job(), &shift(), &worse, &better));
    }

    #[test]
    fn ties_fall_through_to_no_op_seniority_chain_when_job_has_no_sort_order() {
        let data1 = employee_data(true, 1);
        let data2 = employee_data(true, 1);

        assert!(!VariableChecker.is_employee_better(&job(), &shift(), &data1, &data2));
    }
}
