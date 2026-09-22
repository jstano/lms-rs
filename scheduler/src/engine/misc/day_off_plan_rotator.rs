//! Port of `com.unifocus.watson.server.scheduler.engine.misc.DayOffPlanRotator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! DayOffPlanRotator.java` + its Groovy/Spock test
//! (`taps/src/junit/.../misc/DayOffPlanRotatorTest.groovy`) — a pure interaction test (asserts
//! `rotateEmployees` is invoked once per loaded `DayOffPlan` with the full employee list, via
//! `1 * dayOffPlan1.rotateEmployees(employees)`), not a value assertion, so nothing from it was
//! transcribed into `java_parity_tests`; the real logic it's exercising lives in
//! `DayOffPlan.rotateEmployees` (see `entity::day_off_plan`'s own tests instead).
//!
//! Java relies on Hibernate's implicit dirty-checking to persist the `DayOffPlan.lastRotated`/
//! `Employee.currentPatternNo` mutations `rotateEmployees` makes — this crate has no persistence
//! layer yet (`PLAN_SCHEDULER.md`'s Verification section), so `rotate_day_off_plans` returns the
//! mutated day-off plans and employees instead of silently discarding the mutation, for a future
//! save layer to write back.

use crate::engine::io::ports::EmployeePort;
use crate::engine::misc::ports::DayOffPlanPort;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::day_off_plan::DayOffPlan;
use crate::entity::employee::Employee;

/// `DayOffPlanRotator`.
pub struct DayOffPlanRotator<'a> {
    day_off_plans: &'a dyn DayOffPlanPort,
    employees: &'a dyn EmployeePort,
}

impl<'a> DayOffPlanRotator<'a> {
    pub fn new(day_off_plans: &'a dyn DayOffPlanPort, employees: &'a dyn EmployeePort) -> Self {
        Self {
            day_off_plans,
            employees,
        }
    }

    /// `rotateDayOffPlans(ScheduleModel)`.
    pub fn rotate_day_off_plans(
        &self,
        schedule_model: &ScheduleModel,
    ) -> (Vec<DayOffPlan>, Vec<Employee>) {
        let property = schedule_model.property();

        let mut all_employees_for_property = self.employees.find_all_for_property(property.id());
        let mut day_off_plans = self.day_off_plans.find_all_for_property(property.id());

        for day_off_plan in &mut day_off_plans {
            day_off_plan.rotate_employees(property, &mut all_employees_for_property);
        }

        (day_off_plans, all_employees_for_property)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::day_off_pattern::DayOffPattern;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::property::Property;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    struct FakeDayOffPlanPort {
        plans: Vec<DayOffPlan>,
    }

    impl DayOffPlanPort for FakeDayOffPlanPort {
        fn find_all_for_property(&self, _property_id: i32) -> Vec<DayOffPlan> {
            self.plans.clone()
        }
    }

    struct FakeEmployeePort {
        employees: Vec<Employee>,
    }

    impl EmployeePort for FakeEmployeePort {
        fn employees_active_with_jobs_during_period(
            &self,
            _date_range: &date_range_rs::DateRange,
            _job_ids: &[i32],
        ) -> Vec<Employee> {
            unimplemented!("not exercised by DayOffPlanRotator's tests")
        }

        fn find_all_for_property(&self, _property_id: i32) -> Vec<Employee> {
            self.employees.clone()
        }
    }

    fn employee_on_plan(id: i32, plan: &DayOffPlan, current_pattern_no: i32) -> Employee {
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
        .with_day_off_plan(plan.clone(), current_pattern_no)
    }

    #[test]
    fn rotates_every_loaded_plan_against_the_full_employee_list() {
        let period_start = LocalDate::of(2024, 1, 1);
        let range = DateRange::new(period_start, period_start);
        let property = Property::new(1, range).with_period_start_date(period_start);

        let plan1 = DayOffPlan::new(
            1,
            1,
            vec![
                DayOffPattern::new(1, Vec::new()),
                DayOffPattern::new(2, Vec::new()),
            ],
        );
        let plan2 = DayOffPlan::new(
            2,
            1,
            vec![
                DayOffPattern::new(1, Vec::new()),
                DayOffPattern::new(2, Vec::new()),
            ],
        );

        let employee1 = employee_on_plan(1, &plan1, 1);
        let employee2 = employee_on_plan(2, &plan2, 1);

        let day_off_plan_port = FakeDayOffPlanPort {
            plans: vec![plan1, plan2],
        };
        let employee_port = FakeEmployeePort {
            employees: vec![employee1, employee2],
        };
        let rotator = DayOffPlanRotator::new(&day_off_plan_port, &employee_port);

        let schedule_model = ScheduleModel::new(property, range);
        let (plans, employees) = rotator.rotate_day_off_plans(&schedule_model);

        assert_eq!(plans[0].last_rotated(), Some(period_start));
        assert_eq!(plans[1].last_rotated(), Some(period_start));
        assert!(employees.iter().all(|e| e.current_pattern_no() == 2));
    }
}
