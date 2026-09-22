//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! WeeklyJobRotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/WeeklyJobRotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_context::WeeklyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::weekly_rotation_plan_checker_factory::WeeklyRotationPlanCheckerFactory;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `WeeklyJobRotationPlanChecker`.
pub struct WeeklyJobRotationPlanChecker {
    factory: WeeklyRotationPlanCheckerFactory,
}

impl WeeklyJobRotationPlanChecker {
    pub fn new() -> Self {
        Self {
            factory: WeeklyRotationPlanCheckerFactory,
        }
    }
}

impl Default for WeeklyJobRotationPlanChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationPlanChecker for WeeklyJobRotationPlanChecker {
    fn can_employee_work_shift(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
        rotation_plan: RotationPlan,
    ) -> bool {
        self.factory.instance(self).can_employee_work_shift(
            employee_data,
            employee_shift,
            rotation_plan,
        )
    }
}

impl WeeklyRotationPlanCheckerContext for WeeklyJobRotationPlanChecker {
    fn has_matching_shift_on_date(
        &self,
        schedules: &Schedules,
        employee_shift: &EmployeeShift,
        date: LocalDate,
    ) -> bool {
        schedules.has_shift_with_job_on_date(employee_shift.job_id(), date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;

    fn employee_data_with_shifts(shift_dates: &[LocalDate]) -> EmployeeData {
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![],
            vec![],
        );
        let mut employee_data = EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0);

        for (index, date) in shift_dates.iter().enumerate() {
            let shift = EmployeeShift::new(
                index as i32,
                *date,
                date.at_time(joda_rs::LocalTime::of(9, 0, 0)),
                500,
                None,
            );
            employee_data.add_employee_shift(shift);
        }

        employee_data
    }

    fn plan(rotate_interval: i32) -> RotationPlan {
        RotationPlan::new(rotate_interval, false, true, true)
    }

    #[test]
    fn a_shift_that_would_extend_a_run_past_the_limit_is_blocked() {
        // Already worked job 500 on Mon/Tue/Wed (3 consecutive days); a 4th consecutive day
        // (Thursday) exceeds a rotate_interval of 3.
        let mon = LocalDate::of(2024, 1, 1);
        let tue = LocalDate::of(2024, 1, 2);
        let wed = LocalDate::of(2024, 1, 3);
        let thu = LocalDate::of(2024, 1, 4);

        let employee_data = employee_data_with_shifts(&[mon, tue, wed]);
        let candidate_shift = EmployeeShift::new(
            99,
            thu,
            thu.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            500,
            None,
        );

        let checker = WeeklyJobRotationPlanChecker::new();
        let can_work = checker.can_employee_work_shift(&employee_data, &candidate_shift, plan(3));

        assert!(!can_work);
    }

    #[test]
    fn a_shift_within_the_limit_is_allowed() {
        let mon = LocalDate::of(2024, 1, 1);
        let tue = LocalDate::of(2024, 1, 2);

        let employee_data = employee_data_with_shifts(&[mon]);
        let candidate_shift = EmployeeShift::new(
            99,
            tue,
            tue.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            500,
            None,
        );

        let checker = WeeklyJobRotationPlanChecker::new();
        let can_work = checker.can_employee_work_shift(&employee_data, &candidate_shift, plan(3));

        assert!(can_work);
    }
}
