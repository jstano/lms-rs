//! Port of `com.unifocus.watson.server.scheduler.engine.model.ShiftList`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/ShiftList.java`.
//! Flat holder of two independent shift collections; see `DATA_MODEL.md` §4.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::planned_shift::PlannedShift;
use std::collections::HashSet;

/// `ShiftList`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShiftList {
    employee_shifts: Vec<EmployeeShift>,
    planned_shifts: Vec<Option<PlannedShift>>,
}

impl ShiftList {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getEmployeeShifts()`.
    pub fn employee_shifts(&self) -> &[EmployeeShift] {
        &self.employee_shifts
    }

    /// `getPlannedShifts()` — Java's list may contain `null`; see
    /// [`planned_shift_ids`](Self::planned_shift_ids)'s note.
    pub fn planned_shifts(&self) -> &[Option<PlannedShift>] {
        &self.planned_shifts
    }

    /// `getEmployeeShiftIDs()`.
    pub fn employee_shift_ids(&self) -> HashSet<i32> {
        self.employee_shifts.iter().map(EmployeeShift::id).collect()
    }

    /// `getPlannedShiftIDs()` — Java null-checks each element before taking its id (see
    /// `PARITY_AUDIT.md` finding 6); `planned_shifts` is `Vec<Option<PlannedShift>>` here for
    /// exactly that reason, filtered here rather than at insertion.
    pub fn planned_shift_ids(&self) -> HashSet<i32> {
        self.planned_shifts
            .iter()
            .filter_map(|s| s.as_ref())
            .map(PlannedShift::id)
            .collect()
    }

    /// `addEmployeeShift(EmployeeShift)`.
    pub fn add_employee_shift(&mut self, employee_shift: EmployeeShift) {
        self.employee_shifts.push(employee_shift);
    }

    /// `addPlannedShift(PlannedShift)`.
    pub fn add_planned_shift(&mut self, planned_shift: Option<PlannedShift>) {
        self.planned_shifts.push(planned_shift);
    }
}
