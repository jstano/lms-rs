//! Port of `SchedLockoutInPunchValidationRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockoutInPunchValidationRuleImpl.java`.
//!
//! Hold an in punch to its schedule. Two independent switches, `lockout` and
//! `warn`, each with their own grace window around the scheduled start; a punch
//! inside the window passes, one outside is rejected or warned about.
//!
//! On top of that sits a job check: if the punch names a different job than the
//! schedule it matched, `jobLockLevel` decides whether that is fatal, a
//! warning, or ignored.
//!
//! # The subclass overrides three of the base's failure shapes
//!
//! `jobLockoutFailure`, `jobOverridableLockoutFailure` and
//! `jobLockoutWarnFailure` are all overridden here, and the overrides differ
//! from the base in whether they set `managerOverridable`. The base sets it on
//! two of them; this subclass sets it on none. Reading only the base would get
//! every job-mismatch rejection wrong.

use crate::common::enums::schedule_lockout_level::ScheduleLockoutLevel;
use crate::common::enums::uftc_punch_type::UFTCPunchType;
use crate::entity::employee::Employee;
use crate::entity::punch_log::PunchLog;
use crate::entity::time_clock_result::TimeClockServerResult;
use crate::entity::time_clock_result::TimeClockServerResult as Result_;
use crate::rules::algorithm::punchvalidation::config::{
    INVALID_JOB_WARNING_MESSAGE, JOB_LOCK_LEVEL, JOB_LOCKOUT_MESSAGE, LOCK_GRACE_POST,
    LOCK_GRACE_PRE, LOCKOUT, SchedLockoutInPunchValidationRuleConfig, WARN, WARN_GRACE_POST,
    WARN_GRACE_PRE, WARNING_MESSAGE,
};
use crate::rules::algorithm::punchvalidation::{
    PunchValidationRule, ScheduleShiftForPunchValidation, employee_has_invalid_job_code,
    find_closest_schedule_to_punch, in_between_exclusive_pre_and_post_times,
    invalid_job_code_failure, lockout_failure, message_for_type, outside_pre_and_post_times,
    warning_failure,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// `jobLockoutFailure`, as this subclass overrides it — **not** manager
/// overridable, unlike the base.
fn job_lockout_failure(params: &RuleParams) -> TimeClockServerResult {
    Result_::failure(
        false,
        message_for_type(params, "res_jobLockoutMessage", JOB_LOCKOUT_MESSAGE),
        "res_jobRestriction",
    )
}

/// `jobOverridableLockoutFailure`, as this subclass overrides it — **not**
/// manager overridable, unlike the base.
fn job_overridable_lockout_failure(params: &RuleParams) -> TimeClockServerResult {
    Result_::failure(
        true,
        message_for_type(
            params,
            "res_jobLockoutOverridableMessage",
            INVALID_JOB_WARNING_MESSAGE,
        ),
        "res_jobRestriction",
    )
}

/// `jobLockoutWarnFailure`, as this subclass overrides it. Two differences from
/// the base: not manager overridable, and the reject key is `res_jobRestriction`
/// rather than `res_scheduleWarningPeriod`.
fn job_lockout_warn_failure(params: &RuleParams) -> TimeClockServerResult {
    Result_::failure(
        true,
        message_for_type(params, "res_jobLockoutMessage", JOB_LOCKOUT_MESSAGE),
        "res_jobRestriction",
    )
    .with_message(message_for_type(
        params,
        "res_scheduleGracePeriodWarning",
        WARNING_MESSAGE,
    ))
}

/// The four grace boundaries around a scheduled start.
/// `SchedLockoutInPunchValidationRuleImpl.setGracePeriods` — fields in Java,
/// since the bean is request-scoped; a value here.
struct GracePeriods {
    lock_pre: LocalDateTime,
    lock_post: LocalDateTime,
    warn_pre: LocalDateTime,
    warn_post: LocalDateTime,
}

impl GracePeriods {
    fn of(params: &RuleParams, schedule_start: LocalDateTime) -> Self {
        Self {
            lock_pre: schedule_start.minus_minutes(i64::from(params.int_at(LOCK_GRACE_PRE))),
            lock_post: schedule_start.plus_minutes(i64::from(params.int_at(LOCK_GRACE_POST))),
            warn_pre: schedule_start.minus_minutes(i64::from(params.int_at(WARN_GRACE_PRE))),
            warn_post: schedule_start.plus_minutes(i64::from(params.int_at(WARN_GRACE_POST))),
        }
    }
}

/// Hold an in punch to its schedule. `SchedLockoutInPunchValidationRuleImpl`.
pub struct SchedLockoutInPunchValidationRule;

impl PunchValidationRule for SchedLockoutInPunchValidationRule {
    fn execute(
        &self,
        employee: Option<&Employee>,
        punch_log: Option<&PunchLog>,
        job_id: i32,
        scheduled_shifts: &[ScheduleShiftForPunchValidation],
        params: &RuleParams,
    ) -> TimeClockServerResult {
        let params = params.fixed(&SchedLockoutInPunchValidationRuleConfig.default_values());

        let lockout = params.bool_at(LOCKOUT);
        let warn = params.bool_at(WARN);
        let job_lock_level = params
            .get(JOB_LOCK_LEVEL)
            .and_then(ScheduleLockoutLevel::from_code)
            .unwrap_or(ScheduleLockoutLevel::None);

        // With neither switch set the rule does nothing, and Java never looks
        // at the punch — which is why the tests can pass null for it.
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
            // No schedule at all: lockout rejects, warn merely warns.
            return if lockout {
                lockout_failure(&params)
            } else {
                warning_failure(&params)
            };
        };

        let schedule_job_id = best_schedule.job_id();
        let grace = GracePeriods::of(&params, best_schedule.start_date_time());

        if within_grace_period(lockout, warn, in_punch, &grace) {
            grace_period_result(&params, job_id, job_lock_level, schedule_job_id)
        } else {
            failure_result(
                &params,
                job_id,
                lockout,
                warn,
                job_lock_level,
                in_punch,
                schedule_job_id,
                &grace,
            )
        }
    }
}

