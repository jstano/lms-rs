//! Port of `com.unifocus.watson.common.enums.PlannedShiftSortingMethod`.
//!
//! Ground truth not read directly — only the four variants
//! `PlannedShiftSorterFactory`'s comparator map uses are confirmed; the real Java enum may have
//! more (unconfirmed, same caveat as `JcSortOrderType`).

/// How a job's planned shifts are ordered before being offered for scheduling.
/// `PlannedShiftSortingMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlannedShiftSortingMethod {
    ByDay,
    Cascade,
    ModifiedPeak,
    Peak,
}
