//! Port of `com.unifocus.watson.common.enums.EmployeeStatusType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/EmployeeStatusType.java`. Only
//! the code/name pair is modeled — `getCode()`/`fromCode`/the resource-key-backed `toString()`
//! aren't needed by anything ported (see `entity::shift_error_type::ShiftErrorType`'s doc for why
//! this crate skips `ResourceMgr` translation generally).

/// `EmployeeStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmployeeStatusType {
    Active,
    LeaveOfAbsence,
    Rehire,
    Terminated,
    Transferred,
}
