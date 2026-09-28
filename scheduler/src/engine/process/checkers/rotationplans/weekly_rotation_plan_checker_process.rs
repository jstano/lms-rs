//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! WeeklyRotationPlanCheckerProcess`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/WeeklyRotationPlanCheckerProcess.java`.
//!
//! Java builds its prior/future windows as `ArbitraryDateRange.of(plannedShiftDate.minusDays(1),
//! plannedShiftDate.minusDays(consecutiveDaysAllowed))` — start *later* than end — relying on
//! `ArbitraryDateRange` iterating from the first argument toward the second, i.e. walking
//! backward from the day immediately before the shift outward. That's load-bearing: the loop
//! stops at the first non-matching day, so iteration order changes the answer.
//! `date_range_rs::DateRange` doesn't carry that reversed-iteration behavior, so this steps the
//! dates by hand (`minus_days(1)`, `minus_days(2)`, …) instead of building a `DateRange` at all,
//! rather than risk silently iterating the wrong direction.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::engine::process::checkers::rotationplans::rotation_plan_utils::is_rotation_plan_applicable;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_context::WeeklyRotationPlanCheckerContext;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `WeeklyRotationPlanCheckerProcess`.
pub struct WeeklyRotationPlanCheckerProcess<'a> {
    context: &'a dyn WeeklyRotationPlanCheckerContext,
}

impl<'a> WeeklyRotationPlanCheckerProcess<'a> {
    pub fn new(context: &'a dyn WeeklyRotationPlanCheckerContext) -> Self {
        Self { context }
    }

    /// Counts outward from `anchor_date` in steps of `step_days` (negative for "prior", positive
    /// for "future"), stopping at the first date with no matching shift.
    fn count_consecutive_days(
        &self,
        employee_shift: &EmployeeShift,
        schedules: &Schedules,
        anchor_date: LocalDate,
        step_days: i64,
        consecutive_days_allowed: i32,
    ) -> i32 {
        let mut number_of_matched_shifts = 0;

        for offset in 1..=consecutive_days_allowed as i64 {
            let date = if step_days < 0 {
                anchor_date.minus_days(offset)
            } else {
                anchor_date.plus_days(offset)
            };

            if !self
                .context
                .has_matching_shift_on_date(schedules, employee_shift, date)
            {
                break;
            }

            number_of_matched_shifts += 1;
        }

        number_of_matched_shifts
    }
}

impl RotationPlanChecker for WeeklyRotationPlanCheckerProcess<'_> {
    fn can_employee_work_shift(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
        rotation_plan: RotationPlan,
    ) -> bool {
        if !is_rotation_plan_applicable(employee_data, rotation_plan) {
            return true;
        }

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

        let planned_shift_date = employee_shift.shift_date();
        let consecutive_days_allowed = rotation_plan.rotate_interval();

        let consecutive_shifts_before = self.count_consecutive_days(
            employee_shift,
            &schedules,
            planned_shift_date,
            -1,
            consecutive_days_allowed,
        );
        let consecutive_shifts_after = self.count_consecutive_days(
            employee_shift,
            &schedules,
            planned_shift_date,
            1,
            consecutive_days_allowed,
        );

        // `+ 1` accounts for the shift's own day; equivalent to Java's `<= consecutiveDaysAllowed`.
        consecutive_shifts_before + consecutive_shifts_after < consecutive_days_allowed
    }
}
