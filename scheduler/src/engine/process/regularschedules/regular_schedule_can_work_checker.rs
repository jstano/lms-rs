//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleCanWorkChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleCanWorkChecker.java`. Two deliberate divergences from the
//! literal Java signature/body:
//!
//! **Takes `&mut EmployeeData` directly, rather than re-looking it up.** Java's
//! `canEmployeeWorkShift(ScheduleModel, EmployeeShift)` does its own
//! `scheduleModel.getEmployeeList().getEmployeeData(...)` lookup, separate from the one
//! `RegularScheduleSingleShift` already did for `storeOvertimeAddShiftAndCalculate` — free in
//! Java's shared object graph, not in Rust. This port takes the already-taken `EmployeeData`
//! straight from `RegularScheduleSingleShift`'s one take/reinsert scope (`EmployeeList::
//! take_employee_data`), the same idiom `PreScheduleProcess` established (`PARITY_AUDIT.md`
//! finding 30).
//!
//! **Clones the resolved `JobData` before running the checker list.** `EmployeeAvailabilityChecker`
//! needs a `&JobData` for the duration of the check, but this method also needs `&mut
//! ScheduleModel` (every checker can write a note into the model's log on failure) — Rust can't
//! alias a `&JobData` borrowed out of `schedule_model` with `&mut ScheduleModel` at the same time.
//! No checker in this list mutates the job's scheduled data (only `ScheduleSaver`, downstream,
//! after every check passes, does that), so an owned clone is behaviorally identical to Java's
//! live reference and resolves the conflict without a take/reinsert dance.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::regularschedules::regular_schedule_can_work_checker_factory::RegularScheduleCanWorkCheckerFactory;
use crate::entity::employee_shift::EmployeeShift;

/// `RegularScheduleCanWorkChecker`.
pub struct RegularScheduleCanWorkChecker<'a> {
    factory: &'a RegularScheduleCanWorkCheckerFactory<'a>,
}

impl<'a> RegularScheduleCanWorkChecker<'a> {
    pub fn new(factory: &'a RegularScheduleCanWorkCheckerFactory<'a>) -> Self {
        Self { factory }
    }

    /// `canEmployeeWorkShift(ScheduleModel, EmployeeShift)` — see module doc for the
    /// `EmployeeData`/`JobData` signature divergences.
    pub fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let job_data = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(employee_shift.job_id()))
            .cloned();

        let Some(job_data) = job_data else {
            return false;
        };

        for checker in self.factory.can_work_checkers(&job_data) {
            if !checker.can_employee_work_shift(schedule_model, employee_data, employee_shift) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::process::checkers::employee_certifications_checker::EmployeeCertificationPort;
    use crate::engine::process::checkers::schedule_restriction_rule_checker::{
        ScheduleRestrictionCheckResult, ScheduleRestrictionRulePort,
    };
    use crate::engine::process::ports::AssignmentPort;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_time_off::EmployeeTimeOff;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use joda_rs::{LocalDate, LocalDateTime};
    use std::cell::Cell;

    struct NoAssignmentsPort;
    impl AssignmentPort for NoAssignmentsPort {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }

    /// Records whether it was ever called — the two checkers ordered last
    /// (`ScheduleRestrictionRuleChecker`, `EmployeeCertificationsChecker`) should never run once
    /// an earlier checker (`EmployeeTimeOffChecker`, second in the fixed order) has already
    /// failed. Same "spy with a `Cell<bool>`" treatment as `PARITY_AUDIT.md` finding 26.
    struct SpyRestrictionsPort {
        called: Cell<bool>,
    }
    impl ScheduleRestrictionRulePort for SpyRestrictionsPort {
        fn check(
            &self,
            _employee_data: &EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> ScheduleRestrictionCheckResult {
            self.called.set(true);
            ScheduleRestrictionCheckResult {
                ok: true,
                message: None,
            }
        }
    }

    struct SpyCertificationsPort {
        called: Cell<bool>,
    }
    impl EmployeeCertificationPort for SpyCertificationsPort {
        fn is_certified(
            &self,
            _employee_data: &EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> bool {
            self.called.set(true);
            true
        }
    }

    fn job() -> Assignment {
        Assignment::new(
            1,
            "Job 1",
            false,
            None,
            None,
            None,
            false,
            Vec::<AssignmentSortOrder>::new(),
            Vec::new(),
            None,
        )
    }

    fn employee_with_conflicting_time_off() -> (Employee, ScheduleCalcDataSet) {
        let start = LocalDateTime::of(2024, 1, 1, 9, 0, 0);
        let end = LocalDateTime::of(2024, 1, 1, 17, 0, 0);
        let data_set = ScheduleCalcDataSet::new(
            Vec::new(),
            vec![EmployeeTimeOff::new(start, end)],
            Default::default(),
        );
        let employee = Employee::new(
            1,
            "Employee 1",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
        (employee, data_set)
    }

    #[test]
    fn short_circuits_before_the_last_two_checkers_when_an_earlier_one_fails() {
        let range =
            date_range_rs::DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 1));
        let property = crate::entity::property::Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);
        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job()));
        schedule_model.set_job_list(job_list);

        let (employee, data_set) = employee_with_conflicting_time_off();
        let mut employee_data = EmployeeData::new(employee, data_set, 0);

        let employee_shift = EmployeeShift::new(
            0,
            LocalDate::of(2024, 1, 1),
            LocalDateTime::of(2024, 1, 1, 9, 0, 0),
            1,
            None,
        )
        .with_end_date_time(LocalDateTime::of(2024, 1, 1, 17, 0, 0));

        let assignments = NoAssignmentsPort;
        let restrictions = SpyRestrictionsPort {
            called: Cell::new(false),
        };
        let certifications = SpyCertificationsPort {
            called: Cell::new(false),
        };
        let factory =
            RegularScheduleCanWorkCheckerFactory::new(&assignments, &restrictions, &certifications);
        let checker = RegularScheduleCanWorkChecker::new(&factory);

        let can_work = checker.can_employee_work_shift(
            &mut schedule_model,
            &mut employee_data,
            &employee_shift,
        );

        assert!(!can_work);
        assert!(!restrictions.called.get());
        assert!(!certifications.called.get());
    }
}
