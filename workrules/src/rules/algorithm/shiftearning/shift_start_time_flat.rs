//! Port of `ShiftStartTimeFlatRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/ShiftStartTimeFlatRuleImpl.java`.
//!
//! A night-shift style premium: if the shift **starts** inside the configured
//! `Start Time`–`End Time` window, pays `round(diffRate × netHours, 2)` dollars
//! for the whole shift.
//!
//! The window may cross midnight (end before start rolls to the next day); it
//! is built for the shift date and the days either side, so a start at 01:00 is
//! caught by the window that opened the previous evening. Both ends are
//! **inclusive**. The start is the shift's last IN punch's adjusted time when
//! `actualPunchTime` is on, otherwise its rounded time.
//!
//! The shift must have no errors, both times, and be open for editing.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::dates::is_between;
use crate::common::enums::punch_type::PunchType;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DIFF_RATE_PROP, EARNING, END_TIME_PROP, START_TIME_PROP,
    ShiftStartTimeFlatRuleConfig,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_window_utility::{
    convert_end_time, convert_start_time,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalTime;
use std::cell::RefCell;

/// `ShiftStartTimeFlatRuleImpl`.
pub struct ShiftStartTimeFlatRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> ShiftStartTimeFlatRule<E> {
    /// Build the rule over the earning writer.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> ShiftEarningRule for ShiftStartTimeFlatRule<E> {
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&ShiftStartTimeFlatRuleConfig.default_values());
        if !shift_is_eligible(shift, time_card) || !shift.has_both_times() {
            return;
        }

        let start_time = LocalTime::parse(params.get(START_TIME_PROP).unwrap_or("00:00:00"));
        let end_time = LocalTime::parse(params.get(END_TIME_PROP).unwrap_or("00:00:00"));
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        // Java scans every punch and keeps the last IN.
        let Some(in_index) = shift
            .punches()
            .iter()
            .rposition(|p| p.punch_type() == PunchType::In)
        else {
            return;
        };
        let in_punch = shift.punch(in_index);
        let Some(shift_start) = (if use_actual {
            in_punch.adj_time()
        } else {
            in_punch.rounded_time()
        }) else {
            return;
        };

        let starts_in_premium_period = [0i64, 1, -1].into_iter().any(|offset| {
            let date = shift.shift_date().plus_days(offset);
            let from = convert_start_time(date, start_time);
            let to = convert_end_time(date, start_time, end_time);
            is_between(shift_start, from, to)
        });

        if starts_in_premium_period {
            let mut earning = new_shift_earning(shift, params.int_at(EARNING), rule_item, true);
            earning.set_dollars(round_to(
                params.double_at(DIFF_RATE_PROP) * shift.net_hours_using(use_actual),
                2,
            ));
            earning.set_hours(0.0);
            earning.set_rate(0.0);
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_item.name());

            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(day: i32, h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, day, h, m, 0)
    }
    fn shift(start: LocalDateTime, end: LocalDateTime, net: f64) -> EmployeeShift {
        EmployeeShift::new(
            7,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, start),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, end),
            ],
        )
        .with_times(Some(start), Some(end))
        .with_net_hours(net)
    }
    fn run(s: &EmployeeShift, p: RuleParams) -> TimeCardData {
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        ShiftStartTimeFlatRule::new(SavedEarnings::default()).execute(
            s,
            &mut c,
            &RuleItem::new(2, 1, "SST", RuleClass::ShiftStartFlatDif, p),
        );
        c
    }
    fn night() -> RuleParams {
        rule_params! {
            DIFF_RATE_PROP => "0.5", EARNING => "3", START_TIME_PROP => "22:00:00",
            END_TIME_PROP => "06:00:00", ACTUAL_PUNCH_PROP => "false"
        }
    }

    #[test]
    fn a_start_inside_the_window_pays_the_rate_times_the_whole_shift() {
        let s = shift(at(6, 23, 0), at(7, 7, 0), 8.0);
        let c = run(&s, night());
        let e = &c.earnings()[0];
        assert_eq!(
            (e.dollars(), e.hours(), e.rate(), e.total_dollars()),
            (4.0, 0.0, 0.0, 4.0)
        );
        assert_eq!(e.shift_id(), Some(7));
    }

    #[test]
    fn an_early_morning_start_is_caught_by_the_window_that_opened_the_night_before() {
        // Shift date is the 6th; 01:00 on the 6th is inside the window that
        // ran 22:00 on the 5th to 06:00 on the 6th.
        let s = shift(at(6, 1, 0), at(6, 9, 0), 8.0);
        assert_eq!(run(&s, night()).earnings().len(), 1);
    }

    #[test]
    fn the_window_bounds_are_inclusive_and_outside_it_pays_nothing() {
        for (h, m, expected) in [(22, 0, 1), (6, 0, 1), (6, 1, 0), (21, 59, 0), (12, 0, 0)] {
            let s = shift(at(6, h, m), at(6, h, m).plus_hours(4), 4.0);
            assert_eq!(run(&s, night()).earnings().len(), expected, "{h}:{m}");
        }
    }

    #[test]
    fn rounded_times_decide_when_actual_punches_are_off() {
        let mut s = shift(at(6, 21, 0), at(7, 5, 0), 8.0);
        s.punch_cursor(0).set_rounded_time(Some(at(6, 22, 0)));
        let mut rounded = night();
        rounded.set(ACTUAL_PUNCH_PROP, "false");
        assert_eq!(run(&s, rounded).earnings().len(), 1);
        let mut actual = night();
        actual.set(ACTUAL_PUNCH_PROP, "true");
        assert!(run(&s, actual).earnings().is_empty());
    }

    #[test]
    fn a_shift_without_both_times_or_with_no_in_punch_pays_nothing() {
        let open = EmployeeShift::new(
            7,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(at(6, 23, 0)), None);
        assert!(run(&open, night()).earnings().is_empty());
        let no_in = EmployeeShift::new(
            7,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(at(6, 23, 0)), Some(at(7, 5, 0)));
        assert!(run(&no_in, night()).earnings().is_empty());
    }

    #[test]
    fn dollars_are_rounded_to_cents() {
        let s = shift(at(6, 23, 0), at(7, 7, 0), 7.33);
        let c = run(&s, night());
        assert_eq!(c.earnings()[0].dollars(), 3.67);
    }

    #[test]
    fn missing_parameters_use_the_default_night_window() {
        let s = shift(at(6, 23, 0), at(7, 7, 0), 8.0);
        let c = run(&s, RuleParams::new());
        assert_eq!(c.earnings().len(), 1);
    }
}
