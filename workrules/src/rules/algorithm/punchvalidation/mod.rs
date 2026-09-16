//! Punch validation. `RuleType::PunchValidation` — four rules.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchvalidation/`.
//!
//! Decide whether a time clock should accept a punch an employee has just
//! offered. Alone among the families ported so far, these **return** a value —
//! a [`TimeClockServerResult`] — rather than mutating an entity, because the
//! answer travels back to the clock.
//!
//! Every rejection is one of a small set of shapes, built by the helpers below:
//! whether the employee can override at the clock, whether a manager can, which
//! resource key names the reason, and which message to show.

pub mod config;
pub mod no_in;
pub mod sched_lock_allow_unsched;
pub mod sched_lockout_in;
pub mod sched_lockout_out;

use crate::common::dates::is_between_exclusive;
use crate::entity::employee::Employee;
use crate::entity::punch_log::PunchLog;
use crate::entity::time_clock_result::TimeClockServerResult;
use crate::rules::params::RuleParams;
use joda_rs::LocalDateTime;

/// A scheduled shift, reduced to what punch validation needs.
/// `ScheduleShiftForPunchValidation`.
///
/// Deliberately not [`EmployeeShift`](crate::entity::employee_shift::EmployeeShift):
/// Java declares a separate three-field class for this, because the clock asks
/// before any shift exists to match against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleShiftForPunchValidation {
    job_id: i32,
    start_date_time: LocalDateTime,
    end_date_time: LocalDateTime,
}

impl ScheduleShiftForPunchValidation {
    /// Build a scheduled shift.
    pub fn new(job_id: i32, start_date_time: LocalDateTime, end_date_time: LocalDateTime) -> Self {
        Self {
            job_id,
            start_date_time,
            end_date_time,
        }
    }

    /// `getJobID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> LocalDateTime {
        self.start_date_time
    }

    /// `getEndDateTime()`.
    pub fn end_date_time(&self) -> LocalDateTime {
        self.end_date_time
    }
}

/// A punch validation rule.
///
/// `PunchValidationRuleImpl.execute(Employee, PunchLogDTO, int, List, Map)`.
/// Every argument may be absent in Java — the Groovy tests call it with `null`
/// for the employee and the punch when checking that an unconfigured rule
/// short-circuits — so the two that can be are `Option` here.
pub trait PunchValidationRule {
    /// Should the clock accept this punch?
    fn execute(
        &self,
        employee: Option<&Employee>,
        punch_log: Option<&PunchLog>,
        job_id: i32,
        scheduled_shifts: &[ScheduleShiftForPunchValidation],
        params: &RuleParams,
    ) -> TimeClockServerResult;
}

/// The schedule starting nearest the punch.
/// `PunchValidationRuleImpl.findClosestScheduleToPunch`.
///
/// Measured against each schedule's **start**. The out-punch rule overrides
/// this to measure from the end instead — see
/// [`sched_lockout_out`] — so this is the in-punch form, not a shared one.
/// `None` only when there are no schedules at all.
pub fn find_closest_schedule_to_punch(
    scheduled_shifts: &[ScheduleShiftForPunchValidation],
    punch_date_time: LocalDateTime,
) -> Option<&ScheduleShiftForPunchValidation> {
    let distance = |schedule: &ScheduleShiftForPunchValidation| {
        (schedule.start_date_time().epoch_seconds() - punch_date_time.epoch_seconds()).abs()
    };

    let mut best: Option<&ScheduleShiftForPunchValidation> = None;
    for schedule in scheduled_shifts {
        // Strictly closer, so the first of two equidistant schedules wins.
        if best.is_none_or(|current| distance(schedule) < distance(current)) {
            best = Some(schedule);
        }
    }
    best
}

/// Is the punch inside either of two exclusive windows?
/// `PunchValidationRuleImpl.inBetweenExclusivePreAndPostTimes`.
pub fn in_between_exclusive_pre_and_post_times(
    punch: LocalDateTime,
    pre1: LocalDateTime,
    pre2: LocalDateTime,
    post1: LocalDateTime,
    post2: LocalDateTime,
) -> bool {
    is_between_exclusive(punch, pre1, pre2) || is_between_exclusive(punch, post1, post2)
}

/// Is the punch outside the window entirely?
/// `PunchValidationRuleImpl.outsidePreAndPostTimes`.
pub fn outside_pre_and_post_times(
    punch: LocalDateTime,
    pre: LocalDateTime,
    post: LocalDateTime,
) -> bool {
    punch.is_before(pre) || punch.is_after(post)
}

/// Is the employee not engaged in this job on the punch's date?
/// `PunchValidationRuleImpl.employeeHasInvalidJobCode`.
///
/// A missing employee counts as invalid: Java would throw, and there is no
/// reading under which an absent employee holds a valid job.
pub fn employee_has_invalid_job_code(
    employee: Option<&Employee>,
    punch_log: &PunchLog,
    job_id: i32,
) -> bool {
    employee.is_none_or(|employee| {
        employee
            .employee_job_status(job_id, punch_log.punch_date_time().to_local_date())
            .is_none()
    })
}

