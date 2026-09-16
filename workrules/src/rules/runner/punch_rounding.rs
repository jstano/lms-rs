//! Port of `PunchRoundingRunner` and `PunchRoundingUtil`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/punchrounding/`.
//!
//! The first runner ported, and the simplest complete one in the engine. For
//! each shift on a time card it:
//!
//! 1. skips the shift if it has no punches, or if the period is not open for
//!    editing;
//! 2. resets every punch's rounded time back to its adjusted time, and the
//!    shift date back to the in punch's adjusted date;
//! 3. resolves the `PUNCH_ROUNDING` rule set and runs each rule item against
//!    every punch — or, if no rule set is configured, runs
//!    [`PropertyDataPrr`](crate::rules::rule_class::RuleClass::PropertyDataPrr)
//!    on its own defaults;
//! 4. corrects the shift date from the rounded in punch, since rounding can
//!    push a punch into the next day.
//!
//! Step 2 is what makes the family idempotent across a whole recalculation, and
//! step 4 is why `MinuteRoundingRuleImplTest`'s "round to midnight" row matters:
//! a punch at 23:56 rounds to 00:00 the next day, and the shift follows it.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::punchrounding::back_from_break_with_grace::BackFromBreakWithGraceRule;
use crate::rules::algorithm::punchrounding::minute_rounding::MinuteRoundingRule;
use crate::rules::algorithm::punchrounding::property_data_rounding::PropertyDataRoundingRule;
use crate::rules::algorithm::punchrounding::round_to_schedule::{
    RoundInToScheduleRule, RoundOutToScheduleRule,
};
use crate::rules::algorithm::punchrounding::worked_hours_rounding::WorkedHoursRoundingRule;
use crate::rules::algorithm::punchrounding::{PunchRoundingRule, config};
use crate::rules::params::RuleParams;
use crate::rules::ports::PropertyDataPort;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use crate::rules::rule_type::RuleType;
use crate::{common::enums::punch_type::PunchType, entity::rule_set::RuleSet};

/// Runs the punch-rounding family over a shift. `PunchRoundingRunner`.
pub struct PunchRoundingRunner<P: PropertyDataPort> {
    property_data_rounding: PropertyDataRoundingRule<P>,
}

impl<P: PropertyDataPort> PunchRoundingRunner<P> {
    /// Build the runner over the settings port its fallback rule needs.
    pub fn new(property_data: P) -> Self {
        Self {
            property_data_rounding: PropertyDataRoundingRule::new(property_data),
        }
    }

    /// Round every punch on a shift.
    ///
    /// `PunchRoundingRunner.punchRoundShift`. `rule_set` is the resolved
    /// `PUNCH_ROUNDING` set, or `None` when nothing is configured for this
    /// employee, job and date.
    ///
    /// # Panics
    ///
    /// If `rule_set` is not a `PUNCH_ROUNDING` set. Resolution is asked for a
    /// type, so a set of any other type here is a wiring bug, not data.
    pub fn round_shift(
        &self,
        shift: &mut EmployeeShift,
        rule_set: Option<&RuleSet>,
        time_card: Option<&dyn TimeCard>,
        open_for_editing: bool,
    ) {
        if let Some(rule_set) = rule_set {
            assert_eq!(
                rule_set.rule_type(),
                RuleType::PunchRounding,
                "the runner was given a rule set of the wrong type"
            );
        }

        if shift.punch_count() == 0 || !open_for_editing {
            return;
        }

        reset_rounded_times_and_shift_date(shift);

        match rule_set.filter(|rule_set| !rule_set.is_empty()) {
            Some(rule_set) => {
                for rule_item in rule_set.rule_items() {
                    self.run_for_each_punch(
                        shift,
                        rule_item.rule_class(),
                        rule_item.params().clone(),
                        time_card,
                    );
                }
            }
            None => {
                // No rule set, or one with no items: Java falls back to
                // PROPERTY_DATA_PRR on its own defaults.
                let defaults = config::PropertyDataRoundingRuleConfig.default_values();
                self.run_for_each_punch(shift, RuleClass::PropertyDataPrr, defaults, time_card);
            }
        }

        correct_shift_date_using_rounded_in_punch(shift);
    }

