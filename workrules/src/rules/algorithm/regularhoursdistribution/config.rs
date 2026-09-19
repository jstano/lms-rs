//! Port of
//! `com.unifocus.watson.common.labor.rules.algorithm.regularhoursdistribution.*RuleConfig`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/regularhoursdistribution/`.
//!
//! All three extend
//! [`SingleDistributionTypeRuleConfig`](crate::rules::algorithm::single_distribution_type_config),
//! and the two day-split rules add one more parameter, the boundary they split
//! shifts on.

use crate::rules::algorithm::single_distribution_type_config::{
    single_distribution_type_default_values, single_distribution_type_validate,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

/// The time of day a shift is split at. `DAY_START_AND_END_TIME`, shared by
/// `RegularHoursByDayRuleConfig` and `RegularHoursByWorkWeekRuleConfig` —
/// declared identically in both rather than pulled onto a common base, which
/// this crate mirrors as one constant rather than two.
pub const DAY_START_AND_END_TIME: &str = "dayStartAndEndTime";

/// `RegularHoursOnShiftDateRuleConfig` — no parameters beyond the shared one.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegularHoursOnShiftDateRuleConfig;

impl RuleConfig for RegularHoursOnShiftDateRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::RegHoursRhd
    }

    fn default_values(&self) -> RuleParams {
        single_distribution_type_default_values()
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        single_distribution_type_validate(params)
    }
}

/// `RegularHoursByDayRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegularHoursByDayRuleConfig;

impl RuleConfig for RegularHoursByDayRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::RegHoursByDayRhd
    }

    fn default_values(&self) -> RuleParams {
        let mut params = single_distribution_type_default_values();
        params.set(DAY_START_AND_END_TIME, "00:00:00");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        single_distribution_type_validate(params)
    }
}

/// `RegularHoursByWorkWeekRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegularHoursByWorkWeekRuleConfig;

impl RuleConfig for RegularHoursByWorkWeekRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::RegHoursByWorkWeekRhd
    }

    fn default_values(&self) -> RuleParams {
        let mut params = single_distribution_type_default_values();
        params.set(DAY_START_AND_END_TIME, "00:00:00");
        params
    }

    fn validate(&self, params: &RuleParams) -> ValidationResults {
        single_distribution_type_validate(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::algorithm::single_distribution_type_config::HOURS_DISTRIBUTION_TYPE_ID;

    #[test]
    fn on_shift_date_defaults_to_the_regular_bucket_and_nothing_else() {
        let defaults = RegularHoursOnShiftDateRuleConfig.default_values();

        assert_eq!(defaults.int_at(HOURS_DISTRIBUTION_TYPE_ID), 1);
    }

    #[test]
    fn by_day_defaults_to_midnight() {
        let defaults = RegularHoursByDayRuleConfig.default_values();

        assert_eq!(defaults.int_at(HOURS_DISTRIBUTION_TYPE_ID), 1);
        assert_eq!(defaults.get(DAY_START_AND_END_TIME), Some("00:00:00"));
    }

    #[test]
    fn by_work_week_defaults_to_midnight() {
        let defaults = RegularHoursByWorkWeekRuleConfig.default_values();

        assert_eq!(defaults.int_at(HOURS_DISTRIBUTION_TYPE_ID), 1);
        assert_eq!(defaults.get(DAY_START_AND_END_TIME), Some("00:00:00"));
    }

    #[test]
    fn each_config_knows_its_rule_class() {
        assert_eq!(
            RegularHoursOnShiftDateRuleConfig.rule_class(),
            RuleClass::RegHoursRhd
        );
        assert_eq!(
            RegularHoursByDayRuleConfig.rule_class(),
            RuleClass::RegHoursByDayRhd
        );
        assert_eq!(
            RegularHoursByWorkWeekRuleConfig.rule_class(),
            RuleClass::RegHoursByWorkWeekRhd
        );
    }
}