/// A configured message, or the built-in one.
///
/// `PunchValidationRuleImpl.getMessageForType`: an absent or blank parameter
/// means "use the default". The default is passed as its **resource key** —
/// see [`TimeClockServerResult`] for why.
pub fn message_for_type(params: &RuleParams, default_key: &str, key: &str) -> String {
    let configured = params.get(key).unwrap_or("");
    if configured.trim().is_empty() {
        default_key.to_string()
    } else {
        configured.to_string()
    }
}

/// The punch names a job the employee does not hold. `invalidJobCodeFailure`.
pub fn invalid_job_code_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        false,
        message_for_type(params, "res_jobLockoutMessage", config::JOB_LOCKOUT_MESSAGE),
        "res_invalidJobCode",
    )
}

/// The punch is for a different job than the schedule, and that is a warning.
/// `PunchValidationRuleImpl.jobOverridableLockoutFailure` — the **base** form.
///
/// `SchedLockoutInPunchValidationRuleImpl` overrides this and the two below.
/// The overrides live in that module; the difference is whether they set
/// `managerOverridable`, and for one of them, which resource key names the
/// reason. Using the wrong form gets every job-mismatch rejection wrong, which
/// is why they are spelled out separately rather than shared.
pub fn job_overridable_lockout_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        true,
        message_for_type(
            params,
            "res_jobLockoutOverridableMessage",
            config::INVALID_JOB_WARNING_MESSAGE,
        ),
        "res_jobRestriction",
    )
    .manager_overridable()
}

/// The punch is for a different job than the schedule, and that is a lockout.
/// `PunchValidationRuleImpl.jobLockoutFailure` — the **base** form.
pub fn job_lockout_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        false,
        message_for_type(params, "res_jobLockoutMessage", config::JOB_LOCKOUT_MESSAGE),
        "res_jobRestriction",
    )
    .manager_overridable()
}

/// A job mismatch inside the warning period.
/// `PunchValidationRuleImpl.jobLockoutWarnFailure` — the **base** form.
///
/// Note its reject key is `res_scheduleWarningPeriod`, not `res_jobRestriction`
/// as in the other two — the subclass override changes that.
pub fn job_lockout_warn_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        true,
        message_for_type(params, "res_jobLockoutMessage", config::JOB_LOCKOUT_MESSAGE),
        "res_scheduleWarningPeriod",
    )
    .with_message(message_for_type(
        params,
        "res_scheduleGracePeriodWarning",
        config::WARNING_MESSAGE,
    ))
}

/// The punch falls in the grace period around the schedule. `warningFailure`.
pub fn warning_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        true,
        message_for_type(
            params,
            "res_scheduleGracePeriodWarning",
            config::WARNING_MESSAGE,
        ),
        "res_scheduleWarningPeriod",
    )
}

/// The punch is outside the schedule and locked out. `lockoutFailure`.
pub fn lockout_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        false,
        message_for_type(
            params,
            "res_scheduleLockoutMessage",
            config::LOCKOUT_MESSAGE,
        ),
        "res_scheduleLockout",
    )
    .manager_overridable()
}

/// There is no schedule for this punch at all. `notScheduledFailure`.
pub fn not_scheduled_failure(params: &RuleParams) -> TimeClockServerResult {
    TimeClockServerResult::failure(
        true,
        message_for_type(
            params,
            "res_notScheduledWarning",
            config::UNSCHEDULED_MESSAGE,
        ),
        "res_notScheduled",
    )
}

