//! Port of `NoInPunchValidationRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/NoInPunchValidationRuleImpl.java`.
//!
//! Accept everything. Twenty-four lines in Java, whose `execute` is a single
//! `return new TimeClockServerResultDTO()`.
//!
//! It exists so that a site can configure "no in-punch validation" explicitly,
//! rather than leaving the rule type unconfigured — which is not the same
//! thing, because an unconfigured rule type falls through to whatever the
//! property default is.

use crate::entity::employee::Employee;
use crate::entity::punch_log::PunchLog;
use crate::entity::time_clock_result::TimeClockServerResult;
use crate::rules::algorithm::punchvalidation::{
    PunchValidationRule, ScheduleShiftForPunchValidation,
};
use crate::rules::params::RuleParams;

/// Accept every punch. `NoInPunchValidationRuleImpl`.
pub struct NoInPunchValidationRule;

impl PunchValidationRule for NoInPunchValidationRule {
    fn execute(
        &self,
        _employee: Option<&Employee>,
        _punch_log: Option<&PunchLog>,
        _job_id: i32,
        _scheduled_shifts: &[ScheduleShiftForPunchValidation],
        _params: &RuleParams,
    ) -> TimeClockServerResult {
        TimeClockServerResult::success()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::uftc_punch_type::UFTCPunchType;
    use joda_rs::LocalDateTime;

    #[test]
    fn everything_is_accepted() {
        let punch = PunchLog::new(LocalDateTime::of(2010, 1, 2, 12, 0, 0), UFTCPunchType::In);

        let result =
            NoInPunchValidationRule.execute(None, Some(&punch), 99, &[], &RuleParams::new());

        assert!(result.is_success());
        assert!(result.messages().is_empty());
        assert_eq!(result.reject_resource_key(), None);
    }

    #[test]
    fn even_a_punch_with_no_context_at_all_is_accepted() {
        assert!(
            NoInPunchValidationRule
                .execute(None, None, 0, &[], &RuleParams::new())
                .is_success()
        );
    }
}
