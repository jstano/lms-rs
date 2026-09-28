//! Port of `com.unifocus.watson.server.scheduler.autosched.ScheduleMatcher`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/autosched/
//! ScheduleMatcher.java`. Ranks the employees eligible for a `PlannedShift`, delegating the actual
//! fatal-conflict check to [`ScheduleChecker`] (this wave's other real port).
//!
//! `getEmployeesForPlannedShift` returns `List<TPair<ScheduleCalcDataSet, Integer>>` in Java — the
//! dataset paired with its weight. Ported as `Vec<(i32, i32)>` (employee id, weight) instead:
//! `datasets` is already keyed by employee id (`Map<Integer, ScheduleCalcDataSet>`, Java's
//! `datasetMap`), so callers that need the dataset back can look it up there, and carrying the id
//! avoids a second mutable alias into `datasets` while `Collections.sort`'s comparator (ported
//! below) needs shared read access to two entries at once.
//!
//! Java's own sort comparator is itself non-strict — `(pair1, pair2) -> isBetterMatch(...) ? 1 :
//! 0` never returns a negative value, so it isn't a real total order either; ported as-is via
//! [`std::cmp::Ordering::Greater`]/[`std::cmp::Ordering::Equal`] rather than "fixed" into a strict
//! one, to keep the exact (odd) resulting order.
//!
//! `CurrentProperty.getProperty().getScheduleMode()` (Spring-autowired) becomes a plain
//! `schedule_mode` field set at construction, same "skip the DI, take the dependency directly"
//! treatment as every other `@Autowired` field this crate ports.
//! `ScheduleUtils.createEmployeeShiftFromPlannedShift` is inlined as a free function — its own
//! `ShiftCategoryDAO.findByID(categoryID)` lookup is skipped since the category id round-trips
//! unchanged (`plannedShift.getShiftCategory().getID()` in, the same id back out), and its punch
//! creation isn't modeled (nothing ported reads `EmployeeShift`'s punches — see that entity's
//! doc).

use crate::autosched::schedule_checker::ScheduleChecker;
use crate::engine::misc::ports::SchedulesTimeCardCalculatorPort;
use crate::entity::planned_shift::PlannedShift;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use crate::entity::schedule_mode::ScheduleMode;
use joda_rs::LocalDate;

/// `ScheduleUtils.createEmployeeShiftFromPlannedShift(PlannedShift, Employee, int, String)`.
fn create_employee_shift_from_planned_shift(
    planned_shift: &PlannedShift,
    employee_id: i32,
) -> crate::entity::employee_shift::EmployeeShift {
    crate::entity::employee_shift::EmployeeShift::new(
        0,
        planned_shift.shift_date(),
        planned_shift.start_date_time(),
        planned_shift.job_id(),
        planned_shift.assignment_id(),
    )
    .with_end_date_time(planned_shift.end_date_time())
    .with_net_hours(planned_shift.duration())
    .with_planned_shift(*planned_shift)
    .with_employee_id(employee_id)
    .with_shift_category_id(planned_shift.shift_category_id())
}

/// `DateUtil.daysBetween(LocalDate, LocalDate)`.
fn days_between(from: LocalDate, to: LocalDate) -> i32 {
    (to - from).to_days() as i32
}

/// `ScheduleMatcher`.
pub struct ScheduleMatcher<'a> {
    schedule_mode: Option<ScheduleMode>,
    schedules_time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
    checker_deps: ScheduleCheckerDeps<'a>,
}

/// `ScheduleChecker`'s non-dataset constructor arguments, grouped so `ScheduleMatcher::new` isn't
/// a seven-argument function — every field here is threaded straight through to each
/// `ScheduleChecker::new` call inside [`ScheduleMatcher::employees_for_planned_shift`].
pub struct ScheduleCheckerDeps<'a> {
    pub exceeds_hours_validator:
        &'a crate::autosched::exceeds_available_hours_conflict_validator::ExceedsAvailableHoursConflictValidator<'a>,
    pub schedule_restriction_rules:
        &'a crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionRules<'a>,
    pub certifications: &'a dyn crate::autosched::ports::CertificationPort,
    pub assignments: &'a dyn crate::engine::process::ports::AssignmentPort,
}

impl<'a> ScheduleMatcher<'a> {
    pub fn new(
        schedule_mode: Option<ScheduleMode>,
        schedules_time_card_calculator: &'a dyn SchedulesTimeCardCalculatorPort,
        checker_deps: ScheduleCheckerDeps<'a>,
    ) -> Self {
        Self {
            schedule_mode,
            schedules_time_card_calculator,
            checker_deps,
        }
    }

