//! Port of the `com.unifocus.watson.common.enums` types the rules engine uses.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/`.
//!
//! These share the coded-enum shape described in [`crate::common::coded_enum`]:
//! a short `code` that is what lives in the database and in rule parameters,
//! and a `from_code` lookup. The `resourceKey` each carries for i18n display
//! does not come across — the engine never displays anything.
//!
//! `RuleType` is not here; it belongs with `RuleClass` in the engine core,
//! since the two are defined against each other. `AlertType`,
//! `EmployeePointsType` and `PropertyDataKey` arrive with the families that
//! need them — the first two are entangled with each other and with alert
//! levels and categories, and `PropertyDataKey` is a 312-line key catalogue
//! that only property-data lookups touch.

pub mod accrual_applied_type;
pub mod change_reason_type;
pub mod earn_type;
pub mod earning_source;
pub mod employee_calculation_mode;
pub mod employee_event_type;
pub mod employee_pay_type;
pub mod holiday_calculation_period_type;
pub mod label_source;
pub mod meal_punch_permission;
pub mod module;
pub mod pay_period_type;
pub mod punch_source;
pub mod punch_type;
pub mod schedule_lockout_level;
pub mod schedule_mode;
pub mod shift_adjust_source;
pub mod shift_adjust_type;
pub mod shift_error_type;
pub mod shift_type;
pub mod start_calculation_from_type;
pub mod time_off_status;
pub mod uftc_punch_type;
pub mod uom;
pub mod violation_event_unit_type;
