//! Port of `com.unifocus.watson.common.enums.EmployeeAvailType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/EmployeeAvailType.java`. Only
//! the variants themselves are modeled — `getCode()`/`fromCode` aren't read by any ported call
//! site.

/// `EmployeeAvailType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmployeeAvailType {
    AvailableToWork,
    RequiredOff,
    PreferredOff,
    RotatedOff,
}
