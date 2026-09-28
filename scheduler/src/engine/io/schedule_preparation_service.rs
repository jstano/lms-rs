//! Port of `com.unifocus.watson.server.scheduler.engine.io.SchedulePreparationService`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! SchedulePreparationService.java` + its Groovy/Spock test
//! (`SchedulePreparationServiceTest.groovy`), transcribed below. `progress.setMessage(...)`
//! isn't ported — `ScheduleModel` doesn't carry `progress` yet (see its own doc).
//!
//! **Identity-shared mutation order trap**: Java's `removeEmployeeShift` calls
//! `oldShiftList.addEmployeeShift(employeeShift)` *before* `employeeShift.setPlannedShift(null)`
//! — but since both `oldShiftList`'s copy and the local variable are the *same* Hibernate-managed
//! object, nulling it afterward changes what `oldShiftList` holds too. `EmployeeShift` here is a
//! `Copy` value type, so the two operations don't share identity — this port nulls
//! `planned_shift` *before* adding to `old_shift_list`, to land on the same final state Java's
//! shared mutation produces, not the same statement order.

use crate::engine::io::ports::EmployeeShiftPort;
use crate::engine::model::schedule_model::ScheduleModel;

/// `SchedulePreparationService`.
pub struct SchedulePreparationService<'a> {
    employee_shifts: &'a dyn EmployeeShiftPort,
}

impl<'a> SchedulePreparationService<'a> {
    pub fn new(employee_shifts: &'a dyn EmployeeShiftPort) -> Self {
        Self { employee_shifts }
    }

