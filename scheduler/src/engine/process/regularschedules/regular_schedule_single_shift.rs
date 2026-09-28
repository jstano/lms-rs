//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleSingleShift`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleSingleShift.java`. The take/mutate/reinsert point for this
//! wave — same `EmployeeList::take_employee_data`/`add_employee_data` idiom `PreScheduleProcess`
//! established (`PARITY_AUDIT.md` finding 30), needed here for the same reason: this loop must
//! hold `&mut EmployeeData` (nested inside `ScheduleModel`'s `EmployeeList`) alongside `&mut
//! ScheduleModel` at once.

use crate::engine::misc::calculate_data_set::CalculateDataSet;
use crate::engine::misc::schedule_saver::ScheduleSaver;
use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::regularschedules::regular_schedule_can_work_checker::RegularScheduleCanWorkChecker;
use crate::engine::process::regularschedules::regular_schedule_employee_shift_creator::RegularScheduleEmployeeShiftCreator;
use joda_rs::LocalDate;

/// `RegularScheduleSingleShift`.
pub struct RegularScheduleSingleShift<'a> {
    schedule_saver: &'a ScheduleSaver,
    regular_schedule_can_work_checker: &'a RegularScheduleCanWorkChecker<'a>,
    regular_schedule_employee_shift_creator: &'a RegularScheduleEmployeeShiftCreator<'a>,
    calculate_data_set: &'a CalculateDataSet<'a>,
}

impl<'a> RegularScheduleSingleShift<'a> {
    pub fn new(
        schedule_saver: &'a ScheduleSaver,
        regular_schedule_can_work_checker: &'a RegularScheduleCanWorkChecker<'a>,
        regular_schedule_employee_shift_creator: &'a RegularScheduleEmployeeShiftCreator<'a>,
        calculate_data_set: &'a CalculateDataSet<'a>,
    ) -> Self {
        Self {
            schedule_saver,
            regular_schedule_can_work_checker,
            regular_schedule_employee_shift_creator,
            calculate_data_set,
        }
    }

    /// `scheduleRegularShift(ScheduleModel, RegularSchedule, LocalDate)`.
    pub fn schedule_regular_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        regular_schedule: &RegularSchedule,
        job_id: i32,
        shift_date: LocalDate,
    ) {
        let Some(mut employee_data) =
            schedule_model
                .employee_list_mut()
                .and_then(|employee_list| {
                    employee_list.take_employee_data(regular_schedule.employee_id())
                })
        else {
            return;
        };

        let mut employee_shift = self
            .regular_schedule_employee_shift_creator
            .create_employee_shift(
                schedule_model,
                regular_schedule,
                employee_data.employee(),
                job_id,
                shift_date,
            );

        self.calculate_data_set
            .store_overtime_add_shift_and_calculate(
                schedule_model,
                &mut employee_data,
                &mut employee_shift,
            );

        if self
            .regular_schedule_can_work_checker
            .can_employee_work_shift(schedule_model, &mut employee_data, &employee_shift)
        {
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
