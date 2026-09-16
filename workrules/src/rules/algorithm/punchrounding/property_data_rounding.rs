//! Port of `PropertyDataRoundingRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/punchrounding/PropertyDataRoundingRuleImpl.java`.
//!
//! Round each punch to a threshold that comes from the **site's settings**
//! rather than the rule's own parameters. That makes it the fallback the runner
//! reaches for when no punch-rounding rule set is configured at all.
//!
//! The setting is one string that is read two ways: a single number applied to
//! every punch type, or four comma-separated numbers for in, out, break and
//! back in that order. Anything else — absent, unparseable, or a comma list
//! that is not exactly four long — leaves the built-in default of 15 minutes in
//! place, silently.

use crate::common::dates::round_date_time_to_threshold_minutes;
use crate::common::enums::punch_type::PunchType;
use crate::common::rounding::RoundingOption;
use crate::entity::employee_shift::PunchCursor;
use crate::rules::algorithm::punchrounding::config::PropertyDataRoundingRuleConfig;
use crate::rules::algorithm::punchrounding::{PunchRoundingRule, can_round};
use crate::rules::params::RuleParams;
use crate::rules::ports::{PropertyDataKey, PropertyDataPort};
use crate::rules::rule_config::RuleConfig;

/// The threshold used for every punch type when the site has not set one.
const DEFAULT_THRESHOLD_MINUTES: i32 = 15;

/// How many values the comma form must carry to be honoured.
const COMMA_FORM_LENGTH: usize = 4;

/// Round each punch to the site's configured threshold.
/// `PropertyDataRoundingRuleImpl`.
///
/// Holds its port the way the Java class holds
/// `@Resource private PropertyDataDAO propertyDataDAO` — an injected field, not
/// a parameter, which is why the family's `execute` needs no extra argument.
pub struct PropertyDataRoundingRule<P: PropertyDataPort> {
    property_data: P,
}

impl<P: PropertyDataPort> PropertyDataRoundingRule<P> {
    /// Build the rule over a settings port.
    pub fn new(property_data: P) -> Self {
        Self { property_data }
    }

    /// The four thresholds — in, out, break, back — for a site.
    ///
    /// `PropertyDataRoundingRuleImpl.execute`'s parsing block, which starts
    /// from the field defaults of 15 and overwrites them only when the setting
    /// is present and well formed.
    fn thresholds(&self, property_id: i32) -> [i32; 4] {
        let mut thresholds = [DEFAULT_THRESHOLD_MINUTES; 4];

        let Some(setting) = self
            .property_data
            .value(property_id, PropertyDataKey::PunchRoundingThreshold)
        else {
            return thresholds;
        };

        if setting.contains(',') {
            let parts: Vec<&str> = setting.split(',').collect();
            // Java checks the length and silently ignores anything else.
            if parts.len() == COMMA_FORM_LENGTH {
                for (slot, part) in thresholds.iter_mut().zip(parts) {
                    *slot = part.trim().parse().unwrap_or(DEFAULT_THRESHOLD_MINUTES);
                }
            }
        } else if let Ok(threshold) = setting.trim().parse() {
            thresholds = [threshold; 4];
        }

        thresholds
    }
}

