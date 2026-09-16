//! Port of `com.unifocus.watson.common.labor.rules.algorithm.punchvalidation.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/punchvalidation/`.
//!
//! Five Java classes, four catalogue entries — `PunchValidationRuleConfig` is
//! the base. Three of the four extend it; `NoInPunchValidationRuleConfig`
//! extends `AbstractRuleConfig` directly and has no parameters at all, which
//! matches its algorithm doing nothing.
//!
//! The five message parameters all default to the empty string, and the base's
//! `getMessageForType` reads that as "use the built-in message" — see
//! [`message_for_type`](super::message_for_type).

use crate::rule_params;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;

/// Reject a punch outside the schedule. `PunchValidationRuleConfig.LOCKOUT_PROP`.
pub const LOCKOUT: &str = "lockout";
/// Minutes before a scheduled start that lockout still permits.
pub const LOCK_GRACE_PRE: &str = "lockGracePreSched";
/// Minutes after a scheduled start that lockout still permits.
pub const LOCK_GRACE_POST: &str = "lockGracePostSched";
/// Warn about a punch outside the schedule rather than reject it.
pub const WARN: &str = "warn";
/// Minutes before a scheduled start that pass without a warning.
pub const WARN_GRACE_PRE: &str = "warnGracePreSched";
/// Minutes after a scheduled start that pass without a warning.
pub const WARN_GRACE_POST: &str = "warnGracePostSched";
/// Override for the grace-period warning message.
pub const WARNING_MESSAGE: &str = "warningMessage";
/// Override for the schedule lockout message.
pub const LOCKOUT_MESSAGE: &str = "lockoutMessage";
/// Override for the job lockout message.
pub const JOB_LOCKOUT_MESSAGE: &str = "jobLockoutMessage";
/// Override for the overridable invalid-job message.
pub const INVALID_JOB_WARNING_MESSAGE: &str = "invalidJobWarningMessage";
/// Override for the not-scheduled message. Note the key drops the "d" —
/// `unscheduleMessage`, not `unscheduledMessage`.
pub const UNSCHEDULED_MESSAGE: &str = "unscheduleMessage";

/// How hard to push back when the punch is for a different job than the
/// schedule. Holds a
/// [`ScheduleLockoutLevel`](crate::common::enums::schedule_lockout_level::ScheduleLockoutLevel)
/// code.
pub const JOB_LOCK_LEVEL: &str = "jobLockLevel";
/// Minutes before a scheduled shift a punch may be offered.
pub const PRE_PUNCH: &str = "prePunch";
/// Minutes after a scheduled shift a punch may be offered.
pub const POST_PUNCH: &str = "postPunch";
/// Whether an unscheduled punch is permitted.
pub const UNSCHEDULED: &str = "unscheduled";

/// The eleven parameters every schedule-lockout validation config inherits.
/// `PunchValidationRuleConfig.getDefaultValues()`.
///
/// Both `lockout` and `warn` default to `false`, which is what makes an
/// unconfigured rule a no-op: the rules return success immediately when neither
/// is set.
pub fn punch_validation_default_values() -> RuleParams {
    rule_params! {
        LOCKOUT => "false",
        LOCK_GRACE_PRE => "30",
        LOCK_GRACE_POST => "30",
        WARN => "false",
        WARN_GRACE_PRE => "15",
        WARN_GRACE_POST => "15",
        WARNING_MESSAGE => "",
        LOCKOUT_MESSAGE => "",
        JOB_LOCKOUT_MESSAGE => "",
        INVALID_JOB_WARNING_MESSAGE => "",
        UNSCHEDULED_MESSAGE => "",
    }
}

/// Accept every in punch. `NoInPunchValidationRuleConfig`.
///
/// No parameters: it extends `AbstractRuleConfig`, not the validation base.
pub struct NoInPunchValidationRuleConfig;

impl RuleConfig for NoInPunchValidationRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoIpvr
    }

    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// Lock an in punch to its schedule. `SchedLockoutInPunchValidationRuleConfig`.
pub struct SchedLockoutInPunchValidationRuleConfig;

impl RuleConfig for SchedLockoutInPunchValidationRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::SchLockoutIpvr
    }

    fn default_values(&self) -> RuleParams {
        let mut defaults = punch_validation_default_values();
        defaults.set(JOB_LOCK_LEVEL, "NONE");
        defaults
    }
}

/// Lock an out punch to its schedule. `SchedLockoutOutPunchValidationRuleConfig`.
pub struct SchedLockoutOutPunchValidationRuleConfig;

