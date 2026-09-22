//! Port of `com.unifocus.watson.server.scheduler.engine.process.projectedhoursreducers.
//! PercentProjectedHoursReducer`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! projectedhoursreducers/PercentProjectedHoursReducer.java` + its Groovy/Spock test
//! (`PercentProjectedHoursReducerTest.groovy`), transcribed below. Does nothing if
//! `schedule_model.employee_list()` is unset — same reasoning as `FlatProjectedHoursReducer`'s
//! own doc.

use crate::engine::misc::employee_data_services::EmployeeDataServices;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::projectedhoursreducers::projected_hours_reducer::ProjectedHoursReducer;

/// `PercentProjectedHoursReducer`.
pub struct PercentProjectedHoursReducer<'a> {
    employee_data_services: &'a EmployeeDataServices,
}

impl<'a> PercentProjectedHoursReducer<'a> {
    pub fn new(employee_data_services: &'a EmployeeDataServices) -> Self {
        Self {
            employee_data_services,
        }
    }
}

impl ProjectedHoursReducer for PercentProjectedHoursReducer<'_> {
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

        // If projected hours for the week are more than the hours available, adjust each day's
        // projected hours by the percent short.
        if projected_hours > available_hours {
            let percent_short = 1.0 - ((projected_hours - available_hours) / projected_hours);

            for date in date_range.dates() {
                let projected_hours_for_date = job_data.projected_hours().hours_for_date(date);

                job_data
                    .projected_hours_mut()
                    .set_hours_for_date(date, projected_hours_for_date * percent_short);
            }
        }
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `PercentProjectedHoursReducerTest.testReduceProjectedHours`
    //! (`taps/src/junit/.../projectedhoursreducers/PercentProjectedHoursReducerTest.groovy`).
    //! Same "mock `sumEmployeeAvailableHours` directly" simplification as
    //! `FlatProjectedHoursReducer`'s own transcribed test.

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
    fn scales_each_dates_projected_hours_by_the_percent_short() {
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
        let reducer = PercentProjectedHoursReducer::new(&employee_data_services);

        reducer.reduce_projected_hours(&schedule_model, &mut job_data);

        assert_eq!(job_data.projected_hours().hours_for_date(start_date), 25.0);
        assert_eq!(job_data.projected_hours().hours_for_date(end_date), 50.0);
    }
}
