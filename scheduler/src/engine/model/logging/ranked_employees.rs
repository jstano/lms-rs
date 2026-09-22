//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.RankedEmployees`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! RankedEmployees.java`. `Stack<EmployeeData>`, only ever pushed — ported as a plain `Vec<i32>`
//! of employee ids (see `EmployeeLogEntry`'s doc on id-keying); no need for real stack
//! `pop`/`peek` semantics unless a later-ported caller turns out to use them.

/// `RankedEmployees`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RankedEmployees {
    employee_ids: Vec<i32>,
}

impl RankedEmployees {
    pub fn new() -> Self {
        Self::default()
    }

    /// `pushEmployee(EmployeeData)`.
    pub fn push_employee(&mut self, employee_id: i32) {
        self.employee_ids.push(employee_id);
    }

    /// `getEmployeeStack()`.
    pub fn employee_ids(&self) -> &[i32] {
        &self.employee_ids
    }
}
