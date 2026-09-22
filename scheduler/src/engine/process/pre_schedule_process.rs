//! Port of `com.unifocus.watson.server.scheduler.engine.process.PreScheduleProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! PreScheduleProcess.java`. `ScheduleEngine`'s pipeline step 6 — `PLAN_SCHEDULER.md`.
//! `scheduleModel.getProgress().setMessage(...)` isn't ported — `ScheduleModel.progress` isn't
//! modeled yet (nothing reads/writes it before this file; see that type's doc), so this step
//! doesn't gain it just to write one message with no reader.
//!
//! `PreScheduleProcessTest.groovy` is a pure interaction test (mocks every collaborator, asserts
//! call order/count — `1 * calculateDataSet.storeOvertimeAddShiftAndCalculate(...)`, etc.), the
//! same shape as `DayOffPlanRotatorTest.groovy` (`PARITY_AUDIT.md` finding 24) — nothing from it
//! carries a real value assertion to transcribe.
//!
//! **First real caller needing simultaneous `&mut ScheduleModel` and `&mut EmployeeData`, where
//! `EmployeeData` is nested inside `ScheduleModel`'s `EmployeeList`.** Java's single mutable
//! object graph gives every checker both for free (`CanWorkChecker::can_employee_work_shift`
//! takes both); Rust can't alias a `&mut EmployeeData` borrowed out of `schedule_model` with
//! `&mut ScheduleModel` at the same time. Unlike `SchedulePreparationService`'s
//! `shift_preparation_fields` (finding 28, a fixed *set* of fields), the one `EmployeeData` this
//! loop needs is chosen dynamically per iteration (keyed by the loaded `PreSchedule`'s employee),
//! so a split-borrow accessor doesn't fit — solved instead by temporarily removing that one
//! employee's data from the list (`EmployeeList::take_employee_data`), operating on it as a fully
//! independent owned value alongside `&mut ScheduleModel`, then reinserting it
//! (`EmployeeList::add_employee_data`) once done.

use crate::engine::io::pre_schedule_loader::PreScheduleLoader;
use crate::engine::misc::calculate_data_set::CalculateDataSet;
use crate::engine::misc::employee_shift_creator::EmployeeShiftCreator;
use crate::engine::misc::planned_shift_creator::PlannedShiftCreator;
use crate::engine::misc::schedule_saver::ScheduleSaver;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::employee_time_off_checker::EmployeeTimeOffChecker;

/// `EmployeeShiftFlagConstants.PRESCHEDULE_FLAG`.
const PRESCHEDULE_FLAG: i32 = 131_072;

/// `PreScheduleProcess`.
pub struct PreScheduleProcess<'a> {
    pre_schedule_loader: &'a PreScheduleLoader<'a>,
    schedule_saver: &'a ScheduleSaver,
    planned_shift_creator: &'a PlannedShiftCreator,
    employee_shift_creator: &'a EmployeeShiftCreator,
    employee_time_off_checker: &'a EmployeeTimeOffChecker,
    calculate_data_set: &'a CalculateDataSet<'a>,
}

