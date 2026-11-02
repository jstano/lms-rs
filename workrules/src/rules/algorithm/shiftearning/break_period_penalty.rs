//! Port of `BreakPeriodPenaltyRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/BreakPeriodPenaltyRuleImpl.java`.
//!
//! The meal-break penalty (the "one hour's pay if you did not get a meal break
//! by the fifth hour" shape). Up to two penalties of `hoursPerPenalty` hours at
//! the shift's job rate, capped at `maxDailyPenalty` for the day:
//!
//! - **First break:** if the shift is at least `firstMinShiftLength` hours,
//!   look for a *valid* break (at least `minBreakLength`) that **starts less
//!   than `firstBreakLimit` hours after the first IN**. If one exists it is
//!   *used up* (removed from the list); if not, a penalty is written.
//! - **Second break:** only if the cap is still open, and the shift is at least
//!   `secMinShiftLength` hours: look in the *remaining* valid breaks for one
//!   starting within `secBreakLimit` hours of the IN. None: a penalty, with
//!   `" - second break"` appended to the note.
//!
//! So a single early break satisfies the first test only, and the second needs
//! its own break (an early break cannot be counted twice).
//!
//! # The cap counts across the day's shifts
//!
//! `penaltyCount` starts as the number of RULE earnings on **the shift's date**
//! belonging to **this rule item** — including ones written for other shifts
//! that day — so `maxDailyPenalty` is per day, not per shift. Each penalty
//! written raises the count, and the rule does not run at all if the cap is
//! already reached. (Java would throw on a RULE earning with no rule item; the
//! port ignores one — divergence 104.)
//!
//! Needs both times, no persisted errors and an open period. No Groovy spec;
//! behaviour tests are written from the Java.

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, BreakPeriodPenaltyRuleConfig, EARNING, FIRST_BREAK_LIMIT,
    FIRST_MIN_SHIFT_LENGTH, HOURS_PER_PENALTY, MAX_DAILY_PENALTY, MIN_BREAK_LENGTH,
    SEC_BREAK_LIMIT, SEC_MIN_SHIFT_LENGTH,
};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDateTime;
use std::cell::RefCell;

/// `BreakPeriodPenaltyRuleImpl`.
pub struct BreakPeriodPenaltyRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> BreakPeriodPenaltyRule<T, E, M> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_penalty(
        &self,
        time_card: &mut dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
        earning_type_id: i32,
        hours: f64,
        suffix: &str,
    ) -> bool {
        let Some(earning_type) = self.earning_types.find_by_id(earning_type_id) else {
            return false;
        };
        let Some(employee) = time_card.employee() else {
            return false;
        };
        let Some(rate) = earning_rate_for_job(
            employee,
            shift.job_id(),
            shift.shift_date(),
            &earning_type,
            &self.min_wage,
        ) else {
            return false;
        };
        let mut earning = new_shift_earning(shift, earning_type_id, rule_item, true);
        earning.set_hours(hours);
        earning.set_rate(rate);
        earning.calc_and_set_total_dollars();
        earning.set_note(format!("{}{suffix}", rule_item.name()));

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
        true
    }
}

