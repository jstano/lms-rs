//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleEmployeeShiftCreator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleEmployeeShiftCreator.java`. `ShiftCategory shiftCategory = null`
//! is intentional in Java (regular shifts have no category, unlike `PreSchedule`'s) — passed
//! through as `None` explicitly rather than left to a future "fix" that looks up a default.
//! `EmployeeShiftFlagConstants.REGULAR_FLAG = 65536`.

use crate::engine::misc::employee_shift_creator::EmployeeShiftCreator;
use crate::engine::misc::planned_shift_helper::PlannedShiftHelper;
use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use joda_rs::LocalDate;

/// `EmployeeShiftFlagConstants.REGULAR_FLAG`.
const REGULAR_FLAG: i32 = 65_536;

/// `RegularScheduleEmployeeShiftCreator`.
pub struct RegularScheduleEmployeeShiftCreator<'a> {
    employee_shift_creator: &'a EmployeeShiftCreator,
    planned_shift_helper: &'a PlannedShiftHelper<'a>,
}

impl<'a> RegularScheduleEmployeeShiftCreator<'a> {
    pub fn new(
        employee_shift_creator: &'a EmployeeShiftCreator,
        planned_shift_helper: &'a PlannedShiftHelper<'a>,
    ) -> Self {
        Self {
            employee_shift_creator,
            planned_shift_helper,
        }
    }

    /// `createEmployeeShift(ScheduleModel, RegularSchedule, LocalDate)`.
    pub fn create_employee_shift(
        &self,
        schedule_model: &ScheduleModel,
        regular_schedule: &RegularSchedule,
        employee: &Employee,
        job_id: i32,
        shift_date: LocalDate,
    ) -> EmployeeShift {
        let planned_shift = self
            .planned_shift_helper
            .find_or_create_matching_planned_shift(
                schedule_model,
                regular_schedule,
                employee,
                job_id,
                shift_date,
            );

        self.employee_shift_creator
            .create_shift(employee, planned_shift, None, REGULAR_FLAG)
    }
}
