//! Port of `com.unifocus.watson.server.scheduler.engine.io.SaveScheduleSnapshotService`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! SaveScheduleSnapshotService.java`. `scheduleModel.getProgress().ping()` is dropped, not
//! stubbed — `Progress` isn't modeled in this crate (`ScheduleModel`'s own doc). The actual clone
//! (`EmployeeShiftCloner`) is deferred behind [`EmployeeShiftClonerPort`](crate::engine::io::
//! ports::EmployeeShiftClonerPort) — see that trait's doc; this service's own real work is the
//! delete/load/clone/save *sequencing*, which is ported for real.

use crate::engine::io::ports::{EmployeeShiftClonerPort, EmployeeShiftPort, PlannedShiftPort};
use crate::engine::model::schedule_model::ScheduleModel;
use std::collections::HashSet;

/// `SaveScheduleSnapshotService`.
pub struct SaveScheduleSnapshotService<'a> {
    employee_shifts: &'a dyn EmployeeShiftPort,
    planned_shifts: &'a dyn PlannedShiftPort,
    employee_shift_cloner: &'a dyn EmployeeShiftClonerPort,
}

impl<'a> SaveScheduleSnapshotService<'a> {
    pub fn new(
        employee_shifts: &'a dyn EmployeeShiftPort,
        planned_shifts: &'a dyn PlannedShiftPort,
        employee_shift_cloner: &'a dyn EmployeeShiftClonerPort,
    ) -> Self {
        Self {
            employee_shifts,
            planned_shifts,
            employee_shift_cloner,
        }
    }

    /// `saveScheduleSnapshot(ScheduleModel)`.
    pub fn save_schedule_snapshot(&self, schedule_model: &ScheduleModel) {
        self.delete_old_generated_schedules(schedule_model);

        self.copy_scheduled_shifts_to_generated_shifts(schedule_model);
    }

    /// `deleteOldGeneratedSchedules(ScheduleModel)`.
    fn delete_old_generated_schedules(&self, schedule_model: &ScheduleModel) {
        let job_ids: HashSet<i32> = schedule_model
            .job_list()
            .map(|jobs| jobs.job_ids().collect())
            .unwrap_or_default();

        self.employee_shifts
            .bulk_delete_employee_shifts_for_jobs_in_current_property(
                &job_ids,
                schedule_model.date_range(),
            );

        self.planned_shifts
            .bulk_delete_planned_shifts_for_current_property(&job_ids, schedule_model.date_range());
    }

    /// `copyScheduledShiftsToGeneratedShifts(ScheduleModel)`.
    fn copy_scheduled_shifts_to_generated_shifts(&self, schedule_model: &ScheduleModel) {
        let employee_ids: HashSet<i32> = schedule_model
            .employee_list()
            .map(|employees| employees.employee_ids().collect())
            .unwrap_or_default();

        for employee_shift in self
            .employee_shifts
            .employee_schedule_shifts_for_period(&employee_ids, schedule_model.date_range())
        {
            let cloned_employee_shift = self
                .employee_shift_cloner
                .clone_employee_shift_as_generated(&employee_shift);

            if let Some(planned_shift) = cloned_employee_shift.planned_shift() {
                self.planned_shifts.save(&planned_shift);
            }
            self.employee_shifts.save(&cloned_employee_shift);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::cell::RefCell;

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

    #[derive(Default)]
    struct FakePorts {
        deleted_employee_shift_job_ids: RefCell<Option<HashSet<i32>>>,
        deleted_planned_shift_job_ids: RefCell<Option<HashSet<i32>>>,
        loaded_employee_ids: RefCell<Option<HashSet<i32>>>,
        to_load: Vec<EmployeeShift>,
        saved_employee_shifts: RefCell<Vec<EmployeeShift>>,
        saved_planned_shifts: RefCell<Vec<PlannedShift>>,
    }

    impl EmployeeShiftPort for FakePorts {
        fn evict(&self, _employee_shift_id: i32) {}

        fn bulk_delete_by_shift_id(&self, _shift_ids: &HashSet<i32>) {}

        fn save(&self, employee_shift: &EmployeeShift) {
            self.saved_employee_shifts
                .borrow_mut()
                .push(employee_shift.clone());
        }

        fn bulk_delete_employee_shifts_for_jobs_in_current_property(
            &self,
            job_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) {
            *self.deleted_employee_shift_job_ids.borrow_mut() = Some(job_ids.clone());
        }

        fn employee_schedule_shifts_for_period(
            &self,
            employee_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) -> Vec<EmployeeShift> {
            *self.loaded_employee_ids.borrow_mut() = Some(employee_ids.clone());
            self.to_load.clone()
        }
    }

    impl PlannedShiftPort for FakePorts {
        fn save(&self, planned_shift: &PlannedShift) {
            self.saved_planned_shifts.borrow_mut().push(*planned_shift);
        }

        fn bulk_delete_planned_shifts_for_current_property(
            &self,
            job_ids: &HashSet<i32>,
            _date_range: &DateRange,
        ) {
            *self.deleted_planned_shift_job_ids.borrow_mut() = Some(job_ids.clone());
        }
    }

    impl EmployeeShiftClonerPort for FakePorts {
        fn clone_employee_shift_as_generated(
            &self,
            employee_shift: &EmployeeShift,
        ) -> EmployeeShift {
            employee_shift.clone()
        }
    }

    #[test]
    fn deletes_generated_schedules_for_the_current_job_list_then_clones_and_saves_scheduled_shifts()
    {
        let date_range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));

        let mut job_list = JobList::new();
        job_list.add_job_data(JobData::new(job(1)));

        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);
        schedule_model.set_job_list(job_list);
        schedule_model.set_employee_list(EmployeeList::new());

        let planned_shift = PlannedShift::new(
            10,
            1,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 1).at_time(joda_rs::LocalTime::of(9, 0, 0)),
            8.0,
            None,
        );
        let scheduled_shift = EmployeeShift::new(
            20,
            LocalDate::of(2024, 1, 1),
            LocalDate::of(2024, 1, 1).at_time(joda_rs::LocalTime::of(9, 0, 0)),
            1,
            None,
        )
        .with_planned_shift(planned_shift);

        let ports = FakePorts {
            to_load: vec![scheduled_shift.clone()],
            ..Default::default()
        };
        let service = SaveScheduleSnapshotService::new(&ports, &ports, &ports);

        service.save_schedule_snapshot(&schedule_model);

        assert_eq!(
            *ports.deleted_employee_shift_job_ids.borrow(),
            Some(HashSet::from([1]))
        );
        assert_eq!(
            *ports.deleted_planned_shift_job_ids.borrow(),
            Some(HashSet::from([1]))
        );
        assert_eq!(*ports.saved_employee_shifts.borrow(), vec![scheduled_shift]);
        assert_eq!(*ports.saved_planned_shifts.borrow(), vec![planned_shift]);
    }
}
