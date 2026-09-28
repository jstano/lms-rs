//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.VariableJobScheduler`.
//!
//! Ground truth: `taps/.../process/variable/VariableJobScheduler.java`. `sortPlannedShifts`
//! (Java's private helper) is inlined directly rather than kept as a separate method — it's a
//! one-line delegation to `PlannedShiftSorterFactory`/`PlannedShiftSorter` with no other caller.

use crate::engine::model::job_data::JobData;
use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::planned_shift_sorter_factory::PlannedShiftSorterFactory;
use crate::engine::process::variable::filters::job_level_employee_filter::JobLevelEmployeeFilter;
use crate::engine::process::variable::planned_shift_scheduler::PlannedShiftScheduler;

/// `VariableJobScheduler`.
pub struct VariableJobScheduler<'a> {
    planned_shift_scheduler: &'a PlannedShiftScheduler<'a>,
}

impl<'a> VariableJobScheduler<'a> {
    pub fn new(planned_shift_scheduler: &'a PlannedShiftScheduler<'a>) -> Self {
        Self {
            planned_shift_scheduler,
        }
    }

    /// `scheduleJob(ScheduleModel, JobData, int)`.
    pub fn schedule_job(&self, schedule_model: &mut ScheduleModel, job_id: i32, job_level: i32) {
        let job_level_employee_filter = JobLevelEmployeeFilter::new(job_level);

        let Some(job_data) = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(job_id))
        else {
            return;
        };

        let planned_shifts = PlannedShiftSorterFactory::planned_shift_sorter(job_data)
            .sort_planned_shifts(schedule_model, job_data);

        for planned_shift in planned_shifts {
            let exceeds = schedule_model
                .job_list()
                .and_then(|job_list| job_list.job_data(job_id))
                .is_none_or(|job_data: &JobData| {
                    job_data.will_scheduled_hours_exceed_projected_hours(
                        planned_shift.shift_date(),
                        planned_shift.duration(),
                    )
                });

            if exceeds {
                continue;
            }

            let planned_shift_log = PlannedShiftLog::new(planned_shift);

            schedule_model
                .job_schedule_log(job_id)
                .schedule_log(Box::new(job_level_employee_filter))
                .add_planned_shift_log(planned_shift_log.clone());

            schedule_model.set_current_planned_shift_log(Some(planned_shift_log));

            self.planned_shift_scheduler.schedule_planned_shift(
                schedule_model,
                planned_shift,
                &job_level_employee_filter,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::misc::calculate_data_set::CalculateDataSet;
    use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
    use crate::engine::misc::schedule_saver::ScheduleSaver;
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_list::JobList;
    use crate::engine::process::checkers::employee_job_status_checker::{
        EmployeeActiveOnDatePort, EmployeeJobStatusChecker,
    };
    use crate::engine::process::ports::OvertimeForDateRangePort;
    use crate::engine::process::variable::variable_can_work_checker::VariableCanWorkChecker;
    use crate::engine::process::variable::variable_can_work_checker_factory::VariableCanWorkCheckerFactory;
    use crate::engine::process::variable::variable_checker::VariableChecker;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee::Employee;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

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
    impl crate::engine::process::checkers::employee_monthly_available_hours_checker::MonthlyContractHoursAvailablePort
        for AlwaysAvailableContractHours
    {
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
    impl crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionRulePort
        for AlwaysPassesRestrictions
    {
        fn check(
            &self,
            _employee_data: &EmployeeData,
            _employee_shift: &EmployeeShift,
        ) -> crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionCheckResult
        {
            crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionCheckResult {
                ok: true,
                message: None,
            }
        }
    }

    struct AlwaysCertified;
    impl
        crate::engine::process::checkers::employee_certifications_checker::EmployeeCertificationPort
        for AlwaysCertified
    {
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

    fn shift_on(date: LocalDate, duration: f64) -> PlannedShift {
        PlannedShift::new(
            0,
            500,
            date,
            date.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            duration,
            None,
        )
    }

    /// Mirrors `VariableJobSchedulerTest.groovy#testScheduleJob`'s outcome (an interaction test
    /// there): a shift is skipped — no `PlannedShiftLog` recorded, no scheduling attempted — when
    /// scheduling it would exceed the job's projected hours for that date; other shifts still get
    /// a log entry.
    #[test]
    fn skips_shifts_that_would_exceed_projected_hours_for_their_date() {
        let date1 = LocalDate::of(2013, 1, 1);
        let date2 = LocalDate::of(2013, 1, 2);
        let date_range = DateRange::new(date1, date2);

        let mut job_data = JobData::new(job());
        job_data
            .projected_hours_mut()
            .set_hours_for_date(date1, 8.0);
        job_data
            .projected_hours_mut()
            .set_hours_for_date(date2, 0.0);
        job_data.planned_shifts_mut().push(shift_on(date1, 8.0));
        job_data.planned_shifts_mut().push(shift_on(date2, 8.0));

        let mut job_list = JobList::new();
        job_list.add_job_data(job_data);

        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);
        schedule_model.set_job_list(job_list);
        schedule_model.set_employee_list(EmployeeList::new());

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
        let schedule_saver = ScheduleSaver;
        let planned_shift_scheduler = PlannedShiftScheduler::new(
            &schedule_saver,
            &variable_can_work_checker,
            &variable_checker,
            &calculate_data_set,
            &employee_job_status_checker,
        );

        let variable_job_scheduler = VariableJobScheduler::new(&planned_shift_scheduler);

        variable_job_scheduler.schedule_job(&mut schedule_model, 500, 1);

        let job_schedule_log = schedule_model.job_schedule_log(500);
        let keys = job_schedule_log.employee_filter_keys().to_vec();
        assert_eq!(keys.len(), 1);
        let schedule_log = job_schedule_log.schedule_log(Box::new(JobLevelEmployeeFilter::new(1)));

        // date1 (8h projected, 8h shift) doesn't exceed; date2 (0h projected, 8h shift) does.
        assert_eq!(schedule_log.planned_shift_logs().len(), 1);
        assert_eq!(
            schedule_log.planned_shift_logs()[0]
                .planned_shift()
                .shift_date(),
            date1
        );
    }
}