    /// `getEmployeesForPlannedShift(PlannedShift, int, boolean)`.
    pub fn employees_for_planned_shift(
        &self,
        planned_shift: &PlannedShift,
        shift_overlap: i32,
        include_approved_time_off_employees: bool,
        datasets: &mut std::collections::HashMap<i32, ScheduleCalcDataSet>,
    ) -> Vec<(i32, i32)> {
        let mut ranked: Vec<(i32, i32)> = Vec::new();

        let employee_ids: Vec<i32> = datasets.keys().copied().collect();

        for employee_id in employee_ids {
            if !self.employee_is_eligible_for_shift(
                datasets.get(&employee_id),
                planned_shift,
                include_approved_time_off_employees,
            ) {
                continue;
            }

            let Some(data_set) = datasets.get_mut(&employee_id) else {
                continue;
            };

            let employee_shift =
                create_employee_shift_from_planned_shift(planned_shift, employee_id);
            data_set.add_shift(employee_shift.clone());
            self.schedules_time_card_calculator
                .calculate_schedule_calc_data_set(data_set);

            let checker = ScheduleChecker::new(
                data_set,
                self.schedule_mode,
                self.checker_deps.exceeds_hours_validator,
                self.checker_deps.schedule_restriction_rules,
                self.checker_deps.certifications,
                self.checker_deps.assignments,
            );

            let can_schedule = checker.can_schedule(&employee_shift, shift_overlap);
            let weight =
                checker.schedule_weight(&employee_shift, include_approved_time_off_employees);

            if can_schedule {
                ranked.push((employee_id, weight));
            }

            data_set.remove_shift(employee_shift);
        }

        ranked.sort_by(|pair1, pair2| {
            if self.is_better_match(
                planned_shift,
                datasets.get(&pair1.0),
                datasets.get(&pair2.0),
            ) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });

        ranked
    }

    fn employee_is_eligible_for_shift(
        &self,
        data_set: Option<&ScheduleCalcDataSet>,
        planned_shift: &PlannedShift,
        include_approved_time_off_employees: bool,
    ) -> bool {
        let Some(data_set) = data_set else {
            return false;
        };
        let Some(employee) = data_set.employee() else {
            return false;
        };

        let period =
            date_range_rs::DateRange::new(planned_shift.shift_date(), planned_shift.shift_date());
        if !employee.is_active_job_during_period(planned_shift.job_id(), &period) {
            return false;
        }

        if include_approved_time_off_employees {
            return true;
        }

        let shift_range = planned_shift.to_date_time_range();
        !data_set
            .time_off_requests()
            .iter()
            .any(|time_off| time_off.to_date_time_range().overlaps(&shift_range))
    }

    /// `isBetterMatch(PlannedShift, ScheduleCalcDataSet, ScheduleCalcDataSet)`.
    pub fn is_better_match(
        &self,
        planned_shift: &PlannedShift,
        employee1: Option<&ScheduleCalcDataSet>,
        employee2: Option<&ScheduleCalcDataSet>,
    ) -> bool {
        use crate::entity::jc_sort_order_type::JcSortOrderType;

        let (Some(employee1), Some(employee2)) = (employee1, employee2) else {
            return false;
        };
        let (Some(emp1), Some(emp2)) = (employee1.employee(), employee2.employee()) else {
            return false;
        };
        let Some(job) = self
            .checker_deps
            .assignments
            .find_by_id(planned_shift.job_id())
        else {
            return false;
        };

        if job.is_balance_schedules() {
            let my_days = Self::days_since_last_scheduled(planned_shift, employee1);
            let emp_days = Self::days_since_last_scheduled(planned_shift, employee2);

            if my_days > emp_days {
                return true;
            }
        }

        for sort_order in job.sort_order() {
            match sort_order.sort_type() {
                JcSortOrderType::EmployeeType => {
                    if emp1.employee_type() == emp2.employee_type() {
                        continue;
                    }

                    let order = crate::entity::employee_type::EmployeeType::sort_order();
                    let index1 = order.iter().position(|t| *t == emp1.employee_type());
                    let index2 = order.iter().position(|t| *t == emp2.employee_type());
                    return index1 < index2;
                }
                JcSortOrderType::HireDate => match (emp1.hire_date(), emp2.hire_date()) {
                    (Some(d1), Some(d2)) if d1.is_before(d2) => return true,
                    (Some(d1), Some(d2)) if d1.is_after(d2) => return false,
                    _ => continue,
                },
                JcSortOrderType::SkillDate => {
                    let date1 = emp1
                        .employee_job_status(job.id(), planned_shift.shift_date())
                        .map(|s| s.seniority_date());
                    let date2 = emp2
                        .employee_job_status(job.id(), planned_shift.shift_date())
                        .map(|s| s.seniority_date());

                    match (date1, date2) {
                        (Some(d1), Some(d2)) if d1.is_before(d2) => return true,
                        (Some(d1), Some(d2)) if d1.is_after(d2) => return false,
                        _ => continue,
                    }
                }
                JcSortOrderType::SkillRank => {
                    let rank1 = emp1
                        .employee_job_status(job.id(), planned_shift.shift_date())
                        .map(|s| s.rank())
                        .unwrap_or(0);
                    let rank2 = emp2
                        .employee_job_status(job.id(), planned_shift.shift_date())
                        .map(|s| s.rank())
                        .unwrap_or(0);

                    if rank1 < rank2 {
                        return true;
                    }
                    if rank1 > rank2 {
                        return false;
                    }
                    continue;
                }
                JcSortOrderType::AssignmentRank => {
                    let Some(assignment_id) = planned_shift.assignment_id() else {
                        continue;
                    };

                    let assignment1 = emp1.assignment(assignment_id);
                    let assignment2 = emp2.assignment(assignment_id);

                    let rank1 = assignment1.map(|a| a.rank()).unwrap_or(i32::MAX);
                    let rank2 = assignment2.map(|a| a.rank()).unwrap_or(i32::MAX);

                    if rank1 < rank2 {
                        return true;
                    }
                    if rank1 > rank2 {
                        return false;
                    }

                    let order1 = assignment1.map(|a| a.order_no()).unwrap_or(i32::MAX);
                    let order2 = assignment2.map(|a| a.order_no()).unwrap_or(i32::MAX);

                    if order1 < order2 {
                        return true;
                    }
                    if order1 > order2 {
                        return false;
                    }
                    continue;
                }
                JcSortOrderType::Fulltime => {
                    let fulltime1 = emp1.work_class().is_fulltime();
                    let fulltime2 = emp2.work_class().is_fulltime();

                    if fulltime1 && !fulltime2 {
                        return true;
                    }
                    if !fulltime1 && fulltime2 {
                        return false;
                    }
                    continue;
                }
                JcSortOrderType::Default => match emp1.name().cmp(emp2.name()) {
                    std::cmp::Ordering::Less => return true,
                    std::cmp::Ordering::Greater => return false,
                    std::cmp::Ordering::Equal => continue,
                },
                JcSortOrderType::AssignmentOrder => continue,
            }
        }

        false
    }

