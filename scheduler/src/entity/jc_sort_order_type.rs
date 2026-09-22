//! Port of `com.unifocus.watson.common.enums.JCSortOrderType`.
//!
//! Ground truth: `SeniorityComparatorFactory.java`'s comparator map — the eight variants that map
//! to a `SeniorityComparator` there are the only ones confirmed to exist; the real Java enum may
//! have more (unconfirmed — the enum's own source wasn't read for this wave, only its usage).

/// A job's seniority tie-break rule. `JCSortOrderType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JcSortOrderType {
    AssignmentOrder,
    AssignmentRank,
    Default,
    EmployeeType,
    Fulltime,
    HireDate,
    SkillDate,
    SkillRank,
}
