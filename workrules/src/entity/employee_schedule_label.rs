//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeScheduleLabel`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeScheduleLabel.java`.
//!
//! A short text tag on one employee's day — "OFF", "VAC" — applied by a
//! manager or by a `schedulelabel` rule. The daily-differential rules gate on
//! it. The `extraData` blob does not come across; nothing the ported rules
//! do reads it. The rule item id does, for the `schedulelabel` rules that write labels.

use crate::common::enums::label_source::LabelSource;
use joda_rs::LocalDate;

/// A label on an employee's day. `EmployeeScheduleLabel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeScheduleLabel {
    employee_id: i32,
    date: LocalDate,
    label: String,
    source: LabelSource,
    rule_item_id: Option<i32>,
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
            rule_item_id: None,
        }
    }

    /// Record the rule item that made this label. `setRuleItemID()`.
    #[must_use]
    pub fn with_rule_item_id(mut self, rule_item_id: i32) -> Self {
        self.rule_item_id = Some(rule_item_id);
        self
    }

    /// `getRuleItemID()`.
    pub fn rule_item_id(&self) -> Option<i32> {
        self.rule_item_id
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