impl RuleConfig for SchedLockoutOutPunchValidationRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::SchLockoutOpvr
    }

    fn default_values(&self) -> RuleParams {
        let mut defaults = punch_validation_default_values();
        defaults.set(UNSCHEDULED, "false");
        defaults.set(PRE_PUNCH, "240");
        defaults.set(POST_PUNCH, "240");
        defaults
    }
}

/// Lock an in punch to its schedule, but permit unscheduled punches.
/// `SchedLockAllowUnschedIPVRuleConfig`.
pub struct SchedLockAllowUnschedIpvRuleConfig;

impl RuleConfig for SchedLockAllowUnschedIpvRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::SchLockAllowUnIpvr
    }

    fn default_values(&self) -> RuleParams {
        let mut defaults = punch_validation_default_values();
        defaults.set(PRE_PUNCH, "240");
        defaults.set(POST_PUNCH, "240");
        defaults.set(JOB_LOCK_LEVEL, "NONE");
        defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule_type::RuleType;

    #[test]
    fn the_configs_cover_every_catalogue_entry() {
        let covered = [
            NoInPunchValidationRuleConfig.rule_class(),
            SchedLockoutInPunchValidationRuleConfig.rule_class(),
            SchedLockoutOutPunchValidationRuleConfig.rule_class(),
            SchedLockAllowUnschedIpvRuleConfig.rule_class(),
        ];

        let catalogue = RuleClass::of_type(RuleType::PunchValidation);
        assert_eq!(catalogue.len(), 4);
        for rule_class in catalogue {
            assert!(
                covered.contains(&rule_class),
                "{rule_class:?} has no config"
            );
        }
    }

    #[test]
    fn an_unconfigured_rule_is_a_no_op_because_both_switches_are_off() {
        let defaults = punch_validation_default_values();
        assert!(!defaults.bool_at(LOCKOUT));
        assert!(!defaults.bool_at(WARN));
    }

    #[test]
    fn the_lockout_grace_is_wider_than_the_warning_grace() {
        let defaults = punch_validation_default_values();
        assert_eq!(defaults.int_at(LOCK_GRACE_PRE), 30);
        assert_eq!(defaults.int_at(LOCK_GRACE_POST), 30);
        assert_eq!(defaults.int_at(WARN_GRACE_PRE), 15);
        assert_eq!(defaults.int_at(WARN_GRACE_POST), 15);
    }

    #[test]
    fn every_message_override_defaults_to_empty() {
        let defaults = punch_validation_default_values();
        for key in [
            WARNING_MESSAGE,
            LOCKOUT_MESSAGE,
            JOB_LOCKOUT_MESSAGE,
            INVALID_JOB_WARNING_MESSAGE,
            UNSCHEDULED_MESSAGE,
        ] {
            assert_eq!(defaults.get(key), Some(""));
        }
    }

    #[test]
    fn the_unscheduled_message_key_is_missing_its_d() {
        // "unscheduleMessage" in Java, not "unscheduledMessage". Pinned so a
        // well-meaning correction cannot silently break every configured
        // override at a live site.
        assert_eq!(UNSCHEDULED_MESSAGE, "unscheduleMessage");
    }

    #[test]
    fn no_in_validation_has_no_parameters_at_all() {
        assert!(NoInPunchValidationRuleConfig.default_values().is_empty());
    }

    #[test]
    fn the_three_schedule_rules_inherit_the_base_parameters() {
        for defaults in [
            SchedLockoutInPunchValidationRuleConfig.default_values(),
            SchedLockoutOutPunchValidationRuleConfig.default_values(),
            SchedLockAllowUnschedIpvRuleConfig.default_values(),
        ] {
            assert!(defaults.contains(LOCKOUT));
            assert!(defaults.contains(WARN_GRACE_PRE));
        }
    }

    #[test]
    fn each_schedule_rule_adds_only_what_it_uses() {
        let lockout_in = SchedLockoutInPunchValidationRuleConfig.default_values();
        assert_eq!(lockout_in.get(JOB_LOCK_LEVEL), Some("NONE"));
        assert!(
            !lockout_in.contains(PRE_PUNCH),
            "the in rule has no punch window"
        );

        let lockout_out = SchedLockoutOutPunchValidationRuleConfig.default_values();
        assert_eq!(lockout_out.int_at(PRE_PUNCH), 240);
        assert!(!lockout_out.bool_at(UNSCHEDULED));
        assert!(
            !lockout_out.contains(JOB_LOCK_LEVEL),
            "the out rule has no job lock"
        );

        let allow_unsched = SchedLockAllowUnschedIpvRuleConfig.default_values();
        assert_eq!(allow_unsched.int_at(POST_PUNCH), 240);
        assert_eq!(allow_unsched.get(JOB_LOCK_LEVEL), Some("NONE"));
    }
}
