//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleSingleDate`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleSingleDate.java`.
//!
//! **Takes `job_id: i32`, not `&JobData`.** Java holds a `JobData` reference across this whole
//! loop, reading it again after each shift to decide whether to stop — safe there because
//! `ScheduleSaver`/`CalculateDataSet` mutate that same shared object in place. Rust can't hold
//! `&JobData` across a call needing `&mut ScheduleModel`, and unlike `EmployeeData` it can't be
//! taken out either (`ScheduleSaver` looks the job up *by id* through `schedule_model.job_list_mut()`
//! — a taken-out job would silently fail to record the new scheduled hours). Resolved by carrying
//! `job_id` and re-fetching a fresh, short-lived `&JobData` at each read point, dropped before the
//! next call that needs `&mut ScheduleModel`.
//!
//! **The early `return` breaks the whole date's remaining schedule list, not just one shift** —
//! translated as a plain `return` from this function, since the `for` loop here *is* the whole
//! function body (no outer loop level to also escape).
//!
//! **`employee.isPermanent()` gates both projected-hours checks even inside "the permanent
//! process"** — the regular-schedule list for a job/date can include non-permanent (rotation/
//! on-call) employees too, and the gate still applies to them; the guard is per-employee, not
//! process-wide.

use crate::engine::misc::projected_hours_checker::ProjectedHoursChecker;
use crate::engine::model::regular_schedules::RegularSchedules;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
use crate::engine::process::regularschedules::regular_schedule_single_shift::RegularScheduleSingleShift;
use crate::entity::employee_type::EmployeeType;
use joda_rs::LocalDate;

/// `RegularScheduleSingleDate`.
pub struct RegularScheduleSingleDate<'a> {
    regular_schedule_single_shift: &'a RegularScheduleSingleShift<'a>,
    projected_hours_checker: &'a ProjectedHoursChecker,
}

impl<'a> RegularScheduleSingleDate<'a> {
    pub fn new(
        regular_schedule_single_shift: &'a RegularScheduleSingleShift<'a>,
        projected_hours_checker: &'a ProjectedHoursChecker,
    ) -> Self {
        Self {
            regular_schedule_single_shift,
            projected_hours_checker,
        }
    }

    /// `scheduleDate(ScheduleModel, RegularSchedules, JobData, LocalDate)`.
    pub fn schedule_date(
        &self,
        schedule_model: &mut ScheduleModel,
        regular_schedules: &RegularSchedules,
        job_id: i32,
        shift_date: LocalDate,
        active_on_date: &dyn EmployeeActiveOnDatePort,
    ) {
        let Some(job_data) = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(job_id))
        else {
            return;
        };

        let schedules_for_job_and_date = regular_schedules.regular_schedules_for_job_and_date(
            schedule_model,
            job_data,
            shift_date,
            active_on_date,
        );

