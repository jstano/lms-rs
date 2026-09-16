//! Port of `com.unifocus.watson.common.labor.rules.algorithm.punchrounding.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/punchrounding/`.
//!
//! Seven Java classes, six catalogue entries: `PunchRoundingRuleConfig` is the
//! abstract base the others extend and is not itself a [`RuleClass`].
//!
//! Only the runtime half comes across — parameter keys, defaults, and the
//! constraints inside `validateProperties`. The l2fprod `Property` construction
//! each class spends 80 lines on is configuration UI.
//!
//! Note the inheritance is not uniform, and it decides which parameters a rule
//! sees: `MinuteRounding`, `PropertyDataRounding` and `WorkedHoursRounding`
//! extend the base and so inherit the eight manual/clock gates, while
//! `BackFromBreakWithGrace`, `RoundInToSchedule` and `RoundOutToSchedule` build
//! their defaults from scratch and carry only the gates they need.

use crate::rule_params;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// Whether hand-keyed in punches are rounded. `PunchRoundingRuleConfig.MANUAL_IN`.
pub const MANUAL_IN: &str = "manualIn";
/// Whether hand-keyed out punches are rounded.
pub const MANUAL_OUT: &str = "manualOut";
/// Whether hand-keyed break punches are rounded.
pub const MANUAL_BREAK: &str = "manualBreak";
/// Whether hand-keyed back punches are rounded.
pub const MANUAL_BACK: &str = "manualBack";
/// Whether clock in punches are rounded.
pub const CLOCK_IN: &str = "clockIn";
/// Whether clock out punches are rounded.
pub const CLOCK_OUT: &str = "clockOut";
/// Whether clock break punches are rounded.
pub const CLOCK_BREAK: &str = "clockBreak";
/// Whether clock back punches are rounded.
pub const CLOCK_BACK: &str = "clockBack";

/// Minutes to round an in punch to. `MinuteRoundingRuleConfig.IN_PUNCH_ROUND_TO`.
pub const IN_PUNCH_ROUND_TO: &str = "inPunchRoundTo";
/// Minutes to round an out punch to.
pub const OUT_PUNCH_ROUND_TO: &str = "outPunchRoundTo";
/// Minutes to round a break punch to.
pub const BREAK_PUNCH_ROUND_TO: &str = "breakPunchRoundTo";
/// Minutes to round a back punch to.
pub const BACK_PUNCH_ROUND_TO: &str = "backPunchRoundTo";

/// Minutes to round to. `WorkedHoursRoundingRuleConfig.ROUND_TO`, and the same
/// key on both round-to-schedule configs.
pub const ROUND_TO: &str = "roundTo";

/// The expected break length in minutes.
/// `BackFromBreakWithGraceRuleConfig.BREAK_LENGTH`.
pub const BREAK_LENGTH: &str = "breakLength";
/// How far a break may differ from that and still be rounded.
pub const GRACE_PERIOD: &str = "gracePeriod";

/// Whether a punch may only match a schedule for the same job. `MATCH_JOB`.
pub const MATCH_JOB: &str = "matchJob";
/// How long before a scheduled start a punch may still match it.
pub const GRACE_PRE_START: &str = "gracePreSchedStart";
/// How long after a scheduled start a punch may still match it.
pub const GRACE_POST_START: &str = "gracePostSchedStart";
/// How long before a scheduled end a punch may still match it.
pub const GRACE_PRE_END: &str = "gracePreSchedEnd";
/// How long after a scheduled end a punch may still match it.
pub const GRACE_POST_END: &str = "gracePostSchedEnd";
/// Which way to round. `ROUNDING_OPTION`, holding a
/// [`RoundingOption`](crate::common::rounding::RoundingOption) code.
pub const ROUNDING_OPTION: &str = "roundingOption";

/// The eight manual/clock gates every base-derived rounding config carries.
/// `PunchRoundingRuleConfig.getDefaultValues()` — all rounding is on by default.
pub fn punch_rounding_default_values() -> RuleParams {
    rule_params! {
        MANUAL_IN => "true",
        MANUAL_OUT => "true",
        MANUAL_BREAK => "true",
        MANUAL_BACK => "true",
        CLOCK_IN => "true",
        CLOCK_OUT => "true",
        CLOCK_BREAK => "true",
        CLOCK_BACK => "true",
    }
}

/// Round each punch to the nearest N minutes. `MinuteRoundingRuleConfig`.
pub struct MinuteRoundingRuleConfig;

