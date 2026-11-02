//! Port of `TimeOfDayPctDifferentialRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/TimeOfDayPctDifferentialRateRuleImpl.java`.
//!
//! The most configurable shift differential, and the family's hardest rule: a
//! premium for hours worked in a daily window, as a **percentage of the job
//! rate plus a flat differential**, with start-time gating, hour and
//! percent-of-shift thresholds, rounding, a cap, and a per-day split.
//!
//! # Gating
//!
//! The shift needs no errors, both times and an open period, **and its first IN
//! must fall inside the `earliestStartTime`–`latestStartTime` window** (built,
//! like the premium window, for the shift date and the days either side; kept
//! only where the window's start day is a configured weekday; inclusive ends).
//!
//! # Per premium period
//!
//! The premium windows (`Start Time`–`End Time`, same construction) are each
//! tried. For one:
//!
//! 1. **Premium seconds** — the overlap of the shift with the window.
//! 2. It must be above zero, at least `minHrsBetween` hours, and its share of
//!    the shift's hours must be non-zero, at least `pctHrsBetween` and either at
//!    most `maxPctHrsBetween` or the max must be 1 or more.
//! 3. **Paid hours** — with `payNetHrs`, the shift's whole net hours; otherwise
//!    the premium seconds less the breaks that overlap them — rounded by
//!    `roundingOption` to a multiple of `roundTo` minutes, then to two places,
//!    and capped at `maxHrsPaid`.
//!
//! # Which date it lands on
//!
//! With hours distributed to **one** date: the shift's date. With more than one:
//!
//! - `payNetHrs`: one earning per distribution date, each the share
//!   `round_percent(date hours ÷ net hours)` of the paid hours;
//! - otherwise the premium window is cut at the pay group's work-day start: if
//!   the cut falls inside the window (the shift date's, then the next day's)
//!   there is an earning for the part before it, dated the shift date, and one
//!   for the rest, dated the next day; if neither cut falls inside, a single
//!   earning on the day the window started.
//!
//! # Pricing
//!
//! Each earning is priced for **its own date**: the job's rate on that date
//! (floored at minimum wage if the type says so) × `pctWage`, then + `diffRate`,
//! rounded to four places at each step. `DOLLARS` types get `rate × hours` as
//! dollars; others get the hours and the rate. An earning with neither is
//! dropped.
//!
//! Java iterates a `HashMap` for the `payNetHrs` split; the port goes in date
//! order (divergence 106).

use crate::common::dates::{duration_in_seconds, translate_dow_from_iso};
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::{round_currency, round_hours, round_percent, round_raw_hours};
use crate::common::rounding::RoundingOption;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DIFF_RATE_PROP, DOW_PROP, EARLY_START_TIME_PROP, EARNING, END_TIME_PROP,
    LATE_START_TIME_PROP, MAX_HRS_PAID, MAX_PCT_HRS_BETWEEN, MIN_HRS_BETWEEN, MIN_PCT_HRS_BETWEEN,
    PAY_NET_HRS, PCT_WAGE_PROP, ROUND_TO, ROUNDING_OPTION, START_TIME_PROP,
    TimeOfDayPctDifferentialRateRuleConfig,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_window_utility::time_ranges_including_overlaps;
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime, LocalTime};
use std::cell::RefCell;
use std::collections::BTreeMap;

const SECONDS_PER_HOUR: f64 = 3600.0;
const SECONDS_PER_MINUTE: i32 = 60;

/// `TimeOfDayPctDifferentialRateRuleImpl`.
pub struct TimeOfDayPctDifferentialRateRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    TimeOfDayPctDifferentialRateRule<T, E, M>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }
}

