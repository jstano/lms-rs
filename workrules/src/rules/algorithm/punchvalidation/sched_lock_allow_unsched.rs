//! Port of `SchedLockAllowUnschedIPVRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockAllowUnschedIPVRuleImpl.java`.
//!
//! Hold an in punch to its schedule — but let a punch far enough from any
//! schedule through as *unscheduled* rather than rejecting it. Between the
//! grace window and that outer allowance sits the band where the rule bites.
//!
//! Almost a twin of
//! [`SchedLockoutInPunchValidationRule`](super::sched_lockout_in::SchedLockoutInPunchValidationRule),
//! and the differences are all in the tail:
//!
//! * no schedule at all is a **not-scheduled** failure a manager may override,
//!   where the lockout rule locks out or warns;
//! * outside the allowed pre/post punch window is likewise not-scheduled,
//!   rather than a lockout;
//! * it uses the **base** job-failure shapes, which set `managerOverridable` —
//!   the lockout rule overrides all three and does not.

use crate::common::enums::schedule_lockout_level::ScheduleLockoutLevel;
use crate::common::enums::uftc_punch_type::UFTCPunchType;
use crate::entity::employee::Employee;
use crate::entity::punch_log::PunchLog;
use crate::entity::time_clock_result::TimeClockServerResult;
use crate::rules::algorithm::punchvalidation::config::{
    JOB_LOCK_LEVEL, LOCK_GRACE_POST, LOCK_GRACE_PRE, LOCKOUT, POST_PUNCH, PRE_PUNCH,
    SchedLockAllowUnschedIpvRuleConfig, WARN, WARN_GRACE_POST, WARN_GRACE_PRE,
};
use crate::rules::algorithm::punchvalidation::{
    PunchValidationRule, ScheduleShiftForPunchValidation, employee_has_invalid_job_code,
    find_closest_schedule_to_punch, in_between_exclusive_pre_and_post_times,
    invalid_job_code_failure, job_lockout_failure, job_lockout_warn_failure,
    job_overridable_lockout_failure, lockout_failure, not_scheduled_failure,
    not_scheduled_manager_overridable_failure, warning_failure,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// The six boundaries around a scheduled start. `setGracePeriods`.
struct GracePeriods {
    lock_pre: LocalDateTime,
    lock_post: LocalDateTime,
    warn_pre: LocalDateTime,
    warn_post: LocalDateTime,
    allow_pre: LocalDateTime,
    allow_post: LocalDateTime,
}

impl GracePeriods {
    fn of(params: &RuleParams, schedule_start: LocalDateTime) -> Self {
        Self {
            lock_pre: schedule_start.minus_minutes(i64::from(params.int_at(LOCK_GRACE_PRE))),
            lock_post: schedule_start.plus_minutes(i64::from(params.int_at(LOCK_GRACE_POST))),
            warn_pre: schedule_start.minus_minutes(i64::from(params.int_at(WARN_GRACE_PRE))),
            warn_post: schedule_start.plus_minutes(i64::from(params.int_at(WARN_GRACE_POST))),
            allow_pre: schedule_start.minus_minutes(i64::from(params.int_at(PRE_PUNCH))),
            allow_post: schedule_start.plus_minutes(i64::from(params.int_at(POST_PUNCH))),
        }
    }
}

/// Hold an in punch to its schedule, allowing unscheduled punches.
/// `SchedLockAllowUnschedIPVRuleImpl`.
pub struct SchedLockAllowUnschedIpvRule;

impl PunchValidationRule for SchedLockAllowUnschedIpvRule {
    fn execute(
        &self,
        employee: Option<&Employee>,
        punch_log: Option<&PunchLog>,
        job_id: i32,
        scheduled_shifts: &[ScheduleShiftForPunchValidation],
        params: &RuleParams,
    ) -> TimeClockServerResult {
        let params = params.fixed(&SchedLockAllowUnschedIpvRuleConfig.default_values());

        let lockout = params.bool_at(LOCKOUT);
        let warn = params.bool_at(WARN);
        let job_lock_level = params
            .get(JOB_LOCK_LEVEL)
            .and_then(ScheduleLockoutLevel::from_code)
            .unwrap_or(ScheduleLockoutLevel::None);

        if !lockout && !warn {
            return TimeClockServerResult::success();
        }

        let Some(punch_log) = punch_log else {
            return TimeClockServerResult::success();
        };

        if punch_log.punch_type() != UFTCPunchType::In {
            return TimeClockServerResult::success();
        }

        if employee_has_invalid_job_code(employee, punch_log, job_id) {
            return invalid_job_code_failure(&params);
        }

        let in_punch = punch_log.punch_date_time();

        let Some(best_schedule) = find_closest_schedule_to_punch(scheduled_shifts, in_punch) else {
            // Where the lockout rule rejects, this one calls it unscheduled.
            return not_scheduled_manager_overridable_failure(&params);
        };

        let schedule_job_id = best_schedule.job_id();
        let grace = GracePeriods::of(&params, best_schedule.start_date_time());

        if within_grace_period(lockout, warn, in_punch, &grace) {
            grace_period_result(&params, job_id, job_lock_level, schedule_job_id)
        } else {
            failure_result(
                &params,
                lockout,
                warn,
                job_id,
                job_lock_level,
                in_punch,
                schedule_job_id,
                &grace,
            )
        }
    }
}

/// `withinGracePeriod` — identical to the lockout rule's, including the
/// asymmetry: the lockout window is only consulted when `warn` is off.
fn within_grace_period(
    lockout: bool,
    warn: bool,
    in_punch: LocalDateTime,
    grace: &GracePeriods,
) -> bool {
    let within_warn =
        warn && in_punch.is_after(grace.warn_pre) && in_punch.is_before(grace.warn_post);
    let within_lockout = !warn
        && lockout
        && in_punch.is_after(grace.lock_pre)
        && in_punch.is_before(grace.lock_post);

    within_warn || within_lockout
}

/// `getGracePeriodResult`. Uses the **base** job-failure shapes.
fn grace_period_result(
    params: &RuleParams,
    job_id: i32,
    job_lock_level: ScheduleLockoutLevel,
    schedule_job_id: i32,
) -> TimeClockServerResult {
    if job_id == schedule_job_id {
        return TimeClockServerResult::success();
    }

    match job_lock_level {
        ScheduleLockoutLevel::Lock => job_lockout_failure(params),
        ScheduleLockoutLevel::Warn => job_overridable_lockout_failure(params),
        ScheduleLockoutLevel::None => TimeClockServerResult::success(),
    }
}

/// `getFailureResult` — three branches, and the shape that distinguishes this
/// rule from the plain lockout one.
#[allow(clippy::too_many_arguments)]
fn failure_result(
    params: &RuleParams,
    lockout: bool,
    warn: bool,
    job_id: i32,
    job_lock_level: ScheduleLockoutLevel,
    in_punch: LocalDateTime,
    schedule_job_id: i32,
    grace: &GracePeriods,
) -> TimeClockServerResult {
    // Both switches: the bands between the warn and lockout windows warn.
    if warn
        && lockout
        && in_between_exclusive_pre_and_post_times(
            in_punch,
            grace.lock_pre,
            grace.warn_pre,
            grace.warn_post,
            grace.lock_post,
        )
    {
        return warning_period_failure(params, job_id, job_lock_level, schedule_job_id);
    }

    // Warn only: the bands between the warn window and the outer allowance
    // warn; beyond them the punch is simply unscheduled.
    if warn && !lockout {
        return if in_between_exclusive_pre_and_post_times(
            in_punch,
            grace.allow_pre,
            grace.warn_pre,
            grace.warn_post,
            grace.allow_post,
        ) {
            warning_period_failure(params, job_id, job_lock_level, schedule_job_id)
        } else {
            not_scheduled_failure(params)
        };
    }

    // Lockout (with or without warn, having fallen past the first branch): the
    // bands between the lockout window and the allowance lock out.
    if in_between_exclusive_pre_and_post_times(
        in_punch,
        grace.allow_pre,
        grace.lock_pre,
        grace.lock_post,
        grace.allow_post,
    ) {
        lockout_failure(params)
    } else {
        not_scheduled_failure(params)
    }
}

/// `getWarningPeriodFailure`. Uses the **base** job-failure shapes.
fn warning_period_failure(
    params: &RuleParams,
    job_id: i32,
    job_lock_level: ScheduleLockoutLevel,
    schedule_job_id: i32,
) -> TimeClockServerResult {
    if job_id != schedule_job_id {
        match job_lock_level {
            ScheduleLockoutLevel::Lock => return job_lockout_failure(params),
            ScheduleLockoutLevel::Warn => return job_lockout_warn_failure(params),
            ScheduleLockoutLevel::None => {}
        }
    }

    warning_failure(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::rule_params;
    use joda_rs::LocalDate;

    fn noon() -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, 12, 0, 0)
    }

    fn employee() -> Employee {
        Employee::new(
            100,
            11,
            "Alex Kim",
            vec![status(1, 0), status(2, 1), status(3, 99)],
        )
    }

    fn status(id: i32, job_id: i32) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            id,
            100,
            job_id,
            LocalDate::of(2010, 1, 1),
            LocalDate::of(2010, 12, 31),
            EmployeePayType::Hourly,
            12.50,
            false,
        )
    }

    fn schedules() -> Vec<ScheduleShiftForPunchValidation> {
        vec![ScheduleShiftForPunchValidation::new(
            1,
            noon(),
            noon().plus_hours(8),
        )]
    }

    #[test]
    fn no_schedule_at_all_is_an_overridable_not_scheduled_failure() {
        // Where the plain lockout rule locks out, this one allows the punch
        // through as unscheduled.
        let punch = PunchLog::new(noon(), UFTCPunchType::In);

        let result = SchedLockAllowUnschedIpvRule.execute(
            Some(&employee()),
            Some(&punch),
            0,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(!result.is_success());
        assert_eq!(result.reject_resource_key(), Some("res_notScheduled"));
        assert!(result.overridable());
        assert!(result.is_manager_overridable());
    }

    #[test]
    fn it_uses_the_base_job_failure_shapes() {
        // The distinguishing difference from SchedLockoutIn, which overrides
        // all three and drops managerOverridable.
        let punch = PunchLog::new(noon(), UFTCPunchType::In);

        let result = SchedLockAllowUnschedIpvRule.execute(
            Some(&employee()),
            Some(&punch),
            99,
            &schedules(),
            &rule_params! { LOCKOUT => "true", JOB_LOCK_LEVEL => "LOCK" },
        );

        assert_eq!(result.reject_resource_key(), Some("res_jobRestriction"));
        assert!(result.is_manager_overridable());
    }

    #[test]
    fn a_punch_on_the_right_job_and_on_time_passes() {
        let punch = PunchLog::new(noon(), UFTCPunchType::In);

        let result = SchedLockAllowUnschedIpvRule.execute(
            Some(&employee()),
            Some(&punch),
            1,
            &schedules(),
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(result.is_success());
    }
}

/// Cases ported from the Java engine's own test suite.
///
/// Transcribed from
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockAllowUnschedIPVRuleImplTest.groovy`,
/// including its `where:` tables row for row. Times are pinned to a fixed date
/// rather than `new LocalDateTime()`, and default messages are asserted by
/// resource key, as elsewhere in this family.
#[cfg(test)]
mod java_parity_tests {
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
    /// — any job resolves.
    fn employee() -> Employee {
        Employee::new(
            100,
            11,
            "Mock",
            (0..=99)
                .map(|job_id| {
                    EmployeeJobStatus::new(
                        job_id,
                        100,
                        job_id,
                        LocalDate::of(2010, 1, 1),
                        LocalDate::of(2010, 12, 31),
                        EmployeePayType::Hourly,
                        12.50,
                        false,
                    )
                })
                .collect(),
        )
    }

    /// The spec's single schedule, on job 1, starting at noon.
    fn scheduled_shifts() -> Vec<ScheduleShiftForPunchValidation> {
        vec![ScheduleShiftForPunchValidation::new(
            1,
            noon_today(),
            noon_today().plus_hours(1),
        )]
    }

    fn run(offset_minutes: i64, job_id: i32, params: RuleParams) -> TimeClockServerResult {
        let punch = PunchLog::new(noon_today().plus_minutes(offset_minutes), UFTCPunchType::In);
        SchedLockAllowUnschedIpvRule.execute(
            Some(&employee()),
            Some(&punch),
            job_id,
            &scheduled_shifts(),
            &params,
        )
    }

    fn switches(lockout: bool, warn: bool, job_lock_level: &str) -> RuleParams {
        rule_params! {
            LOCKOUT => if lockout { "true" } else { "false" },
            WARN => if warn { "true" } else { "false" },
            JOB_LOCK_LEVEL => job_lock_level,
        }
    }

    #[test]
    fn not_selecting_lockout_or_warn_always_returns_a_successful_result() {
        let result = SchedLockAllowUnschedIpvRule.execute(None, None, 0, &[], &RuleParams::new());
        assert!(result.is_success());
    }

    #[rstest]
    #[case(UFTCPunchType::Out)]
    #[case(UFTCPunchType::Break)]
    #[case(UFTCPunchType::Back)]
    #[case(UFTCPunchType::Meal)]
    fn always_returns_successful_for_non_in_punch_types(#[case] punch_type: UFTCPunchType) {
        let punch = PunchLog::new(noon_today(), punch_type);

        let result = SchedLockAllowUnschedIpvRule.execute(
            None,
            Some(&punch),
            0,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(result.is_success());
    }

    #[test]
    fn return_failure_message_when_schedule_does_not_exist() {
        let result = SchedLockAllowUnschedIpvRule.execute(
            Some(&employee()),
            Some(&PunchLog::new(noon_today(), UFTCPunchType::In)),
            0,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(!result.is_success());
        assert!(result.overridable());
        assert_eq!(result.reject_resource_key(), Some("res_notScheduled"));
        assert!(result.has_message("res_notScheduledWarning"));
        assert!(result.is_manager_overridable());
    }

    #[rstest]
    // lockout | warn | jobLockLevel | punch offset | overridable
    #[case(false, true, "LOCK", 14, false)]
    #[case(false, true, "LOCK", -14, false)]
    #[case(false, true, "WARN", 14, true)]
    #[case(false, true, "WARN", -14, true)]
    #[case(true, false, "LOCK", 29, false)]
    #[case(true, false, "LOCK", -29, false)]
    #[case(true, false, "WARN", 29, true)]
    #[case(true, false, "WARN", -29, true)]
    #[case(true, true, "LOCK", 29, false)]
    #[case(true, true, "LOCK", -29, false)]
    #[case(false, true, "LOCK", 239, false)]
    #[case(false, true, "LOCK", -239, false)]
    fn in_punches_with_a_job_lock_level_of_lock_or_warn_fail(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] job_lock_level: &str,
        #[case] offset_minutes: i64,
        #[case] overridable: bool,
    ) {
        // jobID 99 against a schedule on job 1.
        let result = run(offset_minutes, 99, switches(lockout, warn, job_lock_level));

        assert!(!result.is_success());
        assert_eq!(result.overridable(), overridable);
        assert!(result.has_message(if overridable {
            "res_jobLockoutOverridableMessage"
        } else {
            "res_jobLockoutMessage"
        }));
        assert_eq!(result.reject_resource_key(), Some("res_jobRestriction"));
        assert!(result.is_manager_overridable());
    }

    #[rstest]
    // lockout | jobLockLevel | jobID | punch offset
    #[case(true, "NONE", 0, 29)]
    #[case(true, "NONE", 0, -29)]
    #[case(false, "NONE", 0, 16)]
    #[case(false, "NONE", 0, -16)]
    #[case(true, "LOCK", 1, 29)]
    #[case(true, "WARN", 1, 29)]
    #[case(false, "LOCK", 1, 29)]
    #[case(false, "WARN", 1, 29)]
    fn in_punches_outside_of_the_grace_times_return_a_schedule_warning_failure(
        #[case] lockout: bool,
        #[case] job_lock_level: &str,
        #[case] job_id: i32,
        #[case] offset_minutes: i64,
    ) {
        let result = run(
            offset_minutes,
            job_id,
            switches(lockout, true, job_lock_level),
        );

        assert!(!result.is_success());
        assert!(result.overridable());
        assert!(result.has_message("res_scheduleGracePeriodWarning"));
        assert_eq!(
            result.reject_resource_key(),
            Some("res_scheduleWarningPeriod")
        );
    }

    #[rstest]
    // lockout | warn | punch offset
    #[case(false, true, 240)]
    #[case(false, true, -240)]
    #[case(true, false, 240)]
    #[case(true, false, -240)]
    fn in_punches_outside_the_allowed_window_return_a_not_scheduled_failure(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] offset_minutes: i64,
    ) {
        let result = run(offset_minutes, 0, switches(lockout, warn, "NONE"));

        assert!(!result.is_success());
        assert!(result.overridable());
        assert!(result.has_message("res_notScheduledWarning"));
        assert_eq!(result.reject_resource_key(), Some("res_notScheduled"));
    }

    #[rstest]
    #[case(31)]
    #[case(-31)]
    #[case(239)]
    #[case(-239)]
    fn in_punches_between_the_lockout_and_allowed_times_lock_out(#[case] offset_minutes: i64) {
        let result = run(offset_minutes, 0, rule_params! { LOCKOUT => "true" });

        assert!(!result.is_success());
        assert!(!result.overridable());
        assert!(result.has_message("res_scheduleLockoutMessage"));
        assert_eq!(result.reject_resource_key(), Some("res_scheduleLockout"));
        assert!(result.is_manager_overridable());
    }

    #[rstest]
    // lockout | warn | jobLockLevel | jobID | punch offset
    #[case(false, true, "NONE", 0, 14)]
    #[case(false, true, "LOCK", 1, 14)]
    #[case(false, true, "WARN", 1, 14)]
    #[case(true, false, "NONE", 0, 14)]
    #[case(true, false, "LOCK", 1, 14)]
    #[case(true, false, "WARN", 1, 14)]
    fn in_punches_in_the_grace_period_return_a_successful_result(
        #[case] lockout: bool,
        #[case] warn: bool,
        #[case] job_lock_level: &str,
        #[case] job_id: i32,
        #[case] offset_minutes: i64,
    ) {
        let result = run(
            offset_minutes,
            job_id,
            switches(lockout, warn, job_lock_level),
        );

        assert!(result.is_success());
    }
}
