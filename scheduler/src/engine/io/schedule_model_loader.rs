//! Port of `com.unifocus.watson.server.scheduler.engine.io.ScheduleModelLoader`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! ScheduleModelLoader.java`. The `ResourceMgr.lookup(...)`/`progress.setMessage(...)` calls
//! before each phase aren't ported — `ScheduleModel` doesn't carry `progress` yet (see its own
//! doc); add these back if/when a caller needs progress reporting.
//!
//! Top-level Phase 2 step 1 entry point: `load` returns `None` only if `ScheduleModelCreator`
//! couldn't resolve the property (see its own doc).

use crate::engine::generate_schedules_parameters::GenerateSchedulesParameters;
use crate::engine::io::employee_list_loader::EmployeeListLoader;
use crate::engine::io::job_list_loader::JobListLoader;
use crate::engine::io::planned_shift_loader::PlannedShiftLoader;
use crate::engine::io::pre_schedule_job_loader::PreScheduleJobLoader;
use crate::engine::io::schedule_model_creator::ScheduleModelCreator;
use crate::engine::model::schedule_model::ScheduleModel;

/// `ScheduleModelLoader`.
pub struct ScheduleModelLoader<'a> {
    schedule_model_creator: ScheduleModelCreator<'a>,
    job_list_loader: JobListLoader<'a>,
    pre_schedule_job_loader: PreScheduleJobLoader<'a>,
    employee_list_loader: EmployeeListLoader<'a>,
    planned_shift_loader: PlannedShiftLoader<'a>,
}

