//! Port of `StartEndBetweenPremiumRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/StartEndBetweenPremiumRuleImpl.java`.
//!
//! A flat premium for shifts that **start** inside one time-of-day window or
//! **end** inside another (an "overnight" premium). Each of the two tests that
//! passes is one occurrence; each occurrence is a separate earning of
//! `earningAmount` dollars — unless `payOnlyOnce`, which caps it at one.
//!
//! The windows are `earliest`–`latest` times of day and may cross midnight; they
//! are built for the shift date and the days either side
//! ([`time_ranges_including_overlaps`](super::utility::shift_time_window_utility::time_ranges_including_overlaps))
//! and tested **inclusive of both ends**. The times tested are the first IN and
//! first OUT punch, adjusted (`useActualTime`) or rounded.
//!
//! The shift needs both times, no persisted errors and an open period. The
//! earning is dollars only, attached to the shift, with the fixed note
//! `"Start or End Between Premium Rule"`; its total is set directly rather than
//! calculated.

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    EARLIEST_END_TIME_PROP, EARLIEST_START_TIME_PROP, EARNING, EARNING_AMOUNT_PROP,
    LATEST_END_TIME_PROP, LATEST_START_TIME_PROP, PAY_ONLY_ONCE_PROP,
    StartEndBetweenPremiumRuleConfig, USE_ACTUAL_TIME_PROP,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_window_utility::time_ranges_including_overlaps;
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::{LocalDate, LocalDateTime, LocalTime};
use std::cell::RefCell;

/// The fixed note on every earning.
pub const NOTE: &str = "Start or End Between Premium Rule";

