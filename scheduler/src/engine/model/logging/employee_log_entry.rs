//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.EmployeeLogEntry`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! EmployeeLogEntry.java`. `employee_id` replaces the owned `EmployeeData` Java holds — see
//! `DATA_MODEL.md` §5 / `PARITY_AUDIT.md` finding 5 (entity-identity-keyed collections become
//! id-keyed here).

/// `EmployeeLogEntry`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeLogEntry {
    employee_id: i32,
    notes: Option<String>,
}

impl EmployeeLogEntry {
    pub fn new(employee_id: i32) -> Self {
        Self {
            employee_id,
            notes: None,
        }
    }

    /// `getEmployeeData()` — id only, see module doc.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getNotes()`.
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// `setNotes(String)`.
    pub fn set_notes(&mut self, notes: impl Into<String>) {
        self.notes = Some(notes.into());
    }
}