impl RuleConfig for MinuteRoundingRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::MinutePrr
    }

    fn default_values(&self) -> RuleParams {
        let mut defaults = punch_rounding_default_values();
        defaults.set(IN_PUNCH_ROUND_TO, "15");
        defaults.set(OUT_PUNCH_ROUND_TO, "15");
        defaults.set(BREAK_PUNCH_ROUND_TO, "15");
        defaults.set(BACK_PUNCH_ROUND_TO, "15");
        defaults
    }

    /// Each threshold must be in 1..=60. Java raises one message per offending
    /// field, naming it.
    fn validate(&self, params: &RuleParams) -> ValidationResults {
        let mut results = ValidationResults::new();
        for key in [
            IN_PUNCH_ROUND_TO,
            OUT_PUNCH_ROUND_TO,
            BREAK_PUNCH_ROUND_TO,
            BACK_PUNCH_ROUND_TO,
        ] {
            let round_to = params.try_int_at(key).unwrap_or_default();
            if round_to <= 0 || round_to > 60 {
                results.push(format!(
                    "{key} must be greater than 0 and less than or equal to 60"
                ));
            }
        }
        results
    }
}

/// Round using the site's configured threshold. `PropertyDataRoundingRuleConfig`.
///
/// Adds nothing to the base: its threshold comes from property data at run time,
/// not from a parameter.
pub struct PropertyDataRoundingRuleConfig;

impl RuleConfig for PropertyDataRoundingRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::PropertyDataPrr
    }

    fn default_values(&self) -> RuleParams {
        punch_rounding_default_values()
    }
}

/// Round the shift's total worked hours. `WorkedHoursRoundingRuleConfig`.
pub struct WorkedHoursRoundingRuleConfig;

impl RuleConfig for WorkedHoursRoundingRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::WorkedHoursPrr
    }

    fn default_values(&self) -> RuleParams {
        let mut defaults = punch_rounding_default_values();
        defaults.set(ROUND_TO, "15");
        defaults
    }
}

/// Round a back punch to a full break. `BackFromBreakWithGraceRuleConfig`.
///
/// Does **not** extend the base, so it carries none of the manual/clock gates —
/// and its algorithm never consults them.
pub struct BackFromBreakWithGraceRuleConfig;

impl RuleConfig for BackFromBreakWithGraceRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::BackGracePrr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            BREAK_LENGTH => "30",
            GRACE_PERIOD => "5",
        }
    }
}

/// Round an in punch to a matching scheduled start. `RoundInToScheduleRuleConfig`.
pub struct RoundInToScheduleRuleConfig;

impl RuleConfig for RoundInToScheduleRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::InToSchedPrr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            MANUAL_IN => "true",
            CLOCK_IN => "true",
            GRACE_PRE_START => "15",
            GRACE_POST_START => "15",
            MATCH_JOB => "true",
            ROUND_TO => "15",
            ROUNDING_OPTION => "NEAR",
        }
    }
}

/// Round an out punch to a matching scheduled end. `RoundOutToScheduleRuleConfig`.
pub struct RoundOutToScheduleRuleConfig;

