//! Port of `com.unifocus.watson.common.enums.ProjectedHoursReductionMethod`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/
//! ProjectedHoursReductionMethod.java` — a two-value enum, confirmed by direct read (not a
//! guess from call sites, unlike `EmployeeType`/`JCSortOrderType`'s placeholders —
//! `PARITY_AUDIT.md` finding 8). `fromCode`/`getCode`/`getResourceKey`/`toString` aren't
//! modeled; nothing ported reads them.

/// `ProjectedHoursReductionMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedHoursReductionMethod {
    Flat,
    Percent,
}
