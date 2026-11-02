//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeScheduleLabel`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeScheduleLabel.java`.
//!
//! A short text tag on one employee's day — "OFF", "VAC" — applied by a
//! manager or by a `schedulelabel` rule. The daily-differential rules gate on
//! it. The rule item id and `extraData` come across for the `schedulelabel` and
//! `schedulelabeladdendum` families that write them.

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
    id: i32,
    extra_data: Option<String>,
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
            id: 0,
            extra_data: None,
        }
    }

    /// Record the rule item that made this label. `setRuleItemID()`.
    #[must_use]
    pub fn with_rule_item_id(mut self, rule_item_id: i32) -> Self {
        self.rule_item_id = Some(rule_item_id);
        self
    }

    /// Set the database id. `setId()`.
    #[must_use]
    pub fn with_id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// The text a label addendum rule attached. `getExtraData()`.
    pub fn extra_data(&self) -> Option<&str> {
        self.extra_data.as_deref()
    }

    /// `setExtraData()`.
    pub fn set_extra_data(&mut self, extra_data: impl Into<String>) {
        self.extra_data = Some(extra_data.into());
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
