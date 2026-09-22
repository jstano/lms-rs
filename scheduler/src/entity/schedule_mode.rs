//! Port of `com.unifocus.watson.common.enums.ScheduleMode`.
//!
//! Ground truth not read directly — only the two variants
//! `EmployeeMonthlyAvailableHoursChecker`/`EmployeeWeeklyAvailableHoursChecker` compare against
//! are confirmed; the real Java enum may have more (unconfirmed, same caveat as
//! `JcSortOrderType`/`EmployeeType`).

/// How a property's schedules are period-bounded. `ScheduleMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleMode {
    Weekly,
    Monthly,
}