impl<'a> PreScheduleProcess<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pre_schedule_loader: &'a PreScheduleLoader<'a>,
        schedule_saver: &'a ScheduleSaver,
        planned_shift_creator: &'a PlannedShiftCreator,
        employee_shift_creator: &'a EmployeeShiftCreator,
        employee_time_off_checker: &'a EmployeeTimeOffChecker,
        calculate_data_set: &'a CalculateDataSet<'a>,
    ) -> Self {
        Self {
            pre_schedule_loader,
            schedule_saver,
            planned_shift_creator,
            employee_shift_creator,
            employee_time_off_checker,
            calculate_data_set,
        }
    }

    /// `preScheduleEmployees(ScheduleModel)`.
    pub fn pre_schedule_employees(&self, schedule_model: &mut ScheduleModel) {
        let pre_schedules = self.pre_schedule_loader.load_pre_schedules(schedule_model);

        for pre_schedule in pre_schedules {
            let employee_id = pre_schedule.employee_id();

            let Some(mut employee_data) = schedule_model
                .employee_list_mut()
                .and_then(|employee_list| employee_list.take_employee_data(employee_id))
            else {
                continue;
            };

            let planned_shift = self
                .planned_shift_creator
                .create_planned_shift_from_pre_schedule(&pre_schedule, employee_data.employee());
            let mut employee_shift = self.employee_shift_creator.create_shift(
                employee_data.employee(),
                planned_shift,
                pre_schedule.shift_category_id(),
                PRESCHEDULE_FLAG,
            );

            self.calculate_data_set
                .store_overtime_add_shift_and_calculate(
                    schedule_model,
                    &mut employee_data,
                    &mut employee_shift,
                );

            if self.employee_time_off_checker.can_employee_work_shift(
                schedule_model,
                &mut employee_data,
                &employee_shift,
            ) {
                self.schedule_saver
                    .save_schedule(schedule_model, employee_shift);
            } else {
                self.calculate_data_set
                    .remove_shift_and_calculate(&mut employee_data, employee_shift);
            }

            if let Some(employee_list) = schedule_model.employee_list_mut() {
                employee_list.add_employee_data(employee_data);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::io::ports::PreScheduleDAOPort;
    use crate::engine::misc::ports::{ScheduleLunchRunnerPort, SchedulesTimeCardCalculatorPort};
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::engine::process::ports::OvertimeForDateRangePort;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_time_off::EmployeeTimeOff;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::pre_schedule::PreSchedule;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalTime};

    struct FakePreScheduleDAOPort {
        rows: Vec<PreSchedule>,
    }

    impl PreScheduleDAOPort for FakePreScheduleDAOPort {
        fn load_pre_schedules_for_jobs_and_date_range(
            &self,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<PreSchedule> {
            self.rows.clone()
        }
    }

    struct NoOpTimeCardCalculator;

    impl SchedulesTimeCardCalculatorPort for NoOpTimeCardCalculator {
        fn calculate_overtime_for_schedule_calc_data_set(
            &self,
            _data_set: &mut ScheduleCalcDataSet,
        ) {
        }
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

    fn employee(id: i32) -> Employee {
        Employee::new(
            id,
            format!("Employee {id}"),
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    /// Only `employee1` has a time-off request overlapping the pre-scheduled shift, so only
    /// `employee2`'s shift should end up saved — same shape as `PreScheduleProcessTest.groovy`'s
    /// "only saves schedules for employees that can work the planned shift" (a pure interaction
    /// test there; this asserts the same outcome through real state instead of mock call counts,
    /// since this crate has no mocking layer — see the module doc).
    #[test]
    fn only_saves_schedules_for_employees_that_can_work_the_planned_shift() {
        let date1 = LocalDate::of(2024, 1, 1);
        let date_range = DateRange::new(date1, date1);
        let start = date1.at_time(LocalTime::of(9, 0, 0));
        let end = date1.at_time(LocalTime::of(17, 0, 0));

        let pre_schedule1 = PreSchedule::new(0, 1, 1, start, end);
        let pre_schedule2 = PreSchedule::new(0, 2, 1, start, end);

        let mut data_set1 = ScheduleCalcDataSet::new(
            Vec::new(),
            vec![EmployeeTimeOff::new(start, end)],
            Default::default(),
        );
        data_set1.set_employee(employee(1));
        let mut data_set2 = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default());
        data_set2.set_employee(employee(2));

        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(employee(1), data_set1, 0));
        employee_list.add_employee_data(EmployeeData::new(employee(2), data_set2, 0));

        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));

        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);
        schedule_model.set_job_list(job_list);
        schedule_model.set_employee_list(employee_list);

        let dao = FakePreScheduleDAOPort {
            rows: vec![pre_schedule1, pre_schedule2],
        };
        let pre_schedule_loader = PreScheduleLoader::new(&dao);
        let schedule_saver = ScheduleSaver;
        let planned_shift_creator = PlannedShiftCreator;
        let employee_shift_creator = EmployeeShiftCreator;
        let employee_time_off_checker = EmployeeTimeOffChecker;
        let time_card_calculator = NoOpTimeCardCalculator;
        let lunch_runner = NoOpLunchRunner;
        let overtime = ZeroOvertimePort;
        let calculate_data_set =
            CalculateDataSet::new(&time_card_calculator, &lunch_runner, &overtime);

        let process = PreScheduleProcess::new(
            &pre_schedule_loader,
            &schedule_saver,
            &planned_shift_creator,
            &employee_shift_creator,
            &employee_time_off_checker,
            &calculate_data_set,
        );

        process.pre_schedule_employees(&mut schedule_model);

        let saved_shifts = schedule_model.new_shift_list().employee_shifts();
        assert_eq!(saved_shifts.len(), 1);
        assert_eq!(saved_shifts[0].employee_id(), 2);

        let employee_list = schedule_model.employee_list().unwrap();
        assert!(
            employee_list
                .employee_data(1)
                .unwrap()
                .data_set()
                .shifts()
                .is_empty()
        );
        assert_eq!(
            employee_list
                .employee_data(2)
                .unwrap()
                .data_set()
                .shifts()
                .len(),
            1
        );
    }
}
