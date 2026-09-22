//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.rotationplans.
//! DailyJobRotationPlanChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! rotationplans/DailyJobRotationPlanChecker.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedules::Schedules;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_context::DailyRotationPlanCheckerContext;
use crate::engine::process::checkers::rotationplans::daily_rotation_plan_checker_factory::DailyRotationPlanCheckerFactory;
use crate::engine::process::checkers::rotationplans::rotation_plan_checker::RotationPlanChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::LocalDate;

/// `DailyJobRotationPlanChecker`.
pub struct DailyJobRotationPlanChecker {
    factory: DailyRotationPlanCheckerFactory,
}

impl DailyJobRotationPlanChecker {
    pub fn new() -> Self {
        Self {
            factory: DailyRotationPlanCheckerFactory,
        }
    }
}

impl Default for DailyJobRotationPlanChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationPlanChecker for DailyJobRotationPlanChecker {
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

impl DailyRotationPlanCheckerContext for DailyJobRotationPlanChecker {
    fn has_matching_shift_on_date(
        &self,
        schedules: &Schedules,
        employee_shift: &EmployeeShift,
        date: LocalDate,
    ) -> bool {
        schedules.has_shift_with_job_on_date(employee_shift.job_id(), date)
    }

    fn rotation_interval(
        &self,
        employee_data: &EmployeeData,
        rotation_plan: RotationPlan,
        date: LocalDate,
    ) -> i32 {
        rotation_plan.rotate_interval().min(
            employee_data
                .employee()
                .employee_job_statuses_for_date(date)
                .len() as i32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;

    fn employee_data_with_one_job_status_and_shift(shift_date: LocalDate) -> EmployeeData {
        let status = EmployeeJobStatus::new(
            500,
            None,
            LocalDate::of(2020, 1, 1),
            LocalDate::of(2099, 1, 1),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            0.0,
            false,
            1,
        );
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
            vec![status],
        );
        let mut employee_data = EmployeeData::new(employee, ScheduleCalcDataSet::default(), 0);
        employee_data.add_employee_shift(EmployeeShift::new(
            1,
            shift_date,
            shift_date.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            500,
            None,
        ));
        employee_data
    }

    #[test]
    fn the_same_job_the_next_day_is_blocked_when_the_rotation_interval_reaches_it() {
        // Worked job 500 on Monday; only one job status on file caps rotation_interval at
        // min(config, 1) = 1, so Tuesday (1 day later) falls inside the checked window.
        let monday = LocalDate::of(2024, 1, 1);
        let tuesday = LocalDate::of(2024, 1, 2);

        let employee_data = employee_data_with_one_job_status_and_shift(monday);
        let candidate_shift = EmployeeShift::new(
            99,
            tuesday,
            tuesday.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            500,
            None,
        );

        let checker = DailyJobRotationPlanChecker::new();
        let rotation_plan = RotationPlan::new(5, true, true, true);
        let can_work =
            checker.can_employee_work_shift(&employee_data, &candidate_shift, rotation_plan);

        assert!(!can_work);
    }

    #[test]
    fn the_same_job_far_enough_away_is_allowed() {
        let monday = LocalDate::of(2024, 1, 1);
        let far_later = LocalDate::of(2024, 2, 1);

        let employee_data = employee_data_with_one_job_status_and_shift(monday);
        let candidate_shift = EmployeeShift::new(
            99,
            far_later,
            far_later.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            500,
            None,
        );

        let checker = DailyJobRotationPlanChecker::new();
        let rotation_plan = RotationPlan::new(5, true, true, true);
        let can_work =
            checker.can_employee_work_shift(&employee_data, &candidate_shift, rotation_plan);

        assert!(can_work);
    }
}
