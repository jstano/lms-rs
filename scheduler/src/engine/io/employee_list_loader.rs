//! Port of `com.unifocus.watson.server.scheduler.engine.io.EmployeeListLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! EmployeeListLoader.java`.
//!
//! Java seeds `new Random(dateRange.getStartDate().toDateTimeAtStartOfDay().toDate().getTime())`
//! with the JVM's default time zone; this ports `LocalDate::at_start_of_day().epoch_seconds()`
//! (UTC — `joda_rs` has no default-zone concept) instead. The seed only has to be stable within
//! one Rust run and distinct per employee (`EmployeeData.random` is a last-resort sort tiebreak,
//! never compared across processes or persisted — see `common::java_random`'s doc), so the
//! time-zone divergence from Java's actual seed value isn't behavior-affecting here.
//!
//! `getEmpIDs` (build a `Set<Integer>` of employee ids) isn't a separate function — folded into
//! `load` as a plain `.map(Employee::id).collect()`.

use crate::common::java_random::JavaRandom;
use crate::engine::io::ports::{EmployeePort, SchedulesTimeCardPort};
use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::employee_list::EmployeeList;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee_calculation_mode::EmployeeCalculationMode;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use date_range_rs::DateRange;
use std::collections::HashMap;

/// `EmployeeListLoader`.
pub struct EmployeeListLoader<'a> {
    employees: &'a dyn EmployeePort,
    schedules_time_card: &'a dyn SchedulesTimeCardPort,
}

impl<'a> EmployeeListLoader<'a> {
    pub fn new(
        employees: &'a dyn EmployeePort,
        schedules_time_card: &'a dyn SchedulesTimeCardPort,
    ) -> Self {
        Self {
            employees,
            schedules_time_card,
        }
    }

    /// `load(ScheduleModel)`.
    pub fn load(&self, schedule_model: &ScheduleModel) -> EmployeeList {
        let date_range = schedule_model.date_range();
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| job_list.job_ids().collect())
            .unwrap_or_default();

        let employees = self
            .employees
            .employees_active_with_jobs_during_period(date_range, &job_ids);
        let employee_ids: Vec<i32> = employees.iter().map(|e| e.id()).collect();

        let data_sets = self
            .schedules_time_card
            .load_schedule_calc_dataset(date_range, &employee_ids);

        self.create_employee_list(date_range, data_sets)
    }

    fn create_employee_list(
        &self,
        date_range: &DateRange,
        data_sets: HashMap<i32, ScheduleCalcDataSet>,
    ) -> EmployeeList {
        let seed = date_range.start_date().at_start_of_day().epoch_seconds() * 1_000;
        let mut random_generator = JavaRandom::new(seed);

        let mut employee_list = EmployeeList::new();

        for mut data_set in data_sets.into_values() {
            data_set.set_calculation_mode(EmployeeCalculationMode::AutoSchedule);
            data_set.set_dataset_start_date(date_range.start_date());

            let Some(employee) = data_set.employee().cloned() else {
                continue;
            };

            employee_list.add_employee_data(EmployeeData::new(
                employee,
                data_set,
                random_generator.next_i32(),
            ));
        }

        employee_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::work_class::WorkClass;
    use joda_rs::LocalDate;

    struct FakeEmployeePort {
        employees: Vec<Employee>,
    }

    impl EmployeePort for FakeEmployeePort {
        fn employees_active_with_jobs_during_period(
            &self,
            _date_range: &DateRange,
            _job_ids: &[i32],
        ) -> Vec<Employee> {
            self.employees.clone()
        }

        fn find_all_for_property(&self, _property_id: i32) -> Vec<Employee> {
            self.employees.clone()
        }
    }

    struct FakeSchedulesTimeCardPort {
        data_sets: HashMap<i32, ScheduleCalcDataSet>,
    }

    impl SchedulesTimeCardPort for FakeSchedulesTimeCardPort {
        fn load_schedule_calc_dataset(
            &self,
            _date_range: &DateRange,
            _employee_ids: &[i32],
        ) -> HashMap<i32, ScheduleCalcDataSet> {
            self.data_sets.clone()
        }
    }

    fn employee(id: i32) -> Employee {
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
    }

    #[test]
    fn wires_each_data_set_to_its_employee_with_auto_schedule_mode_and_the_dataset_start_date() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let mut data_sets = HashMap::new();
        let mut data_set = ScheduleCalcDataSet::default();
        data_set.set_employee(employee(1));
        data_sets.insert(1, data_set);

        let employee_port = FakeEmployeePort {
            employees: vec![employee(1)],
        };
        let time_card_port = FakeSchedulesTimeCardPort { data_sets };
        let loader = EmployeeListLoader::new(&employee_port, &time_card_port);

        let property = crate::entity::property::Property::new(1, range);
        let schedule_model = ScheduleModel::new(property, range);

        let employee_list = loader.load(&schedule_model);

        let loaded = employee_list.employee_data(1).unwrap();
        assert_eq!(loaded.employee().id(), 1);
        assert_eq!(
            loaded.data_set().calculation_mode(),
            Some(EmployeeCalculationMode::AutoSchedule)
        );
        assert_eq!(
            loaded.data_set().dataset_start_date(),
            Some(range.start_date())
        );
    }

    #[test]
    fn skips_data_sets_with_no_employee() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let mut data_sets = HashMap::new();
        data_sets.insert(1, ScheduleCalcDataSet::default());

        let employee_port = FakeEmployeePort { employees: vec![] };
        let time_card_port = FakeSchedulesTimeCardPort { data_sets };
        let loader = EmployeeListLoader::new(&employee_port, &time_card_port);

        let property = crate::entity::property::Property::new(1, range);
        let schedule_model = ScheduleModel::new(property, range);

        let employee_list = loader.load(&schedule_model);

        assert_eq!(employee_list.employee_ids().count(), 0);
    }
}
