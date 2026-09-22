//! Port of `com.unifocus.watson.common.enums.EmployeeCalculationMode`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/EmployeeCalculationMode.java` —
//! a plain three-value enum, all three variants carried across even though only `AutoSchedule` is
//! written anywhere in this crate so far (`EmployeeListLoader.createEmployeeList`).

/// `EmployeeCalculationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmployeeCalculationMode {
    Ta,
    AutoSchedule,
    EditSchedule,
}
