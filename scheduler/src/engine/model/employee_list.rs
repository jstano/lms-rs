//! Port of `com.unifocus.watson.server.scheduler.engine.model.EmployeeList`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! EmployeeList.java`. Id-keyed collection of `EmployeeData`; see `DATA_MODEL.md` §4.

use crate::engine::model::employee_data::EmployeeData;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// `EmployeeList`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmployeeList {
    employee_data: HashMap<i32, EmployeeData>,
}

impl EmployeeList {
    pub fn new() -> Self {
        Self::default()
    }

    /// `addEmployeeData(EmployeeData)`.
    pub fn add_employee_data(&mut self, employee_data: EmployeeData) {
        self.employee_data
            .insert(employee_data.employee().id(), employee_data);
    }

    /// `getEmployeeData(Employee)`.
    pub fn employee_data(&self, employee_id: i32) -> Option<&EmployeeData> {
        self.employee_data.get(&employee_id)
    }

    /// `getEmployeeData(Employee)` (mutable).
    pub fn employee_data_mut(&mut self, employee_id: i32) -> Option<&mut EmployeeData> {
        self.employee_data.get_mut(&employee_id)
    }

    /// Removes and returns one employee's data, for a caller that needs simultaneous mutable
    /// access to both this `EmployeeList`'s owner (`ScheduleModel`) and the single `EmployeeData`
    /// it holds — Rust can't alias `&mut ScheduleModel` with `&mut EmployeeData` borrowed out of
    /// it, where Java's shared object graph gives `PreScheduleProcess` both for free (see
    /// `PARITY_AUDIT.md`'s finding on `PreScheduleProcess`). Pair with
    /// [`add_employee_data`](Self::add_employee_data) to put it back once the caller is done.
    pub fn take_employee_data(&mut self, employee_id: i32) -> Option<EmployeeData> {
        self.employee_data.remove(&employee_id)
    }

    /// `getEmployeeIDs()`.
    pub fn employee_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.employee_data.keys().copied()
    }

    /// `getEmployeeDataList()` — sorted by employee name, case-insensitive.
    pub fn employee_data_list(&self) -> Vec<&EmployeeData> {
        let mut list: Vec<&EmployeeData> = self.employee_data.values().collect();
        list.sort_by_key(|e| e.employee().name().to_lowercase());
        list
    }

    /// `getUnsortedEmployeeDataList()` — Java's `HashMap` iteration order is unspecified; this
    /// carries no ordering guarantee either, matching that (see `DATA_MODEL.md` §4).
    pub fn unsorted_employee_data_list(&self) -> Vec<&EmployeeData> {
        self.employee_data.values().collect()
    }

    /// `getEmployeeDataList()` (mutable) — `SchedulePreparationService` mutates each employee's
    /// shift data in place; order doesn't affect the outcome (each employee is processed
    /// independently), so this carries no sort, unlike the read-only `employee_data_list`.
    pub fn employee_data_list_mut(&mut self) -> impl Iterator<Item = &mut EmployeeData> {
        self.employee_data.values_mut()
    }

    /// `getEmployeesWithJob(Assignment, LocalDate)`.
    pub fn employees_with_job(&self, job_id: i32, date: LocalDate) -> Vec<&EmployeeData> {
        self.employee_data
            .values()
            .filter(|e| e.has_job_at_any_level(job_id, date))
            .collect()
    }
}
