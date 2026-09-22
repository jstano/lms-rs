//! Port of `com.unifocus.watson.server.hibernate.entity.WorkClass`.
//!
//! Ground truth not read directly — only the two fields `EmployeeData.getBaseAvailableHours`
//! (fallback when the employee has no override) and `FullTimeComparator` read.

/// `WorkClass`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkClass {
    hours_available: f64,
    is_fulltime: bool,
}

impl WorkClass {
    pub fn new(hours_available: f64, is_fulltime: bool) -> Self {
        Self {
            hours_available,
            is_fulltime,
        }
    }

    /// `getHoursAvailable()`.
    pub fn hours_available(&self) -> f64 {
        self.hours_available
    }

    /// `isFulltime()`.
    pub fn is_fulltime(&self) -> bool {
        self.is_fulltime
    }
}
