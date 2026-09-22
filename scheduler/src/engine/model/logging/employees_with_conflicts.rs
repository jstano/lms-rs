//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.EmployeesWithConflicts`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! EmployeesWithConflicts.java`. Keyed by employee id rather than an owned `EmployeeData` — see
//! `EmployeeLogEntry`'s doc / `PARITY_AUDIT.md` finding 5.

use crate::engine::model::logging::employee_log_entry::EmployeeLogEntry;
use std::collections::HashMap;

/// `EmployeesWithConflicts`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmployeesWithConflicts {
    log_entries: Vec<EmployeeLogEntry>,
    by_employee_id: HashMap<i32, usize>,
}

impl EmployeesWithConflicts {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getLogEntries()`.
    pub fn log_entries(&self) -> &[EmployeeLogEntry] {
        &self.log_entries
    }

    /// `addLogEntry(EmployeeLogEntry)`.
    pub fn add_log_entry(&mut self, entry: EmployeeLogEntry) {
        self.by_employee_id
            .insert(entry.employee_id(), self.log_entries.len());
        self.log_entries.push(entry);
    }

    /// `removeLogEntry(EmployeeLogEntry)`.
    pub fn remove_log_entry(&mut self, employee_id: i32) {
        if let Some(index) = self.by_employee_id.remove(&employee_id) {
            self.log_entries.remove(index);
            for value in self.by_employee_id.values_mut() {
                if *value > index {
                    *value -= 1;
                }
            }
        }
    }

    /// `setNotesForEmployee(EmployeeData, String)`. Java NPEs if no entry exists for the
    /// employee yet (`.get(employeeData).setNotes(...)` with no null check); this no-ops
    /// instead. Every caller so far (`AbstractCanWorkChecker`) only calls this for an employee
    /// already ranked into the log, so the case shouldn't occur in practice — but it fails
    /// silently here rather than loudly, unlike Java. Revisit if a caller proves that assumption
    /// wrong.
    pub fn set_notes_for_employee(&mut self, employee_id: i32, notes: impl Into<String>) {
        if let Some(&index) = self.by_employee_id.get(&employee_id) {
            self.log_entries[index].set_notes(notes);
        }
    }
}
