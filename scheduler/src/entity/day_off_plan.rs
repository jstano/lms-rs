//! Port of `com.unifocus.watson.server.hibernate.entity.DayOffPlan`.
//!
//! Ground truth: `getDayOffPattern(int)` (read by `EmployeeDayOffRotationPlanChecker`, Phase 1)
//! plus `rotateEmployees(List<Employee>)` (`DayOffPlanRotator`, Phase 2 step 2) — the rest of the
//! class not read directly.
//!
//! `id`/`number_weeks`/`last_rotated` were added for `rotate_employees`. `property` is a method
//! parameter, not a field: Java holds a live `Property` reference and reads
//! `getPeriodStartDate()` fresh on every call — same "read live data through a parameter, not an
//! owned copy" treatment as `RegularSchedule.job()` (`PARITY_AUDIT.md` finding 9).

use crate::entity::day_off_pattern::DayOffPattern;
use crate::entity::employee::Employee;
use crate::entity::property::Property;
use joda_rs::LocalDate;

/// A day-off rotation plan, made up of numbered patterns. `DayOffPlan`.
#[derive(Debug, Clone, PartialEq)]
pub struct DayOffPlan {
    id: i32,
    number_weeks: i32,
    last_rotated: Option<LocalDate>,
    patterns: Vec<DayOffPattern>,
}

impl DayOffPlan {
    pub fn new(id: i32, number_weeks: i32, patterns: Vec<DayOffPattern>) -> Self {
        Self {
            id,
            number_weeks,
            last_rotated: None,
            patterns,
        }
    }

    /// `setLastRotated(LocalDate)`, set ahead of time rather than through the setter (only
    /// `rotate_employees` itself calls the real setter, at the end of a successful rotation).
    #[must_use]
    pub fn with_last_rotated(mut self, last_rotated: LocalDate) -> Self {
        self.last_rotated = Some(last_rotated);
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getLastRotated()`.
    pub fn last_rotated(&self) -> Option<LocalDate> {
        self.last_rotated
    }

    /// `getDayOffPattern(int)`.
    pub fn day_off_pattern(&self, pattern_no: i32) -> Option<&DayOffPattern> {
        self.patterns.iter().find(|p| p.pattern_no() == pattern_no)
    }

    /// `rotateEmployees(List<Employee>)`. Does nothing if `property.period_start_date()` is
    /// unset — Java would NPE there; every real pipeline run has it set by the time step 2 runs
    /// (`ScheduleModelLoader`, step 1, always loads a real property first).
    pub fn rotate_employees(&mut self, property: &Property, employees: &mut [Employee]) {
        let Some(period_start_date) = property.period_start_date() else {
            return;
        };

        let date_to_rotate = match self.last_rotated {
            None => period_start_date,
            Some(last_rotated) => last_rotated.plus_weeks(i64::from(self.number_weeks)),
        };

        if !period_start_date.is_on_or_after(date_to_rotate) {
            return;
        }

        for employee in employees.iter_mut() {
            let is_this_plan = employee.day_off_plan().map(DayOffPlan::id) == Some(self.id);

            if is_this_plan {
                let mut current_pattern_no = employee.current_pattern_no() + 1;

                if current_pattern_no > self.patterns.len() as i32 {
                    current_pattern_no = 1;
                }

                employee.set_current_pattern_no(current_pattern_no);
            }
        }

        self.last_rotated = Some(period_start_date);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    fn property_with_period_start(date: LocalDate) -> Property {
        let range = DateRange::new(date, date);
        Property::new(1, range).with_period_start_date(date)
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
    fn first_rotation_advances_employees_on_this_plan_and_records_last_rotated() {
        let period_start = LocalDate::of(2024, 1, 1);
        let property = property_with_period_start(period_start);

        let mut plan = DayOffPlan::new(
            1,
            1,
            vec![
                DayOffPattern::new(1, Vec::new()),
                DayOffPattern::new(2, Vec::new()),
            ],
        );
        let mut employees = vec![employee_on_plan(1, &plan, 1), employee_on_plan(2, &plan, 2)];

        plan.rotate_employees(&property, &mut employees);

        assert_eq!(employees[0].current_pattern_no(), 2);
        assert_eq!(employees[1].current_pattern_no(), 1); // wraps past the last pattern
        assert_eq!(plan.last_rotated(), Some(period_start));
    }

    #[test]
    fn employees_on_a_different_plan_are_left_alone() {
        let period_start = LocalDate::of(2024, 1, 1);
        let property = property_with_period_start(period_start);

        let mut plan = DayOffPlan::new(1, 1, vec![DayOffPattern::new(1, Vec::new())]);
        let other_plan = DayOffPlan::new(2, 1, vec![DayOffPattern::new(1, Vec::new())]);
        let mut employees = vec![employee_on_plan(1, &other_plan, 1)];

        plan.rotate_employees(&property, &mut employees);

        assert_eq!(employees[0].current_pattern_no(), 1);
    }

    #[test]
    fn does_not_rotate_again_before_number_weeks_have_elapsed() {
        let last_rotated = LocalDate::of(2024, 1, 1);
        let period_start = last_rotated.plus_weeks(1); // one week later, plan rotates every 2
        let property = property_with_period_start(period_start);

        let mut plan = DayOffPlan::new(1, 2, vec![DayOffPattern::new(1, Vec::new())])
            .with_last_rotated(last_rotated);
        let mut employees = vec![employee_on_plan(1, &plan, 1)];

        plan.rotate_employees(&property, &mut employees);

        assert_eq!(employees[0].current_pattern_no(), 1);
        assert_eq!(plan.last_rotated(), Some(last_rotated));
    }
}