    /// Run one rule class against every punch on the shift.
    /// `PunchRoundingRunner.runRuleForEachPunch`.
    ///
    /// Java resolves the class to a bean and casts; here the match *is* the
    /// dispatch. A rule class outside this family does nothing, which is the
    /// closest honest equivalent of Java's missing-bean failure.
    fn run_for_each_punch(
        &self,
        shift: &mut EmployeeShift,
        rule_class: RuleClass,
        params: RuleParams,
        time_card: Option<&dyn TimeCard>,
    ) {
        for index in 0..shift.punch_count() {
            let mut punch = shift.punch_cursor(index);
            match rule_class {
                RuleClass::MinutePrr => MinuteRoundingRule.execute(&mut punch, &params),
                RuleClass::WorkedHoursPrr => WorkedHoursRoundingRule.execute(&mut punch, &params),
                RuleClass::BackGracePrr => {
                    BackFromBreakWithGraceRule.execute(&mut punch, &params);
                }
                RuleClass::PropertyDataPrr => {
                    self.property_data_rounding.execute(&mut punch, &params);
                }
                RuleClass::InToSchedPrr => {
                    RoundInToScheduleRule.execute(&mut punch, time_card, &params);
                }
                RuleClass::OutToSchedPrr => {
                    RoundOutToScheduleRule.execute(&mut punch, time_card, &params);
                }
                _ => {}
            }
        }
    }
}

/// Put every punch back to its adjusted time and the shift date back to the in
/// punch's. `PunchRoundingUtil.resetRoundedTimesAndShiftDate`.
pub fn reset_rounded_times_and_shift_date(shift: &mut EmployeeShift) {
    if let Some(index) = punch_index_of_type(shift, PunchType::In)
        && let Some(adj_time) = shift.punch(index).adj_time()
    {
        shift.set_shift_date(adj_time.to_local_date());
    }

    for index in 0..shift.punch_count() {
        let adj_time = shift.punch(index).adj_time();
        shift.punch_cursor(index).set_rounded_time(adj_time);
    }
}

/// Move the shift onto the date its rounded in punch landed on.
/// `PunchRoundingUtil.correctShiftDateUsingRoundedInPunch`.
///
/// Rounding can push an in punch across midnight, and the shift follows it.
pub fn correct_shift_date_using_rounded_in_punch(shift: &mut EmployeeShift) {
    if let Some(index) = punch_index_of_type(shift, PunchType::In)
        && let Some(rounded) = shift.punch(index).rounded_time()
    {
        shift.set_shift_date(rounded.to_local_date());
    }
}