impl<'a> ScheduleModelLoader<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schedule_model_creator: ScheduleModelCreator<'a>,
        job_list_loader: JobListLoader<'a>,
        pre_schedule_job_loader: PreScheduleJobLoader<'a>,
        employee_list_loader: EmployeeListLoader<'a>,
        planned_shift_loader: PlannedShiftLoader<'a>,
    ) -> Self {
        Self {
            schedule_model_creator,
            job_list_loader,
            pre_schedule_job_loader,
            employee_list_loader,
            planned_shift_loader,
        }
    }

    /// `load(GenerateSchedulesParameters, Progress)`.
    pub fn load(&self, params: &GenerateSchedulesParameters) -> Option<ScheduleModel> {
        let mut schedule_model = self.schedule_model_creator.create_schedule_model(params)?;

        self.load_jobs(params, &mut schedule_model);
        self.load_planned_shifts(&mut schedule_model);
        self.load_employees(&mut schedule_model);

        Some(schedule_model)
    }

    fn load_jobs(&self, params: &GenerateSchedulesParameters, schedule_model: &mut ScheduleModel) {
        schedule_model.set_job_list(self.job_list_loader.load_job_list(params));

        self.pre_schedule_job_loader
            .load_pre_scheduled_jobs(schedule_model);
    }

    fn load_planned_shifts(&self, schedule_model: &mut ScheduleModel) {
        let property_id = schedule_model.property().id();
        let date_range = *schedule_model.date_range();

        if let Some(job_list) = schedule_model.job_list_mut() {
            self.planned_shift_loader
                .load_planned_shifts(property_id, &date_range, job_list);
        }
    }

    fn load_employees(&self, schedule_model: &mut ScheduleModel) {
        let employee_list = self.employee_list_loader.load(schedule_model);
        schedule_model.set_employee_list(employee_list);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::io::forecast_planned_shift_loader::ForecastPlannedShiftLoader;
    use crate::engine::io::original_projected_hours_loader::OriginalProjectedHoursLoader;
    use crate::engine::io::ports::{
        EmployeePort, JobLoaderPort, OriginalProjectedHoursResult, PlannedShiftQueryPort,
        PreScheduleJobclassPort, PropertyPort, SchedulesTimeCardPort,
    };
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::pre_schedule_jobclass::PreScheduleJobclass;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalDateTime};
    use std::collections::HashMap;

    struct FakePropertyPort {
        property: Property,
    }

    impl PropertyPort for FakePropertyPort {
        fn find_by_id(&self, _id: i32) -> Option<Property> {
            Some(self.property.clone())
        }
    }

    struct FakeJobLoaderPort {
        jobs: Vec<Assignment>,
    }

    impl JobLoaderPort for FakeJobLoaderPort {
        fn load_jobs(&self, _params: &GenerateSchedulesParameters) -> Vec<Assignment> {
            self.jobs.clone()
        }
    }

    struct FakePreScheduleJobclassPort;

    impl PreScheduleJobclassPort for FakePreScheduleJobclassPort {
        fn find_all_for_property(&self, _property_id: i32) -> Vec<PreScheduleJobclass> {
            Vec::new()
        }
    }

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

    struct FakePlannedShiftQueryPort {
        shifts: Vec<PlannedShift>,
    }

    impl PlannedShiftQueryPort for FakePlannedShiftQueryPort {
        fn forecast_planned_shifts_for_jobs_and_dates(
            &self,
            _property_id: i32,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<PlannedShift> {
            self.shifts.clone()
        }

        fn original_projected_hours_by_job_and_date(
            &self,
            _property_id: i32,
            _job_ids: &[i32],
            _date_range: &DateRange,
        ) -> Vec<OriginalProjectedHoursResult> {
            Vec::new()
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
    fn loads_a_schedule_model_with_jobs_planned_shifts_and_employees_wired_together() {
        let date = LocalDate::of(2024, 1, 1);
        let range = DateRange::new(date, date);

        let property_port = FakePropertyPort {
            property: Property::new(1, range),
        };
        let job_loader_port = FakeJobLoaderPort { jobs: vec![job(1)] };
        let pre_schedule_port = FakePreScheduleJobclassPort;

        let mut data_set = ScheduleCalcDataSet::default();
        data_set.set_employee(employee(1));
        let mut data_sets = HashMap::new();
        data_sets.insert(1, data_set);
        let employee_port = FakeEmployeePort {
            employees: vec![employee(1)],
        };
        let time_card_port = FakeSchedulesTimeCardPort { data_sets };

        let planned_shift_port = FakePlannedShiftQueryPort {
            shifts: vec![PlannedShift::new(
                1,
                1,
                date,
                LocalDateTime::of(2024, 1, 1, 9, 0, 0),
                8.0,
                None,
            )],
        };

        let loader = ScheduleModelLoader::new(
            ScheduleModelCreator::new(&property_port),
            JobListLoader::new(&job_loader_port),
            PreScheduleJobLoader::new(&pre_schedule_port),
            EmployeeListLoader::new(&employee_port, &time_card_port),
            PlannedShiftLoader::new(
                ForecastPlannedShiftLoader::new(&planned_shift_port),
                OriginalProjectedHoursLoader::new(&planned_shift_port),
            ),
        );

        let params = GenerateSchedulesParameters::new(range, 1);
        let schedule_model = loader.load(&params).unwrap();

        let job_list = schedule_model.job_list().unwrap();
        assert!(job_list.contains_job(1));
        assert_eq!(job_list.job_data(1).unwrap().planned_shifts().len(), 1);

        let employee_list = schedule_model.employee_list().unwrap();
        assert!(employee_list.employee_data(1).is_some());
    }

    #[test]
    fn returns_none_when_the_property_does_not_resolve() {
        struct MissingPropertyPort;
        impl PropertyPort for MissingPropertyPort {
            fn find_by_id(&self, _id: i32) -> Option<Property> {
                None
            }
        }

        let property_port = MissingPropertyPort;
        let job_loader_port = FakeJobLoaderPort { jobs: Vec::new() };
        let pre_schedule_port = FakePreScheduleJobclassPort;
        let employee_port = FakeEmployeePort {
            employees: Vec::new(),
        };
        let time_card_port = FakeSchedulesTimeCardPort {
            data_sets: HashMap::new(),
        };
        let planned_shift_port = FakePlannedShiftQueryPort { shifts: Vec::new() };

        let loader = ScheduleModelLoader::new(
            ScheduleModelCreator::new(&property_port),
            JobListLoader::new(&job_loader_port),
            PreScheduleJobLoader::new(&pre_schedule_port),
            EmployeeListLoader::new(&employee_port, &time_card_port),
            PlannedShiftLoader::new(
                ForecastPlannedShiftLoader::new(&planned_shift_port),
                OriginalProjectedHoursLoader::new(&planned_shift_port),
            ),
        );

        let date = LocalDate::of(2024, 1, 1);
        let range = DateRange::new(date, date);
        let params = GenerateSchedulesParameters::new(range, 1);

        assert!(loader.load(&params).is_none());
    }
}