impl<P: PropertyDataPort> PunchRoundingRule for PropertyDataRoundingRule<P> {
    fn execute(&self, punch: &mut PunchCursor<'_>, params: &RuleParams) {
        let params = params.fixed(&PropertyDataRoundingRuleConfig.default_values());

        if !can_round(punch, &params) {
            return;
        }

        // Java walks punch.getEmployeeShift().getJob().getProperty(); the port
        // needs only the id, which the shift already carries.
        let property_id = punch.shift().property_id();
        let thresholds = self.thresholds(property_id);

        let threshold = match punch.punch_type() {
            PunchType::In => thresholds[0],
            PunchType::Out => thresholds[1],
            PunchType::Break => thresholds[2],
            PunchType::Back => thresholds[3],
            other => panic!("the punch type is not valid: {other:?}"),
        };

        let rounded = punch.adj_time().map(|adj_time| {
            round_date_time_to_threshold_minutes(adj_time, threshold, RoundingOption::Nearest)
        });

        punch.set_rounded_time(rounded);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::rule_params;
    use crate::rules::algorithm::punchrounding::config::MANUAL_IN;
    use joda_rs::{LocalDate, LocalDateTime};
    use rstest::rstest;

    /// A settings port holding one value, or nothing.
    struct Settings(Option<&'static str>);

    impl PropertyDataPort for Settings {
        fn value(&self, _property_id: i32, key: PropertyDataKey) -> Option<String> {
            match key {
                PropertyDataKey::PunchRoundingThreshold => self.0.map(str::to_string),
                _ => None,
            }
        }
        fn value_as_double(&self, property_id: i32, key: PropertyDataKey) -> Option<f64> {
            self.value(property_id, key)?.parse().ok()
        }
        fn value_as_boolean(&self, property_id: i32, key: PropertyDataKey) -> Option<bool> {
            Some(self.value(property_id, key)?.eq_ignore_ascii_case("true"))
        }
    }

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn shift(punch_type: PunchType, time: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![EmployeeShiftPunch::new(
                1,
                punch_type,
                PunchSource::Clock,
                time,
            )],
        )
    }

    fn round(
        setting: Option<&'static str>,
        punch_type: PunchType,
        at_time: LocalDateTime,
    ) -> LocalDateTime {
        let mut shift = shift(punch_type, at_time);
        PropertyDataRoundingRule::new(Settings(setting))
            .execute(&mut shift.punch_cursor(0), &RuleParams::new());
        shift.punch(0).rounded_time().unwrap()
    }

    #[test]
    fn an_unset_threshold_falls_back_to_fifteen_minutes() {
        assert_eq!(round(None, PunchType::In, at(7, 40)), at(7, 45));
    }

    #[test]
    fn a_single_value_applies_to_every_punch_type() {
        for punch_type in [
            PunchType::In,
            PunchType::Out,
            PunchType::Break,
            PunchType::Back,
        ] {
            assert_eq!(round(Some("30"), punch_type, at(7, 40)), at(7, 30));
        }
    }

    #[rstest]
    // "in,out,break,back" — a different threshold each way.
    #[case(PunchType::In, at(7, 40), at(7, 40))]
    #[case(PunchType::Out, at(7, 40), at(7, 45))]
    #[case(PunchType::Break, at(7, 40), at(7, 30))]
    #[case(PunchType::Back, at(7, 40), at(8, 0))]
    fn the_comma_form_gives_each_punch_type_its_own_threshold(
        #[case] punch_type: PunchType,
        #[case] punch_at: LocalDateTime,
        #[case] expected: LocalDateTime,
    ) {
        // 1 minute in, 15 out, 30 break, 60 back.
        assert_eq!(round(Some("1,15,30,60"), punch_type, punch_at), expected);
    }

    #[rstest]
    #[case(Some("1,15,30"), "three values")]
    #[case(Some("1,15,30,60,90"), "five values")]
    #[case(Some("not a number"), "unparseable")]
    #[case(Some(""), "empty")]
    fn a_malformed_setting_silently_falls_back_to_fifteen(
        #[case] setting: Option<&'static str>,
        #[case] why: &str,
    ) {
        // Java neither logs nor throws here; the field defaults simply stand.
        assert_eq!(
            round(setting, PunchType::In, at(7, 40)),
            at(7, 45),
            "should have fallen back: {why}"
        );
    }

    #[test]
    fn a_gated_off_punch_is_left_alone() {
        let mut shift = shift(PunchType::In, at(7, 40));
        // The gate is on the manual side, and this punch is from a clock, so
        // set the clock gate.
        PropertyDataRoundingRule::new(Settings(Some("30"))).execute(
            &mut shift.punch_cursor(0),
            &rule_params! { crate::rules::algorithm::punchrounding::config::CLOCK_IN => "false" },
        );

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 40)));
    }

    #[test]
    fn the_manual_gate_does_not_gate_a_clock_punch() {
        let mut shift = shift(PunchType::In, at(7, 40));

        PropertyDataRoundingRule::new(Settings(Some("30"))).execute(
            &mut shift.punch_cursor(0),
            &rule_params! { MANUAL_IN => "false" },
        );

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 30)));
    }

    #[test]
    fn rounding_an_in_punch_moves_the_shift_start() {
        let mut shift = shift(PunchType::In, at(7, 40));

        PropertyDataRoundingRule::new(Settings(None))
            .execute(&mut shift.punch_cursor(0), &RuleParams::new());

        assert_eq!(shift.start_date_time(), Some(at(7, 45)));
    }

    #[test]
    fn the_thresholds_are_read_in_declaration_order() {
        let rule = PropertyDataRoundingRule::new(Settings(Some("1,2,3,4")));
        assert_eq!(rule.thresholds(11), [1, 2, 3, 4]);
    }

    #[test]
    fn a_partly_unparseable_comma_form_defaults_only_the_bad_field() {
        // Java's Integer.parseInt would throw here; returning the default for
        // the offending field keeps the rule running on the rest, which is the
        // conservative reading. Recorded in PARITY_AUDIT.md.
        let rule = PropertyDataRoundingRule::new(Settings(Some("1,oops,3,4")));
        assert_eq!(rule.thresholds(11), [1, 15, 3, 4]);
    }
}
