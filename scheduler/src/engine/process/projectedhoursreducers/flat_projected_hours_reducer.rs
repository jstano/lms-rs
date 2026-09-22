//! Port of `com.unifocus.watson.server.scheduler.engine.process.projectedhoursreducers.
//! FlatProjectedHoursReducer`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! projectedhoursreducers/FlatProjectedHoursReducer.java` + its Groovy/Spock test
//! (`FlatProjectedHoursReducerTest.groovy`), transcribed below. Does nothing if
//! `schedule_model.employee_list()` is unset — Java would NPE there; every real pipeline run has
//! it set by step 1 (`EmployeeListLoader`) before step 3 runs.

use crate::common::numbers::round_hours;
use crate::engine::misc::employee_data_services::EmployeeDataServices;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::projectedhoursreducers::projected_hours_reducer::ProjectedHoursReducer;

const DAYS_PER_WEEK: f64 = 7.0;

/// `FlatProjectedHoursReducer`.
pub struct FlatProjectedHoursReducer<'a> {
    employee_data_services: &'a EmployeeDataServices,
}

impl<'a> FlatProjectedHoursReducer<'a> {
    pub fn new(employee_data_services: &'a EmployeeDataServices) -> Self {
        Self {
            employee_data_services,
        }
    }
}

impl ProjectedHoursReducer for FlatProjectedHoursReducer<'_> {
    fn reduce_projected_hours(&self, schedule_model: &ScheduleModel, job_data: &mut JobData) {
        let Some(employee_list) = schedule_model.employee_list() else {
            return;
        };

        let date_range = *schedule_model.date_range();
        let projected_hours = job_data
            .projected_hours()
            .sum_hours_for_date_range(&date_range);

        let employees_with_job =
            employee_list.employees_with_job(job_data.job().id(), date_range.start_date());
        let available_hours = self
            .employee_data_services
            .sum_employee_available_hours(&employees_with_job);

        if projected_hours > available_hours {
            let projected_hours_per_day = round_hours(available_hours / DAYS_PER_WEEK);

            for date in date_range.dates() {
                job_data
                    .projected_hours_mut()
                    .set_hours_for_date(date, projected_hours_per_day);
            }
        }
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `FlatProjectedHoursReducerTest.testReduceProjectedHours`
    //! (`taps/src/junit/.../projectedhoursreducers/FlatProjectedHoursReducerTest.groovy`).
    //! The Groovy test mocks `EmployeeDataServices.sumEmployeeAvailableHours` directly (`>> 75.0`)
    //! rather than constructing employees with real hours; ported the same way — one employee
    //! whose available hours sum to 75.0 (`work_class` hours available, no per-employee
    //! override), rather than reproducing the mock's exact employee count.

    use super::*;
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::property::Property;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    fn job() -> Assignment {
        Assignment::new(
            1,
            "Job",
            false,
            None,
            None,
            None,
            false,
            Vec::<AssignmentSortOrder>::new(),
            Vec::new(),
            None,
        )
    }

    #[test]
    fn reduces_every_date_to_the_rounded_available_hours_per_day_when_projected_exceeds_available()
    {
        let start_date = LocalDate::of(2013, 1, 1);
        let end_date = LocalDate::of(2013, 1, 2);
        let date_range = DateRange::new(start_date, end_date);

        let mut job_data = JobData::new(job());
        job_data
            .projected_hours_mut()
            .add_hours_to_date(start_date, 100.0);
        job_data
            .projected_hours_mut()
            .add_hours_to_date(end_date, 200.0);

        let job_status = EmployeeJobStatus::new(
            1,
            None,
            LocalDate::of(2000, 1, 1),
            LocalDate::of(2099, 1, 1),
            true,
            1,
            LocalDate::of(2000, 1, 1),
            0.0,
            false,
            1,
        );
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            None,
            WorkClass::new(75.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![job_status],
        );
        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(employee, Default::default(), 0));

        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);
        schedule_model.set_employee_list(employee_list);

        let employee_data_services = EmployeeDataServices::new();
        let reducer = FlatProjectedHoursReducer::new(&employee_data_services);

        reducer.reduce_projected_hours(&schedule_model, &mut job_data);

        assert_eq!(job_data.projected_hours().hours_for_date(start_date), 10.71);
        assert_eq!(job_data.projected_hours().hours_for_date(end_date), 10.71);
    }
}