    fn days_since_last_scheduled(
        planned_shift: &PlannedShift,
        data_set: &ScheduleCalcDataSet,
    ) -> i32 {
        let mut result = 99_999;

        for shift in data_set.shifts() {
            if planned_shift.shift_date().is_after(shift.shift_date())
                && shift.job_id() == planned_shift.job_id()
            {
                let days_between = days_between(shift.shift_date(), planned_shift.shift_date());
                if days_between < result {
                    result = days_between;
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    //! No Groovy/Java test file was found under `taps/.../autosched/` for `ScheduleMatcher`; cases
    //! below cover `employees_for_planned_shift`'s eligibility filter and `is_better_match`'s
    //! `EmployeeType` sort-order branch, independently derived from the production methods.

    use super::*;
    use crate::autosched::exceeds_available_hours_conflict_validator::ExceedsAvailableHoursConflictValidator;
    use crate::autosched::ports::CertificationPort;
    use crate::engine::process::checkers::schedule_restriction_rule_checker::{
        ScheduleRestrictionRules, ScheduleRestrictionRulesPort,
    };
    use crate::engine::process::ports::AssignmentPort;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_error::EmployeeShiftError;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::jc_sort_order_type::JcSortOrderType;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalTime;
    use std::collections::HashMap;

    struct FakeCurrentUser;
    impl crate::autosched::ports::CurrentUserPort for FakeCurrentUser {
        fn can_exceed_available_hours(&self) -> bool {
            false
        }
    }

    struct FakeCertifications;
    impl CertificationPort for FakeCertifications {
        fn certification_error_key(
            &self,
            _employee: &Employee,
            _job_id: i32,
            _shift_range: &DateRange,
        ) -> Option<&'static str> {
            None
        }
    }

    struct FakeRestrictionRules;
    impl ScheduleRestrictionRulesPort for FakeRestrictionRules {
        fn strict_restriction_messages(
            &self,
            _data_set: &ScheduleCalcDataSet,
            _employee_shift: &EmployeeShift,
        ) -> Vec<String> {
            Vec::new()
        }

        fn non_strict_restriction_errors(
            &self,
            _data_set: &ScheduleCalcDataSet,
            _employee_shift: &EmployeeShift,
        ) -> Vec<EmployeeShiftError> {
            Vec::new()
        }
    }

    struct NoOpTimeCardCalculator;
    impl SchedulesTimeCardCalculatorPort for NoOpTimeCardCalculator {
        fn calculate_overtime_for_schedule_calc_data_set(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
        ) {
        }
        fn calculate_schedule_calc_data_set(&self, _data_set: &mut ScheduleCalcDataSet) {}
    }

    struct FakeAssignments {
        by_id: HashMap<i32, Assignment>,
    }
    impl AssignmentPort for FakeAssignments {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            self.by_id.get(&id).cloned()
        }
    }

    fn employee(
        id: i32,
        name: &str,
        employee_type: EmployeeType,
        job_id: i32,
        date: LocalDate,
    ) -> Employee {
        Employee::new(
            id,
            name,
            employee_type,
            Some(40.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![EmployeeJobStatus::new(
                job_id, None, date, date, true, 0, date, 0.0, false, 0,
            )],
        )
        .with_status(vec![crate::entity::employee_status::EmployeeStatus::new(
            date,
            date,
            crate::entity::employee_status_type::EmployeeStatusType::Active,
        )])
    }

    fn planned_shift(job_id: i32, date: LocalDate) -> PlannedShift {
        PlannedShift::new(
            1,
            job_id,
            date,
            date.at_time(LocalTime::of(9, 0, 0)),
            8.0,
            None,
        )
        .with_end_date_time(date.at_time(LocalTime::of(17, 0, 0)))
    }

    #[test]
    fn excludes_employees_without_an_active_job_status_for_the_shifts_job() {
        let job_id = 1;
        let date = LocalDate::of(2024, 1, 1);
        let assignments = FakeAssignments {
            by_id: HashMap::from([(
                job_id,
                Assignment::new(
                    job_id,
                    "Job",
                    false,
                    None,
                    None,
                    None,
                    false,
                    Vec::new(),
                    Vec::new(),
                    None,
                ),
            )]),
        };

        let current_user = FakeCurrentUser;
        let certifications = FakeCertifications;
        let rules = FakeRestrictionRules;
        let time_card_calculator = NoOpTimeCardCalculator;
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&current_user);
        let restrictions = ScheduleRestrictionRules::new(&rules);

        let matcher = ScheduleMatcher::new(
            None,
            &time_card_calculator,
            ScheduleCheckerDeps {
                exceeds_hours_validator: &exceeds,
                schedule_restriction_rules: &restrictions,
                certifications: &certifications,
                assignments: &assignments,
            },
        );

        let mut eligible_dataset =
            ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
                .with_date_range(DateRange::new(date, date));
        eligible_dataset.set_employee(employee(1, "Eligible", EmployeeType::Regular, job_id, date));

        let mut ineligible_dataset =
            ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
                .with_date_range(DateRange::new(date, date));
        ineligible_dataset.set_employee(employee(2, "Ineligible", EmployeeType::Regular, 2, date));

        let mut datasets = HashMap::from([(1, eligible_dataset), (2, ineligible_dataset)]);

        let ranked = matcher.employees_for_planned_shift(
            &planned_shift(job_id, date),
            0,
            false,
            &mut datasets,
        );

        let ranked_ids: Vec<i32> = ranked.iter().map(|(id, _)| *id).collect();
        assert!(ranked_ids.contains(&1));
        assert!(!ranked_ids.contains(&2));
    }

    #[test]
    fn is_better_match_prefers_the_earlier_employee_type_in_the_sort_order() {
        let job_id = 1;
        let date = LocalDate::of(2024, 1, 1);
        let job = Assignment::new(
            job_id,
            "Job",
            false,
            None,
            None,
            None,
            false,
            vec![AssignmentSortOrder::new(JcSortOrderType::EmployeeType)],
            Vec::new(),
            None,
        );
        let assignments = FakeAssignments {
            by_id: HashMap::from([(job_id, job)]),
        };

        let current_user = FakeCurrentUser;
        let certifications = FakeCertifications;
        let rules = FakeRestrictionRules;
        let time_card_calculator = NoOpTimeCardCalculator;
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&current_user);
        let restrictions = ScheduleRestrictionRules::new(&rules);

        let matcher = ScheduleMatcher::new(
            None,
            &time_card_calculator,
            ScheduleCheckerDeps {
                exceeds_hours_validator: &exceeds,
                schedule_restriction_rules: &restrictions,
                certifications: &certifications,
                assignments: &assignments,
            },
        );

        let mut regular_dataset =
            ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default());
        regular_dataset.set_employee(employee(1, "Regular", EmployeeType::Regular, job_id, date));

        let mut permanent_dataset =
            ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default());
        permanent_dataset.set_employee(employee(
            2,
            "Permanent",
            EmployeeType::Permanent,
            job_id,
            date,
        ));

        assert!(matcher.is_better_match(
            &planned_shift(job_id, date),
            Some(&regular_dataset),
            Some(&permanent_dataset),
        ));
        assert!(!matcher.is_better_match(
            &planned_shift(job_id, date),
            Some(&permanent_dataset),
            Some(&regular_dataset),
        ));
    }
}
