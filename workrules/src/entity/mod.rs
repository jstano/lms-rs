//! The entity model the rules operate on.
//!
//! Ported from `com.unifocus.watson.server.hibernate.entity`. Those are large
//! JPA entities — `Employee` is 1,486 lines, `Assignment` 1,133 — but the rules
//! read only a narrow slice of each, typically 8 to 15 fields. Only that slice
//! comes across, as plain owned structs.
//!
//! There is no lazy loading here. Java leans on JPA to navigate
//! `EmployeeShift.employee`, `.job`, `EmployeeEarning.earningType` and so on at
//! the moment a rule asks; the engine is handed a fully materialised graph
//! instead, and back-references become ids so ownership stays one-way.
//!
//! The exception is `EmployeeShift`, which genuinely owns its punches — see
//! that module for why the punch/shift coupling could not become an id.

pub mod accrual_transaction;
pub mod assignment;
pub mod assignment_pay_rate;
pub mod calc_data_set_stat;
pub mod earning_type;
pub mod employee;
pub mod employee_earning;
pub mod employee_job_status;
pub mod employee_shift;
pub mod employee_shift_punch;
pub mod flsa_data;
pub mod holiday;
pub mod hours_distribution;
pub mod hours_distribution_type;
pub mod planned_shift;
pub mod property;
pub mod punch_log;
pub mod rule_item;
pub mod rule_set;
pub mod shift_category;
pub mod time_card;
pub mod time_clock_result;
