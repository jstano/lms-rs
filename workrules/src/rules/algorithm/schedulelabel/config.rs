//! Port of `com.unifocus.watson.common.labor.rules.algorithm.schedulelabel.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelabel/`.
//!
//! All five extend `LabelRuleConfig`, which — unlike `PriorityRuleConfig` —
//! defaults `priority` to **1**, rejects a priority below 1, and (for four of
//! the five) requires the label text to be non-blank and at most 10
//! characters. Display names in validation messages are placeholders for
//! localized strings; the conditions are exact.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{PRIORITY, RuleConfig, ValidationResults, priority_of};

/// `DefaultPropertyFactory.SCHEDULE_LABEL`.
pub const SCHEDULE_LABEL: &str = "scheduleLabel";
/// `DefaultPropertyFactory.MAX_LABELS`.
pub const MAX_LABELS: &str = "maxLabels";
/// `DefaultPropertyFactory.TIME_OFF_TYPES`.
pub const TIME_OFF_TYPES: &str = "timeOffTypes";

/// `LabelRuleConfig.getDefaultValues`.
fn label_defaults() -> RuleParams {
    let mut p = RuleParams::new();
    p.set(PRIORITY, "1");
    p
}

/// `LabelRuleConfig.validateProperties`.
fn validate_priority(p: &RuleParams, r: &mut ValidationResults) {
    if p.int_at(PRIORITY) < 1 {
        r.push("Priority must be greater than or equal to 1.".to_string());
    }
}

/// `LabelRuleConfig.validateLabelText`.
fn validate_label_text(p: &RuleParams, r: &mut ValidationResults) {
    let text = p.get(SCHEDULE_LABEL).unwrap_or("").trim();
    if text.is_empty() || text.len() > 10 {
        r.push(
            "Label text must be set to a non empty value less than or equal to 10 characters long"
                .to_string(),
        );
    }
}

fn validate_max_labels(p: &RuleParams, r: &mut ValidationResults) {
    if p.int_at(MAX_LABELS) <= 0 {
        r.push("Max labels must be set to a value greater than 0".to_string());
    }
}

macro_rules! label_config {
    ($name:ident, $class:expr, $defaults:expr, $validate:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl RuleConfig for $name {
            fn rule_class(&self) -> RuleClass {
                $class
            }
            fn default_values(&self) -> RuleParams {
                let mut p = label_defaults();
                #[allow(clippy::redundant_closure_call)]
                ($defaults)(&mut p);
                p
            }
            fn validate(&self, p: &RuleParams) -> ValidationResults {
                // Java builds its properties from default-filled values.
                let p = &p.fixed(&self.default_values());
                let mut r = ValidationResults::new();
                validate_priority(p, &mut r);
                #[allow(clippy::redundant_closure_call)]
                ($validate)(p, &mut r);
                r
            }
            fn priority(&self, params: &RuleParams) -> Option<i32> {
                Some(priority_of(self, params))
            }
        }
    };
}

label_config!(
    NoLabelRuleConfig,
    RuleClass::NoLabelLb,
    |_p: &mut RuleParams| {},
    |_p: &RuleParams, _r: &mut ValidationResults| {}
);

label_config!(
    ApprovedTimeOffLabelRuleConfig,
    RuleClass::ApprovedTimeOffLb,
    |p: &mut RuleParams| {
        p.set(SCHEDULE_LABEL, "");
        p.set(TIME_OFF_TYPES, "[]");
    },
    |p: &RuleParams, r: &mut ValidationResults| {
        validate_label_text(p, r);
        if p.get(TIME_OFF_TYPES).unwrap_or("[]") == "[]" {
            r.push("At least one time off type must be selected.".to_string());
        }
    }
);

label_config!(
    AvailableNotScheduledRuleConfig,
    RuleClass::AvailNotSchedLb,
    |p: &mut RuleParams| {
        p.set(MAX_LABELS, "1");
        p.set(SCHEDULE_LABEL, "");
    },
    |p: &RuleParams, r: &mut ValidationResults| {
        validate_max_labels(p, r);
        validate_label_text(p, r);
    }
);

label_config!(
    NotScheduledOrOffRuleConfig,
    RuleClass::NotSchedLb,
    |p: &mut RuleParams| {
        p.set(SCHEDULE_LABEL, "");
        p.set(TIME_OFF_TYPES, "[]");
    },
    |p: &RuleParams, r: &mut ValidationResults| validate_label_text(p, r)
);

label_config!(
    UnavailableAllDayRuleConfig,
    RuleClass::UnavailLb,
    |p: &mut RuleParams| {
        p.set(MAX_LABELS, "1");
        p.set(SCHEDULE_LABEL, "");
    },
    |p: &RuleParams, r: &mut ValidationResults| {
        validate_max_labels(p, r);
        validate_label_text(p, r);
    }
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn priority_defaults_to_one_not_the_priority_config_default() {
        assert_eq!(
            UnavailableAllDayRuleConfig.default_values().get(PRIORITY),
            Some("1")
        );
        assert_eq!(
            UnavailableAllDayRuleConfig.priority(&RuleParams::new()),
            Some(1)
        );
        assert_eq!(
            NoLabelRuleConfig.priority(&rule_params! { PRIORITY => "7" }),
            Some(7)
        );
    }

    #[test]
    fn label_text_must_be_one_to_ten_characters() {
        let ok = rule_params! { SCHEDULE_LABEL => "FLEX", MAX_LABELS => "1" };
        assert!(UnavailableAllDayRuleConfig.validate(&ok).is_empty());
        for bad in ["", "   ", "ELEVENCHARS"] {
            let p = rule_params! { SCHEDULE_LABEL => bad, MAX_LABELS => "1" };
            assert_eq!(UnavailableAllDayRuleConfig.validate(&p).len(), 1, "{bad:?}");
        }
    }

    #[test]
    fn max_labels_priority_and_time_off_types_are_validated() {
        let p = rule_params! { SCHEDULE_LABEL => "X", MAX_LABELS => "0", PRIORITY => "0" };
        assert_eq!(AvailableNotScheduledRuleConfig.validate(&p).len(), 2);
        let p = rule_params! { SCHEDULE_LABEL => "X", TIME_OFF_TYPES => "[]" };
        assert_eq!(ApprovedTimeOffLabelRuleConfig.validate(&p).len(), 1);
        let p = rule_params! { SCHEDULE_LABEL => "X", TIME_OFF_TYPES => "[1]" };
        assert!(ApprovedTimeOffLabelRuleConfig.validate(&p).is_empty());
        // NotScheduledOrOff does not require a time off type.
        let p = rule_params! { SCHEDULE_LABEL => "X" };
        assert!(NotScheduledOrOffRuleConfig.validate(&p).is_empty());
        assert!(NoLabelRuleConfig.validate(&RuleParams::new()).is_empty());
    }
}