    /// `prepareShiftsForScheduling(ScheduleModel, boolean)`.
    pub fn prepare_shifts_for_scheduling(
        &self,
        schedule_model: &mut ScheduleModel,
        clear_schedules: bool,
    ) {
        let (date_range, mut job_list, employee_list, old_shift_list, cleared_employee_shifts) =
            schedule_model.shift_preparation_fields();

        let Some(employee_list) = employee_list else {
            return;
        };

        for employee_data in employee_list.employee_data_list_mut() {
            let shifts = employee_data.data_set().shifts().to_vec();
            let mut kept_shifts = Vec::with_capacity(shifts.len());

            for mut employee_shift in shifts {
                let in_scope = date_range.contains_date(employee_shift.shift_date())
                    && job_list
                        .as_deref()
                        .is_some_and(|jobs| jobs.contains_job(employee_shift.job_id()));

                if in_scope {
                    let planned_shift = employee_shift.planned_shift();

                    if clear_schedules {
                        if let Some(planned_shift) = planned_shift {
                            cleared_employee_shifts.insert(planned_shift.id(), employee_shift.id());
                        }

                        employee_shift.set_planned_shift(None);
                        let employee_shift_id = employee_shift.id();
                        old_shift_list.add_employee_shift(employee_shift);
                        self.employee_shifts.evict(employee_shift_id);

                        continue;
                    }

                    if let Some(planned_shift) = planned_shift
                        && let Some(job_data) = job_list
                            .as_deref_mut()
                            .and_then(|jobs| jobs.job_data_mut(employee_shift.job_id()))
                    {
                        job_data.remove_planned_shift(planned_shift);
                    }
                }

                kept_shifts.push(employee_shift);
            }

            *employee_data.data_set_mut().shifts_mut() = kept_shifts;
        }
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `SchedulePreparationServiceTest.{testPrepareShiftsForSchedulingClearSchedulesFalse,
    //! testPrepareShiftsForSchedulingClearSchedulesTrue}`
    //! (`taps/src/junit/.../io/SchedulePreparationServiceTest.groovy`).
    //!
    //! Both Groovy cases build three employee shifts on one employee: `employeeShift1`
    //! (job1, date1, has a planned shift) — in scope; `employeeShift2` (job2, date1) — job2 has
    //! no `JobData` in `jobList`, out of scope; `employeeShift3` (job1, date2) — `dateRange` only
    //! covers date1, out of scope. Only `employeeShift1` is ever touched.

    use super::*;
    use crate::engine::model::employee_data::EmployeeData;
    use crate::engine::model::employee_list::EmployeeList;
    use crate::engine::model::job_data::JobData;
    use crate::engine::model::job_list::JobList;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::employee::Employee;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::planned_shift::PlannedShift;
    use crate::entity::property::Property;
    use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct SpyEmployeeShiftPort {
        evicted: RefCell<Vec<i32>>,
    }

    impl EmployeeShiftPort for SpyEmployeeShiftPort {
        fn evict(&self, employee_shift_id: i32) {
            self.evicted.borrow_mut().push(employee_shift_id);
        }

        fn bulk_delete_by_shift_id(&self, _shift_ids: &std::collections::HashSet<i32>) {}

        fn save(&self, _employee_shift: &crate::entity::employee_shift::EmployeeShift) {}

        fn bulk_delete_employee_shifts_for_jobs_in_current_property(
            &self,
            _job_ids: &std::collections::HashSet<i32>,
            _date_range: &date_range_rs::DateRange,
        ) {
        }

        fn employee_schedule_shifts_for_period(
            &self,
            _employee_ids: &std::collections::HashSet<i32>,
            _date_range: &date_range_rs::DateRange,
        ) -> Vec<crate::entity::employee_shift::EmployeeShift> {
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

    fn employee() -> Employee {
        Employee::new(
            1,
            "Employee",
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

    /// Builds `date1`/`date2`, the three planned shifts, the three employee shifts, `job_data1`
    /// (already holding `planned_shift1`), and a `ScheduleModel` with a single-day date range
    /// (`date1` only) and job1 (not job2) in its job list.
    #[allow(clippy::type_complexity)]
    fn setup() -> (
        LocalDate,
        LocalDate,
        PlannedShift,
        PlannedShift,
        PlannedShift,
        EmployeeShift,
        EmployeeShift,
        EmployeeShift,
        ScheduleModel,
    ) {
        let date1 = LocalDate::of(2013, 1, 1);
        let date2 = LocalDate::of(2013, 1, 2);
        let date_range = DateRange::new(date1, date1);

        let planned_shift1 = PlannedShift::new(
            1,
            1,
            date1,
            date1.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            8.0,
            None,
        );
        let planned_shift2 = PlannedShift::new(
            2,
            2,
            date1,
            date1.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            8.0,
            None,
        );
        let planned_shift3 = PlannedShift::new(
            3,
            1,
            date2,
            date2.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            8.0,
            None,
        );

        let mut job_data1 = JobData::new(job(1));
        job_data1.planned_shifts_mut().push(planned_shift1);

        let employee_shift1 = EmployeeShift::new(
            1,
            date1,
            date1.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            1,
            None,
        )
        .with_planned_shift(planned_shift1);
        let employee_shift2 = EmployeeShift::new(
            2,
            date1,
            date1.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            2,
            None,
        )
        .with_planned_shift(planned_shift2);
        let employee_shift3 = EmployeeShift::new(
            3,
            date2,
            date2.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            1,
            None,
        )
        .with_planned_shift(planned_shift3);

        let mut data_set = ScheduleCalcDataSet::default();
        data_set.set_employee(employee());
        data_set.add_shift(employee_shift1.clone());
        data_set.add_shift(employee_shift2.clone());
        data_set.add_shift(employee_shift3.clone());

        let mut employee_list = EmployeeList::new();
        employee_list.add_employee_data(EmployeeData::new(employee(), data_set, 0));

        let mut job_list = JobList::new();
        job_list.add_job_data(job_data1);

        let property = Property::new(1, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);
        schedule_model.set_job_list(job_list);
        schedule_model.set_employee_list(employee_list);

        (
            date1,
            date2,
            planned_shift1,
            planned_shift2,
            planned_shift3,
            employee_shift1,
            employee_shift2,
            employee_shift3,
            schedule_model,
        )
    }

    #[test]
    fn clear_schedules_false_removes_the_planned_shift_from_job_data_but_keeps_the_employee_shift()
    {
        let (
            _date1,
            _date2,
            planned_shift1,
            _planned_shift2,
            _planned_shift3,
            employee_shift1,
            employee_shift2,
            employee_shift3,
            mut schedule_model,
        ) = setup();

        let port = SpyEmployeeShiftPort::default();
        let service = SchedulePreparationService::new(&port);

        service.prepare_shifts_for_scheduling(&mut schedule_model, false);

        assert!(port.evicted.borrow().is_empty());

        let job_data1 = schedule_model.job_list().unwrap().job_data(1).unwrap();
        assert!(!job_data1.planned_shifts().contains(&planned_shift1));

        let shifts = employee_data(&schedule_model).data_set().shifts();
        assert!(shifts.contains(&employee_shift1));
        assert!(shifts.contains(&employee_shift2));
        assert!(shifts.contains(&employee_shift3));

        assert!(schedule_model.old_shift_list().employee_shifts().is_empty());
    }

    #[test]
    fn clear_schedules_true_moves_the_in_scope_shift_to_the_old_shift_list_and_clears_its_planned_shift()
     {
        let (
            _date1,
            _date2,
            planned_shift1,
            _planned_shift2,
            _planned_shift3,
            employee_shift1,
            employee_shift2,
            employee_shift3,
            mut schedule_model,
        ) = setup();

        let port = SpyEmployeeShiftPort::default();
        let service = SchedulePreparationService::new(&port);

        service.prepare_shifts_for_scheduling(&mut schedule_model, true);

        assert_eq!(*port.evicted.borrow(), vec![employee_shift1.id()]);

        assert_eq!(
            schedule_model
                .cleared_employee_shifts()
                .get(&planned_shift1.id()),
            Some(&employee_shift1.id())
        );

        // job_data1 still holds planned_shift1 — the clear-schedules branch never calls
        // `remove_planned_shift` (only the non-clear branch does).
        let job_data1 = schedule_model.job_list().unwrap().job_data(1).unwrap();
        assert!(job_data1.planned_shifts().contains(&planned_shift1));

        let shifts = employee_data(&schedule_model).data_set().shifts();
        assert!(!shifts.contains(&employee_shift1));
        assert!(shifts.contains(&employee_shift2));
        assert!(shifts.contains(&employee_shift3));

        let old_shifts = schedule_model.old_shift_list().employee_shifts();
        assert_eq!(old_shifts.len(), 1);
        assert_eq!(old_shifts[0].id(), employee_shift1.id());
        assert_eq!(old_shifts[0].planned_shift(), None);
    }

    fn employee_data(schedule_model: &ScheduleModel) -> &EmployeeData {
        schedule_model
            .employee_list()
            .unwrap()
            .employee_data(1)
            .unwrap()
    }
}
