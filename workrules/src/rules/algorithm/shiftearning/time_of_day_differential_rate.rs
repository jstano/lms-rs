//! Port of `TimeOfDayDifferentialRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/TimeOfDayDifferentialRateRuleImpl.java`.
//!
//! A shift differential for hours worked inside a daily window (default
//! 22:00–06:00), paid at a flat `diffRate` — either per hour or, for a
//! `DOLLARS` type, in dollars.
//!
//! # Building the premium periods
//!
//! The window is built for the shift date and the days either side (it can cross
//! midnight) and **kept only if its start falls on a configured weekday**
//! (`dow`, database numbering). Each kept period is handled on its own and can
//! produce its own earning.
//!
//! # Measuring each period
//!
//! The period is narrowed to the part the shift overlaps — a period the shift
//! never touches is skipped; one the shift contains whole is left alone; a
//! shift inside the period, or overlapping one end, trims the period to it.
//! Then each **break** is subtracted, case by case (inside, swallowing, running
//! off the end, running in from before), rounding each subtraction to two
//! places. Cases are tested in that order and a break matching none is skipped.
//!
//! The result is paid if it is above zero and at least `minHrsBetween`, clamped
//! to `[minAmount, maxAmount]` hours.
//!
//! # The unit decides the shape
//!
//! `HOURS`, `HOURS_DOLLARS`, `UNITS` and `DAYS`: that many hours at `diffRate`.
//! `DOLLARS`: `round(diffRate × hours, 2)` dollars. **Any other unit** writes
//! neither, and an earning with no dollars and no hours is dropped silently.
//!
//! Needs both times, no persisted errors and an open period. A shift with no IN
//! or OUT punch makes Java throw; the port skips it. No Groovy spec; behaviour
//! tests are written from the Java.

use crate::common::dates::{duration_in_fractional_hours, translate_dow_from_iso};
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DIFF_RATE_PROP, DOW_PROP, EARNING, END_TIME_PROP, MAX_HOURS_PAID,
    MIN_HOURS_PAID, MIN_HRS_BETWEEN, START_TIME_PROP, TimeOfDayDifferentialRateRuleConfig,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_window_utility::{
    convert_end_time, convert_start_time,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDateTime, LocalTime};
use std::cell::RefCell;

/// `TimeOfDayDifferentialRateRuleImpl`.
pub struct TimeOfDayDifferentialRateRule<T: EarningTypePort, E: EmployeeEarningPort> {
    earning_types: T,
    earnings: RefCell<E>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort> TimeOfDayDifferentialRateRule<T, E> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
        }
    }
}

/// The part of `period` the shift overlaps, per the Java's `else if` ladder, or
/// `None` when the shift misses it.
fn narrowed_to_shift(
    period: DateTimeRange,
    shift_start: LocalDateTime,
    shift_end: LocalDateTime,
) -> Option<DateTimeRange> {
    let (ps, pe) = (period.start(), period.end());
    if shift_start >= pe || shift_end <= ps {
        return None;
    }
    Some(if shift_start <= ps && shift_end >= pe {
        period
    } else if shift_start > ps && shift_end < pe {
        DateTimeRange::of(shift_start, shift_end)
    } else if shift_end > ps && shift_end < pe {
        DateTimeRange::of(ps, shift_end)
    } else if shift_start > ps && shift_start < pe {
        DateTimeRange::of(shift_start, pe)
    } else {
        period
    })
}

