//! Port of `com.unifocus.watson.common.labor.rules.algorithm.schedulechangevalidation.*Config`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulechangevalidation/`.
//!
//! A three-level hierarchy in Java: `ScheduleChangeValidationRuleBaseConfig` ←
//! `ShiftModificationPenaltyRuleConfig` ← `AddRemoveHoursPenaltyRuleBaseConfig`
//! ← `AddHoursPenaltyRuleConfig` / `RemoveHoursPenaltyRuleConfig`. Each level adds
//! parameters to the one above, so the Rust configs build on a shared function.
//!
//! `priority` is an ordinary integer parameter here (default 0), **not** a
//! `PriorityRuleConfig`: the runner's priority sort ignores it, so all three
//! rank at the default and keep rule-set order.
//!
//! Display names in validation messages are placeholders; conditions are exact.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

pub const FIRST_THRESHOLD: &str = "firstThreshold";
pub const SECOND_THRESHOLD: &str = "secondThreshold";
pub const USE_PP_START_DATE: &str = "usePlanningPeriodStartDate";
pub const MIN_SHIFT_LENGTH: &str = "minShiftLength";
pub const MAX_SHIFT_LENGTH: &str = "maxShiftLength";
pub const EARNING_TYPE: &str = "earningType";
pub const EARNING_AMOUNT: &str = "earningAmount";
pub const PRIORITY: &str = "priority";
pub const MIN_ADJUSTMENT_THRESHOLD: &str = "minAdjustmentThreshold";
pub const PERCENTAGE_OF_HOURS: &str = "percentageOfHours";

fn base_defaults() -> RuleParams {
    let mut p = RuleParams::new();
    p.set(FIRST_THRESHOLD, "10.0");
    p.set(SECOND_THRESHOLD, "1.0");
    p.set(PRIORITY, "0");
    p.set(USE_PP_START_DATE, "false");
    p.set(MIN_SHIFT_LENGTH, "4.0");
    p.set(MAX_SHIFT_LENGTH, "10.0");
    p.set(EARNING_TYPE, "0");
    p.set(EARNING_AMOUNT, "1");
    p
}

fn add_remove_defaults() -> RuleParams {
    let mut p = base_defaults();
    p.set(MIN_ADJUSTMENT_THRESHOLD, "0");
    p.set(PERCENTAGE_OF_HOURS, "false");
    p
}

/// `ScheduleChangeValidationRuleBaseConfig.validateProperties`.
fn validate_base(p: &RuleParams, r: &mut ValidationResults) {
    let name = "Schedule Change Validation";
    for (key, label) in [
        (FIRST_THRESHOLD, "First Threshold"),
        (SECOND_THRESHOLD, "Second Threshold"),
        (MIN_SHIFT_LENGTH, "Min Shift Length"),
        (MAX_SHIFT_LENGTH, "Max Shift Length"),
        (EARNING_AMOUNT, "Earning Amount"),
    ] {
        if p.double_at(key) < 0.0 {
            r.push(format!(
                "{name}: {label} must be greater than or equal to 0"
            ));
        }
    }
    if p.double_at(FIRST_THRESHOLD) <= p.double_at(SECOND_THRESHOLD) {
        r.push(format!(
            "{name}: First Threshold must be greater than Second Threshold"
        ));
    }
    if p.double_at(MAX_SHIFT_LENGTH) <= p.double_at(MIN_SHIFT_LENGTH) {
        r.push(format!(
            "{name}: Max Shift Length must be greater than Min Shift Length"
        ));
    }
    if p.int_at(EARNING_TYPE) <= 0 {
        r.push(format!("{name}: Earning Type must be selected"));
    }
}

/// `AddRemoveHoursPenaltyRuleBaseConfig.validateProperties`.
fn validate_add_remove(p: &RuleParams, r: &mut ValidationResults) {
    validate_base(p, r);
    if p.int_at(MIN_ADJUSTMENT_THRESHOLD) < 0 {
        r.push("Schedule Change Validation: Minimum Adjustment Threshold must be greater than or equal to 0".to_string());
    }
}

macro_rules! penalty_config {
    ($name:ident, $class:expr, $defaults:expr, $validate:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl RuleConfig for $name {
            fn rule_class(&self) -> RuleClass {
                $class
            }
            fn default_values(&self) -> RuleParams {
                $defaults()
            }
            fn validate(&self, p: &RuleParams) -> ValidationResults {
                let p = &p.fixed(&self.default_values());
                let mut r = ValidationResults::new();
                $validate(p, &mut r);
                r
            }
        }
    };
}

penalty_config!(
    ShiftModificationPenaltyRuleConfig,
    RuleClass::ShiftModificationSchedValidation,
    base_defaults,
    validate_base
);
penalty_config!(
    AddHoursPenaltyRuleConfig,
    RuleClass::AddHoursSchedValidation,
    add_remove_defaults,
    validate_add_remove
);
penalty_config!(
    RemoveHoursPenaltyRuleConfig,
    RuleClass::RemoveHoursSchedValidation,
    add_remove_defaults,
    validate_add_remove
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn defaults_stack_by_level() {
        assert_eq!(
            ShiftModificationPenaltyRuleConfig
                .default_values()
                .get(MIN_ADJUSTMENT_THRESHOLD),
            None
        );
        assert_eq!(
            AddHoursPenaltyRuleConfig
                .default_values()
                .get(MIN_ADJUSTMENT_THRESHOLD),
            Some("0")
        );
        assert_eq!(
            RemoveHoursPenaltyRuleConfig
                .default_values()
                .get(PERCENTAGE_OF_HOURS),
            Some("false")
        );
        assert_eq!(
            ShiftModificationPenaltyRuleConfig
                .default_values()
                .get(FIRST_THRESHOLD),
            Some("10.0")
        );
    }

    #[test]
    fn thresholds_lengths_and_the_earning_type_are_validated() {
        let ok = rule_params! { EARNING_TYPE => "5" };
        assert!(ShiftModificationPenaltyRuleConfig.validate(&ok).is_empty());
        // The default earning type 0 is "not selected".
        assert_eq!(
            ShiftModificationPenaltyRuleConfig
                .validate(&RuleParams::new())
                .len(),
            1
        );
        let bad = rule_params! {
            EARNING_TYPE => "5", FIRST_THRESHOLD => "1", SECOND_THRESHOLD => "1",
            MIN_SHIFT_LENGTH => "5", MAX_SHIFT_LENGTH => "4", MIN_ADJUSTMENT_THRESHOLD => "-1"
        };
        assert_eq!(AddHoursPenaltyRuleConfig.validate(&bad).len(), 3);
    }
}
