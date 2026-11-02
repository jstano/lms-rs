//! Port of `com.unifocus.rms.timekeeping.api.earning.earningrequest.EarningRequestDto`.
//!
//! A request to create an earning, handed back to the caller instead of being
//! saved: the `schedulechangevalidation` rules return these so the scheduling UI
//! can show the penalty and let a manager confirm or overwrite it.

use crate::common::enums::earning_source::EarningSource;
use joda_rs::LocalDate;

/// An earning that has been asked for but not made. `EarningRequestDto`.
#[derive(Debug, Clone, PartialEq)]
pub struct EarningRequest {
    /// `getId()`; 0 for a new request.
    pub id: i32,
    /// `getEmployeeId()`.
    pub employee_id: i32,
    /// `getEarningTypeId()`.
    pub earning_type_id: i32,
    /// `getJobId()`.
    pub job_id: i32,
    /// `getEarningDate()`.
    pub earning_date: LocalDate,
    /// `getPayDate()`.
    pub pay_date: LocalDate,
    /// `getSource()`.
    pub source: EarningSource,
    /// `getCost()`: dollars.
    pub cost: f64,
    /// `getHours()`.
    pub hours: f64,
    /// `getRate()`.
    pub rate: f64,
    /// `getNote()`.
    pub note: String,
    /// `isForceDuplicate()`.
    pub force_duplicate: bool,
}
