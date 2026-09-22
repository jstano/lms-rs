//! Port of `com.unifocus.watson.server.hibernate.entity.DayOffPattern`.
//!
//! Ground truth not read directly — only the one method `EmployeeDayOffRotationPlanChecker`
//! calls.

use joda_rs::DayOfWeek;

/// One day-off rotation pattern. `DayOffPattern`.
#[derive(Debug, Clone, PartialEq)]
pub struct DayOffPattern {
    pattern_no: i32,
    workable_days: Vec<DayOfWeek>,
}

impl DayOffPattern {
    pub fn new(pattern_no: i32, workable_days: Vec<DayOfWeek>) -> Self {
        Self {
            pattern_no,
            workable_days,
        }
    }

    /// The pattern number this instance answers for — `DayOffPlan.getDayOffPattern(int)` looks
    /// this collection up by it.
    pub fn pattern_no(&self) -> i32 {
        self.pattern_no
    }

    /// `canWorkOnDayOfWeek(DayOfWeek)`.
    pub fn can_work_on_day_of_week(&self, day_of_week: DayOfWeek) -> bool {
        self.workable_days.contains(&day_of_week)
    }
}
