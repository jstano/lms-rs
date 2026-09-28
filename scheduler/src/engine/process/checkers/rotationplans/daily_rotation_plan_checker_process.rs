//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! DailyRotationPlanCheckerProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/DailyRotationPlanCheckerProcess.java`. `shift_date.minus_days(interval)..=
//! shift_date.plus_days(interval)` stands in for `ArbitraryDateRange.of(...)` — both are a plain
//! inclusive calendar-date span; see `Schedules`'s doc for the general note on `DateRange`
//! ports (`planner`'s `TimeCard` doc, cited there).

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_context::DailyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::engine::process::checkers::rotationplans::rotation_plan_utils::is_rotation_plan_applicable;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use date_range_rs::DateRange;

/// `DailyRotationPlanCheckerProcess`.
pub struct DailyRotationPlanCheckerProcess<'a> {
    context: &'a dyn DailyRotationPlanCheckerContext,
}

impl<'a> DailyRotationPlanCheckerProcess<'a> {
    pub fn new(context: &'a dyn DailyRotationPlanCheckerContext) -> Self {
        Self { context }
    }
}

impl RotationPlanChecker for DailyRotationPlanCheckerProcess<'_> {
    fn can_employee_work_shift(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
        rotation_plan: RotationPlan,
    ) -> bool {
        if !is_rotation_plan_applicable(employee_data, rotation_plan) {
            return true;
        }

        let shift_date = employee_shift.shift_date();

        let employee_shifts: Vec<EmployeeShift> = employee_data
            .data_set()
            .shifts()
            .iter()
            .filter(|s| *s != employee_shift)
            .cloned()
            .collect();
        let schedules = Schedules::new(
            employee_shifts,
            employee_data.data_set().time_off_requests().to_vec(),
        );

        let rotation_interval =
            self.context
                .rotation_interval(employee_data, rotation_plan, shift_date);

        let range = DateRange::new(
            shift_date.minus_days(rotation_interval as i64),
            shift_date.plus_days(rotation_interval as i64),
        );

        for date in range.dates() {
            if date != shift_date
                && self
                    .context
                    .has_matching_shift_on_date(&schedules, employee_shift, date)
            {
                return false;
            }
        }

        true
    }
}
