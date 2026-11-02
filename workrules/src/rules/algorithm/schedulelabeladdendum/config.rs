//! Port of `com.unifocus.watson.common.labor.rules.algorithm.schedulelabeladdendum.*RuleConfig`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulelabeladdendum/`.
//! Display names in validation messages are placeholders; the conditions are exact.

use crate::rules::algorithm::schedulelabel::config::SCHEDULE_LABEL;
use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::{RuleConfig, ValidationResults};
use joda_rs::LocalDate;

/// `AccrualBalanceLabelAddendumRuleConfig.ACCRUAL_PROP`.
pub const ACCRUAL_PROP: &str = "accrual";
/// `BetweenDatesScheduleLabelCountRuleConfig.SCHEDULE_LABEL_PROP`.
pub const SCHEDULE_LABEL_PROP: &str = "scheduleLabel";
/// `BetweenDatesScheduleLabelCountRuleConfig.REFERENCE_DATE_START_PROP`.
pub const REFERENCE_DATE_START_PROP: &str = "startDate";
/// `BetweenDatesScheduleLabelCountRuleConfig.REFERENCE_DATE_END_PROP`.
pub const REFERENCE_DATE_END_PROP: &str = "endDate";

const LABEL_MESSAGE: &str = "Schedule label must not be empty and less than 10 characters";

fn validate_label_text(text: &str, r: &mut ValidationResults) {
    let text = text.trim();
    if text.is_empty() || text.len() > 10 {
        r.push(LABEL_MESSAGE.to_string());
    }
}

/// Parse `yyyy-MM-dd`; `None` for anything else.
pub(crate) fn try_parse_date(text: &str) -> Option<LocalDate> {
    let mut parts = text.trim().split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let (y, m, d) = (
        y.parse::<i32>().ok()?,
        m.parse::<i32>().ok()?,
        d.parse::<i32>().ok()?,
    );
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| LocalDate::of(y, m, d))
}

/// `NoOpScheduleLabelAddendumRuleConfig`: no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpScheduleLabelAddendumRuleConfig;

impl RuleConfig for NoOpScheduleLabelAddendumRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::NoOpSchAddnm
    }
    fn default_values(&self) -> RuleParams {
        RuleParams::new()
    }
}

/// `AccrualBalanceLabelAddendumRuleConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccrualBalanceLabelAddendumRuleConfig;

impl RuleConfig for AccrualBalanceLabelAddendumRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::AccrualBalanceSchAddnm
    }
    fn default_values(&self) -> RuleParams {
        let mut p = RuleParams::new();
        p.set(SCHEDULE_LABEL, "");
        p.set(ACCRUAL_PROP, "0");
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let mut r = ValidationResults::new();
        validate_label_text(p.get(SCHEDULE_LABEL).unwrap_or(""), &mut r);
        if p.int_at(ACCRUAL_PROP) <= 0 {
            r.push("Accrual must be selected".to_string());
        }
        r
    }
}

/// `BetweenDatesScheduleLabelCountRuleConfig`.
///
/// Both reference dates default to **today**, as in Java.
#[derive(Debug, Clone, Copy, Default)]
pub struct BetweenDatesScheduleLabelCountRuleConfig;

impl RuleConfig for BetweenDatesScheduleLabelCountRuleConfig {
    fn rule_class(&self) -> RuleClass {
        RuleClass::BtwnDtsLblCntSchAddnm
    }
    fn default_values(&self) -> RuleParams {
        let today = LocalDate::now().to_string();
        let mut p = RuleParams::new();
        p.set(SCHEDULE_LABEL_PROP, "");
        p.set(REFERENCE_DATE_START_PROP, today.clone());
        p.set(REFERENCE_DATE_END_PROP, today);
        p
    }
    fn validate(&self, p: &RuleParams) -> ValidationResults {
        let p = &p.fixed(&self.default_values());
        let mut r = ValidationResults::new();
        validate_label_text(p.get(SCHEDULE_LABEL_PROP).unwrap_or(""), &mut r);

        let start = p.get(REFERENCE_DATE_START_PROP).and_then(try_parse_date);
        let end = p.get(REFERENCE_DATE_END_PROP).and_then(try_parse_date);
        let (Some(start), Some(end)) = (start, end) else {
            r.push("Both reference dates are required".to_string());
            return r;
        };
        if start >= end {
            r.push("The start date must come before the end date".to_string());
        }
        if end > start.plus_years(1) {
            r.push("The end date must not be over a year after the start date".to_string());
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn label_text_and_accrual_are_validated() {
        let ok = rule_params! { SCHEDULE_LABEL => "VAC", ACCRUAL_PROP => "3" };
        assert!(
            AccrualBalanceLabelAddendumRuleConfig
                .validate(&ok)
                .is_empty()
        );
        let bad = rule_params! { SCHEDULE_LABEL => "", ACCRUAL_PROP => "0" };
        assert_eq!(
            AccrualBalanceLabelAddendumRuleConfig.validate(&bad).len(),
            2
        );
    }

    #[test]
    fn reference_dates_must_be_ordered_and_within_a_year() {
        let v = |s: &str, e: &str| {
            BetweenDatesScheduleLabelCountRuleConfig.validate(&rule_params! {
                SCHEDULE_LABEL_PROP => "X", REFERENCE_DATE_START_PROP => s, REFERENCE_DATE_END_PROP => e
            })
        };
        assert!(v("2017-04-15", "2018-04-15").is_empty());
        assert_eq!(v("2017-04-15", "2017-04-15").len(), 1);
        assert_eq!(v("2017-04-15", "2018-04-16").len(), 1);
        assert_eq!(v("2017-04-15", "").len(), 1);
    }

    #[test]
    fn the_reference_dates_default_to_today() {
        let p = BetweenDatesScheduleLabelCountRuleConfig.default_values();
        assert_eq!(
            p.get(REFERENCE_DATE_START_PROP),
            p.get(REFERENCE_DATE_END_PROP)
        );
        assert!(try_parse_date(p.get(REFERENCE_DATE_START_PROP).unwrap()).is_some());
    }
}