        for regular_schedule in &schedules_for_job_and_date {
            let employee_type = schedule_model
                .employee_list()
                .and_then(|employee_list| {
                    employee_list.employee_data(regular_schedule.employee_id())
                })
                .map(|employee_data| employee_data.employee().employee_type());

            let Some(employee_type) = employee_type else {
                continue;
            };
            let is_permanent = employee_type == EmployeeType::Permanent;

            let will_exceed = !is_permanent
                && schedule_model
                    .job_list()
                    .and_then(|job_list| job_list.job_data(job_id))
                    .is_some_and(|job_data| {
                        self.projected_hours_checker
                            .will_scheduled_hours_exceed_projected_hours(
                                job_data,
                                shift_date,
                                regular_schedule.duration(),
                            )
                    });

            if !will_exceed {
                self.regular_schedule_single_shift.schedule_regular_shift(
                    schedule_model,
                    regular_schedule,
                    job_id,
                    shift_date,
                );
            }

            // stop scheduling regular schedules if we've exceeded the projected hours for the day
            let has_exceeded = !is_permanent
                && schedule_model
                    .job_list()
                    .and_then(|job_list| job_list.job_data(job_id))
                    .is_some_and(|job_data| job_data.has_exceeded_projected_hours(shift_date));

            if has_exceeded {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::misc::calculate_data_set::CalculateDataSet;
    use crate::engine::misc::employee_shift_creator::EmployeeShiftCreator;
    use crate::engine::misc::planned_shift_creator::PlannedShiftCreator;
    use crate::engine::misc::planned_shift_helper::PlannedShiftHelper;
    use crate::engine::misc::planned_shift_matcher::PlannedShiftMatcher;
    use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
    use crate::engine::misc::schedule_saver::ScheduleSaver;
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::process::checkers::employee_certifications_checker::EmployeeCertificationPort;
    use crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort;
    use crate::engine::process::checkers::schedule_restriction_rule_checker::{
        ScheduleRestrictionCheckResult, ScheduleRestrictionRulePort,
    };
    use crate::engine::process::ports::{AssignmentPort, OvertimeForDateRangePort};
    use crate::engine::process::regularschedules::regular_schedule_can_work_checker::RegularScheduleCanWorkChecker;
    use crate::engine::process::regularschedules::regular_schedule_can_work_checker_factory::RegularScheduleCanWorkCheckerFactory;
    use crate::engine::process::regularschedules::regular_schedule_employee_shift_creator::RegularScheduleEmployeeShiftCreator;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_regular_period::EmployeeRegularPeriod;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalTime;

    struct NoOpTimeCardCalculator;
    impl SchedulesTimeCardCalculatorPort for NoOpTimeCardCalculator {
        fn calculate_overtime_for_schedule_calc_data_set(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
        ) {
        }

        fn calculate_schedule_calc_data_set(&self, _data_set: &mut ScheduleCalcDataSet) {}
    }

    /// Sets `net_hours` to the planned shift's duration — simulates the real
    /// `ScheduleLunchRunner`'s worked-hours calculation closely enough for `ScheduleSaver` to
    /// have something nonzero to add to `JobData::scheduled_hours`.
    struct DurationAsNetHoursLunchRunner;
    impl ScheduleLunchRunnerPort for DurationAsNetHoursLunchRunner {
        fn run_rules(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
            employee_shift: &mut EmployeeShift,
        ) {
            let duration = employee_shift
                .planned_shift()
                .map(|planned_shift| planned_shift.duration())
                .unwrap_or(0.0);
            *employee_shift = employee_shift.clone().with_net_hours(duration);
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

    struct NoAssignmentsPort;
    impl AssignmentPort for NoAssignmentsPort {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }

    struct NoRestrictionsPort;
    impl ScheduleRestrictionRulePort for NoRestrictionsPort {
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

    struct AlwaysCertifiedPort;
    impl EmployeeCertificationPort for AlwaysCertifiedPort {
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

    fn job(id: i32) -> Assignment {
        Assignment::new(
            id,
            format!("Job {id}"),
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

    fn employee(id: i32, employee_type: EmployeeType, job_id: i32) -> Employee {
        let job_status = EmployeeJobStatus::new(
            job_id,
            None,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 7),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            40.0,
            false,
            0,
        );

        Employee::new(
            id,
            format!("Employee {id}"),
            employee_type,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![job_status],
        )
    }

    fn regular_period(employee_id: i32, job_id: i32, duration: f64) -> EmployeeRegularPeriod {
        EmployeeRegularPeriod::new(
            employee_id,
            Some(job_id),
            None,
            joda_rs::DayOfWeek::Monday,
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
            duration,
        )
    }

    /// Wires the full step 7-8 chain the way `PermanentScheduleProcess`/`RegularScheduleProcess`
    /// do, so `RegularScheduleSingleDate::schedule_date` can be exercised directly.
    struct Fixture {
        time_card_calculator: NoOpTimeCardCalculator,
        lunch_runner: DurationAsNetHoursLunchRunner,
        overtime: ZeroOvertimePort,
        assignments: NoAssignmentsPort,
        restrictions: NoRestrictionsPort,
        certifications: AlwaysCertifiedPort,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                time_card_calculator: NoOpTimeCardCalculator,
                lunch_runner: DurationAsNetHoursLunchRunner,
                overtime: ZeroOvertimePort,
                assignments: NoAssignmentsPort,
                restrictions: NoRestrictionsPort,
                certifications: AlwaysCertifiedPort,
            }
        }

        fn run(&self, schedule_model: &mut ScheduleModel, regular_schedules: &RegularSchedules) {
            let calculate_data_set = CalculateDataSet::new(
                &self.time_card_calculator,
                &self.lunch_runner,
                &self.overtime,
            );
            let schedule_saver = ScheduleSaver;
            let planned_shift_matcher = PlannedShiftMatcher;
            let planned_shift_creator = PlannedShiftCreator;
            let planned_shift_helper =
                PlannedShiftHelper::new(&planned_shift_matcher, &planned_shift_creator);
            let employee_shift_creator = EmployeeShiftCreator;
            let regular_schedule_employee_shift_creator = RegularScheduleEmployeeShiftCreator::new(
                &employee_shift_creator,
                &planned_shift_helper,
            );
            let factory = RegularScheduleCanWorkCheckerFactory::new(
                &self.assignments,
                &self.restrictions,
                &self.certifications,
            );
            let can_work_checker = RegularScheduleCanWorkChecker::new(&factory);
            let single_shift = RegularScheduleSingleShift::new(
                &schedule_saver,
                &can_work_checker,
                &regular_schedule_employee_shift_creator,
                &calculate_data_set,
            );
            let projected_hours_checker = ProjectedHoursChecker;
            let single_date =
                RegularScheduleSingleDate::new(&single_shift, &projected_hours_checker);

            single_date.schedule_date(
                schedule_model,
                regular_schedules,
                1,
                LocalDate::of(2024, 1, 1),
                &AlwaysActive,
            );
        }
    }

    /// Three candidates for the same job/date; the first one's own hours exceed the day's
    /// projected hours once scheduled — the loop must stop entirely (a plain `return`, not
    /// `continue`), so the third candidate (whose 0-hour duration would otherwise sail through
    /// the pre-check) never gets scheduled either.
    #[test]
    fn exceeding_projected_hours_stops_scheduling_the_rest_of_the_date_not_just_one_shift() {
        let date = LocalDate::of(2024, 1, 1);
        let range = DateRange::new(date, date);
        let property = Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);

        let mut job_data = JobData::new(job(1));
        job_data.projected_hours_mut().set_hours_for_date(date, 5.0);
        let mut job_list = JobList::new();
        job_list.add_job_data(job_data);
        schedule_model.set_job_list(job_list);

        let mut employee_list = EmployeeList::new();
        for id in [1, 2, 3] {
            employee_list.add_employee_data(EmployeeData::new(
                employee(id, EmployeeType::Regular, 1),
                ScheduleCalcDataSet::default(),
                0,
            ));
        }
        schedule_model.set_employee_list(employee_list);

        let regular_schedules = RegularSchedules::new(
            &schedule_model,
            vec![
                regular_period(1, 1, 5.0),
                regular_period(2, 1, 100.0),
                regular_period(3, 1, 0.0),
            ],
        );

        Fixture::new().run(&mut schedule_model, &regular_schedules);

        let scheduled_employee_ids: Vec<i32> = schedule_model
            .new_shift_list()
            .employee_shifts()
            .iter()
            .map(EmployeeShift::employee_id)
            .collect();

        assert_eq!(scheduled_employee_ids, vec![1]);
    }

    /// A permanent employee bypasses the projected-hours gate entirely (schedules despite
    /// exceeding, and doesn't trip the early-return check afterward) — but a non-permanent
    /// employee later in the *same* job/date list still gets the normal gating, proving the
    /// `isPermanent()` guard is per-employee, not a blanket exemption for this process.
    #[test]
    fn permanent_employees_bypass_the_projected_hours_gate_but_others_in_the_list_still_get_it() {
        let date = LocalDate::of(2024, 1, 1);
        let range = DateRange::new(date, date);
        let property = Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);

        let mut job_data = JobData::new(job(1));
        job_data.projected_hours_mut().set_hours_for_date(date, 1.0);
        let mut job_list = JobList::new();
        job_list.add_job_data(job_data);
        schedule_model.set_job_list(job_list);

        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(
            employee(1, EmployeeType::Permanent, 1),
            ScheduleCalcDataSet::default(),
            0,
        ));
        employee_list.add_employee_data(EmployeeData::new(
            employee(2, EmployeeType::Regular, 1),
            ScheduleCalcDataSet::default(),
            0,
        ));
        schedule_model.set_employee_list(employee_list);

        let regular_schedules = RegularSchedules::new(
            &schedule_model,
            vec![regular_period(1, 1, 10.0), regular_period(2, 1, 10.0)],
        );

        Fixture::new().run(&mut schedule_model, &regular_schedules);

        let scheduled_employee_ids: Vec<i32> = schedule_model
            .new_shift_list()
            .employee_shifts()
            .iter()
            .map(EmployeeShift::employee_id)
            .collect();

        assert_eq!(scheduled_employee_ids, vec![1]);
    }
}