/// `withinGracePeriod`.
///
/// Note the asymmetry: the warn window is checked whenever `warn` is set, but
/// the lockout window only when `warn` is **not** set. With both switches on,
/// the warn window alone decides whether a punch is clean.
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

/// `getGracePeriodResult` — the punch is on time, so only the job can fail it.
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

/// `getFailureResult`.
#[allow(clippy::too_many_arguments)]
fn failure_result(
    params: &RuleParams,
    job_id: i32,
    lockout: bool,
    warn: bool,
    job_lock_level: ScheduleLockoutLevel,
    in_punch: LocalDateTime,
    schedule_job_id: i32,
    grace: &GracePeriods,
) -> TimeClockServerResult {
    if in_warning_period(lockout, warn, in_punch, grace) {
        warning_period_failure(params, job_id, job_lock_level, schedule_job_id)
    } else {
        lockout_failure(params)
    }
}

/// `inWarningPeriod`.
///
/// With both switches on, the warning period is the pair of bands *between* the
/// warn window and the wider lockout window. With only `warn` on, it is
/// everything outside the warn window.
fn in_warning_period(
    lockout: bool,
    warn: bool,
    in_punch: LocalDateTime,
    grace: &GracePeriods,
) -> bool {
    if warn && lockout {
        return in_between_exclusive_pre_and_post_times(
            in_punch,
            grace.lock_pre,
            grace.warn_pre,
            grace.warn_post,
            grace.lock_post,
        );
    }

    warn && !lockout && outside_pre_and_post_times(in_punch, grace.warn_pre, grace.warn_post)
}

