//! Port of `com.unifocus.watson.common.enums.EmployeePayType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/EmployeePayType.java`. Only the
//! variants exist here (no code/resource-key fields) since nothing in `scheduler` reads those.

/// `EmployeePayType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmployeePayType {
    Hourly,
    Piece,
    SalariedExempt,
    SalariedNonExempt,
    Contract,
}
