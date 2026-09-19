//! `RuleType::ScheduleRestriction` — six concrete rules behind
//! `ScheduleRestrictionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulerestriction/`.
//!
//! The first family ported for scheduling proper, at the user's request —
//! ahead of the other 22 not-yet-started families, and the only one of the
//! seven prioritized families whose rules actually gate whether a shift can
//! be scheduled at all, rather than adjusting or pricing one already on the
//! calendar.
//!
//! # The shape is new: a result, not a write
//!
//! Every family ported before this one either writes a rate/distribution
//! field or produces an earning. `ScheduleRestrictionRuleImpl` computes
//! instead: `canEmployeeWorkShift(ScheduleCalcDataSet, EmployeeShift,
//! RuleItem) -> ScheduleRestrictionResult`, plus a second method,
//! `isStrict(RuleItem)`, that decides whether the caller should hard-block
//! the shift or only warn. Nothing here mutates its arguments.
//!
//! `ScheduleCalcDataSet` needs no new Rust type — it is reached as `&dyn
//! TimeCard`, the same one-struct-for-both-implementations shape divergence
//! 23 already settled for the actuals/schedule split.
//!
//! # `MonthlyRequiredDaysOffRuleImpl` is not ported
//!
//! It is the only rule in the family needing genuinely new infrastructure:
//! `Property.getPlanningPeriodType()`/`getPlanningPeriod()` (a "monthly
//! planning period" concept distinct from the pay period this crate already
//! carries), `DaysOffCalculator` (a standalone calculator class, not a DAO
//! method), and `EmployeeTimeOff`/`TORDistribution` entities — none ported.
//! Deferred the way `AnnualSalaryOverHoursRegRateRuleImpl` was deferred from
//! `regularrate`: a catalogue entry (`MonthlyRequiredDaysOffSrr`) exists, no
//! algorithm here yet.
//!
//! Ported cases: `NoRestrictionRuleImplTest.groovy`,
//! `ScheduleRestrictionResultTest.groovy`,
//! `EarliestStartLatestEndTimeRuleImplTest.groovy`,
//! `MaxDaysWorkedPerWeekRuleImplTest.groovy`, `MaxHoursOnDayRuleImplTest.groovy`,
//! `MaxHoursPerWeekRuleImplTest.groovy` — full spec coverage for every rule
//! ported.

pub mod config;
pub mod earliest_start_latest_end_time;
pub mod max_days_worked_per_week;
pub mod max_hours_on_day;
pub mod max_hours_per_week;
pub mod no_restriction;

use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::config::{
    STRICT_MODE, schedule_restriction_default_values,
};

/// Whether an employee can work a proposed shift. `ScheduleRestrictionRuleImpl`.
pub trait ScheduleRestrictionRule {
    /// `canEmployeeWorkShift(ScheduleCalcDataSet, EmployeeShift, RuleItem)`.
    fn can_employee_work_shift(
        &self,
        dataset: &dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult;

    /// Whether a violation should hard-block the shift (`true`) or only
    /// warn. `isStrict(RuleItem)`.
    ///
    /// `BaseScheduleRestrictionRuleImpl.isStrict` reads `STRICT_MODE`
    /// straight off the raw params, unfixed — `params.get(STRICT_MODE)`, no
    /// `fixMap` call, `null` treated as `false`. Reading it through
    /// `.fixed()` instead (this crate's uniform shape, divergence 9) lands
    /// on the same answer here because the config's own default is also
    /// `"false"`. [`NoRestrictionRuleImpl`](no_restriction::NoRestrictionRule)
    /// overrides this to a hardcoded `false` rather than reading any
    /// parameter, the one override in the family.
    fn is_strict(&self, rule_item: &RuleItem) -> bool {
        rule_item
            .params()
            .fixed(&schedule_restriction_default_values())
            .bool_at(STRICT_MODE)
    }
}

/// Whether a shift may be scheduled, and why not if it may not.
/// `ScheduleRestrictionResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleRestrictionResult {
    error: Option<ShiftErrorType>,
}

impl ScheduleRestrictionResult {
    /// No restriction applies. `new ScheduleRestrictionResult(null)`, the
    /// shape every rule's `okResult()` helper builds.
    pub fn ok() -> Self {
        Self { error: None }
    }

    /// A restriction was violated. `errorResult(ShiftErrorType)`.
    pub fn error(error: ShiftErrorType) -> Self {
        Self { error: Some(error) }
    }

    /// `isOK()`.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// `getShiftErrorType()`.
    pub fn shift_error_type(&self) -> Option<ShiftErrorType> {
        self.error
    }

    /// `getMessage()` — the error type's own Java `toString()`, or `None`
    /// where Java returns `null`.
    pub fn message(&self) -> Option<&'static str> {
        self.error.map(|error| error.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ok_result_carries_no_error_or_message() {
        let result = ScheduleRestrictionResult::ok();
        assert!(result.is_ok());
        assert_eq!(result.shift_error_type(), None);
        assert_eq!(result.message(), None);
    }

    #[test]
    fn an_error_result_carries_the_error_and_its_name_as_the_message() {
        let result = ScheduleRestrictionResult::error(ShiftErrorType::RequiredDaysOff);
        assert!(!result.is_ok());
        assert_eq!(
            result.shift_error_type(),
            Some(ShiftErrorType::RequiredDaysOff)
        );
        assert_eq!(result.message(), Some("REQUIRED_DAYS_OFF"));
    }

    mod java_parity_tests {
        use super::*;

        /// `ScheduleRestrictionResultTest`: "should be ok if there is no
        /// error".
        #[test]
        fn should_be_ok_if_there_is_no_error() {
            assert!(ScheduleRestrictionResult::ok().is_ok());
            assert!(!ScheduleRestrictionResult::error(ShiftErrorType::RequiredDaysOff).is_ok());
        }

        /// `ScheduleRestrictionResultTest`: "message should be error type
        /// toString".
        #[test]
        fn message_should_be_error_type_to_string() {
            assert_eq!(ScheduleRestrictionResult::ok().message(), None);
            assert_eq!(
                ScheduleRestrictionResult::error(ShiftErrorType::RequiredDaysOff).message(),
                Some("REQUIRED_DAYS_OFF")
            );
        }
    }
}