/// `StartEndBetweenPremiumRuleImpl`.
pub struct StartEndBetweenPremiumRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> StartEndBetweenPremiumRule<E> {
    /// Build the rule over the earning writer (the earning type is only stored).
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

fn falls_within(
    date: LocalDate,
    time: LocalDateTime,
    earliest: LocalTime,
    latest: LocalTime,
) -> bool {
    time_ranges_including_overlaps(date, earliest, latest)
        .iter()
        .any(|range| range.contains(time))
}

fn payable_occurrences(shift: &EmployeeShift, params: &RuleParams) -> usize {
    let use_actual = params.bool_at(USE_ACTUAL_TIME_PROP);
    let time = |key: &str| LocalTime::parse(params.get(key).unwrap_or("00:00:00"));
    let date = shift.shift_date();

    let start_ok = shift
        .punch_time_of_type(PunchType::In, use_actual)
        .is_some_and(|t| {
            falls_within(
                date,
                t,
                time(EARLIEST_START_TIME_PROP),
                time(LATEST_START_TIME_PROP),
            )
        });
    let end_ok = shift
        .punch_time_of_type(PunchType::Out, use_actual)
        .is_some_and(|t| {
            falls_within(
                date,
                t,
                time(EARLIEST_END_TIME_PROP),
                time(LATEST_END_TIME_PROP),
            )
        });

    let occurrences = usize::from(start_ok) + usize::from(end_ok);
    if params.bool_at(PAY_ONLY_ONCE_PROP) && occurrences > 0 {
        1
    } else {
        occurrences
    }
}

impl<E: EmployeeEarningPort> ShiftEarningRule for StartEndBetweenPremiumRule<E> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&StartEndBetweenPremiumRuleConfig.default_values());

        if !shift.has_both_times() || !shift_is_eligible(shift, time_card) {
            return;
        }
        let occurrences = payable_occurrences(shift, &params);
        if occurrences == 0 {
            return;
        }

        let amount = params.double_at(EARNING_AMOUNT_PROP);
        let earnings: Vec<_> = (0..occurrences)
            .map(|_| {
                let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, true);
                earning.set_dollars(amount);
                earning.set_total_dollars(amount);
                earning.set_note(NOTE);
                earning
            })
            .collect();

        let mut port = self.earnings.borrow_mut();
        port.save_all(earnings.clone());
        time_card.earnings_mut().extend(earnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_class::RuleClass;

    fn ldt(y: i32, m: i32, d: i32, h: i32, mi: i32) -> LocalDateTime {
        LocalDateTime::of(y, m, d, h, mi, 0)
    }

    /// One row of the Groovy `where:` table.
    struct Row {
        start: LocalDateTime,
        end: Option<LocalDateTime>,
        errors: bool,
        before_calc_date: bool,
        window: Option<(&'static str, &'static str)>,
        pay_once: &'static str,
        actual_time: &'static str,
        earnings: usize,
    }

    fn run_row(row: &Row) -> (usize, TimeCardData) {
        let shift_date = row.start.to_local_date();
        let start_punch = {
            let mut p = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, row.start);
            p.set_adj_time(Some(row.start));
            p.set_punch_time(Some(row.start));
            p
        };
        let mut shift =
            EmployeeShift::new(1, 1, 9, shift_date, ShiftType::Actual, vec![start_punch])
                .with_times(Some(row.start), row.end)
                .with_errors(if row.errors {
                    vec![ShiftErrorType::MissingOut]
                } else {
                    Vec::new()
                });
        shift
            .punch_cursor(0)
            .set_rounded_time(Some(row.start.minus_minutes(5)));
        if let Some(end) = row.end {
            let mut out = EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, end);
            out.set_adj_time(Some(end));
            shift.add_punch(out);
            shift
                .punch_cursor(1)
                .set_rounded_time(Some(end.plus_minutes(5)));
        }
        // Row start/end times were set directly above; the cursor calls moved
        // them to the rounded times, so put them back as the fixture built them.
        let shift = shift.with_times(Some(row.start), row.end);

        let calc_start = if row.before_calc_date {
            shift_date.plus_days(6)
        } else {
            shift_date
        };
        let mut card = TimeCardData::new()
            .with_shifts(vec![shift.clone()])
            .with_calculation_start_date(calc_start);

        let mut params = StartEndBetweenPremiumRuleConfig.default_values();
        params.set(PAY_ONLY_ONCE_PROP, row.pay_once);
        params.set(USE_ACTUAL_TIME_PROP, row.actual_time);
        if let Some((earliest, latest)) = row.window {
            params.set(EARLIEST_START_TIME_PROP, earliest);
            params.set(LATEST_START_TIME_PROP, latest);
            params.set(EARLIEST_END_TIME_PROP, earliest);
            params.set(LATEST_END_TIME_PROP, latest);
        }

        let saved = SavedEarnings::default();
        StartEndBetweenPremiumRule::new(saved.clone()).execute(
            &shift,
            &mut card,
            &RuleItem::new(4, 1, "x", RuleClass::StartEndBetweenDif, params),
        );
        let count = saved.0.borrow().len();
        (count, card)
    }

    // ---- java_parity_tests: StartEndBetweenPremiumRuleImplTest.groovy ----

    #[test]
    fn execute_creates_earnings_for_a_shift_that_starts_or_ends_in_the_windows() {
        let w0_8 = Some(("00:00:00", "08:00:00"));
        let w23_1 = Some(("23:00:00", "01:00:00"));
        let rows = [
            // start, end, errors, beforeCalc, window, payOnce, actual, earnings
            (ldt(2014, 1, 1, 0, 0), None, false, false, None, "", "", 0),
            (
                ldt(2014, 1, 1, 0, 0),
                Some(ldt(2014, 1, 1, 8, 0)),
                true,
                false,
                None,
                "",
                "",
                0,
            ),
            (
                ldt(2014, 1, 1, 0, 0),
                Some(ldt(2014, 1, 1, 8, 0)),
                false,
                true,
                None,
                "",
                "",
                0,
            ),
            (
                ldt(2013, 12, 31, 23, 0),
                Some(ldt(2014, 1, 1, 9, 0)),
                false,
                false,
                w0_8,
                "false",
                "true",
                0,
            ),
            (
                ldt(2014, 1, 1, 0, 0),
                Some(ldt(2014, 1, 1, 9, 0)),
                false,
                false,
                w0_8,
                "false",
                "true",
                1,
            ),
            (
                ldt(2014, 1, 1, 0, 0),
                Some(ldt(2014, 1, 1, 8, 0)),
                false,
                false,
                w0_8,
                "false",
                "true",
                2,
            ),
            (
                ldt(2014, 1, 1, 0, 0),
                Some(ldt(2014, 1, 1, 8, 0)),
                false,
                false,
                w0_8,
                "true",
                "true",
                1,
            ),
            (
                ldt(2014, 1, 1, 23, 0),
                Some(ldt(2014, 1, 2, 1, 0)),
                false,
                false,
                w23_1,
                "false",
                "true",
                2,
            ),
            (
                ldt(2014, 1, 1, 23, 0),
                Some(ldt(2014, 1, 2, 1, 0)),
                false,
                false,
                w23_1,
                "false",
                "false",
                0,
            ),
            (
                ldt(2014, 1, 1, 23, 5),
                Some(ldt(2014, 1, 2, 0, 55)),
                false,
                false,
                w23_1,
                "false",
                "false",
                2,
            ),
            (
                ldt(2014, 1, 1, 23, 5),
                Some(ldt(2014, 1, 2, 0, 55)),
                false,
                false,
                w23_1,
                "true",
                "false",
                1,
            ),
        ];
        for (i, (start, end, errors, before_calc_date, window, pay_once, actual_time, earnings)) in
            rows.into_iter().enumerate()
        {
            let row = Row {
                start,
                end,
                errors,
                before_calc_date,
                window,
                pay_once,
                actual_time,
                earnings,
            };
            let (saved, card) = run_row(&row);
            assert_eq!(saved, row.earnings, "row {}", i + 1);
            assert_eq!(
                card.earnings().len(),
                row.earnings,
                "row {}: also on the card",
                i + 1
            );
            for e in card.earnings() {
                assert_eq!(e.dollars(), 350.0);
                assert_eq!(e.total_dollars(), 350.0);
                assert_eq!(e.note(), NOTE);
                assert_eq!(e.source(), EarningSource::Rule);
                assert_eq!(e.earning_date(), row.start.to_local_date());
                assert_eq!(e.pay_date(), Some(row.start.to_local_date()));
                assert_eq!(e.shift_id(), Some(1));
                assert_eq!(e.rule_item_id(), Some(4));
            }
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_windows_ends_are_inclusive() {
        let row = Row {
            start: ldt(2014, 1, 1, 8, 0), // exactly the window's end
            end: Some(ldt(2014, 1, 1, 12, 0)),
            errors: false,
            before_calc_date: false,
            window: Some(("00:00:00", "08:00:00")),
            pay_once: "false",
            actual_time: "true",
            earnings: 1,
        };
        assert_eq!(run_row(&row).0, 1);
    }

    #[test]
    fn the_amount_comes_from_the_earning_amount_parameter() {
        let shift = EmployeeShift::new(
            1,
            1,
            9,
            LocalDate::of(2014, 1, 1),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(
                    1,
                    PunchType::In,
                    PunchSource::Clock,
                    ldt(2014, 1, 1, 1, 0),
                ),
                EmployeeShiftPunch::new(
                    2,
                    PunchType::Out,
                    PunchSource::Clock,
                    ldt(2014, 1, 1, 12, 0),
                ),
            ],
        )
        .with_times(Some(ldt(2014, 1, 1, 1, 0)), Some(ldt(2014, 1, 1, 12, 0)));
        let mut card = TimeCardData::new().with_shifts(vec![shift.clone()]);
        StartEndBetweenPremiumRule::new(SavedEarnings::default()).execute(
            &shift,
            &mut card,
            &RuleItem::new(
                4,
                1,
                "x",
                RuleClass::StartEndBetweenDif,
                rule_params! { EARNING_AMOUNT_PROP => "12.5" },
            ),
        );
        assert_eq!(
            card.earnings().len(),
            1,
            "the default window 00:00-05:30 catches a 01:00 start"
        );
        assert_eq!(card.earnings()[0].dollars(), 12.5);
    }
}