/// `getPremiumPeriods`: the windows for the shift date and its neighbours whose
/// start falls on a configured weekday.
fn premium_periods(
    shift_date: LocalDate,
    start: LocalTime,
    end: LocalTime,
    params: &RuleParams,
) -> Vec<DateTimeRange> {
    let dows = ids_for_key(DOW_PROP, params);
    time_ranges_including_overlaps(shift_date, start, end)
        .into_iter()
        .filter(|p| {
            dows.contains(&translate_dow_from_iso(
                p.start().to_local_date().day_of_week().value(),
            ))
        })
        .collect()
}

/// `getPremiumSecondsFromPeriod`.
fn premium_seconds_from_period(
    shift: &EmployeeShift,
    period: &DateTimeRange,
    use_actual: bool,
) -> i32 {
    let (Some(shift_start), Some(shift_end)) = (
        shift.punch_time_of_type(PunchType::In, use_actual),
        shift.punch_time_of_type(PunchType::Out, use_actual),
    ) else {
        return 0;
    };
    let (ps, pe) = (period.start(), period.end());
    if shift_start >= pe || shift_end <= ps {
        return 0;
    }
    let (start, end) = if shift_start > ps && shift_end < pe {
        (shift_start, shift_end)
    } else if shift_end > ps && shift_end < pe {
        (ps, shift_end)
    } else if shift_start > ps && shift_start < pe {
        (shift_start, pe)
    } else {
        (ps, pe)
    };
    duration_in_seconds(start, end)
}

/// `getPremiumSecondsMinusBreakTimeForPeriod`.
fn premium_seconds_minus_breaks(
    shift: &EmployeeShift,
    period: &DateTimeRange,
    use_actual: bool,
) -> i32 {
    let mut seconds = premium_seconds_from_period(shift, period, use_actual);
    let (ps, pe) = (period.start(), period.end());
    for brk in shift.breaks_using(use_actual) {
        let (bs, be) = (brk.start(), brk.end());
        if ps <= bs && pe >= be {
            seconds -= duration_in_seconds(bs, be);
        } else if ps > bs && pe < be {
            seconds = 0;
        } else if pe > bs && pe < be {
            seconds -= duration_in_seconds(bs, pe);
        } else if ps > bs && ps < be {
            seconds -= duration_in_seconds(ps, be);
        }
    }
    seconds
}

/// `roundHoursBasedOnOptions`.
fn round_hours_based_on_options(seconds: f64, params: &RuleParams) -> f64 {
    let option = RoundingOption::from_code(params.get(ROUNDING_OPTION).unwrap_or("NONE"))
        .unwrap_or(RoundingOption::None);
    let round_to = params.int_at(ROUND_TO) * SECONDS_PER_MINUTE;
    let rounded = option.round(seconds as i32, round_to);
    round_hours(f64::from(rounded) / SECONDS_PER_HOUR)
}

