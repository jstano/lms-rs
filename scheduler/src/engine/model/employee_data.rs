//! Port of `com.unifocus.watson.server.scheduler.engine.model.EmployeeData`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! EmployeeData.java`. Per-employee scheduling state for one pipeline run; see `DATA_MODEL.md`
//! §4.
//!
//! `storePreScheduleCheckOvertime` now resolves `ScheduleCalcDataSet.getOvertimeForDateRange`
//! (a real calculation not yet ported — see that type's doc) through
//! [`OvertimeForDateRangePort`], `CalculateDataSet` (Phase 2 step 6)'s first real caller.
//! `getMinHoursOff`/`getMinDaysOff` now resolve their `Assignment` parent-chain walk through
//! [`AssignmentPort`], the same shape as `workrules`'s `AssignmentPort` (see `engine::process::
//! ports`'s doc).

use crate::engine::model::weekly_available_hours::WeeklyAvailableHours;
use crate::engine::process::ports::{AssignmentPort, OvertimeForDateRangePort};
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

const HOURS_PER_DAY: f64 = 24.0;

/// `MINUTES_PER_DAY * DAYS_PER_WEEK` — a constant, not really per-employee data (see
/// `DATA_MODEL.md` §4's note on `weight`), but carried as a field to match `EmployeeData`'s
/// shape and `DefaultSeniorityComparator`'s read of it.
const WEIGHT: i32 = 1_440 * 7;

/// `EmployeeData`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeData {
    employee: Employee,
    data_set: ScheduleCalcDataSet,
    weight: i32,
    random: i32,
    weekly_available_hours: HashMap<i32, WeeklyAvailableHours>,
    pre_schedule_check_overtime: f64,
}

impl EmployeeData {
    pub fn new(employee: Employee, data_set: ScheduleCalcDataSet, random: i32) -> Self {
        Self {
            employee,
            data_set,
            weight: WEIGHT,
            random,
            weekly_available_hours: HashMap::new(),
            pre_schedule_check_overtime: 0.0,
        }
    }

    /// `getEmployee()`.
    pub fn employee(&self) -> &Employee {
        &self.employee
    }

    /// `getDataSet()` — see `ScheduleCalcDataSet`'s own doc for why this is a placeholder.
    pub fn data_set(&self) -> &ScheduleCalcDataSet {
        &self.data_set
    }

    /// `getDataSet()` (mutable).
    pub fn data_set_mut(&mut self) -> &mut ScheduleCalcDataSet {
        &mut self.data_set
    }

    /// `getWeight()`.
    pub fn weight(&self) -> i32 {
        self.weight
    }

    /// `getRandom()`.
    pub fn random(&self) -> i32 {
        self.random
    }

    /// `getWeeklyAvailableHours(JobData)` — get-or-insert, keyed by job id (Java keys by the
    /// `JobData` object itself; entity-identity-keyed maps become id-keyed here, see
    /// `PARITY_AUDIT.md` finding 5).
    pub fn weekly_available_hours(&mut self, job_id: i32) -> &mut WeeklyAvailableHours {
        self.weekly_available_hours.entry(job_id).or_default()
    }

    /// `hasJobAtAnyLevel(Assignment, LocalDate)`.
    pub fn has_job_at_any_level(&self, job_id: i32, date: LocalDate) -> bool {
        self.employee
            .employee_job_status(job_id, date)
            .is_some_and(|status| !status.is_sub_only())
    }

    /// `hasJobAtLevel(Assignment, int, LocalDate)`.
    pub fn has_job_at_level(&self, job_id: i32, job_level: i32, date: LocalDate) -> bool {
        self.employee
            .employee_job_statuses_for_date(date)
            .into_iter()
            .find(|status| status.job_id() == job_id && status.schedule_order() == job_level)
            .is_some_and(|status| !status.is_sub_only())
    }

    /// `getPreScheduleCheckOvertime()`.
    pub fn pre_schedule_check_overtime(&self) -> f64 {
        self.pre_schedule_check_overtime
    }

    /// `storePreScheduleCheckOvertime(DateRange)`.
    pub fn store_pre_schedule_check_overtime(
        &mut self,
        date_range: &DateRange,
        overtime: &dyn OvertimeForDateRangePort,
    ) {
        let overtime_hours = overtime.overtime_for_date_range(self, date_range);
        self.pre_schedule_check_overtime = overtime_hours;
    }

    /// `addEmployeeShift(EmployeeShift)`.
    pub fn add_employee_shift(&mut self, employee_shift: EmployeeShift) {
        self.data_set.add_shift(employee_shift);
    }

    /// `removeEmployeeShift(EmployeeShift)`.
    pub fn remove_employee_shift(&mut self, employee_shift: EmployeeShift) {
        self.data_set.remove_shift(employee_shift);
    }

    /// `getEffectiveAvailableHoursForDate(LocalDate)` — calendar hours available in the day
    /// (starting from 24), minus every required-off availability period that starts on `date`.
    pub fn effective_available_hours_for_date(&self, date: LocalDate) -> f64 {
        let mut available_hours_for_day = HOURS_PER_DAY;

        for avail_period in self
            .data_set
            .availability()
            .avail_periods_required_off_only()
        {
            if avail_period.start_date_time().to_local_date() == date {
                available_hours_for_day -= avail_period.duration();
            }
        }

        available_hours_for_day
    }

    /// `getMinHoursOff(Assignment)` — the employee's own override wins; otherwise walks
    /// `assignment` up its parent chain looking for the first override, `0.0` if none exists
    /// anywhere in the chain.
    pub fn min_hours_off(&self, assignment_id: i32, assignments: &dyn AssignmentPort) -> f64 {
        if let Some(hours) = self.employee.min_hours_off() {
            return hours;
        }

        let mut current_id = Some(assignment_id);
        while let Some(id) = current_id {
            let Some(assignment) = assignments.find_by_id(id) else {
                break;
            };
            if let Some(hours) = assignment.min_hours_off() {
                return hours;
            }
            current_id = assignment.parent_assignment_id();
        }

        0.0
    }

    /// `getMinDaysOff(Assignment)` — same shape as [`min_hours_off`](Self::min_hours_off).
    pub fn min_days_off(&self, assignment_id: i32, assignments: &dyn AssignmentPort) -> i32 {
        if let Some(days) = self.employee.min_days_off() {
            return days;
        }

        let mut current_id = Some(assignment_id);
        while let Some(id) = current_id {
            let Some(assignment) = assignments.find_by_id(id) else {
                break;
            };
            if let Some(days) = assignment.min_days_off() {
                return days;
            }
            current_id = assignment.parent_assignment_id();
        }

        0
    }
}