/// `getWarningPeriodFailure`.
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

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn employee() -> Employee {
        Employee::new(
            100,
            11,
            "Alex Kim",
            vec![job_status(1, 1), job_status(2, 2), job_status(3, 99)],
        )
    }

    fn job_status(id: i32, job_id: i32) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            id,
            100,
            job_id,
            LocalDate::of(2010, 1, 1),
            LocalDate::of(2010, 12, 31),
            EmployeePayType::Hourly,
            12.50,
            job_id == 1,
        )
    }

    /// Noon on job 1, and 9pm on job 2 — the Groovy fixture's two schedules.
    fn schedules() -> Vec<ScheduleShiftForPunchValidation> {
        vec![
            ScheduleShiftForPunchValidation::new(1, at(12, 0), at(13, 0)),
            ScheduleShiftForPunchValidation::new(2, at(21, 0), at(22, 0)),
        ]
    }

    fn run(punch_at: LocalDateTime, job_id: i32, params: RuleParams) -> TimeClockServerResult {
        let punch = PunchLog::new(punch_at, UFTCPunchType::In);
        SchedLockoutInPunchValidationRule.execute(
            Some(&employee()),
            Some(&punch),
            job_id,
            &schedules(),
            &params,
        )
    }

    #[test]
    fn a_punch_on_time_and_on_the_right_job_passes() {
        assert!(run(at(12, 0), 1, rule_params! { LOCKOUT => "true" }).is_success());
    }

    #[test]
    fn a_punch_inside_the_lockout_grace_passes() {
        // Lockout grace is 30 minutes either side by default.
        assert!(run(at(11, 45), 1, rule_params! { LOCKOUT => "true" }).is_success());
        assert!(run(at(12, 25), 1, rule_params! { LOCKOUT => "true" }).is_success());
    }

    #[test]
    fn a_punch_outside_the_lockout_grace_is_locked_out() {
        let result = run(at(10, 0), 1, rule_params! { LOCKOUT => "true" });

        assert!(!result.is_success());
        assert_eq!(result.reject_resource_key(), Some("res_scheduleLockout"));
        assert!(result.is_manager_overridable());
        assert!(!result.overridable());
    }

    #[test]
    fn the_grace_window_is_exclusive_at_its_bounds() {
        // Exactly 30 minutes before the schedule is *not* inside the window.
        let result = run(at(11, 30), 1, rule_params! { LOCKOUT => "true" });
        assert!(!result.is_success());
    }

    #[test]
    fn with_warn_only_a_punch_outside_the_warn_grace_is_a_warning() {
        let result = run(at(10, 0), 1, rule_params! { WARN => "true" });

        assert!(!result.is_success());
        assert_eq!(
            result.reject_resource_key(),
            Some("res_scheduleWarningPeriod")
        );
        assert!(result.overridable());
    }

    #[test]
    fn with_both_switches_the_band_between_the_windows_warns() {
        // Warn grace is 15 minutes, lockout 30. A punch 20 minutes early falls
        // between them.
        let params = rule_params! { LOCKOUT => "true", WARN => "true" };
        let result = run(at(11, 40), 1, params);

        assert!(!result.is_success());
        assert_eq!(
            result.reject_resource_key(),
            Some("res_scheduleWarningPeriod")
        );
    }

    #[test]
    fn with_both_switches_outside_the_lockout_window_locks_out() {
        let params = rule_params! { LOCKOUT => "true", WARN => "true" };
        let result = run(at(10, 0), 1, params);

        assert_eq!(result.reject_resource_key(), Some("res_scheduleLockout"));
    }

    #[test]
    fn with_both_switches_the_warn_window_alone_decides_a_clean_punch() {
        // The asymmetry in withinGracePeriod: the lockout window is only
        // consulted when warn is off. Inside warn grace, the punch is clean.
        let params = rule_params! { LOCKOUT => "true", WARN => "true" };
        assert!(run(at(12, 10), 1, params).is_success());
    }

    #[test]
    fn a_job_mismatch_is_ignored_at_lock_level_none() {
        // The punch matches the noon schedule on job 1, but claims job 99.
        assert!(run(at(12, 0), 99, rule_params! { LOCKOUT => "true" }).is_success());
    }

    #[test]
    fn a_job_mismatch_locks_out_at_lock_level_lock() {
        let params = rule_params! { LOCKOUT => "true", JOB_LOCK_LEVEL => "LOCK" };
        let result = run(at(12, 0), 99, params);

        assert!(!result.is_success());
        assert_eq!(result.reject_resource_key(), Some("res_jobRestriction"));
        assert!(!result.overridable());
        assert!(
            !result.is_manager_overridable(),
            "the subclass override does not set it, unlike the base"
        );
    }

    #[test]
    fn a_job_mismatch_is_overridable_at_lock_level_warn() {
        let params = rule_params! { LOCKOUT => "true", JOB_LOCK_LEVEL => "WARN" };
        let result = run(at(12, 0), 99, params);

        assert_eq!(result.reject_resource_key(), Some("res_jobRestriction"));
        assert!(result.overridable());
        assert!(result.has_message("res_jobLockoutOverridableMessage"));
    }

    #[test]
    fn a_job_mismatch_in_the_warning_period_carries_both_messages() {
        let params = rule_params! { LOCKOUT => "true", WARN => "true", JOB_LOCK_LEVEL => "WARN" };
        let result = run(at(11, 40), 99, params);

        assert_eq!(result.reject_resource_key(), Some("res_jobRestriction"));
        assert_eq!(result.messages().len(), 2);
        assert!(result.has_message("res_jobLockoutMessage"));
        assert!(result.has_message("res_scheduleGracePeriodWarning"));
    }

    #[test]
    fn a_configured_message_replaces_the_default() {
        let params = rule_params! {
            LOCKOUT => "true",
            crate::rules::algorithm::punchvalidation::config::LOCKOUT_MESSAGE => "Too early",
        };
        let result = run(at(10, 0), 1, params);

        assert!(result.has_message("Too early"));
        assert!(!result.has_message("res_scheduleLockoutMessage"));
    }
}

