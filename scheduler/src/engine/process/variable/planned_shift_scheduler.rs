//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.PlannedShiftScheduler`.
//!
//! Ground truth: `taps/.../process/variable/PlannedShiftScheduler.java`. The hardest file in this
//! wave — Java's single mutable object graph lets it hold a running "best employee" candidate
//! across a loop over every unsorted employee, each iteration reading/mutating both
//! `ScheduleModel` and one `EmployeeData` at once. Ported using the same
//! `EmployeeList::take_employee_data`/`add_employee_data` idiom `PreScheduleProcess`/
//! `RegularScheduleSingleShift` established (`PARITY_AUDIT.md` finding 30), extended to hold at
//! most **one** `EmployeeData` taken out across iterations at a time — the current best. Every
//! employee that loses (fails a check, or is out-competed) gets reinserted immediately; the
//! winner is only reinserted once, after the final save, mirroring Java's exact "erase and
//! reinsert on every mutation" semantics without ever needing two `EmployeeData`s taken out at
//! once.
//!
//! `job` (`plannedShift.getJob()` in Java, a live `Assignment` reference) is resolved once up
//! front via `.cloned()` — same idiom as `RegularScheduleCanWorkChecker` (finding 34): nothing in
//! this loop mutates the job's sort order, only `ScheduleSaver` (downstream, after the winner is
//! chosen) touches the job's scheduled hours.
//!
//! `plannedShift.getJob().getProperty().getDefaultShiftCategory()` — the `shiftCategory`
//! fallback passed to `EmployeeShiftCreator::create_shift` — is not modeled; same deferred gap as
//! `PARITY_AUDIT.md` finding 37 (`Property.default_shift_category`), a third call site.

use crate::engine::misc::calculate_data_set::CalculateDataSet;
use crate::engine::misc::schedule_saver::ScheduleSaver;
use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::logging::employee_log_entry::EmployeeLogEntry;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeJobStatusChecker;
use crate::engine::process::variable::filters::EmployeeFilter;
use crate::engine::process::variable::variable_can_work_checker::VariableCanWorkChecker;
use crate::engine::process::variable::variable_checker::VariableChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::planned_shift::PlannedShift;

/// `EmployeeShiftFlagConstants.VARIABLE_FLAG`.
const VARIABLE_FLAG: i32 = 262_144;

/// `PlannedShiftScheduler`.
pub struct PlannedShiftScheduler<'a> {
    schedule_saver: &'a ScheduleSaver,
    variable_can_work_checker: &'a VariableCanWorkChecker<'a>,
    variable_checker: &'a VariableChecker,
    calculate_data_set: &'a CalculateDataSet<'a>,
    employee_job_status_checker: &'a EmployeeJobStatusChecker<'a>,
}

impl<'a> PlannedShiftScheduler<'a> {
    pub fn new(
        schedule_saver: &'a ScheduleSaver,
        variable_can_work_checker: &'a VariableCanWorkChecker<'a>,
        variable_checker: &'a VariableChecker,
        calculate_data_set: &'a CalculateDataSet<'a>,
        employee_job_status_checker: &'a EmployeeJobStatusChecker<'a>,
    ) -> Self {
        Self {
            schedule_saver,
            variable_can_work_checker,
            variable_checker,
            calculate_data_set,
            employee_job_status_checker,
        }
    }

