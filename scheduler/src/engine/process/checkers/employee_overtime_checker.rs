//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeOvertimeChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeOvertimeChecker.java`. `dataSet.getOvertimeForDateRange(DateRange)` is a real overtime
//! calculation `ScheduleCalcDataSet` doesn't ground yet (see that type's doc /
//! `PARITY_AUDIT.md` finding 10's pattern) — resolved through
//! [`OvertimeForDateRangePort`](crate::engine::process::ports::OvertimeForDateRangePort) instead
//! of skipping this checker entirely, since everything else in it (the comparison, the note) is
//! real. The port trait itself now lives in `engine::process::ports` (moved there once
//! `EmployeeData`/`CalculateDataSet` also needed it — see that module's doc).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::ports::OvertimeForDateRangePort;
use crate::entity::employee_shift::EmployeeShift;

/// `EmployeeOvertimeChecker`.
pub struct EmployeeOvertimeChecker<'a> {
    overtime: &'a dyn OvertimeForDateRangePort,
}

impl<'a> EmployeeOvertimeChecker<'a> {
    pub fn new(overtime: &'a dyn OvertimeForDateRangePort) -> Self {
        Self { overtime }
    }
}

impl CanWorkChecker for EmployeeOvertimeChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        _employee_shift: &EmployeeShift,
    ) -> bool {
        let new_ot = self
            .overtime
            .overtime_for_date_range(employee_data, schedule_model.date_range());
        let old_ot = employee_data.pre_schedule_check_overtime();

        let can_work = new_ot <= old_ot;

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "The planned shift will cause the employee to be scheduled for overtime",
            );
        }

        can_work
    }
}
