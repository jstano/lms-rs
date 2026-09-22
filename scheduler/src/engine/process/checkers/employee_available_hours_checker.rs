//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! EmployeeAvailableHoursChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! EmployeeAvailableHoursChecker.java`. Dispatches to the monthly or weekly checker by
//! `Property.getScheduleMode()`; Java's lazily-built `checkerMap` is unnecessary here — a `match`
//! on a two-variant enum needs no cache.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::employee_monthly_available_hours_checker::EmployeeMonthlyAvailableHoursChecker;
use crate::engine::process::checkers::employee_weekly_available_hours_checker::EmployeeWeeklyAvailableHoursChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::schedule_mode::ScheduleMode;

/// `EmployeeAvailableHoursChecker`.
pub struct EmployeeAvailableHoursChecker<'a> {
    monthly: EmployeeMonthlyAvailableHoursChecker<'a>,
    weekly: EmployeeWeeklyAvailableHoursChecker<'a>,
}

impl<'a> EmployeeAvailableHoursChecker<'a> {
    pub fn new(
        monthly: EmployeeMonthlyAvailableHoursChecker<'a>,
        weekly: EmployeeWeeklyAvailableHoursChecker<'a>,
    ) -> Self {
        Self { monthly, weekly }
    }
}

impl CanWorkChecker for EmployeeAvailableHoursChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        // Java's `checkerMap.get(scheduleMode)` NPEs if `scheduleMode` isn't `MONTHLY`/`WEEKLY`
        // (impossible for a real Java enum, but `Property::schedule_mode` is `Option` here since
        // nothing loads it yet — see that field's doc). `None`/anything else falls back to the
        // weekly checker rather than panicking.
        let can_work = match schedule_model.property().schedule_mode() {
            Some(ScheduleMode::Monthly) => {
                self.monthly
                    .can_employee_work_shift(schedule_model, employee_data, employee_shift)
            }
            _ => self
                .weekly
                .can_employee_work_shift(schedule_model, employee_data, employee_shift),
        };

        if !can_work {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                "The planned shift will cause the employee to exceed their available hours",
            );
        }

        can_work
    }
}