    /// `schedulePlannedShift(ScheduleModel, PlannedShift, EmployeeFilter)`.
    pub fn schedule_planned_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        planned_shift: PlannedShift,
        employee_filter: &dyn EmployeeFilter,
    ) -> bool {
        let job = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(planned_shift.job_id()))
            .map(|job_data| job_data.job().clone());

        let Some(job) = job else {
            return false;
        };

        let employee_ids = self.filter_employees(schedule_model, employee_filter, &planned_shift);

        let mut best: Option<(EmployeeData, EmployeeShift)> = None;

        for employee_id in employee_ids {
            let Some(mut employee_data) = schedule_model
                .employee_list_mut()
                .and_then(|employee_list| employee_list.take_employee_data(employee_id))
            else {
                continue;
            };

            if let Some(log) = schedule_model.current_planned_shift_log_mut() {
                log.employees_with_conflicts_mut()
                    .add_log_entry(EmployeeLogEntry::new(employee_id));
            }

            let employee_shift_creator =
                crate::engine::misc::employee_shift_creator::EmployeeShiftCreator;
            let mut employee_shift = employee_shift_creator.create_shift(
                employee_data.employee(),
                planned_shift,
                None,
                VARIABLE_FLAG,
            );

            self.calculate_data_set
                .store_overtime_add_shift_and_calculate(
                    schedule_model,
                    &mut employee_data,
                    &mut employee_shift,
                );

            if self.variable_can_work_checker.can_employee_work_shift(
                schedule_model,
                &mut employee_data,
                &employee_shift,
            ) {
                if let Some(log) = schedule_model.current_planned_shift_log_mut() {
                    log.employees_with_conflicts_mut()
                        .remove_log_entry(employee_id);
                }

                let is_better = match &best {
                    None => true,
                    Some((best_data, _)) => self.variable_checker.is_employee_better(
                        &job,
                        &planned_shift,
                        &employee_data,
                        best_data,
                    ),
                };

                if is_better {
                    if let Some((mut old_best_data, old_best_shift)) = best.take() {
                        self.calculate_data_set
                            .remove_shift_and_calculate(&mut old_best_data, old_best_shift);
                        if let Some(employee_list) = schedule_model.employee_list_mut() {
                            employee_list.add_employee_data(old_best_data);
                        }
                    }
                    best = Some((employee_data, employee_shift));
                } else {
                    self.calculate_data_set
                        .remove_shift_and_calculate(&mut employee_data, employee_shift);
                    if let Some(employee_list) = schedule_model.employee_list_mut() {
                        employee_list.add_employee_data(employee_data);
                    }
                }

                if let Some((best_data, _)) = &best {
                    let best_employee_id = best_data.employee().id();
                    if let Some(log) = schedule_model.current_planned_shift_log_mut() {
                        log.ranked_employees_mut().push_employee(best_employee_id);
                    }
                }
            } else {
                self.calculate_data_set
                    .remove_shift_and_calculate(&mut employee_data, employee_shift);
                if let Some(employee_list) = schedule_model.employee_list_mut() {
                    employee_list.add_employee_data(employee_data);
                }
            }
        }

        if let Some((best_data, best_shift)) = best {
            self.schedule_saver
                .save_schedule(schedule_model, best_shift);

            if let Some(employee_list) = schedule_model.employee_list_mut() {
                employee_list.add_employee_data(best_data);
            }

            true
        } else {
            false
        }
    }

    fn filter_employees(
        &self,
        schedule_model: &ScheduleModel,
        employee_filter: &dyn EmployeeFilter,
        planned_shift: &PlannedShift,
    ) -> Vec<i32> {
        let Some(employee_list) = schedule_model.employee_list() else {
            return Vec::new();
        };

        employee_list
            .unsorted_employee_data_list()
            .into_iter()
            .filter(|employee_data| {
                self.employee_active_for_plan(employee_data, planned_shift)
                    && employee_filter.include_employee(
                        schedule_model,
                        employee_data,
                        planned_shift,
                    )
            })
            .map(|employee_data| employee_data.employee().id())
            .collect()
    }

    fn employee_active_for_plan(
        &self,
        employee_data: &EmployeeData,
        planned_shift: &PlannedShift,
    ) -> bool {
        self.employee_job_status_checker
            .can_employee_work_job_on_date(
                employee_data.employee(),
                Some(planned_shift.job_id()),
                planned_shift.shift_date(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
    use crate::engine::process::checkers::employee_certifications_checker::EmployeeCertificationPort;
    use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
    use crate::engine::process::checkers::employee_monthly_available_hours_checker::MonthlyContractHoursAvailablePort;
    use crate::engine::process::checkers::schedule_restriction_rule_checker::{
        ScheduleRestrictionCheckResult, ScheduleRestrictionRulePort,
    };
    use crate::engine::process::ports::OvertimeForDateRangePort;
    use crate::engine::process::variable::filters::JobLevelEmployeeFilter;
    use crate::engine::process::variable::variable_can_work_checker_factory::VariableCanWorkCheckerFactory;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalDateTime};

    struct NoOpTimeCardCalculator;

    impl SchedulesTimeCardCalculatorPort for NoOpTimeCardCalculator {
        fn calculate_overtime_for_schedule_calc_data_set(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
        ) {
        }

        fn calculate_schedule_calc_data_set(&self, _data_set: &mut ScheduleCalcDataSet) {}
    }

    struct NoOpLunchRunner;

    impl ScheduleLunchRunnerPort for NoOpLunchRunner {
        fn run_rules(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
            _employee_shift: &mut EmployeeShift,
        ) {
        }
    }

    struct ZeroOvertimePort;

    impl OvertimeForDateRangePort for ZeroOvertimePort {
        fn overtime_for_date_range(
            &self,
            _employee_data: &mut EmployeeData,
            _date_range: &DateRange,
        ) -> f64 {
            0.0
        }
    }

    struct AlwaysAvailableContractHours;

    impl MonthlyContractHoursAvailablePort for AlwaysAvailableContractHours {
        fn hours_available(
            &self,
            _schedule_model: &ScheduleModel,
            _employee_data: &mut EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> f64 {
            1000.0
        }
    }

    struct AlwaysPassesRestrictions;

    impl ScheduleRestrictionRulePort for AlwaysPassesRestrictions {
        fn check(
            &self,
            _employee_data: &EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> ScheduleRestrictionCheckResult {
            ScheduleRestrictionCheckResult {
                ok: true,
                message: None,
            }
        }
    }

    struct AlwaysCertified;

    impl EmployeeCertificationPort for AlwaysCertified {
        fn is_certified(
            &self,
            _employee_data: &EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> bool {
            true
        }
    }

    struct AlwaysActive;

    impl EmployeeActiveOnDatePort for AlwaysActive {
        fn is_active_on_date(&self, _employee: &Employee, _date: LocalDate) -> bool {
            true
        }
    }

    struct NoAssignments;

    impl crate::engine::process::ports::AssignmentPort for NoAssignments {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }

    fn job() -> Assignment {
        Assignment::new(
            500,
            "Job",
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

    fn employee_with_schedule_order(id: i32, schedule_order: i32) -> Employee {
        let status = EmployeeJobStatus::new(
            500,
            None,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            0.0,
            false,
            schedule_order,
        );
        Employee::new(
            id,
            format!("Employee {id}"),
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            vec![status],
        )
    }

    /// Two employees both pass every check; the one with the lower `EmployeeJobStatus.schedule_order`
    /// should win and get the shift saved (`VariableChecker::compareJobScheduleOrders`).
    #[test]
    fn the_employee_with_the_better_schedule_order_wins_the_shift() {
        let date1 = LocalDate::of(2024, 1, 1);
        let date_range = DateRange::new(date1, date1);
        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);

        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job()));
        schedule_model.set_job_list(job_list);

        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(
            employee_with_schedule_order(1, 5),
            ScheduleCalcDataSet::default(),
            0,
        ));
        employee_list.add_employee_data(EmployeeData::new(
            employee_with_schedule_order(2, 1),
            ScheduleCalcDataSet::default(),
            0,
        ));
        schedule_model.set_employee_list(employee_list);

        schedule_model.set_current_planned_shift_log(Some(PlannedShiftLog::new(shift())));

        let time_card_calculator = NoOpTimeCardCalculator;
        let lunch_runner = NoOpLunchRunner;
        let overtime = ZeroOvertimePort;
        let calculate_data_set =
            CalculateDataSet::new(&time_card_calculator, &lunch_runner, &overtime);

        let assignments = NoAssignments;
        let contract_hours = AlwaysAvailableContractHours;
        let restrictions = AlwaysPassesRestrictions;
        let certifications = AlwaysCertified;
        let can_work_checker_factory = VariableCanWorkCheckerFactory::new(
            &assignments,
            &contract_hours,
            &overtime,
            &restrictions,
            &certifications,
        );
        let variable_can_work_checker = VariableCanWorkChecker::new(&can_work_checker_factory);
        let variable_checker = VariableChecker;

        let active = AlwaysActive;
        let employee_job_status_checker = EmployeeJobStatusChecker::new(&active);

        let scheduler = PlannedShiftScheduler::new(
            &ScheduleSaver,
            &variable_can_work_checker,
            &variable_checker,
            &calculate_data_set,
            &employee_job_status_checker,
        );

        let employee_filter = JobLevelEmployeeFilter::new(-1);

        let scheduled =
            scheduler.schedule_planned_shift(&mut schedule_model, shift(), &employee_filter);

        assert!(scheduled);

        // The winning shift landed in the model's new shift list (`ScheduleSaver::save_schedule`).
        assert_eq!(schedule_model.new_shift_list().employee_shifts().len(), 1);
        assert_eq!(
            schedule_model.new_shift_list().employee_shifts()[0].employee_id(),
            2
        );

        // Employee 2 (schedule_order 1) won; both employees are back in the employee list.
        let employee_list = schedule_model.employee_list().unwrap();
        assert!(employee_list.employee_data(1).is_some());
        assert!(employee_list.employee_data(2).is_some());
        assert_eq!(
            employee_list
                .employee_data(2)
                .unwrap()
                .data_set()
                .shifts()
                .len(),
            1
        );
        assert_eq!(
            employee_list
                .employee_data(1)
                .unwrap()
                .data_set()
                .shifts()
                .len(),
            0
        );
    }
}
