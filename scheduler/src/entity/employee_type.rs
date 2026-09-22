//! Port of `com.unifocus.watson.common.enums.EmployeeType`.
//!
//! Ground truth not read directly — only the variants and the one static method the engine's
//! Phase 1 files reach are confirmed: `REGULAR`/`PERMANENT` (checked by name throughout —
//! `WeeklyAvailableHours`, `RotationPlanUtils`), a general "everyone else" bucket the Java calls
//! variable elsewhere in the engine, and `getEmployeeTypeSortOrder()` (`EmployeeTypeComparator`).
//! The real Java enum may have more variants than `Other` currently covers — unconfirmed.

/// `EmployeeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmployeeType {
    Regular,
    Permanent,
    /// Stand-in for every other Java variant not yet distinguished by any ported call site
    /// (e.g. variable-hour employees). Split out the real variant here the first time a ported
    /// file needs to tell two "other" types apart.
    Other,
}

impl EmployeeType {
    /// `EmployeeType.getEmployeeTypeSortOrder()` — the tie-break order
    /// `EmployeeTypeComparator` indexes into. Ground truth for the real ordering (and the full
    /// variant list) wasn't read for this wave; `[Regular, Permanent, Other]` is a placeholder
    /// that preserves the comparator's *shape* (index difference), not a confirmed order.
    pub fn sort_order() -> &'static [EmployeeType] {
        &[
            EmployeeType::Regular,
            EmployeeType::Permanent,
            EmployeeType::Other,
        ]
    }
}
