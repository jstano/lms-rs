//! Port of `com.unifocus.watson.server.hibernate.entity.Property`.
//!
//! Ground truth not re-read in full for this wave — only `getCurrentWeek()` (`DATA_MODEL.md` §3 /
//! `ScheduleModel.getWeekForDate`), `getScheduleMode()` (`process/checkers/
//! {EmployeeMonthlyAvailableHoursChecker,EmployeeWeeklyAvailableHoursChecker}`), and
//! `getPeriodStartDate()` (`DayOffPlanRotator`/`DayOffPlan.rotateEmployees`, Phase 2 step 2).
//! `getPayPeriodType()` is still not modeled — every ported call site that would need it stays
//! inside the deferred `ContractCalculator` boundary (see `PARITY_AUDIT.md` finding 10).

use crate::entity::schedule_mode::ScheduleMode;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// A physical location. `Property`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    id: i32,
    /// `getCurrentWeek()` — a template weekly range, walked forward/backward by
    /// `getDateRangeContainingDate` to find the week that contains an arbitrary date. Built via
    /// `date_range_rs::WeeklyDateRange::with_end_date`, matching Java's `WeeklyDateRange`.
    current_week: DateRange,
    schedule_mode: Option<ScheduleMode>,
    period_start_date: Option<LocalDate>,
}

impl Property {
    pub fn new(id: i32, current_week: DateRange) -> Self {
        Self {
            id,
            current_week,
            schedule_mode: None,
            period_start_date: None,
        }
    }

    /// `setScheduleMode(ScheduleMode)`.
    #[must_use]
    pub fn with_schedule_mode(mut self, schedule_mode: ScheduleMode) -> Self {
        self.schedule_mode = Some(schedule_mode);
        self
    }

    /// `setPeriodStartDate(LocalDate)`.
    #[must_use]
    pub fn with_period_start_date(mut self, period_start_date: LocalDate) -> Self {
        self.period_start_date = Some(period_start_date);
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getCurrentWeek().getDateRangeContainingDate(date)` — `ScheduleModel.getWeekForDate`
    /// calls this through `Property` directly, so it lives here rather than duplicated there.
    pub fn week_for_date(&self, date: LocalDate) -> DateRange {
        self.current_week.range_containing_date(date)
    }

    /// `getScheduleMode()`. `None` here where Java would presumably never actually be `null` in
    /// a real property, but nothing has grounded that assumption yet — checkers reading this
    /// compare `Some(mode) == Some(ScheduleMode::X)`, so an unset mode is simply never `Monthly`
    /// or `Weekly`, not a crash.
    pub fn schedule_mode(&self) -> Option<ScheduleMode> {
        self.schedule_mode
    }

    /// `getPeriodStartDate()`. `None` here where Java's `LocalDate` field would presumably never
    /// actually be `null` for a property running through the schedule pipeline — see
    /// `DayOffPlan::rotate_employees`'s doc for how its one caller handles an unset value.
    pub fn period_start_date(&self) -> Option<LocalDate> {
        self.period_start_date
    }
}
