//! Port of `com.unifocus.watson.server.scheduler.engine.process.schedulebalancers.
//! EmployeeAvailableHoursBalancer`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! schedulebalancers/EmployeeAvailableHoursBalancer.java` + its Groovy/Spock test
//! (`EmployeeAvailableHoursBalancerTest.groovy`), transcribed below. Does nothing if
//! `schedule_model.employee_list()` is unset — same "Java would NPE, every real pipeline run has
//! it set by step 1" reasoning as `FlatProjectedHoursReducer`'s own doc.
//!
//! `JobData.balance_factor` already defaults to `1.0` (Phase 1's port), which is exactly what
//! Java's field default plus this method's `if (availableHours > projectedHours)` guard leaves
//! it at when the guard doesn't fire — the Groovy test's first two cases (`ahours <= phours`)
//! assert `1.0` for that reason, not because the method sets it.

use crate::engine::misc::employee_data_services::EmployeeDataServices;
use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;

/// `EmployeeAvailableHoursBalancer`.
pub struct EmployeeAvailableHoursBalancer<'a> {
    employee_data_services: &'a EmployeeDataServices,
}

impl<'a> EmployeeAvailableHoursBalancer<'a> {
    pub fn new(employee_data_services: &'a EmployeeDataServices) -> Self {
        Self {
            employee_data_services,
        }
    }

    /// `computeBalanceFactor(ScheduleModel, JobData)`.
    pub fn compute_balance_factor(&self, schedule_model: &ScheduleModel, job_data: &mut JobData) {
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

        if available_hours > projected_hours {
            job_data
                .set_balance_factor(1.0 - ((available_hours - projected_hours) / available_hours));
        }
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `EmployeeAvailableHoursBalancerTest.testComputeBalanceFactor`
    //! (`taps/src/junit/.../schedulebalancers/EmployeeAvailableHoursBalancerTest.groovy`), a
    //! `where:` table over `(ahours, phours, expectedResult)`. The Groovy test mocks
    //! `EmployeeDataServices.sumEmployeeAvailableHours` directly and passes an empty employee
    //! list; ported with one real employee whose work-class hours available equal `ahours`
    //! instead (this crate has no mocking layer — same substitution
    //! `FlatProjectedHoursReducer`'s/`PercentProjectedHoursReducer`'s own transcribed tests use).

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
    use rstest::rstest;

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

    #[rstest]
    #[case(100.0, 100.0, 1.0)]
    #[case(50.0, 100.0, 1.0)]
    #[case(125.0, 100.0, 0.8)]
    #[case(200.0, 100.0, 0.5)]
    fn computes_the_balance_factor_from_available_and_projected_hours(
        #[case] available_hours: f64,
        #[case] projected_hours: f64,
        #[case] expected: f64,
    ) {
        let start_date = LocalDate::of(2013, 1, 1);
        let end_date = LocalDate::of(2013, 1, 7);
        let date_range = DateRange::new(start_date, end_date);

        let mut job_data = JobData::new(job());
        job_data
            .projected_hours_mut()
            .add_hours_to_date(start_date, projected_hours);

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
            WorkClass::new(available_hours, true),
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
        let balancer = EmployeeAvailableHoursBalancer::new(&employee_data_services);

        balancer.compute_balance_factor(&schedule_model, &mut job_data);

        assert_eq!(job_data.balance_factor(), expected);
    }
}