impl RuleConfig for RoundOutToScheduleRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::OutToSchedPrr
    }

    fn default_values(&self) -> RuleParams {
        rule_params! {
            MANUAL_OUT => "true",
            CLOCK_OUT => "true",
            GRACE_PRE_END => "15",
            GRACE_POST_END => "15",
            MATCH_JOB => "true",
            ROUND_TO => "15",
            ROUNDING_OPTION => "NEAR",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::rounding::RoundingOption;
    use crate::rules::rule_type::RuleType;

    #[test]
    fn every_punch_rounding_config_belongs_to_the_family() {
        let configs: Vec<&dyn RuleConfig> = vec![
            &MinuteRoundingRuleConfig,
            &PropertyDataRoundingRuleConfig,
            &WorkedHoursRoundingRuleConfig,
            &BackFromBreakWithGraceRuleConfig,
            &RoundInToScheduleRuleConfig,
            &RoundOutToScheduleRuleConfig,
        ];

        assert_eq!(
            configs.len(),
            6,
            "the catalogue has six punch rounding rules"
        );
        for config in configs {
            assert_eq!(config.rule_class().rule_type(), RuleType::PunchRounding);
        }
    }

    #[test]
    fn the_configs_cover_every_catalogue_entry() {
        let covered = [
            MinuteRoundingRuleConfig.rule_class(),
            PropertyDataRoundingRuleConfig.rule_class(),
            WorkedHoursRoundingRuleConfig.rule_class(),
            BackFromBreakWithGraceRuleConfig.rule_class(),
            RoundInToScheduleRuleConfig.rule_class(),
            RoundOutToScheduleRuleConfig.rule_class(),
        ];

        for rule_class in RuleClass::of_type(RuleType::PunchRounding) {
            assert!(
                covered.contains(&rule_class),
                "{rule_class:?} has no config"
            );
        }
    }

    #[test]
    fn all_rounding_is_enabled_by_default() {
        let defaults = punch_rounding_default_values();
        for key in [
            MANUAL_IN,
            MANUAL_OUT,
            MANUAL_BREAK,
            MANUAL_BACK,
            CLOCK_IN,
            CLOCK_OUT,
            CLOCK_BREAK,
            CLOCK_BACK,
        ] {
            assert!(defaults.bool_at(key), "{key} should default to true");
        }
    }

    #[test]
    fn minute_rounding_defaults_to_fifteen_on_every_punch_type() {
        let defaults = MinuteRoundingRuleConfig.default_values();
        for key in [
            IN_PUNCH_ROUND_TO,
            OUT_PUNCH_ROUND_TO,
            BREAK_PUNCH_ROUND_TO,
            BACK_PUNCH_ROUND_TO,
        ] {
            assert_eq!(defaults.int_at(key), 15);
        }
    }

    #[test]
    fn minute_rounding_inherits_the_manual_and_clock_gates() {
        assert!(MinuteRoundingRuleConfig.default_values().bool_at(MANUAL_IN));
    }

    #[test]
    fn a_threshold_outside_one_to_sixty_is_rejected() {
        assert!(
            MinuteRoundingRuleConfig
                .validate(&MinuteRoundingRuleConfig.default_values())
                .is_empty()
        );

        let mut params = MinuteRoundingRuleConfig.default_values();
        params.set(IN_PUNCH_ROUND_TO, "0");
        assert_eq!(MinuteRoundingRuleConfig.validate(&params).len(), 1);

        params.set(OUT_PUNCH_ROUND_TO, "61");
        assert_eq!(MinuteRoundingRuleConfig.validate(&params).len(), 2);
    }

    #[test]
    fn sixty_is_allowed_and_sixty_one_is_not() {
        let mut params = MinuteRoundingRuleConfig.default_values();
        params.set(IN_PUNCH_ROUND_TO, "60");
        assert!(MinuteRoundingRuleConfig.validate(&params).is_empty());

        params.set(IN_PUNCH_ROUND_TO, "61");
        assert!(!MinuteRoundingRuleConfig.validate(&params).is_empty());
    }

    #[test]
    fn worked_hours_rounding_adds_a_single_threshold_to_the_base() {
        let defaults = WorkedHoursRoundingRuleConfig.default_values();
        assert_eq!(defaults.int_at(ROUND_TO), 15);
        assert!(defaults.bool_at(CLOCK_OUT));
    }

    #[test]
    fn property_data_rounding_adds_nothing_to_the_base() {
        assert_eq!(
            PropertyDataRoundingRuleConfig.default_values(),
            punch_rounding_default_values()
        );
    }

    #[test]
    fn back_from_break_does_not_inherit_the_gates() {
        // Its Java config builds a fresh map rather than calling super.
        let defaults = BackFromBreakWithGraceRuleConfig.default_values();

        assert_eq!(defaults.int_at(BREAK_LENGTH), 30);
        assert_eq!(defaults.int_at(GRACE_PERIOD), 5);
        assert!(!defaults.contains(MANUAL_IN), "no inherited gates");
        assert_eq!(defaults.len(), 2);
    }

    #[test]
    fn the_schedule_rules_carry_only_their_own_direction_of_gate() {
        let round_in = RoundInToScheduleRuleConfig.default_values();
        assert!(round_in.contains(MANUAL_IN));
        assert!(
            !round_in.contains(MANUAL_OUT),
            "an in rule needs no out gate"
        );

        let round_out = RoundOutToScheduleRuleConfig.default_values();
        assert!(round_out.contains(MANUAL_OUT));
        assert!(!round_out.contains(MANUAL_IN));
    }

    #[test]
    fn the_schedule_rules_default_to_rounding_to_the_nearest() {
        let defaults = RoundInToScheduleRuleConfig.default_values();
        assert_eq!(
            RoundingOption::from_code(defaults.get(ROUNDING_OPTION).unwrap()),
            Some(RoundingOption::Nearest)
        );
        assert_eq!(defaults.int_at(GRACE_PRE_START), 15);
        assert_eq!(defaults.int_at(GRACE_POST_START), 15);
        assert!(defaults.bool_at(MATCH_JOB));
    }
}