/// `findBreakWithinRangeOf`: the first break starting less than `hours` after
/// the IN, by index.
fn find_break_within(
    breaks: &[DateTimeRange],
    in_start: Option<LocalDateTime>,
    hours: f64,
) -> Option<usize> {
    let in_start = in_start?;
    breaks
        .iter()
        .position(|b| duration_in_fractional_hours(in_start, b.start()) < hours)
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftEarningRule
    for BreakPeriodPenaltyRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&BreakPeriodPenaltyRuleConfig.default_values());
        let earning_type_id = params.int_at(EARNING);
        let max_daily = params.int_at(MAX_DAILY_PENALTY);
        let hours_penalty = params.double_at(HOURS_PER_PENALTY);
        let min_break = params.double_at(MIN_BREAK_LENGTH);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let shift_hours = shift.net_hours_using(use_actual);
        let in_start = shift.punch_time_of_type(PunchType::In, use_actual);
        let mut breaks: Vec<DateTimeRange> = shift
            .breaks_using(use_actual)
            .into_iter()
            .filter(|b| b.duration().fractional_hours() >= min_break)
            .collect();
        let mut penalty_count = time_card
            .earnings()
            .iter()
            .filter(|e| {
                e.earning_date() == shift.shift_date()
                    && e.source() == EarningSource::Rule
                    && e.rule_item_id() == Some(rule_item.id())
            })
            .count() as i32;

        let should_run = !shift.has_errors()
            && shift.has_both_times()
            && time_card.is_open_for_editing_for_shift(shift)
            && penalty_count < max_daily;
        if !should_run {
            return;
        }

        if shift_hours >= params.double_at(FIRST_MIN_SHIFT_LENGTH) {
            match find_break_within(&breaks, in_start, params.double_at(FIRST_BREAK_LIMIT)) {
                Some(index) => {
                    breaks.remove(index);
                }
                None => {
                    if self.add_penalty(
                        time_card,
                        shift,
                        rule_item,
                        earning_type_id,
                        hours_penalty,
                        "",
                    ) {
                        penalty_count += 1;
                    }
                }
            }
        }

        if penalty_count < max_daily
            && shift_hours >= params.double_at(SEC_MIN_SHIFT_LENGTH)
            && find_break_within(&breaks, in_start, params.double_at(SEC_BREAK_LIMIT)).is_none()
        {
            self.add_penalty(
                time_card,
                shift,
                rule_item,
                earning_type_id,
                hours_penalty,
                " - second break",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const JOB: i32 = 9;
    const NOTE: &str = "Rule: set - Break Penalty";

    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, h, m, 0)
    }
    fn p(id: i32, t: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, when)
    }
    /// IN 06:00, OUT at `out_hour`; breaks given as `(hours after the IN, minutes long)`.
    fn shift(out_hour: i32, breaks: &[(i64, i64)], net: f64) -> EmployeeShift {
        let start = at(6, 0);
        let mut punches = vec![p(1, PunchType::In, start)];
        let mut id = 2;
        for (after_in, len) in breaks {
            let b = start.plus_hours(*after_in);
            punches.push(p(id, PunchType::Break, b));
            punches.push(p(id + 1, PunchType::Back, b.plus_minutes(*len)));
            id += 2;
        }
        punches.push(p(id, PunchType::Out, at(out_hour, 0)));
        EmployeeShift::new(
            7,
            1,
            JOB,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            punches,
        )
        .with_times(Some(start), Some(at(out_hour, 0)))
        .with_net_hours(net)
    }
    fn item(extra: &[(&str, &str)]) -> RuleItem {
        let mut p = rule_params! {
            EARNING => "3", FIRST_BREAK_LIMIT => "5", FIRST_MIN_SHIFT_LENGTH => "5", MIN_BREAK_LENGTH => "0.5",
            SEC_BREAK_LIMIT => "10", SEC_MIN_SHIFT_LENGTH => "10", MAX_DAILY_PENALTY => "1",
            HOURS_PER_PENALTY => "1", ACTUAL_PUNCH_PROP => "false"
        };
        for (k, v) in extra {
            p.set(*k, *v);
        }
        RuleItem::new(2, 1, NOTE, RuleClass::BreakPeriodPenaltyDif, p)
    }
    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                LocalDate::of(2014, 1, 1),
                LocalDate::of(2015, 1, 1),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        )
    }
    fn run(s: &EmployeeShift, extra: &[(&str, &str)], prior: Vec<EmployeeEarning>) -> TimeCardData {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![s.clone()])
            .with_earnings(prior);
        BreakPeriodPenaltyRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            SavedEarnings::default(),
            FixedMinWage(0.0),
        )
        .execute(s, &mut c, &item(extra));
        c
    }

    #[test]
    fn a_long_shift_with_no_break_gets_the_first_penalty() {
        let c = run(&shift(13, &[], 7.0), &[], vec![]);
        let e = &c.earnings()[0];
        assert_eq!((e.hours(), e.rate(), e.total_dollars()), (1.0, 10.0, 10.0));
        assert_eq!(e.note(), NOTE);
        assert_eq!(e.shift_id(), Some(7));
    }

    #[test]
    fn a_shift_under_the_first_minimum_gets_nothing() {
        assert!(
            run(&shift(10, &[], 4.99), &[], vec![])
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn a_valid_break_starting_before_the_first_limit_avoids_the_penalty() {
        assert!(
            run(&shift(13, &[(4, 30)], 7.0), &[], vec![])
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn a_break_that_is_too_short_or_starts_too_late_does_not_count() {
        assert_eq!(
            run(&shift(13, &[(4, 29)], 7.0), &[], vec![])
                .earnings()
                .len(),
            1,
            "too short"
        );
        assert_eq!(
            run(&shift(13, &[(5, 30)], 7.0), &[], vec![])
                .earnings()
                .len(),
            1,
            "starts exactly at the limit"
        );
    }

    #[test]
    fn a_second_penalty_needs_the_cap_open_and_its_own_break() {
        // 11h shift, one early break: first test satisfied (break used up),
        // second test finds nothing left -> second penalty with the suffix.
        let c = run(
            &shift(17, &[(4, 30)], 11.0),
            &[(MAX_DAILY_PENALTY, "2")],
            vec![],
        );
        assert_eq!(c.earnings().len(), 1);
        assert_eq!(c.earnings()[0].note(), format!("{NOTE} - second break"));
    }

    #[test]
    fn two_breaks_satisfy_both_tests() {
        let c = run(
            &shift(17, &[(4, 30), (8, 30)], 10.0),
            &[(MAX_DAILY_PENALTY, "2")],
            vec![],
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_break_on_a_very_long_shift_earns_both_penalties_when_the_cap_allows() {
        let c = run(&shift(17, &[], 11.0), &[(MAX_DAILY_PENALTY, "2")], vec![]);
        let notes: Vec<_> = c.earnings().iter().map(|e| e.note().to_string()).collect();
        assert_eq!(
            notes,
            vec![NOTE.to_string(), format!("{NOTE} - second break")]
        );
    }

    #[test]
    fn the_default_cap_of_one_stops_after_the_first_penalty() {
        let c = run(&shift(17, &[], 11.0), &[], vec![]);
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn penalties_already_given_that_day_by_this_rule_item_count_against_the_cap() {
        let earlier = EmployeeEarning::new(
            0,
            1,
            JOB,
            3,
            LocalDate::of(2014, 5, 6),
            1.0,
            10.0,
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule_item(&item(&[]), Some(99));
        let c = run(&shift(13, &[], 7.0), &[], vec![earlier]);
        assert_eq!(
            c.earnings().len(),
            1,
            "only the earlier one: the cap of 1 is already reached"
        );
    }

    #[test]
    fn another_rule_items_or_another_days_penalty_does_not_count() {
        let other_item = RuleItem::new(
            8,
            1,
            "other",
            RuleClass::BreakPeriodPenaltyDif,
            RuleParams::new(),
        );
        let a = EmployeeEarning::new(
            0,
            1,
            JOB,
            3,
            LocalDate::of(2014, 5, 6),
            1.0,
            10.0,
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule_item(&other_item, None);
        let b = EmployeeEarning::new(
            0,
            1,
            JOB,
            3,
            LocalDate::of(2014, 5, 5),
            1.0,
            10.0,
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule_item(&item(&[]), None);
        let c = run(&shift(13, &[], 7.0), &[], vec![a, b]);
        assert_eq!(c.earnings().len(), 3);
    }

    #[test]
    fn an_errored_or_unfinished_shift_is_skipped() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let bad = shift(13, &[], 7.0).with_errors(vec![ShiftErrorType::MissingOut]);
        assert!(run(&bad, &[], vec![]).earnings().is_empty());
        let open = shift(13, &[], 7.0).with_times(Some(at(6, 0)), None);
        assert!(run(&open, &[], vec![]).earnings().is_empty());
    }
}