/// The first punch of a type. `EmployeeShift.getPunchOfType`.
fn punch_index_of_type(shift: &EmployeeShift, punch_type: PunchType) -> Option<usize> {
    shift
        .punches()
        .iter()
        .position(|punch| punch.punch_type() == punch_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::rule_item::RuleItem;
    use crate::rule_params;
    use crate::rules::ports::PropertyDataKey;
    use joda_rs::{LocalDate, LocalDateTime};

    /// A settings port that knows no thresholds, so the fallback rule uses 15.
    struct NoSettings;

    impl PropertyDataPort for NoSettings {
        fn value(&self, _property_id: i32, _key: PropertyDataKey) -> Option<String> {
            None
        }
        fn value_as_double(&self, _property_id: i32, _key: PropertyDataKey) -> Option<f64> {
            None
        }
        fn value_as_boolean(&self, _property_id: i32, _key: PropertyDataKey) -> Option<bool> {
            None
        }
    }

    fn runner() -> PunchRoundingRunner<NoSettings> {
        PunchRoundingRunner::new(NoSettings)
    }

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn shift(punches: Vec<EmployeeShiftPunch>) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            punches,
        )
    }

    fn punch(id: i32, punch_type: PunchType, time: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Clock, time)
    }

    fn worked_shift() -> EmployeeShift {
        shift(vec![
            punch(1, PunchType::In, at(7, 52)),
            punch(2, PunchType::Out, at(16, 7)),
        ])
    }

    fn minute_rounding_set(params: RuleParams) -> RuleSet {
        RuleSet::new(
            1,
            11,
            "Round to the quarter",
            RuleType::PunchRounding,
            0,
            vec![RuleItem::new(
                10,
                1,
                "Minute rounding",
                RuleClass::MinutePrr,
                params,
            )],
        )
    }

    #[test]
    fn a_configured_rule_runs_against_every_punch() {
        let mut shift = worked_shift();

        runner().round_shift(
            &mut shift,
            Some(&minute_rounding_set(RuleParams::new())),
            None,
            true,
        );

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 45)));
        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn with_no_rule_set_the_property_data_rule_is_the_fallback() {
        // NoSettings means its threshold is the built-in 15 minutes.
        let mut shift = worked_shift();

        runner().round_shift(&mut shift, None, None, true);

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 45)));
        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
    }

    #[test]
    fn an_empty_rule_set_also_falls_back() {
        // Java tests both `ruleSet != null` and `!getRuleItems().isEmpty()`.
        let empty = RuleSet::new(1, 11, "Empty", RuleType::PunchRounding, 0, Vec::new());
        let mut shift = worked_shift();

        runner().round_shift(&mut shift, Some(&empty), None, true);

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 45)));
    }

    #[test]
    fn a_shift_with_no_punches_is_skipped() {
        let mut shift = shift(Vec::new());

        runner().round_shift(&mut shift, None, None, true);

        assert_eq!(shift.shift_date(), LocalDate::of(2010, 1, 2));
    }

    #[test]
    fn a_shift_in_a_closed_period_is_left_alone() {
        let mut shift = worked_shift();

        runner().round_shift(&mut shift, None, None, false);

        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 52)));
    }

    #[test]
    fn the_reset_undoes_a_previous_rounding_before_running() {
        // Rounding is always measured from the adjusted times, so running the
        // runner twice gives the same answer as running it once.
        let mut shift = worked_shift();
        let rule_set = minute_rounding_set(RuleParams::new());

        runner().round_shift(&mut shift, Some(&rule_set), None, true);
        let once = shift.punch(0).rounded_time();
        runner().round_shift(&mut shift, Some(&rule_set), None, true);

        assert_eq!(shift.punch(0).rounded_time(), once);
    }

    #[test]
    fn a_tighter_rule_set_on_a_second_pass_is_not_compounded() {
        let mut shift = worked_shift();

        runner().round_shift(
            &mut shift,
            Some(&minute_rounding_set(RuleParams::new())),
            None,
            true,
        );
        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 45)));

        // Now round to 5 instead: 07:52 goes to 07:50, not 07:45.
        runner().round_shift(
            &mut shift,
            Some(&minute_rounding_set(
                rule_params! { config::IN_PUNCH_ROUND_TO => "5" },
            )),
            None,
            true,
        );
        assert_eq!(shift.punch(0).rounded_time(), Some(at(7, 50)));
    }

    #[test]
    fn rounding_across_midnight_moves_the_shift_date() {
        // The "round to midnight" row of MinuteRoundingRuleImplTest, carried
        // through the runner: the punch lands on the 2nd, and so does the shift.
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2013, 4, 1),
            ShiftType::Actual,
            vec![punch(
                1,
                PunchType::In,
                LocalDateTime::of(2013, 4, 1, 23, 56, 0),
            )],
        );

        runner().round_shift(
            &mut shift,
            Some(&minute_rounding_set(RuleParams::new())),
            None,
            true,
        );

        assert_eq!(
            shift.punch(0).rounded_time(),
            Some(LocalDateTime::of(2013, 4, 2, 0, 0, 0))
        );
        assert_eq!(shift.shift_date(), LocalDate::of(2013, 4, 2));
    }

    #[test]
    fn the_reset_puts_the_shift_date_back_on_the_adjusted_in_punch() {
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2013, 4, 2),
            ShiftType::Actual,
            vec![punch(
                1,
                PunchType::In,
                LocalDateTime::of(2013, 4, 1, 23, 56, 0),
            )],
        );

        reset_rounded_times_and_shift_date(&mut shift);

        assert_eq!(shift.shift_date(), LocalDate::of(2013, 4, 1));
    }

    #[test]
    fn several_rule_items_run_in_order() {
        // Round to 5 first, then to 30. Both measure from the adjusted time, so
        // the last one wins rather than compounding.
        let rule_set = RuleSet::new(
            1,
            11,
            "Two rules",
            RuleType::PunchRounding,
            0,
            vec![
                RuleItem::new(
                    10,
                    1,
                    "To five",
                    RuleClass::MinutePrr,
                    rule_params! { config::IN_PUNCH_ROUND_TO => "5" },
                ),
                RuleItem::new(
                    11,
                    1,
                    "To thirty",
                    RuleClass::MinutePrr,
                    rule_params! { config::IN_PUNCH_ROUND_TO => "30" },
                ),
            ],
        );
        let mut shift = shift(vec![punch(1, PunchType::In, at(7, 52))]);

        runner().round_shift(&mut shift, Some(&rule_set), None, true);

        assert_eq!(shift.punch(0).rounded_time(), Some(at(8, 0)));
    }

    #[test]
    #[should_panic(expected = "wrong type")]
    fn a_rule_set_of_another_type_is_a_wiring_bug() {
        let wrong = RuleSet::new(1, 11, "Accruals", RuleType::BenefitAccrual, 0, Vec::new());

        runner().round_shift(&mut worked_shift(), Some(&wrong), None, true);
    }
}
