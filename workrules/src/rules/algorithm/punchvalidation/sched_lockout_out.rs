//! Port of `SchedLockoutOutPunchValidationRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockoutOutPunchValidationRuleImpl.java`.
//!
//! Hold an out punch to its schedule, mirroring
//! [`SchedLockoutInPunchValidationRule`](super::sched_lockout_in::SchedLockoutInPunchValidationRule)
//! at the other end of the shift. Three differences are worth naming:
//!
//! * it **overrides `findClosestScheduleToPunch`** to measure against each
//!   schedule's `endDateTime` rather than its start — the base's version, which
//!   the in-punch rule uses, measures from the start;
//! * it has no `jobLockLevel`: the punch's job is checked only for validity,
//!   never against the schedule's job;
//! * it adds an `unscheduled` switch, which turns both the lockout and warning
//!   tests inside out. With it off, "outside the grace window" fails. With it
//!   on, only the *bands between* the grace window and the wider pre/post punch
//!   window fail — beyond those, the punch is treated as unscheduled and
//!   allowed through.

use crate::common::enums::uftc_punch_type::UFTCPunchType;
use crate::entity::employee::Employee;
use crate::entity::punch_log::PunchLog;
use crate::entity::time_clock_result::TimeClockServerResult;
use crate::rules::algorithm::punchvalidation::config::{
    LOCK_GRACE_POST, LOCK_GRACE_PRE, LOCKOUT, POST_PUNCH, PRE_PUNCH,
    SchedLockoutOutPunchValidationRuleConfig, UNSCHEDULED, WARN, WARN_GRACE_POST, WARN_GRACE_PRE,
};
use crate::rules::algorithm::punchvalidation::{
    PunchValidationRule, ScheduleShiftForPunchValidation, employee_has_invalid_job_code,
    in_between_exclusive_pre_and_post_times, invalid_job_code_failure, lockout_failure,
    outside_pre_and_post_times, warning_failure,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// The six boundaries around a scheduled end. `setGracePeriods`.
struct GracePeriods {
    lock_pre: LocalDateTime,
    lock_post: LocalDateTime,
    warn_pre: LocalDateTime,
    warn_post: LocalDateTime,
    pre_punch: LocalDateTime,
    post_punch: LocalDateTime,
}

impl GracePeriods {
    fn of(params: &RuleParams, schedule_end: LocalDateTime) -> Self {
        Self {
            lock_pre: schedule_end.minus_minutes(i64::from(params.int_at(LOCK_GRACE_PRE))),
            lock_post: schedule_end.plus_minutes(i64::from(params.int_at(LOCK_GRACE_POST))),
            warn_pre: schedule_end.minus_minutes(i64::from(params.int_at(WARN_GRACE_PRE))),
            warn_post: schedule_end.plus_minutes(i64::from(params.int_at(WARN_GRACE_POST))),
            pre_punch: schedule_end.minus_minutes(i64::from(params.int_at(PRE_PUNCH))),
            post_punch: schedule_end.plus_minutes(i64::from(params.int_at(POST_PUNCH))),
        }
    }
}

/// Hold an out punch to its schedule. `SchedLockoutOutPunchValidationRuleImpl`.
pub struct SchedLockoutOutPunchValidationRule;

impl PunchValidationRule for SchedLockoutOutPunchValidationRule {
    fn execute(
        &self,
        employee: Option<&Employee>,
        punch_log: Option<&PunchLog>,
        job_id: i32,
        scheduled_shifts: &[ScheduleShiftForPunchValidation],
        params: &RuleParams,
    ) -> TimeClockServerResult {
        let params = params.fixed(&SchedLockoutOutPunchValidationRuleConfig.default_values());

        let lockout = params.bool_at(LOCKOUT);
        let warn = params.bool_at(WARN);
        let unscheduled = params.bool_at(UNSCHEDULED);

        let Some(punch_log) = punch_log else {
            return TimeClockServerResult::success();
        };

        // `shouldValidateOutPunch` — everything else falls through to success.
        if !(lockout || warn) || punch_log.punch_type() != UFTCPunchType::Out {
            return TimeClockServerResult::success();
        }

        if employee_has_invalid_job_code(employee, punch_log, job_id) {
            return invalid_job_code_failure(&params);
        }

        let out_punch = punch_log.punch_date_time();

        let Some(best_schedule) = closest_schedule_by_end(scheduled_shifts, out_punch) else {
            return if lockout {
                lockout_failure(&params)
            } else {
                warning_failure(&params)
            };
        };

        let grace = GracePeriods::of(&params, best_schedule.end_date_time());

        if in_lockout_period(out_punch, lockout, unscheduled, &grace) {
            return lockout_failure(&params);
        }

        if in_warning_period(out_punch, lockout, warn, unscheduled, &grace) {
            return warning_failure(&params);
        }

        TimeClockServerResult::success()
    }
}

/// The schedule **ending** nearest the punch.
///
/// `SchedLockoutOutPunchValidationRuleImpl.findClosestScheduleToPunch`, which
/// overrides the base's start-based version. An out punch belongs to the shift
/// it is ending, not the one starting soonest.
fn closest_schedule_by_end(
    scheduled_shifts: &[ScheduleShiftForPunchValidation],
    punch_date_time: LocalDateTime,
) -> Option<&ScheduleShiftForPunchValidation> {
    let distance = |schedule: &ScheduleShiftForPunchValidation| {
        (schedule.end_date_time().epoch_seconds() - punch_date_time.epoch_seconds()).abs()
    };

    let mut best: Option<&ScheduleShiftForPunchValidation> = None;
    for schedule in scheduled_shifts {
        if best.is_none_or(|current| distance(schedule) < distance(current)) {
            best = Some(schedule);
        }
    }
    best
}

/// `inLockoutPeriod`.
fn in_lockout_period(
    out_punch: LocalDateTime,
    lockout: bool,
    unscheduled: bool,
    grace: &GracePeriods,
) -> bool {
    if !lockout {
        return false;
    }

    if unscheduled {
        // Only the two bands between the grace window and the punch window.
        in_between_exclusive_pre_and_post_times(
            out_punch,
            grace.pre_punch,
            grace.lock_pre,
            grace.lock_post,
            grace.post_punch,
        )
    } else {
        outside_pre_and_post_times(out_punch, grace.lock_pre, grace.lock_post)
    }
}

/// `inWarningPeriod`.
fn in_warning_period(
    out_punch: LocalDateTime,
    lockout: bool,
    warn: bool,
    unscheduled: bool,
    grace: &GracePeriods,
) -> bool {
    if !warn {
        return false;
    }

    if lockout {
        return in_between_exclusive_pre_and_post_times(
            out_punch,
            grace.lock_pre,
            grace.warn_pre,
            grace.warn_post,
            grace.lock_post,
        );
    }

    if unscheduled {
        return in_between_exclusive_pre_and_post_times(
            out_punch,
            grace.pre_punch,
            grace.warn_pre,
            grace.warn_post,
            grace.post_punch,
        );
    }

    outside_pre_and_post_times(out_punch, grace.warn_pre, grace.warn_post)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;
    use joda_rs::LocalDateTime;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    #[test]
    fn the_closest_schedule_is_measured_from_its_end() {
        // The base measures from the start; this rule overrides that. The
        // second schedule starts later but ends nearer the punch.
        let schedules = [
            ScheduleShiftForPunchValidation::new(1, at(8, 0), at(16, 0)),
            ScheduleShiftForPunchValidation::new(2, at(15, 0), at(17, 30)),
        ];

        assert_eq!(
            closest_schedule_by_end(&schedules, at(17, 0))
                .unwrap()
                .job_id(),
            2
        );
        // And the base, for contrast, would pick the first.
        assert_eq!(
            super::super::find_closest_schedule_to_punch(&schedules, at(17, 0))
                .unwrap()
                .job_id(),
            2
        );
    }

    #[test]
    fn the_unscheduled_switch_inverts_the_lockout_test() {
        let grace = GracePeriods::of(
            &SchedLockoutOutPunchValidationRuleConfig.default_values(),
            at(16, 0),
        );

        // Far outside everything. Without `unscheduled` that is a lockout;
        // with it, the punch is treated as unscheduled and allowed.
        let far_out = at(23, 0);
        assert!(in_lockout_period(far_out, true, false, &grace));
        assert!(!in_lockout_period(far_out, true, true, &grace));
    }

    #[test]
    fn nothing_is_locked_out_when_lockout_is_off() {
        let grace = GracePeriods::of(
            &SchedLockoutOutPunchValidationRuleConfig.default_values(),
            at(16, 0),
        );
        assert!(!in_lockout_period(at(23, 0), false, false, &grace));
    }

    #[test]
    fn a_configured_message_replaces_the_default() {
        use crate::common::enums::employee_pay_type::EmployeePayType;
        use crate::entity::employee_job_status::EmployeeJobStatus;
        use joda_rs::LocalDate;

        let employee = Employee::new(
            100,
            11,
            "Alex Kim",
            vec![EmployeeJobStatus::new(
                1,
                100,
                1,
                LocalDate::of(2010, 1, 1),
                LocalDate::of(2010, 12, 31),
                EmployeePayType::Hourly,
                12.50,
                true,
            )],
        );
        let punch = PunchLog::new(at(12, 0), UFTCPunchType::Out);

        let result = SchedLockoutOutPunchValidationRule.execute(
            Some(&employee),
            Some(&punch),
            1,
            &[],
            &rule_params! {
                LOCKOUT => "true",
                crate::rules::algorithm::punchvalidation::config::LOCKOUT_MESSAGE => "Too late",
            },
        );

        assert!(result.has_message("Too late"));
    }
}

/// Cases ported from the Java engine's own test suite.
///
/// Transcribed from
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockoutOutPunchValidationRuleImplTest.groovy`,
/// including its `where:` tables row for row.
///
/// The Groovy anchors everything on `new LocalDateTime()` — today at noon —
/// which is pinned to a fixed date here; the rule only compares a punch against
/// a schedule, so the absolute date is immaterial. Default messages are
/// asserted by resource key rather than resolved string, as elsewhere.
#[cfg(test)]
mod java_parity_tests {
    // The transcribed `where:` tables carry one parameter per Groovy column,
    // which is what makes them checkable against the spec side by side.
    #![allow(clippy::too_many_arguments)]

    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::rule_params;
    use joda_rs::LocalDate;
    use rstest::rstest;

    fn noon_today() -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, 12, 0, 0)
    }

    /// `Mock(Employee) { getEmployeeJobStatus(_, today) >> new EmployeeJobStatus() }`
    /// — every job resolves, so the invalid-job branch is never taken.
    fn employee() -> Employee {
        Employee::new(
            100,
            11,
            "Mock",
            vec![EmployeeJobStatus::new(
                1,
                100,
                0,
                LocalDate::of(2010, 1, 1),
                LocalDate::of(2010, 12, 31),
                EmployeePayType::Hourly,
                12.50,
                true,
            )],
        )
    }

    /// The spec's schedules: one ending at noon, one ending nine hours later.
    fn scheduled_shifts() -> Vec<ScheduleShiftForPunchValidation> {
        vec![
            ScheduleShiftForPunchValidation::new(1, noon_today().minus_hours(1), noon_today()),
            ScheduleShiftForPunchValidation::new(
                2,
                noon_today().plus_hours(9),
                noon_today().plus_hours(10),
            ),
        ]
    }

    fn run(
        punch_at: LocalDateTime,
        scheduled_shifts: &[ScheduleShiftForPunchValidation],
        params: RuleParams,
    ) -> TimeClockServerResult {
        let punch = PunchLog::new(punch_at, UFTCPunchType::Out);
        SchedLockoutOutPunchValidationRule.execute(
            Some(&employee()),
            Some(&punch),
            0,
            scheduled_shifts,
            &params,
        )
    }

    fn switches(lockout: bool, warn: bool, unscheduled: bool) -> RuleParams {
        rule_params! {
            LOCKOUT => if lockout { "true" } else { "false" },
            WARN => if warn { "true" } else { "false" },
            UNSCHEDULED => if unscheduled { "true" } else { "false" },
        }
    }

    #[test]
    fn not_selecting_lockout_or_warn_always_returns_a_successful_result() {
        let result =
            SchedLockoutOutPunchValidationRule.execute(None, None, 0, &[], &RuleParams::new());
        assert!(result.is_success());
    }

    #[rstest]
    #[case(UFTCPunchType::In)]
    #[case(UFTCPunchType::Break)]
    #[case(UFTCPunchType::Back)]
    #[case(UFTCPunchType::Tips)]
    #[case(UFTCPunchType::Meal)]
    fn a_non_out_punch_always_returns_a_successful_result(#[case] punch_type: UFTCPunchType) {
        let punch = PunchLog::new(noon_today(), punch_type);

        let result = SchedLockoutOutPunchValidationRule.execute(
            None,
            Some(&punch),
            0,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(result.is_success());
    }

    #[test]
    fn must_be_a_valid_active_employee_to_punch_out() {
        let punch = PunchLog::new(noon_today(), UFTCPunchType::Out);

        let result = SchedLockoutOutPunchValidationRule.execute(
            Some(&employee()),
            Some(&punch),
            99,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(!result.is_success());
        assert!(!result.overridable());
        assert_eq!(result.reject_resource_key(), Some("res_invalidJobCode"));
        assert!(result.has_message("res_jobLockoutMessage"));
    }

    #[rstest]
    // lockout | warn  | rejectResourceKey | messageKey | overridable | managerOverridable
    #[case(
        true,
        false,
        "res_scheduleLockout",
        "res_scheduleLockoutMessage",
        false,
        true
    )]
    #[case(
        false,
        true,
        "res_scheduleWarningPeriod",
        "res_scheduleGracePeriodWarning",
        true,
        false
    )]
    #[case(
        true,
        true,
        "res_scheduleLockout",
        "res_scheduleLockoutMessage",
        false,
        true
    )]
    fn return_failure_message_when_schedule_does_not_exist(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] reject_resource_key: &str,
        #[case] message_key: &str,
        #[case] overridable: bool,
        #[case] manager_overridable: bool,
    ) {
        let result = run(noon_today(), &[], switches(lockout, warn, false));

        assert!(!result.is_success());
        assert_eq!(result.overridable(), overridable);
        assert_eq!(result.reject_resource_key(), Some(reject_resource_key));
        assert!(result.has_message(message_key));
        assert_eq!(result.is_manager_overridable(), manager_overridable);
    }

    #[rstest]
    // lockout | warn | unscheduled | outPunchDateTime (minutes from the schedule end)
    #[case(true, false, false, -30)]
    #[case(true, false, false, 30)]
    #[case(true, false, true, -240)]
    #[case(true, false, true, 240)]
    #[case(false, true, false, -15)]
    #[case(false, true, false, 15)]
    #[case(false, true, true, -240)]
    #[case(false, true, true, 240)]
    #[case(false, false, true, -239)]
    #[case(false, false, true, -241)]
    #[case(true, true, true, -240)]
    #[case(true, true, true, 240)]
    fn out_punches_within_grace_or_outside_the_punch_limit_return_success(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] unscheduled: bool,
        #[case] offset_minutes: i64,
    ) {
        let punch_at = noon_today().plus_minutes(offset_minutes);

        let result = run(
            punch_at,
            &scheduled_shifts(),
            switches(lockout, warn, unscheduled),
        );

        assert!(result.is_success());
    }

    #[rstest]
    // lockout | warn | unscheduled | offset | overridable | managerOverridable | messageKey | rejectResourceKey
    #[case(true, false, false, -31, false, true, "res_scheduleLockoutMessage", "res_scheduleLockout")]
    #[case(
        true,
        false,
        false,
        31,
        false,
        true,
        "res_scheduleLockoutMessage",
        "res_scheduleLockout"
    )]
    #[case(true, false, true, -239, false, true, "res_scheduleLockoutMessage", "res_scheduleLockout")]
    #[case(
        true,
        false,
        true,
        239,
        false,
        true,
        "res_scheduleLockoutMessage",
        "res_scheduleLockout"
    )]
    #[case(false, true, false, -31, true, false, "res_scheduleGracePeriodWarning", "res_scheduleWarningPeriod")]
    #[case(
        false,
        true,
        false,
        31,
        true,
        false,
        "res_scheduleGracePeriodWarning",
        "res_scheduleWarningPeriod"
    )]
    #[case(false, true, true, -239, true, false, "res_scheduleGracePeriodWarning", "res_scheduleWarningPeriod")]
    #[case(
        false,
        true,
        true,
        239,
        true,
        false,
        "res_scheduleGracePeriodWarning",
        "res_scheduleWarningPeriod"
    )]
    #[case(false, true, false, -239, true, false, "res_scheduleGracePeriodWarning", "res_scheduleWarningPeriod")]
    #[case(
        false,
        true,
        false,
        239,
        true,
        false,
        "res_scheduleGracePeriodWarning",
        "res_scheduleWarningPeriod"
    )]
    #[case(true, true, false, -16, true, false, "res_scheduleGracePeriodWarning", "res_scheduleWarningPeriod")]
    #[case(
        true,
        true,
        true,
        16,
        true,
        false,
        "res_scheduleGracePeriodWarning",
        "res_scheduleWarningPeriod"
    )]
    #[case(true, true, true, -239, false, true, "res_scheduleLockoutMessage", "res_scheduleLockout")]
    #[case(
        true,
        true,
        true,
        239,
        false,
        true,
        "res_scheduleLockoutMessage",
        "res_scheduleLockout"
    )]
    fn out_punch_outside_of_grace_period_returns_a_failure(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] unscheduled: bool,
        #[case] offset_minutes: i64,
        #[case] overridable: bool,
        #[case] manager_overridable: bool,
        #[case] message_key: &str,
        #[case] reject_resource_key: &str,
    ) {
        // Note this spec lists its schedules in the opposite order to the
        // success table above, which is what makes the end-based closest-match
        // override observable.
        let schedules = vec![
            ScheduleShiftForPunchValidation::new(
                2,
                noon_today().plus_hours(9),
                noon_today().plus_hours(10),
            ),
            ScheduleShiftForPunchValidation::new(1, noon_today().minus_hours(1), noon_today()),
        ];

        let result = run(
            noon_today().plus_minutes(offset_minutes),
            &schedules,
            switches(lockout, warn, unscheduled),
        );

        assert!(!result.is_success());
        assert_eq!(result.overridable(), overridable);
        assert!(result.has_message(message_key));
        assert_eq!(result.reject_resource_key(), Some(reject_resource_key));
        assert_eq!(result.is_manager_overridable(), manager_overridable);
    }
}