/// As [`not_scheduled_failure`], but a manager may override.
/// `notScheduledManagerOverridableFailure`.
pub fn not_scheduled_manager_overridable_failure(params: &RuleParams) -> TimeClockServerResult {
    not_scheduled_failure(params).manager_overridable()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::uftc_punch_type::UFTCPunchType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::rule_params;
    use joda_rs::LocalDate;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn schedule(job_id: i32, start: LocalDateTime) -> ScheduleShiftForPunchValidation {
        ScheduleShiftForPunchValidation::new(job_id, start, start.plus_hours(8))
    }

    fn employee_in_job(job_id: i32) -> Employee {
        Employee::new(
            100,
            11,
            "Alex Kim",
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

    #[test]
    fn the_closest_schedule_is_measured_from_its_start() {
        let schedules = [schedule(1, at(12, 0)), schedule(2, at(21, 0))];

        let closest = find_closest_schedule_to_punch(&schedules, at(13, 0)).unwrap();

        assert_eq!(closest.job_id(), 1);
    }

    #[test]
    fn the_first_of_two_equidistant_schedules_wins() {
        // Java's test is strictly `<`, so a tie leaves the incumbent.
        let schedules = [schedule(1, at(11, 0)), schedule(2, at(13, 0))];

        assert_eq!(
            find_closest_schedule_to_punch(&schedules, at(12, 0))
                .unwrap()
                .job_id(),
            1
        );
    }

    #[test]
    fn a_schedule_before_the_punch_can_still_be_closest() {
        let schedules = [schedule(1, at(11, 30)), schedule(2, at(18, 0))];

        assert_eq!(
            find_closest_schedule_to_punch(&schedules, at(12, 0))
                .unwrap()
                .job_id(),
            1
        );
    }

    #[test]
    fn no_schedules_means_no_closest() {
        assert!(find_closest_schedule_to_punch(&[], at(12, 0)).is_none());
    }

    #[test]
    fn an_employee_without_the_job_has_an_invalid_job_code() {
        let punch = PunchLog::new(at(12, 0), UFTCPunchType::In);

        assert!(!employee_has_invalid_job_code(
            Some(&employee_in_job(1)),
            &punch,
            1
        ));
        assert!(employee_has_invalid_job_code(
            Some(&employee_in_job(1)),
            &punch,
            99
        ));
    }

    #[test]
    fn a_missing_employee_has_an_invalid_job_code() {
        let punch = PunchLog::new(at(12, 0), UFTCPunchType::In);
        assert!(employee_has_invalid_job_code(None, &punch, 1));
    }

    #[test]
    fn an_employee_outside_their_job_dates_has_an_invalid_job_code() {
        let punch = PunchLog::new(LocalDateTime::of(2011, 6, 1, 12, 0, 0), UFTCPunchType::In);
        assert!(employee_has_invalid_job_code(
            Some(&employee_in_job(1)),
            &punch,
            1
        ));
    }

    #[test]
    fn a_blank_message_parameter_falls_back_to_the_resource_key() {
        let params = rule_params! { config::LOCKOUT_MESSAGE => "   " };
        assert_eq!(
            message_for_type(&params, "res_default", config::LOCKOUT_MESSAGE),
            "res_default"
        );
    }

    #[test]
    fn an_absent_message_parameter_falls_back_too() {
        assert_eq!(
            message_for_type(&RuleParams::new(), "res_default", config::LOCKOUT_MESSAGE),
            "res_default"
        );
    }

    #[test]
    fn a_configured_message_is_used_verbatim() {
        let params = rule_params! { config::LOCKOUT_MESSAGE => "Go away" };
        assert_eq!(
            message_for_type(&params, "res_default", config::LOCKOUT_MESSAGE),
            "Go away"
        );
    }

    #[test]
    fn the_failure_shapes_differ_in_who_may_override() {
        let params = RuleParams::new();

        let invalid_job = invalid_job_code_failure(&params);
        assert!(!invalid_job.overridable());
        assert!(!invalid_job.is_manager_overridable());
        assert_eq!(
            invalid_job.reject_resource_key(),
            Some("res_invalidJobCode")
        );

        let lockout = lockout_failure(&params);
        assert!(!lockout.overridable());
        assert!(lockout.is_manager_overridable());
        assert_eq!(lockout.reject_resource_key(), Some("res_scheduleLockout"));

        let warning = warning_failure(&params);
        assert!(warning.overridable());
        assert!(!warning.is_manager_overridable());

        let not_scheduled = not_scheduled_manager_overridable_failure(&params);
        assert!(not_scheduled.overridable());
        assert!(not_scheduled.is_manager_overridable());
    }

    #[test]
    fn the_base_job_lockout_warning_carries_two_messages_and_a_schedule_key() {
        let result = job_lockout_warn_failure(&RuleParams::new());

        assert_eq!(result.messages().len(), 2);
        assert!(result.has_message("res_jobLockoutMessage"));
        assert!(result.has_message("res_scheduleGracePeriodWarning"));
        assert!(result.overridable());
        assert_eq!(
            result.reject_resource_key(),
            Some("res_scheduleWarningPeriod"),
            "the base uses the schedule key; only the subclass override says jobRestriction"
        );
    }

    #[test]
    fn the_base_job_failures_are_manager_overridable() {
        assert!(job_lockout_failure(&RuleParams::new()).is_manager_overridable());
        assert!(job_overridable_lockout_failure(&RuleParams::new()).is_manager_overridable());
    }

    #[test]
    fn the_windows_are_exclusive_at_both_ends() {
        let start = at(12, 0);
        let end = at(13, 0);

        assert!(!is_between_exclusive(start, start, end));
        assert!(in_between_exclusive_pre_and_post_times(
            at(12, 30),
            start,
            end,
            at(20, 0),
            at(21, 0)
        ));
        assert!(in_between_exclusive_pre_and_post_times(
            at(20, 30),
            start,
            end,
            at(20, 0),
            at(21, 0)
        ));
        assert!(!in_between_exclusive_pre_and_post_times(
            at(15, 0),
            start,
            end,
            at(20, 0),
            at(21, 0)
        ));
    }

    #[test]
    fn outside_the_window_excludes_its_bounds() {
        assert!(outside_pre_and_post_times(at(11, 59), at(12, 0), at(13, 0)));
        assert!(outside_pre_and_post_times(at(13, 1), at(12, 0), at(13, 0)));
        assert!(!outside_pre_and_post_times(at(12, 0), at(12, 0), at(13, 0)));
        assert!(!outside_pre_and_post_times(at(13, 0), at(12, 0), at(13, 0)));
    }
}
