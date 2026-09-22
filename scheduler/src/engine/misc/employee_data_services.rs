//! Port of `com.unifocus.watson.server.scheduler.engine.misc.EmployeeDataServices`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! EmployeeDataServices.java` + its Groovy/Spock test
//! (`taps/src/junit/.../misc/EmployeeDataServicesTest.groovy`), transcribed below —
//! `getHoursAvailable() == null` falls back to `getWorkClass().getHoursAvailable()`, same
//! fallback `WeeklyAvailableHours::base_available_hours` already implements for a different
//! caller (no shared helper extracted — it's a one-line `unwrap_or_else`, not worth a shared
//! function for two call sites).

use crate::engine::model::employee_data::EmployeeData;

/// `EmployeeDataServices`.
#[derive(Debug, Default)]
pub struct EmployeeDataServices;

impl EmployeeDataServices {
    pub fn new() -> Self {
        Self
    }

    /// `sumEmployeeAvailableHours(List<EmployeeData>)`.
    pub fn sum_employee_available_hours(&self, employee_data_list: &[&EmployeeData]) -> f64 {
        employee_data_list
            .iter()
            .map(|employee_data| {
                let employee = employee_data.employee();

                employee
                    .hours_available()
                    .unwrap_or_else(|| employee.work_class().hours_available())
            })
            .sum()
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `EmployeeDataServicesTest.testSumEmployeeAvailableHours`
    //! (`taps/src/junit/.../misc/EmployeeDataServicesTest.groovy`).

    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::work_class::WorkClass;

    #[test]
    fn sums_available_hours_falling_back_to_work_class_when_unset() {
        let work_class = WorkClass::new(60.0, true);

        let employee1 = Employee::new(
            1,
            "Employee 1",
            EmployeeType::Regular,
            Some(40.0),
            work_class,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
        let employee2 = Employee::new(
            2,
            "Employee 2",
            EmployeeType::Regular,
            None,
            work_class,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );

        let employee_data1 = EmployeeData::new(employee1, Default::default(), 0);
        let employee_data2 = EmployeeData::new(employee2, Default::default(), 0);

        let services = EmployeeDataServices::new();
        let total = services.sum_employee_available_hours(&[&employee_data1, &employee_data2]);

        assert_eq!(total, 100.0);
    }
}
