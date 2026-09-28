//! Port of `com.unifocus.watson.server.scheduler.engine.io.RegularScheduleLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! RegularScheduleLoader.java`. Deferred from Phase 2 step 1 (see `PARITY_AUDIT.md` finding 20) —
//! not reachable from `ScheduleModelLoader.load()`'s call graph; `PermanentScheduleProcess`/
//! `RegularScheduleProcess` (steps 7-8) are its first real callers.

use crate::engine::io::ports::EmployeeRegularPeriodDAOPort;
use crate::engine::model::regular_schedules::RegularSchedules;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee_type::EmployeeType;

/// `RegularScheduleLoader`.
pub struct RegularScheduleLoader<'a> {
    employee_regular_periods: &'a dyn EmployeeRegularPeriodDAOPort,
}

impl<'a> RegularScheduleLoader<'a> {
    pub fn new(employee_regular_periods: &'a dyn EmployeeRegularPeriodDAOPort) -> Self {
        Self {
            employee_regular_periods,
        }
    }

    /// `loadRegularSchedules(ScheduleModel, EmployeeType)`.
    pub fn load_regular_schedules(
        &self,
        schedule_model: &ScheduleModel,
        employee_type: EmployeeType,
    ) -> RegularSchedules {
        let job_ids: Vec<i32> = schedule_model
            .job_list()
            .map(|job_list| job_list.job_ids().collect())
            .unwrap_or_default();

        let regular_periods = self
            .employee_regular_periods
            .find_for_jobs_and_employee_type(&job_ids, employee_type);

        RegularSchedules::new(schedule_model, regular_periods)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_regular_period::EmployeeRegularPeriod;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{DayOfWeek, LocalDate, LocalTime};

    struct FakeEmployeeRegularPeriodDAOPort {
        rows: Vec<EmployeeRegularPeriod>,
    }

    impl EmployeeRegularPeriodDAOPort for FakeEmployeeRegularPeriodDAOPort {
        fn find_for_jobs_and_employee_type(
            &self,
            _job_ids: &[i32],
            _employee_type: EmployeeType,
        ) -> Vec<EmployeeRegularPeriod> {
            self.rows.clone()
        }
    }

    fn job(id: i32) -> Assignment {
        Assignment::new(
            id,
            format!("Job {id}"),
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

    fn employee(id: i32) -> Employee {
        let job_status = crate::entity::employee_job_status::EmployeeJobStatus::new(
            1,
            None,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 7),
            true,
            1,
            LocalDate::of(2020, 1, 1),
            40.0,
            false,
            0,
        );

        Employee::new(
            id,
            format!("Employee {id}"),
            EmployeeType::Permanent,
            None,
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![job_status],
        )
    }

    #[test]
    fn loads_regular_schedules_for_employees_in_the_schedule_model() {
        let period = EmployeeRegularPeriod::new(
            1,
            Some(1),
            None,
            DayOfWeek::Monday,
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
            8.0,
        );
        let dao = FakeEmployeeRegularPeriodDAOPort { rows: vec![period] };
        let loader = RegularScheduleLoader::new(&dao);

        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let mut schedule_model = ScheduleModel::new(property, range);

        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));
        schedule_model.set_job_list(job_list);

        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(
            employee(1),
            ScheduleCalcDataSet::default(),
            0,
        ));
        schedule_model.set_employee_list(employee_list);

        let regular_schedules =
            loader.load_regular_schedules(&schedule_model, EmployeeType::Permanent);

        let matches = regular_schedules.regular_schedules_for_job_and_date(
            &schedule_model,
            schedule_model.job_list().unwrap().job_data(1).unwrap(),
            LocalDate::of(2024, 1, 1),
            &AlwaysActive,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].employee_id(), 1);
    }

    struct AlwaysActive;
    impl crate::engine::process::checkers::employee_job_status_checker::EmployeeActiveOnDatePort
        for AlwaysActive
    {
        fn is_active_on_date(&self, _employee: &Employee, _date: LocalDate) -> bool {
            true
        }
    }
}
