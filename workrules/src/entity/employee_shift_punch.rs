//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeShiftPunch`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeShiftPunch.java`.
//!
//! # No back-reference to the shift
//!
//! The Java entity holds `@ManyToOne EmployeeShift employeeShift`, and
//! `setRoundedTime` uses it to call back into the parent:
//!
//! ```java
//! public void setRoundedTime(LocalDateTime roundedTime) {
//!    LocalDateTime oldRoundedTime = this.roundedTime;
//!    this.roundedTime = roundedTime;
//!    if (employeeShift != null && oldRoundedTime != this.roundedTime) {
//!       employeeShift.resetStartAndEndTimesFromPunch(this);
//!    }
//! }
//! ```
//!
//! A punch owned by a shift cannot hold `&mut` to that shift, so the coupling
//! moves up: [`EmployeeShift`] owns its punches and owns the callback, and
//! rules reach a punch through
//! [`PunchCursor`](crate::entity::employee_shift::PunchCursor), whose
//! `set_rounded_time` performs the write *and* the callback.
//!
//! Consequently [`EmployeeShiftPunch::set_rounded_time`] here is the plain
//! field write with no side effect. It is `pub(crate)` so rules cannot reach
//! it and accidentally skip the callback — that is the whole point of the
//! cursor.
//!
//! [`EmployeeShift`]: crate::entity::employee_shift::EmployeeShift

use crate::common::enums::punch_source::PunchSource;
use crate::common::enums::punch_type::PunchType;
use joda_rs::LocalDateTime;

/// One punch on a shift. `EmployeeShiftPunch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeShiftPunch {
    id: i32,
    punch_type: PunchType,
    punch_time: Option<LocalDateTime>,
    adj_time: Option<LocalDateTime>,
    rounded_time: Option<LocalDateTime>,
    badge_no: String,
    source: PunchSource,
}

impl EmployeeShiftPunch {
    /// Build a punch whose three times all start out the same.
    ///
    /// That is the state the Groovy test tables construct: `punchTime`,
    /// `adjTime` and `roundedTime` all set to the recorded time, so a rounding
    /// rule has something to round *from* and its effect is visible as a change
    /// to `roundedTime` alone.
    pub fn new(id: i32, punch_type: PunchType, source: PunchSource, time: LocalDateTime) -> Self {
        Self {
            id,
            punch_type,
            punch_time: Some(time),
            adj_time: Some(time),
            rounded_time: Some(time),
            badge_no: String::new(),
            source,
        }
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// What this punch records. `getPunchType()`.
    pub fn punch_type(&self) -> PunchType {
        self.punch_type
    }

    /// The time as recorded. `getPunchTime()`.
    pub fn punch_time(&self) -> Option<LocalDateTime> {
        self.punch_time
    }

    /// The time after manual adjustment — what rounding rules round *from*.
    /// `getAdjTime()`.
    pub fn adj_time(&self) -> Option<LocalDateTime> {
        self.adj_time
    }

    /// The time after rounding — what rounding rules write. `getRoundedTime()`.
    ///
    /// `None` is meaningful: an absent rounded time is how a missing punch is
    /// represented, and [`EmployeeShift::errors`] reports it as such.
    ///
    /// [`EmployeeShift::errors`]: crate::entity::employee_shift::EmployeeShift::errors
    pub fn rounded_time(&self) -> Option<LocalDateTime> {
        self.rounded_time
    }

    /// Where the punch came from. `getSource()`.
    ///
    /// Rounding rules gate on this: a rule set can round clock punches while
    /// leaving hand-keyed ones alone.
    pub fn source(&self) -> PunchSource {
        self.source
    }

    /// `getBadgeNo()`.
    pub fn badge_no(&self) -> &str {
        &self.badge_no
    }

    /// Set the adjusted time. `setAdjTime()` — no side effects in Java either.
    pub fn set_adj_time(&mut self, adj_time: Option<LocalDateTime>) {
        self.adj_time = adj_time;
    }

    /// Set the badge number. `setBadgeNo()`.
    pub fn set_badge_no(&mut self, badge_no: impl Into<String>) {
        self.badge_no = badge_no.into();
    }

    /// Write the rounded time, with **no** callback to the owning shift.
    ///
    /// Deliberately `pub(crate)`: reaching this directly would skip
    /// `resetStartAndEndTimesFromPunch`, which is exactly the bug the cursor
    /// exists to prevent. Rules go through
    /// [`PunchCursor::set_rounded_time`](crate::entity::employee_shift::PunchCursor::set_rounded_time).
    ///
    /// Returns whether the value actually changed, which is the condition Java
    /// tests before firing the callback.
    pub(crate) fn set_rounded_time(&mut self, rounded_time: Option<LocalDateTime>) -> bool {
        let changed = self.rounded_time != rounded_time;
        self.rounded_time = rounded_time;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    #[test]
    fn a_new_punch_has_all_three_times_equal() {
        let punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(7, 40));

        assert_eq!(punch.punch_time(), Some(at(7, 40)));
        assert_eq!(punch.adj_time(), Some(at(7, 40)));
        assert_eq!(punch.rounded_time(), Some(at(7, 40)));
    }

    #[test]
    fn a_punch_carries_its_type_and_source() {
        let punch = EmployeeShiftPunch::new(1, PunchType::Break, PunchSource::Manual, at(12, 0));

        assert_eq!(punch.id(), 1);
        assert_eq!(punch.punch_type(), PunchType::Break);
        assert_eq!(punch.source(), PunchSource::Manual);
        assert_eq!(punch.badge_no(), "");
    }

    #[test]
    fn setting_the_rounded_time_reports_whether_it_changed() {
        // The condition Java tests before calling back into the shift.
        let mut punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(7, 40));

        assert!(!punch.set_rounded_time(Some(at(7, 40))), "same value");
        assert!(punch.set_rounded_time(Some(at(7, 45))), "different value");
        assert_eq!(punch.rounded_time(), Some(at(7, 45)));
        assert!(punch.set_rounded_time(None), "clearing is a change");
        assert_eq!(punch.rounded_time(), None);
    }

    #[test]
    fn the_adjusted_time_is_independent_of_the_rounded_time() {
        // Rounding rules read adj_time and write rounded_time, so a rule that
        // runs twice rounds from the same base both times.
        let mut punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(7, 40));

        punch.set_rounded_time(Some(at(7, 45)));

        assert_eq!(punch.adj_time(), Some(at(7, 40)));
    }

    #[test]
    fn the_adjusted_time_can_be_replaced() {
        let mut punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(7, 40));
        punch.set_adj_time(Some(at(8, 0)));
        assert_eq!(punch.adj_time(), Some(at(8, 0)));
        assert_eq!(punch.punch_time(), Some(at(7, 40)), "the raw time is kept");
    }
}
