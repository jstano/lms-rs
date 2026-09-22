//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.PlannedShiftLog`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! PlannedShiftLog.java`.

use crate::engine::model::logging::employees_with_conflicts::EmployeesWithConflicts;
use crate::engine::model::logging::ranked_employees::RankedEmployees;
use crate::entity::planned_shift::PlannedShift;

/// `PlannedShiftLog`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedShiftLog {
    planned_shift: PlannedShift,
    employees_with_conflicts: EmployeesWithConflicts,
    ranked_employees: RankedEmployees,
}

impl PlannedShiftLog {
    pub fn new(planned_shift: PlannedShift) -> Self {
        Self {
            planned_shift,
            employees_with_conflicts: EmployeesWithConflicts::new(),
            ranked_employees: RankedEmployees::new(),
        }
    }

    /// `getPlannedShift()`.
    pub fn planned_shift(&self) -> PlannedShift {
        self.planned_shift
    }

    /// `getEmployeesWithConflicts()`.
    pub fn employees_with_conflicts(&self) -> &EmployeesWithConflicts {
        &self.employees_with_conflicts
    }

    /// `getEmployeesWithConflicts()` (mutable).
    pub fn employees_with_conflicts_mut(&mut self) -> &mut EmployeesWithConflicts {
        &mut self.employees_with_conflicts
    }

    /// `getRankedEmployees()`.
    pub fn ranked_employees(&self) -> &RankedEmployees {
        &self.ranked_employees
    }

    /// `getRankedEmployees()` (mutable).
    pub fn ranked_employees_mut(&mut self) -> &mut RankedEmployees {
        &mut self.ranked_employees
    }
}