/// `getPaidHours`.
fn paid_hours(
    shift: &EmployeeShift,
    period: &DateTimeRange,
    params: &RuleParams,
    shift_hours: f64,
) -> f64 {
    let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
    let seconds = if params.bool_at(PAY_NET_HRS) {
        shift_hours * SECONDS_PER_HOUR
    } else {
        f64::from(premium_seconds_minus_breaks(shift, period, use_actual))
    };
    round_hours_based_on_options(seconds, params).min(params.double_at(MAX_HRS_PAID))
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    TimeOfDayPctDifferentialRateRule<T, E, M>
{
    #[allow(clippy::too_many_arguments)]
    fn create_and_save(
        &self,
        shift: &EmployeeShift,
        earning_date: LocalDate,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        hours: f64,
        params: &RuleParams,
    ) {
        let earning_type_id = params.int_at(EARNING);
        let Some(earning_type) = self.earning_types.find_by_id(earning_type_id) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(base) = earning_rate_for_job(
            employee,
            shift.job_id(),
            earning_date,
            &earning_type,
            &self.min_wage,
        ) else {
            return;
        };
        let rate = round_currency(
            round_currency(base * params.double_at(PCT_WAGE_PROP))
                + params.double_at(DIFF_RATE_PROP),
        );

        let mut earning = new_shift_earning(shift, earning_type_id, rule_item, true);
        earning.set_earning_date_and_pay_date(earning_date);
        earning.set_note(rule_item.name());
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(round_currency(rate * hours));
            earning.set_hours(0.0);
            earning.set_rate(0.0);
        } else {
            earning.set_dollars(0.0);
            earning.set_hours(hours);
            earning.set_rate(rate);
        }
        earning.calc_and_set_total_dollars();

        if earning.dollars() != 0.0 || earning.hours() != 0.0 {
            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_earnings_by_day_cut(
        &self,
        shift: &EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        params: &RuleParams,
        premium_period: &DateTimeRange,
        shift_hours: f64,
        paid: f64,
        day_cut: LocalDateTime,
    ) {
        let first_period = DateTimeRange::of(premium_period.start(), day_cut);
        let paid_first = paid_hours(shift, &first_period, params, shift_hours);
        if paid_first > 0.0 {
            self.create_and_save(
                shift,
                shift.shift_date(),
                time_card,
                rule_item,
                paid_first,
                params,
            );
        }
        let paid_second = round_raw_hours(paid - paid_first.max(0.0));
        if paid_second > 0.0 {
            self.create_and_save(
                shift,
                shift.shift_date().plus_days(1),
                time_card,
                rule_item,
                paid_second,
                params,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_multiple_distribution_days(
        &self,
        shift: &EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        params: &RuleParams,
        premium_period: &DateTimeRange,
        shift_hours: f64,
        paid: f64,
    ) {
        if params.bool_at(PAY_NET_HRS) {
            let mut hours_by_date: BTreeMap<LocalDate, f64> = BTreeMap::new();
            for d in shift.hours_distributions() {
                *hours_by_date.entry(d.date()).or_insert(0.0) += d.hours();
            }
            for (date, hours) in hours_by_date {
                let percent = round_percent(hours / shift.net_hours());
                let paid_for_date = round_raw_hours(percent * paid);
                if paid_for_date > 0.0 {
                    self.create_and_save(shift, date, time_card, rule_item, paid_for_date, params);
                }
            }
            return;
        }

        let work_day_start = time_card.work_day_start_time();
        let shift_day_cut = shift.shift_date().at_time(work_day_start);
        let next_day_cut = shift.shift_date().plus_days(1).at_time(work_day_start);
        if premium_period.contains_exclusive(shift_day_cut) {
            self.create_earnings_by_day_cut(
                shift,
                time_card,
                rule_item,
                params,
                premium_period,
                shift_hours,
                paid,
                shift_day_cut,
            );
        } else if premium_period.contains_exclusive(next_day_cut) {
            self.create_earnings_by_day_cut(
                shift,
                time_card,
                rule_item,
                params,
                premium_period,
                shift_hours,
                paid,
                next_day_cut,
            );
        } else {
            let premium_day_cut = premium_period
                .start()
                .to_local_date()
                .at_time(work_day_start);
            let pay_date = if premium_period.start() >= premium_day_cut {
                premium_day_cut.to_local_date()
            } else {
                premium_day_cut.to_local_date().minus_days(1)
            };
            self.create_and_save(shift, pay_date, time_card, rule_item, paid, params);
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftEarningRule
    for TimeOfDayPctDifferentialRateRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&TimeOfDayPctDifferentialRateRuleConfig.default_values());
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        if !(shift_is_eligible(shift, time_card) && shift.has_both_times()) {
            return;
        }

        // The start-time gate.
        let time = |key: &str| LocalTime::parse(params.get(key).unwrap_or("00:00:00"));
        let start_periods = premium_periods(
            shift.shift_date(),
            time(EARLY_START_TIME_PROP),
            time(LATE_START_TIME_PROP),
            &params,
        );
        let starts_in_window = shift
            .punch_time_of_type(PunchType::In, use_actual)
            .is_some_and(|t| start_periods.iter().any(|p| p.contains(t)));
        if !starts_in_window {
            return;
        }

        for premium_period in premium_periods(
            shift.shift_date(),
            time(START_TIME_PROP),
            time(END_TIME_PROP),
            &params,
        ) {
            let shift_hours = shift.net_hours_with_reallocated_undone(use_actual);
            let premium_seconds = premium_seconds_from_period(shift, &premium_period, use_actual);
            let premium_hours = f64::from(premium_seconds) / SECONDS_PER_HOUR;

            let percent = if shift_hours > 0.0 {
                premium_hours / shift_hours
            } else {
                0.0
            };
            let (min_pct, max_pct) = (
                params.double_at(MIN_PCT_HRS_BETWEEN),
                params.double_at(MAX_PCT_HRS_BETWEEN),
            );
            let enough_hours = premium_hours >= params.double_at(MIN_HRS_BETWEEN);
            let valid_percent =
                percent != 0.0 && percent >= min_pct && (percent <= max_pct || max_pct >= 1.0);
            if !(premium_hours > 0.0 && enough_hours && valid_percent) {
                continue;
            }

            let paid = paid_hours(shift, &premium_period, &params, shift_hours);
            if paid <= 0.0 {
                continue;
            }
            if shift.dates_with_hours_distributions().len() > 1 {
                self.handle_multiple_distribution_days(
                    shift,
                    time_card,
                    rule_item,
                    &params,
                    &premium_period,
                    shift_hours,
                    paid,
                );
            } else {
                self.create_and_save(
                    shift,
                    shift.shift_date(),
                    time_card,
                    rule_item,
                    paid,
                    &params,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;

    const JOB: i32 = 1;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }
    fn midnight() -> LocalDateTime {
        today().at_start_of_day()
    }
    fn punch(id: i32, t: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, when)
    }
    fn dist(date: LocalDate, hours: f64) -> HoursDistribution {
        let mut d = HoursDistribution::new(1, date, Some(1), hours, 0.0);
        d.set_original_hours(hours);
        d
    }
    fn hours_between(a: LocalDateTime, b: LocalDateTime) -> f64 {
        crate::common::dates::duration_in_fractional_hours(a, b)
    }

    /// `makeShift(start, end, breakHour, breakDuration)`: offsets are hours (and
    /// minutes for the break) from midnight; the IN punch is a minute after the
    /// shift's start.
    fn make_shift(start: i64, end: i64, break_hour: i64, break_minutes: i64) -> EmployeeShift {
        let start_dt = midnight().plus_hours(start);
        let end_dt = midnight().plus_hours(end);
        let in_punch = start_dt.plus_minutes(1);
        let break_punch = midnight().plus_hours(break_hour);
        let back_punch = break_punch.plus_minutes(break_minutes);
        let net = hours_between(start_dt, end_dt) - hours_between(break_punch, back_punch);
        let punches = if break_minutes > 0 {
            vec![
                punch(1, PunchType::In, in_punch),
                punch(2, PunchType::Break, break_punch),
                punch(3, PunchType::Back, back_punch),
                punch(4, PunchType::Out, end_dt),
            ]
        } else {
            vec![
                punch(1, PunchType::In, in_punch),
                punch(4, PunchType::Out, end_dt),
            ]
        };
        EmployeeShift::new(9, 1, JOB, today(), ShiftType::Actual, punches)
            .with_times(Some(start_dt), Some(end_dt))
            .with_net_hours(net)
            .with_hours_distributions(vec![dist(today(), net)])
    }

    fn make_shift_with_fractional_hours(sh: i64, sm: i64, eh: i64, em: i64) -> EmployeeShift {
        let start_dt = midnight().plus_hours(sh).plus_minutes(sm);
        let end_dt = midnight().plus_hours(eh).plus_minutes(em);
        let net = hours_between(start_dt, end_dt);
        EmployeeShift::new(
            9,
            1,
            JOB,
            today(),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, start_dt),
                punch(2, PunchType::Out, end_dt),
            ],
        )
        .with_times(Some(start_dt), Some(end_dt))
        .with_net_hours(net)
        .with_hours_distributions(vec![dist(today(), net)])
    }

    fn make_shift_with_distributions_on_multiple_days() -> EmployeeShift {
        let start_dt = midnight().plus_hours(19);
        let end_dt = midnight().plus_hours(28);
        let break_punch = midnight().plus_hours(23).plus_minutes(30);
        let back_punch = break_punch.plus_minutes(60);
        let net = hours_between(start_dt, end_dt) - hours_between(break_punch, back_punch);
        EmployeeShift::new(
            9,
            1,
            JOB,
            today(),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, start_dt),
                punch(2, PunchType::Break, break_punch),
                punch(3, PunchType::Back, back_punch),
                punch(4, PunchType::Out, end_dt),
            ],
        )
        .with_times(Some(start_dt), Some(end_dt))
        .with_net_hours(net)
        .with_hours_distributions(vec![dist(today(), 4.5), dist(today().plus_days(1), 3.5)])
    }

    /// `makeRuleItem(option, payNet, minTimeBetween, minPct, maxPct, maxHrs)`.
    fn make_rule_item(
        option: RoundingOption,
        pay_net: &str,
        min_between: &str,
        min_pct: &str,
        max_pct: &str,
        max_hrs: &str,
    ) -> RuleItem {
        let mut p = TimeOfDayPctDifferentialRateRuleConfig.default_values();
        p.set(ROUNDING_OPTION, option.code());
        p.set(PAY_NET_HRS, pay_net);
        p.set(MIN_HRS_BETWEEN, min_between);
        p.set(MIN_PCT_HRS_BETWEEN, min_pct);
        p.set(MAX_PCT_HRS_BETWEEN, max_pct);
        p.set(MAX_HRS_PAID, max_hrs);
        RuleItem::new(1, 1, "Rule Name", RuleClass::TodPctDifRateDif, p)
    }
    fn default_item(option: RoundingOption) -> RuleItem {
        make_rule_item(option, "false", "0", "0", "1", "99")
    }
    fn with_params(item: RuleItem, extra: &[(&str, &str)]) -> RuleItem {
        let mut p = item.params().clone();
        for (k, v) in extra {
            p.set(*k, *v);
        }
        RuleItem::new(item.id(), 1, item.name(), item.rule_class(), p)
    }

    /// `makeDataSet`: rate 1 up to and including today, rate 2 from tomorrow;
    /// the pay period (and so the calculation start) four days back.
    fn data_set() -> TimeCardData {
        let status = |id, rate, from: LocalDate, to: LocalDate| {
            EmployeeJobStatus::new(id, 1, JOB, from, to, EmployeePayType::Hourly, rate, true)
        };
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![
                status(1, 1.0, today().minus_years(1), today()),
                status(2, 2.0, today().plus_days(1), LocalDate::of(9999, 12, 31)),
            ],
        );
        TimeCardData::new()
            .with_employee(employee)
            .with_calculation_start_date(today().minus_days(4))
            .with_work_day_start_time(LocalTime::of(0, 0, 0))
    }

    fn run(
        shift: &EmployeeShift,
        card: &mut TimeCardData,
        uom: UOM,
        item: &RuleItem,
    ) -> SavedEarnings {
        let saved = SavedEarnings::default();
        TimeOfDayPctDifferentialRateRule::new(
            FixedEarningType(earning_type(uom)),
            saved.clone(),
            FixedMinWage(0.0),
        )
        .execute(shift, card, item);
        saved
    }

    fn earnings_for(shift: &EmployeeShift, uom: UOM, item: &RuleItem) -> TimeCardData {
        let mut card = data_set();
        run(shift, &mut card, uom, item);
        card
    }

    // ---- java_parity_tests: TimeOfDayPctDifferentialRateRuleImplTest.groovy ----

    #[test]
    fn total_fractional_hours_rounding_happens_correctly() {
        for (option, expected) in [
            (RoundingOption::None, 2.22),
            (RoundingOption::Nearest, 2.25),
            (RoundingOption::Up, 2.25),
            (RoundingOption::Down, 2.17),
        ] {
            let card = earnings_for(
                &make_shift(17, 25, 22, 47),
                UOM::Hours,
                &default_item(option),
            );
            assert_eq!(card.earnings().len(), 1, "{option:?}");
            assert_eq!(card.earnings()[0].hours(), expected, "{option:?}");
        }
    }

    #[test]
    fn hours_rounding_does_not_happen_when_it_shouldnt_for_down_rounding_watson_11507() {
        let rows = [
            (0, 1.0),
            (5, 1.08),
            (10, 1.17),
            (15, 1.25),
            (20, 1.33),
            (25, 1.42),
            (30, 1.5),
            (35, 1.58),
            (40, 1.67),
            (45, 1.75),
            (50, 1.83),
            (55, 1.92),
        ];
        for (out_minutes, expected) in rows {
            let item = with_params(
                default_item(RoundingOption::Down),
                &[
                    (END_TIME_PROP, "05:00:00"),
                    (MIN_HRS_BETWEEN, "0.01"),
                    (MIN_PCT_HRS_BETWEEN, "0.01"),
                    (PCT_WAGE_PROP, "0.25"),
                ],
            );
            let card = earnings_for(
                &make_shift_with_fractional_hours(13, 30, 23, out_minutes),
                UOM::Hours,
                &item,
            );
            assert_eq!(card.earnings().len(), 1, "23:{out_minutes:02}");
            assert_eq!(card.earnings()[0].hours(), expected, "23:{out_minutes:02}");
        }
    }

    #[test]
    fn shift_with_no_breaks_generates_earning_with_correct_values() {
        // (uom, start, end, hours, rate, dollars)
        let rows = [
            (UOM::Hours, 22, 25, 2.98, 1.0, 0.0),
            (UOM::Hours, 20, 28, 6.0, 1.0, 0.0),
            (UOM::Hours, 25, 33, 4.98, 1.0, 0.0),
            (UOM::Dollars, 22, 25, 0.0, 0.0, 2.98),
            (UOM::Dollars, 20, 28, 0.0, 0.0, 6.0),
            (UOM::Dollars, 25, 33, 0.0, 0.0, 4.98),
        ];
        for (uom, start, end, hours, rate, dollars) in rows {
            let card = earnings_for(
                &make_shift(start, end, 0, 0),
                uom,
                &default_item(RoundingOption::None),
            );
            assert_eq!(card.earnings().len(), 1, "{uom:?} {start}-{end}");
            let e = &card.earnings()[0];
            assert_eq!(
                (e.hours(), e.rate(), e.dollars()),
                (hours, rate, dollars),
                "{uom:?} {start}-{end}"
            );
        }
    }

    #[test]
    fn shift_with_breaks_generates_earning_with_correct_values() {
        // (break hour, duration, hours, scenario)
        let rows = [
            (27, 47, 7.22, "premium contains break period fully"),
            (
                29,
                120,
                7.0,
                "break starts during premium period and ends after premiumPeriod",
            ),
            (
                21,
                180,
                6.0,
                "break starts before premium period and ends during the premium period",
            ),
        ];
        for (break_hour, duration, hours, scenario) in rows {
            let card = earnings_for(
                &make_shift(20, 33, break_hour, duration),
                UOM::Hours,
                &default_item(RoundingOption::None),
            );
            assert_eq!(card.earnings().len(), 1, "{scenario}");
            let e = &card.earnings()[0];
            assert_eq!(
                (e.hours(), e.rate(), e.dollars()),
                (hours, 1.0, 0.0),
                "{scenario}"
            );
        }
    }

    #[test]
    fn shift_with_break_larger_than_premium_period_does_not_generate_an_earning() {
        let card = earnings_for(
            &make_shift(20, 30, 21, 541),
            UOM::Hours,
            &default_item(RoundingOption::None),
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn shift_with_pay_net_generates_earning_with_correct_values() {
        let rows = [
            (UOM::Dollars, 25, 33, 0.0, 0.0, 7.98),
            (UOM::Hours, 20, 28, 7.98, 1.0, 0.0),
        ];
        for (uom, start, end, hours, rate, dollars) in rows {
            let item = make_rule_item(RoundingOption::None, "true", "0", "0", "1", "99");
            let card = earnings_for(&make_shift(start, end, 0, 0), uom, &item);
            assert_eq!(card.earnings().len(), 1, "{uom:?}");
            let e = &card.earnings()[0];
            assert_eq!(
                (e.hours(), e.rate(), e.dollars()),
                (hours, rate, dollars),
                "{uom:?}"
            );
        }
    }

    #[test]
    fn shift_with_max_hours_set_does_not_create_an_earning_with_more_than_max_hrs() {
        let item = make_rule_item(RoundingOption::None, "true", "0", "0", "1", "5");
        let card = earnings_for(&make_shift(20, 28, 0, 0), UOM::Hours, &item);
        assert_eq!(card.earnings().len(), 1);
        let e = &card.earnings()[0];
        assert_eq!((e.hours(), e.rate(), e.dollars()), (5.0, 1.0, 0.0));
    }

    #[test]
    fn there_are_no_earnings_if_there_are_no_premium_hours() {
        let card = earnings_for(
            &make_shift(8, 17, 0, 0),
            UOM::Hours,
            &default_item(RoundingOption::None),
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn shift_not_in_premium_period_causes_no_earning_to_be_made() {
        let card = earnings_for(
            &make_shift(-36, -28, 0, 0),
            UOM::Hours,
            &default_item(RoundingOption::None),
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn there_are_no_earnings_if_the_shift_is_invalid() {
        // (hasErrors, hasBothTimes, shiftDateBeforeCalcDate)
        for (has_errors, has_both_times, before_calc_date) in [
            (false, false, false),
            (true, true, false),
            (false, true, true),
        ] {
            let mut shift = make_shift(0, 0, 0, 0);
            if has_errors {
                shift = shift.with_errors(vec![ShiftErrorType::MissingOut]);
            }
            if !has_both_times {
                shift = shift.with_times(None, Some(midnight()));
            }
            if before_calc_date {
                shift.set_shift_date(today().minus_weeks(1));
            }
            let card = earnings_for(&shift, UOM::Hours, &default_item(RoundingOption::None));
            assert!(
                card.earnings().is_empty(),
                "{has_errors} {has_both_times} {before_calc_date}"
            );
        }
    }

    #[test]
    fn hours_not_meeting_min_time_between_min_pct_or_max_pct_dont_generate_earnings() {
        for (min_between, min_pct, max_pct) in
            [("0", ".63", "1"), ("0", "0", ".6"), ("5", "0", "1")]
        {
            let item = make_rule_item(
                RoundingOption::None,
                "false",
                min_between,
                min_pct,
                max_pct,
                "99",
            );
            let card = earnings_for(&make_shift(25, 33, 0, 0), UOM::Hours, &item);
            assert!(
                card.earnings().is_empty(),
                "{min_between} {min_pct} {max_pct}"
            );
        }
    }

    #[test]
    fn shift_with_multiple_distributions_create_multiple_earnings() {
        // (payNet, hours1, rate1, hours2, rate2)
        for (pay_net, one_hours, one_rate, two_hours, two_rate) in
            [("true", 4.5, 1.0, 3.5, 2.0), ("false", 1.5, 1.0, 3.5, 2.0)]
        {
            let item = with_params(
                default_item(RoundingOption::None),
                &[(PAY_NET_HRS, pay_net)],
            );
            let card = earnings_for(
                &make_shift_with_distributions_on_multiple_days(),
                UOM::Hours,
                &item,
            );
            assert_eq!(card.earnings().len(), 2, "payNet {pay_net}");
            let first = card
                .earnings()
                .iter()
                .find(|e| e.earning_date() == today())
                .unwrap();
            let second = card
                .earnings()
                .iter()
                .find(|e| e.earning_date() == today().plus_days(1))
                .unwrap();
            assert_eq!(
                (first.hours(), first.rate(), first.dollars()),
                (one_hours, one_rate, 0.0),
                "payNet {pay_net}"
            );
            assert_eq!(
                (second.hours(), second.rate(), second.dollars()),
                (two_hours, two_rate, 0.0),
                "payNet {pay_net}"
            );
        }
    }

    #[test]
    fn shift_with_multiple_distributions_that_pays_a_single_earning_puts_it_on_the_correct_date() {
        // (start, end, hours, rate, date)
        let rows = [
            ("22:00:00", "23:00:00", 1.0, 1.0, today()),
            ("01:00:00", "02:30:00", 1.5, 2.0, today().plus_days(1)),
        ];
        for (start, end, hours, rate, date) in rows {
            let item = with_params(
                default_item(RoundingOption::None),
                &[
                    (PAY_NET_HRS, "false"),
                    (START_TIME_PROP, start),
                    (END_TIME_PROP, end),
                ],
            );
            let card = earnings_for(
                &make_shift_with_distributions_on_multiple_days(),
                UOM::Hours,
                &item,
            );
            assert_eq!(card.earnings().len(), 1, "{start}-{end}");
            let e = &card.earnings()[0];
            assert_eq!(
                (e.hours(), e.rate(), e.dollars(), e.earning_date()),
                (hours, rate, 0.0, date),
                "{start}-{end}"
            );
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_start_outside_the_start_window_is_not_paid() {
        // Shift starts 22:01, so a start window ending 22:00 excludes it.
        let item = with_params(
            default_item(RoundingOption::None),
            &[(LATE_START_TIME_PROP, "22:00:00")],
        );
        assert!(
            earnings_for(&make_shift(22, 25, 0, 0), UOM::Hours, &item)
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn the_flat_differential_and_percentage_both_price_the_rate() {
        // rate 1 x 1.5 + 0.25 = 1.75
        let item = with_params(
            default_item(RoundingOption::None),
            &[(PCT_WAGE_PROP, "1.5"), (DIFF_RATE_PROP, "0.25")],
        );
        let card = earnings_for(&make_shift(23, 25, 0, 0), UOM::Hours, &item);
        assert_eq!(card.earnings()[0].rate(), 1.75);
    }

    #[test]
    fn a_configured_weekday_list_drops_windows_starting_on_other_days() {
        // Shift 23:00 Tue - 01:00 Wed: only Tuesday (3)'s window overlaps.
        let item = with_params(default_item(RoundingOption::None), &[(DOW_PROP, "[4]")]);
        assert!(
            earnings_for(&make_shift(23, 25, 0, 0), UOM::Hours, &item)
                .earnings()
                .is_empty()
        );
        let item = with_params(default_item(RoundingOption::None), &[(DOW_PROP, "[3]")]);
        assert_eq!(
            earnings_for(&make_shift(23, 25, 0, 0), UOM::Hours, &item)
                .earnings()
                .len(),
            1
        );
    }

    #[test]
    fn an_unknown_rounding_code_is_treated_as_none() {
        let item = with_params(
            default_item(RoundingOption::None),
            &[(ROUNDING_OPTION, "BOGUS")],
        );
        let card = earnings_for(&make_shift(17, 25, 22, 47), UOM::Hours, &item);
        assert_eq!(card.earnings()[0].hours(), 2.22);
    }
}
