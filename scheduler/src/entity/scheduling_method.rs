//! Port of `com.unifocus.watson.common.enums.SchedulingMethod`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/SchedulingMethod.java`. Only
//! the three variants exist here (no code/resource-key fields) — nothing in `scheduler` reads
//! those.

/// `SchedulingMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchedulingMethod {
    ByJobScheduleOrder,
    BySeniority,
    ByEmployeeSet,
}
