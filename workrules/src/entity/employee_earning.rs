//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeEarning`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeEarning.java`.
//!
//! What an employee is owed for something, on a date, at a rate. Unlike the
//! bigger entities essentially all of this one is load-bearing — 16 of the 32
//! rule families read or write it, and the rate families exist to produce it.
//!
//! An earning a rule creates is stamped
//! [`EarningSource::Rule`](crate::common::enums::earning_source::EarningSource::Rule)
//! and records the rule item that made it, which is how later rules tell their
//! own output apart from hand-entered or imported earnings.

use crate::common::enums::earning_source::EarningSource;
use joda_rs::LocalDate;

/// An amount owed to an employee. `EmployeeEarning`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeEarning {
    id: i32,
    employee_id: i32,
    job_id: i32,
    earning_type_id: i32,
    earning_date: LocalDate,
    pay_date: Option<LocalDate>,
    hours: f64,
    rate: f64,
    dollars: f64,
    total_dollars: f64,
    source: EarningSource,
    rule_item_id: Option<i32>,
    shift_id: Option<i32>,
    note: String,
}

impl EmployeeEarning {
    /// Build an earning.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        employee_id: i32,
        job_id: i32,
        earning_type_id: i32,
        earning_date: LocalDate,
        hours: f64,
        rate: f64,
        source: EarningSource,
    ) -> Self {
        Self {
            id,
            employee_id,
            job_id,
            earning_type_id,
            earning_date,
            pay_date: None,
            hours,
            rate,
            dollars: 0.0,
            total_dollars: 0.0,
            source,
            rule_item_id: None,
            shift_id: None,
            note: String::new(),
        }
    }

    /// Record which rule item produced this earning, and against which shift.
    ///
    /// Java sets these through plain setters after construction; grouping them
    /// keeps the common "a rule made this" case to one call.
    #[must_use]
    pub fn from_rule(mut self, rule_item_id: i32, shift_id: Option<i32>) -> Self {
        self.rule_item_id = Some(rule_item_id);
        self.shift_id = shift_id;
        self.source = EarningSource::Rule;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getEmployee().getID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getEarningType().getID()`.
    pub fn earning_type_id(&self) -> i32 {
        self.earning_type_id
    }

    /// The date the earning is *for*. `getEarningDate()`.
    pub fn earning_date(&self) -> LocalDate {
        self.earning_date
    }

    /// The date it is paid on, if that differs. `getPayDate()`.
    pub fn pay_date(&self) -> Option<LocalDate> {
        self.pay_date
    }

    /// `getHours()`.
    pub fn hours(&self) -> f64 {
        self.hours
    }

    /// `getRate()`.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// `getDollars()`.
    pub fn dollars(&self) -> f64 {
        self.dollars
    }

    /// `getTotalDollars()`.
    pub fn total_dollars(&self) -> f64 {
        self.total_dollars
    }

    /// Where this earning came from. `getSource()`.
    pub fn source(&self) -> EarningSource {
        self.source
    }

    /// The rule item that produced it, if a rule did. `getRuleItem().getID()`.
    pub fn rule_item_id(&self) -> Option<i32> {
        self.rule_item_id
    }

    /// The shift it belongs to, if any. `getShift().getID()`.
    pub fn shift_id(&self) -> Option<i32> {
        self.shift_id
    }

    /// `getNote()`.
    pub fn note(&self) -> &str {
        &self.note
    }

    /// `setHours()`.
    pub fn set_hours(&mut self, hours: f64) {
        self.hours = hours;
    }

    /// `setRate()`.
    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    /// `setDollars()`.
    pub fn set_dollars(&mut self, dollars: f64) {
        self.dollars = dollars;
    }

    /// `setTotalDollars()`.
    pub fn set_total_dollars(&mut self, total_dollars: f64) {
        self.total_dollars = total_dollars;
    }

    /// `setPayDate()`.
    pub fn set_pay_date(&mut self, pay_date: Option<LocalDate>) {
        self.pay_date = pay_date;
    }

    /// `setNote()`.
    pub fn set_note(&mut self, note: impl Into<String>) {
        self.note = note.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earning() -> EmployeeEarning {
        EmployeeEarning::new(
            1,
            100,
            200,
            5,
            LocalDate::of(2010, 1, 2),
            8.0,
            12.50,
            EarningSource::Auto,
        )
    }

    #[test]
    fn an_earning_carries_its_hours_rate_and_source() {
        let e = earning();
        assert_eq!(e.employee_id(), 100);
        assert_eq!(e.job_id(), 200);
        assert_eq!(e.earning_type_id(), 5);
        assert_eq!(e.hours(), 8.0);
        assert_eq!(e.rate(), 12.50);
        assert_eq!(e.source(), EarningSource::Auto);
    }

    #[test]
    fn a_rule_written_earning_records_the_rule_that_made_it() {
        let e = earning().from_rule(10, Some(77));

        assert_eq!(e.source(), EarningSource::Rule);
        assert_eq!(e.rule_item_id(), Some(10));
        assert_eq!(e.shift_id(), Some(77));
    }

    #[test]
    fn an_earning_not_made_by_a_rule_has_no_rule_item() {
        assert_eq!(earning().rule_item_id(), None);
    }

    #[test]
    fn the_pay_date_is_separate_from_the_earning_date() {
        let mut e = earning();
        assert_eq!(e.pay_date(), None);

        e.set_pay_date(Some(LocalDate::of(2010, 1, 15)));

        assert_eq!(e.earning_date(), LocalDate::of(2010, 1, 2));
        assert_eq!(e.pay_date(), Some(LocalDate::of(2010, 1, 15)));
    }

    #[test]
    fn the_money_fields_can_be_written() {
        let mut e = earning();
        e.set_hours(4.0);
        e.set_rate(20.0);
        e.set_dollars(80.0);
        e.set_total_dollars(80.0);
        e.set_note("adjusted");

        assert_eq!(e.hours(), 4.0);
        assert_eq!(e.dollars(), 80.0);
        assert_eq!(e.total_dollars(), 80.0);
        assert_eq!(e.note(), "adjusted");
    }
}