/// Cases ported from the Java engine's own test suite.
///
/// Transcribed from
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/SchedLockoutInPunchValidationRuleImplTest.groovy`.
///
/// Two adjustments, neither of which changes what is being asserted. The Groovy
/// builds its schedules relative to `new LocalDateTime()` — today at noon — so
/// the times here are pinned to a fixed date instead; the rule only ever
/// compares a punch against a schedule, so the absolute date is immaterial. And
/// `messages.contains(ResourceMgr.lookup("res_x"))` becomes
/// `has_message("res_x")`, since default messages are carried as their resource
/// key rather than resolved.
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

    /// `employee = Mock(Employee) { getEmployeeJobStatus(_, today) >> new EmployeeJobStatus() }`
    /// — any job resolves, so the invalid-job branch is not taken.
    fn employee_with_any_job(job_id: i32) -> Employee {
        Employee::new(
            100,
            11,
            "Mock",
            vec![EmployeeJobStatus::new(
                1,
                100,
                job_id,
                LocalDate::of(2010, 1, 1),
                LocalDate::of(2010, 12, 31),
                EmployeePayType::Hourly,
                12.50,
                true,
            )],
        )
    }

    /// The spec's `scheduledShifts` field.
    fn scheduled_shifts() -> Vec<ScheduleShiftForPunchValidation> {
        vec![
            ScheduleShiftForPunchValidation::new(1, noon_today(), noon_today().plus_hours(1)),
            ScheduleShiftForPunchValidation::new(
                2,
                noon_today().plus_hours(9),
                noon_today().plus_hours(10),
            ),
        ]
    }

    #[test]
    fn not_selecting_lockout_or_warn_always_returns_a_successful_result() {
        // execute(null, null, 0, [], params) with the config defaults.
        let result =
            SchedLockoutInPunchValidationRule.execute(None, None, 0, &[], &RuleParams::new());

        assert!(result.is_success());
    }

    #[rstest]
    #[case(UFTCPunchType::Out)]
    #[case(UFTCPunchType::Break)]
    #[case(UFTCPunchType::Back)]
    #[case(UFTCPunchType::Tips)]
    #[case(UFTCPunchType::Gross)]
    #[case(UFTCPunchType::Pieces)]
    #[case(UFTCPunchType::ChargeTips)]
    #[case(UFTCPunchType::Memo1)]
    #[case(UFTCPunchType::Memo2)]
    #[case(UFTCPunchType::Memo3)]
    #[case(UFTCPunchType::Memo4)]
    #[case(UFTCPunchType::OnSite)]
    #[case(UFTCPunchType::OffSite)]
    #[case(UFTCPunchType::Meal)]
    fn always_returns_successful_for_non_in_punch_types(#[case] punch_type: UFTCPunchType) {
        // `where: punchType << UFTCPunchType.values().findAll { it != IN }`
        let punch = PunchLog::new(noon_today(), punch_type);

        let result = SchedLockoutInPunchValidationRule.execute(
            None,
            Some(&punch),
            0,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(result.is_success());
    }

    #[test]
    fn must_be_a_valid_active_employee_to_run_in_punch_validation() {
        // `getEmployeeJobStatus(jobID, today) >> null` for jobID 99.
        let punch = PunchLog::new(noon_today(), UFTCPunchType::In);
        let employee = employee_with_any_job(1);

        let result = SchedLockoutInPunchValidationRule.execute(
            Some(&employee),
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

    #[test]
    fn returns_a_lockout_failure_when_the_schedule_does_not_exist() {
        let punch = PunchLog::new(noon_today(), UFTCPunchType::In);
        let employee = employee_with_any_job(1);

        let result = SchedLockoutInPunchValidationRule.execute(
            Some(&employee),
            Some(&punch),
            1,
            &[],
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(!result.is_success());
        assert_eq!(result.reject_resource_key(), Some("res_scheduleLockout"));
        assert!(result.has_message("res_scheduleLockoutMessage"));
    }

    #[test]
    fn returns_a_warning_when_the_schedule_does_not_exist_and_only_warn_is_set() {
        let punch = PunchLog::new(noon_today(), UFTCPunchType::In);
        let employee = employee_with_any_job(1);

        let result = SchedLockoutInPunchValidationRule.execute(
            Some(&employee),
            Some(&punch),
            1,
            &[],
            &rule_params! { WARN => "true" },
        );

        assert!(!result.is_success());
        assert!(result.overridable());
        assert_eq!(
            result.reject_resource_key(),
            Some("res_scheduleWarningPeriod")
        );
        assert!(result.has_message("res_scheduleGracePeriodWarning"));
    }

    #[test]
    fn the_closest_schedule_is_the_one_validated_against() {
        // The 9pm schedule on job 2 is nearer a 9pm punch than the noon one.
        let punch = PunchLog::new(noon_today().plus_hours(9), UFTCPunchType::In);
        let employee = employee_with_any_job(2);

        let result = SchedLockoutInPunchValidationRule.execute(
            Some(&employee),
            Some(&punch),
            2,
            &scheduled_shifts(),
            &rule_params! { LOCKOUT => "true" },
        );

        assert!(result.is_success());
    }
}
