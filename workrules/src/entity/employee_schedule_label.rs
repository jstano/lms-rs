//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeScheduleLabel`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeScheduleLabel.java`.
//!
//! A short text tag on one employee's day — "OFF", "VAC" — applied by a
//! manager or by a `schedulelabel` rule. The daily-differential rules gate on
//! it. The rule item id and `extraData` blob do not come across; nothing the
//! ported rules do reads them.

use crate::common::enums::label_source::LabelSource;
use joda_rs::LocalDate;

/// A label on an employee's day. `EmployeeScheduleLabel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeScheduleLabel {
    employee_id: i32,
    date: LocalDate,
    label: String,
    source: LabelSource,
}

impl EmployeeScheduleLabel {
    /// Build a label.
    pub fn new(
        employee_id: i32,
        date: LocalDate,
        label: impl Into<String>,
        source: LabelSource,
    ) -> Self {
        Self {
            employee_id,
            date,
            label: label.into(),
            source,
        }
    }

    /// `getEmployeeID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getDate()`.
    pub fn date(&self) -> LocalDate {
        self.date
    }

    /// `getLabel()`.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// `getSource()`.
    pub fn source(&self) -> LabelSource {
        self.source
    }
}
