//! Port of `ShiftDifferenceOTRuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/hoursdistribution/ShiftDifferenceOTRuleConfig.java`.
//! (It lives in Java's `hoursdistribution` package but belongs to
//! `RuleType::ShiftDiffOt`.) Display names in messages are placeholders.

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};

pub const OT_FACTOR_PROP: &str = "otFactorRate";
pub const HOURS_BETWEEN_SHIFTS_PROP: &str = "hoursBetweenShifts";
pub const CONVERT_HOURS_TO_OT_PROP: &str = "convertHoursToOT";
pub const CONVERT_FULL_SECOND_SHIFT_PROP: &str = "convertFullSecondShift";
pub const REDUCE_SECOND_SHIFT_OT_PROP: &str = "reduceSecondShiftOTByFirstShiftOT";
pub const EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP: &str = "excludeShiftsOnSeparateDays";
pub const EXCLUDE_SHIFTS_ON_SAME_DAY_PROP: &str = "excludeShiftsOnSameDay";
pub use crate::rules::algorithm::dailyearning::config::EARNING;

/// `ShiftDifferenceOTRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShiftDifferenceOTRuleConfig;

impl RuleConfig for ShiftDifferenceOTRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::ShiftDifferenceOtSdo
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(OT_FACTOR_PROP, "1.5");
        p.set(EARNING, "0");
        p.set(HOURS_BETWEEN_SHIFTS_PROP, "10.0");
        p.set(CONVERT_HOURS_TO_OT_PROP, "false");
        p.set(CONVERT_FULL_SECOND_SHIFT_PROP, "false");
        p.set(REDUCE_SECOND_SHIFT_OT_PROP, "false");
        p.set(EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP, "false");
        p.set(EXCLUDE_SHIFTS_ON_SAME_DAY_PROP, "false");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let name = "Shift Difference OT";
        let mut r = ValidationResults::new();
        for (key, label) in [
            (OT_FACTOR_PROP, "OT Factor Rate"),
            (HOURS_BETWEEN_SHIFTS_PROP, "Hours Between Shifts"),
        ] {
            if p.double_at(key) < 0.01 {
                r.push(format!(
                    "{name}: {label} must be greater than or equal to 0.01"
                ));
            }
        }
        let (full, convert, reduce) = (
            p.bool_at(CONVERT_FULL_SECOND_SHIFT_PROP),
            p.bool_at(CONVERT_HOURS_TO_OT_PROP),
            p.bool_at(REDUCE_SECOND_SHIFT_OT_PROP),
        );
        if reduce && !convert {
            r.push(format!("{name}: Reduce Second Shift OT should only be selected if Convert Hours To OT is selected."));
        }
        if reduce && full {
            r.push(format!("{name}: 'Convert Full Second Shift' and 'Reduce Second Shift OT' should be mutually exclusive."));
        }
        if p.bool_at(EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP)
            && p.bool_at(EXCLUDE_SHIFTS_ON_SAME_DAY_PROP)
        {
            r.push(format!("{name}: 'Exclude Shifts On Separate Days' and 'Exclude Shifts On Same Day' should be mutually exclusive."));
        }
        r
    }
}