/// One break taken out of `premium_hours`. `None` means "this break does not
/// touch the period" (Java `continue`s).
fn minus_break(premium_hours: f64, period: &DateTimeRange, brk: &DateTimeRange) -> f64 {
    let (ps, pe, bs, be) = (period.start(), period.end(), brk.start(), brk.end());
    if ps <= bs && pe >= be {
        round_to(premium_hours - brk.duration().fractional_hours(), 2)
    } else if ps > bs && pe < be {
        0.0
    } else if pe > bs && pe < be {
        round_to(premium_hours - duration_in_fractional_hours(bs, pe), 2)
    } else if ps > bs && ps < be {
        round_to(premium_hours - duration_in_fractional_hours(ps, be), 2)
    } else {
        premium_hours
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort> ShiftEarningRule
    for TimeOfDayDifferentialRateRule<T, E>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&TimeOfDayDifferentialRateRuleConfig.default_values());
        if !shift_is_eligible(shift, time_card) || !shift.has_both_times() {
            return;
        }

        let diff_rate = params.double_at(DIFF_RATE_PROP);
        let start_time = LocalTime::parse(params.get(START_TIME_PROP).unwrap_or("00:00:00"));
        let end_time = LocalTime::parse(params.get(END_TIME_PROP).unwrap_or("00:00:00"));
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let min_between = params.double_at(MIN_HRS_BETWEEN);
        let min_paid = params.double_at(MIN_HOURS_PAID);
        let max_paid = params.double_at(MAX_HOURS_PAID);
        let dows = ids_for_key(DOW_PROP, &params);

        let (Some(shift_start), Some(shift_end)) = (
            shift.punch_time_of_type(PunchType::In, use_actual),
            shift.punch_time_of_type(PunchType::Out, use_actual),
        ) else {
            return;
        };

        let periods: Vec<DateTimeRange> = [0i64, 1, -1]
            .into_iter()
            .map(|offset| {
                let date = shift.shift_date().plus_days(offset);
                DateTimeRange::of(
                    convert_start_time(date, start_time),
                    convert_end_time(date, start_time, end_time),
                )
            })
            .filter(|p| {
                dows.contains(&translate_dow_from_iso(
                    p.start().to_local_date().day_of_week().value(),
                ))
            })
            .collect();

        let breaks = shift.breaks_using(use_actual);

        for period in periods {
            let Some(period) = narrowed_to_shift(period, shift_start, shift_end) else {
                continue;
            };
            let mut premium_hours = period.duration().fractional_hours();
            for brk in &breaks {
                premium_hours = minus_break(premium_hours, &period, brk);
            }

            if !(premium_hours > 0.0 && premium_hours >= min_between) {
                continue;
            }
            premium_hours = premium_hours.max(min_paid).min(max_paid);

            let earning_type_id = params.int_at(EARNING);
            let Some(earning_type) = self.earning_types.find_by_id(earning_type_id) else {
                continue;
            };
            let mut earning = new_shift_earning(shift, earning_type_id, rule_item, true);
            earning.set_note(rule_item.name());
            match earning_type.uom() {
                UOM::Hours | UOM::HoursDollars | UOM::Units | UOM::Days => {
                    earning.set_dollars(0.0);
                    earning.set_hours(premium_hours);
                    earning.set_rate(diff_rate);
                }
                UOM::Dollars => {
                    earning.set_dollars(round_to(diff_rate * premium_hours, 2));
                    earning.set_hours(0.0);
                    earning.set_rate(0.0);
                }
                UOM::HoursRate => {}
            }
            earning.calc_and_set_total_dollars();

            if earning.dollars() != 0.0 || earning.hours() != 0.0 {
                time_card.earnings_mut().push(earning.clone());
                self.earnings.borrow_mut().save(earning);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    /// 2014-05-06 is a Tuesday: database weekday 3.
    fn base() -> LocalDateTime {
        LocalDate::of(2014, 5, 6).at_start_of_day()
    }
    fn at(hours: i64) -> LocalDateTime {
        base().plus_hours(hours)
    }
    fn p(id: i32, t: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, when)
    }
    /// A shift of IN/OUT hours-from-midnight with optional `(break start, minutes)`.
    fn shift(in_h: i64, out_h: i64, brk: Option<(i64, i64)>) -> EmployeeShift {
        let mut punches = vec![p(1, PunchType::In, at(in_h))];
        if let Some((start, minutes)) = brk {
            punches.push(p(2, PunchType::Break, at(start)));
            punches.push(p(3, PunchType::Back, at(start).plus_minutes(minutes)));
        }
        punches.push(p(4, PunchType::Out, at(out_h)));
        EmployeeShift::new(
            5,
            1,
            9,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            punches,
        )
        .with_times(Some(at(in_h)), Some(at(out_h)))
    }
    fn run(s: &EmployeeShift, uom: UOM, extra: &[(&str, &str)]) -> TimeCardData {
        let mut params = rule_params! {
            DIFF_RATE_PROP => "2", EARNING => "3", START_TIME_PROP => "22:00:00",
            END_TIME_PROP => "06:00:00", ACTUAL_PUNCH_PROP => "false"
        };
        for (k, v) in extra {
            params.set(*k, *v);
        }
        let mut c = TimeCardData::new().with_shifts(vec![s.clone()]);
        // `rounded_time` is what actualPunchTime=false reads, and `new` sets it.
        TimeOfDayDifferentialRateRule::new(
            FixedEarningType(earning_type(uom)),
            SavedEarnings::default(),
        )
        .execute(
            s,
            &mut c,
            &RuleItem::new(2, 1, "TOD", RuleClass::TimeOfDayDifRateDif, params),
        );
        c
    }

    #[test]
    fn a_shift_inside_the_window_is_paid_for_its_whole_length() {
        let c = run(&shift(23, 28, None), UOM::Hours, &[]);
        let e = &c.earnings()[0];
        assert_eq!(
            (e.hours(), e.rate(), e.dollars(), e.total_dollars()),
            (5.0, 2.0, 0.0, 10.0)
        );
        assert_eq!(e.shift_id(), Some(5));
    }

    #[test]
    fn only_the_part_inside_the_window_is_paid() {
        // 20:00-24:00 overlaps 22:00-06:00 for two hours.
        assert_eq!(
            run(&shift(20, 24, None), UOM::Hours, &[]).earnings()[0].hours(),
            2.0
        );
        // 04:00-10:00 overlaps the previous night's window for two hours.
        assert_eq!(
            run(&shift(4, 10, None), UOM::Hours, &[]).earnings()[0].hours(),
            2.0
        );
    }

    #[test]
    fn a_shift_outside_the_window_is_paid_nothing() {
        assert!(
            run(&shift(8, 16, None), UOM::Hours, &[])
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn a_dollars_type_pays_the_rate_times_the_hours_in_dollars() {
        let c = run(&shift(23, 28, None), UOM::Dollars, &[]);
        let e = &c.earnings()[0];
        assert_eq!((e.dollars(), e.hours(), e.rate()), (10.0, 0.0, 0.0));
    }

    #[test]
    fn units_days_and_hours_dollars_pay_hours_at_the_rate_but_hours_rate_pays_nothing() {
        for uom in [UOM::Units, UOM::Days, UOM::HoursDollars] {
            assert_eq!(
                run(&shift(23, 28, None), uom, &[]).earnings().len(),
                1,
                "{uom:?}"
            );
        }
        assert!(
            run(&shift(23, 28, None), UOM::HoursRate, &[])
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn a_break_inside_the_period_is_subtracted() {
        // 22:00-06:00 fully worked (8h) with a 1h break inside: 7h.
        let c = run(&shift(22, 30, Some((26, 60))), UOM::Hours, &[]);
        assert_eq!(c.earnings()[0].hours(), 7.0);
    }

    #[test]
    fn a_break_running_off_the_end_of_the_period_is_clipped() {
        // Shift 22:00-08:00, break 05:00-07:00: only 05:00-06:00 is in the window.
        let c = run(&shift(22, 32, Some((29, 120))), UOM::Hours, &[]);
        // premium period 22:00-06:00 = 8h; break overlaps its last hour.
        assert_eq!(c.earnings()[0].hours(), 7.0);
    }

    #[test]
    fn a_break_running_in_from_before_the_period_is_clipped() {
        // Shift 20:00-30:00, break 21:00-23:00: the window starts at 22:00.
        let c = run(&shift(20, 30, Some((21, 120))), UOM::Hours, &[]);
        assert_eq!(c.earnings()[0].hours(), 7.0);
    }

    #[test]
    fn a_break_swallowing_the_whole_period_leaves_nothing() {
        // Window 01:00-02:00; the shift 22:00-06:00 takes a break 00:00-04:00
        // that covers it, so the premium is zeroed and nothing is paid.
        let s = shift(22, 30, Some((24, 240)));
        let c = run(
            &s,
            UOM::Hours,
            &[(START_TIME_PROP, "01:00:00"), (END_TIME_PROP, "02:00:00")],
        );
        assert!(c.earnings().is_empty());
        // Without the break the same window pays its full hour.
        let c = run(
            &shift(22, 30, None),
            UOM::Hours,
            &[(START_TIME_PROP, "01:00:00"), (END_TIME_PROP, "02:00:00")],
        );
        assert_eq!(c.earnings()[0].hours(), 1.0);
    }

    #[test]
    fn minimum_and_maximum_hours_clamp_the_paid_hours() {
        let short = run(&shift(23, 24, None), UOM::Hours, &[(MIN_HOURS_PAID, "3")]);
        assert_eq!(short.earnings()[0].hours(), 3.0);
        let long = run(&shift(22, 30, None), UOM::Hours, &[(MAX_HOURS_PAID, "6")]);
        assert_eq!(long.earnings()[0].hours(), 6.0);
    }

    #[test]
    fn hours_under_min_hrs_between_are_not_paid() {
        assert!(
            run(
                &shift(23, 24, None),
                UOM::Hours,
                &[(MIN_HRS_BETWEEN, "1.5")]
            )
            .earnings()
            .is_empty()
        );
        assert_eq!(
            run(
                &shift(23, 25, None),
                UOM::Hours,
                &[(MIN_HRS_BETWEEN, "1.5")]
            )
            .earnings()
            .len(),
            1
        );
    }

    #[test]
    fn a_period_whose_start_is_not_on_a_configured_weekday_is_dropped() {
        // Window starting Tuesday (3) only: the shift 23:00 Tue - 05:00 Wed matches...
        assert_eq!(
            run(&shift(23, 29, None), UOM::Hours, &[(DOW_PROP, "[3]")])
                .earnings()
                .len(),
            1
        );
        // ...but with only Wednesday (4), the Tuesday-night window is gone.
        assert!(
            run(&shift(23, 29, None), UOM::Hours, &[(DOW_PROP, "[4]")])
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn several_periods_can_each_earn() {
        // 04:00 Tue to 23:00 Tue touches Monday night's window (04:00-06:00) and
        // Tuesday night's (22:00-23:00).
        let c = run(&shift(4, 23, None), UOM::Hours, &[]);
        let hours: Vec<f64> = c.earnings().iter().map(|e| e.hours()).collect();
        assert_eq!(
            hours,
            vec![1.0, 2.0],
            "the shift date's window first, then the previous day's"
        );
    }

    #[test]
    fn shifts_with_errors_or_missing_times_are_skipped() {
        let bad = shift(23, 28, None).with_errors(vec![ShiftErrorType::MissingOut]);
        assert!(run(&bad, UOM::Hours, &[]).earnings().is_empty());
        let open = shift(23, 28, None).with_times(Some(at(23)), None);
        assert!(run(&open, UOM::Hours, &[]).earnings().is_empty());
    }
}
