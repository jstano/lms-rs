//! Port of `CaliforniaExtendedOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/CaliforniaExtendedOTHrsRuleImpl.java`.
//!
//! `CAL_EXT_HDR`. California overtime in full: a daily limit, a daily
//! double-time limit, a weekly limit, and a consecutive-days rule that
//! suspends the daily limits once the employee has worked too many days in a
//! row. The first rule to use both
//! [`DailyAccumulator`](super::daily_accumulator) and
//! [`WeeklyAccumulator`](super::weekly_accumulator), and the first to prime a
//! week from [`prior_days_calculator`](super::prior_days_calculator).
//!
//! # The shape
//!
//! For each date of the work week, in order:
//!
//! 1. a fresh [`DailyAccumulator`](super::daily_accumulator::DailyAccumulator);
//! 2. the non-exempt shifts with a distribution on that date, advancing the
//!    weekly consecutive-day counter (the wrapping overload);
//! 3. each of their **non-premium** distributions dated that day, in turn:
//!    add its original hours to both accumulators, ask for the premium split,
//!    write the rows, then add the result back to the accumulators.
//!
//! # `shiftOT` is a **maximum**, not a sum
//!
//! ```java
//! double shiftOT = Math.max(weeklyAccumulator.computeWeeklyOT(), dailyAccumulator.computeDailyOvertime(weeklyAccumulator));
//! double shiftDT = dailyAccumulator.computeDailyDoubleTime(weeklyAccumulator);
//! ```
//!
//! An hour that is both daily and weekly overtime is paid once, at whichever
//! measure claims more of the distribution. And **double time is carved out of
//! that same total**, not added to it: the double-time row gets `shiftDT`, the
//! overtime row gets `shiftOT - shiftDT`, and the regular row loses `shiftOT`.
//!
//! # `setOriginalHours` is per distribution
//!
//! `weeklyAccumulator.setOriginalHours(distribution.getOriginalHours())` runs
//! immediately before `computeWeeklyOT`, so the premium formula's cap is
//! *this* distribution's original hours. That is what stops a week's whole
//! excess landing on one row.
//!
//! # The accumulators advance even when nothing is written
//!
//! `addDistributions` is gated on the date being open for editing and on
//! `premiumHours > 0`; the three `add*` calls that follow it are not. So a
//! distribution in a closed pay period still counts toward the week's hours and
//! its computed overtime is still booked as paid. The Java spec's
//! `ot in prior pay period is not double counted toward weekly ot threshold`
//! turns on exactly this.
//!
//! # `convertAllOtToDT` is a second pass
//!
//! When set — and only when `payDT` is also set — the whole week is walked
//! again and each date's overtime row is emptied into its double-time row,
//! creating one if there is none. The overtime row is left in place at zero.
//!
//! # `setBothConsecutiveAndWeeklyOt` does nothing
//!
//! The rule reads `BOTH_CONSECUTIVE_AND_WEEKLY_OT` and hands it to the weekly
//! accumulator, whose field nothing reads. The Java spec case named for it
//! passes on the strength of its *other* parameter; see the parity module.

use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    BOTH_CONSECUTIVE_AND_WEEKLY_OT, CONSEC_DAY_LIMIT, CONSEC_DAYS_IN_WEEK, CONVERT_ALL_OT_TO_DT,
    CaliforniaExtendedOTHrsRuleConfig, DAILY_DT_LIMIT_PROP, DAILY_OT_LIMIT_PROP,
    MAX_CONSEC_DAYS_PD, PAY_DT_PROP, PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::daily_accumulator::DailyAccumulator;
use crate::rules::algorithm::hoursdistribution::prior_days_calculator::calculate_prior_consecutive_days;
use crate::rules::algorithm::hoursdistribution::weekly_accumulator::WeeklyAccumulator;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::EmployeeShiftConsecutiveDaysPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// California overtime with daily, weekly and consecutive-day limits.
/// `CaliforniaExtendedOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaliforniaExtendedOTHrsRule<P: EmployeeShiftConsecutiveDaysPort> {
    consecutive_days: P,
}

impl<P: EmployeeShiftConsecutiveDaysPort> CaliforniaExtendedOTHrsRule<P> {
    /// Build the rule over the lookup that primes a week's consecutive-day
    /// count from days before the time card starts.
    pub fn new(consecutive_days: P) -> Self {
        Self { consecutive_days }
    }
}

/// The two bucket ids this rule writes into.
struct Buckets {
    overtime: i32,
    double_time: i32,
}

impl<P: EmployeeShiftConsecutiveDaysPort> HoursDistributionRule for CaliforniaExtendedOTHrsRule<P> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&CaliforniaExtendedOTHrsRuleConfig.default_values());

        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt_limit = params.double_at(DAILY_DT_LIMIT_PROP);
        let weekly_limit = params.double_at(WEEKLY_LIMIT_PROP);
        let pay_dt = params.bool_at(PAY_DT_PROP);
        let this_week_only = params.bool_at(CONSEC_DAYS_IN_WEEK);
        let consec_day_limit = params.int_at(CONSEC_DAY_LIMIT);
        let max_consec_days = params.int_at(MAX_CONSEC_DAYS_PD);
        let convert_all_ot_to_dt = params.bool_at(CONVERT_ALL_OT_TO_DT);
        let premium_hours_count_towards_weekly_ot =
            params.bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT);
        // Read and passed to the accumulator in Java, where nothing reads it.
        let _both_consecutive_and_weekly_ot = params.bool_at(BOTH_CONSECUTIVE_AND_WEEKLY_OT);

        // "subtract 1, because the day the consecutive limit starts is included"
        let consec_days_modifier = consec_day_limit + max_consec_days - 1;

        let (Some(overtime), Some(double_time)) = (
            time_card.ot_hours_distribution_type_id(),
            time_card.dt_hours_distribution_type_id(),
        ) else {
            return;
        };
        let buckets = Buckets {
            overtime,
            double_time,
        };

        let daily_shift_map = generate_daily_shift_map(time_card, work_week);

        let mut weekly = WeeklyAccumulator::new(weekly_limit, consec_day_limit, max_consec_days);
        weekly.set_premium_hours_count_towards_weekly_ot(premium_hours_count_towards_weekly_ot);

        if !this_week_only {
            let prior = calculate_prior_consecutive_days(
                time_card,
                &self.consecutive_days,
                work_week,
                &[],
                consec_days_modifier,
                false,
            );
            weekly.update_consecutive_days(prior as i32);
        }

        for date in work_week.dates() {
            let shifts = &daily_shift_map[&date];
            let mut daily = DailyAccumulator::new(date, daily_ot_limit, daily_dt_limit);

            weekly.update_consecutive_days_for_shifts(!shifts.is_empty());

            for &shift_index in shifts {
                for distribution_index in regular_distributions_on(time_card, shift_index, date) {
                    create_new_distribution(
                        time_card,
                        shift_index,
                        distribution_index,
                        &buckets,
                        pay_dt,
                        rule_item.id(),
                        &mut weekly,
                        &mut daily,
                    );
                }
            }
        }

        if convert_all_ot_to_dt && pay_dt {
            for date in work_week.dates() {
                for &shift_index in &daily_shift_map[&date] {
                    convert_all_ot_hours_to_dt_hours(
                        time_card,
                        shift_index,
                        &buckets,
                        rule_item.id(),
                        date,
                    );
                }
            }
        }
    }
}

/// `generateDailyShiftMap` over `buildShiftForDateMap`.
///
/// A shift appears under **every** date it has a distribution for, so an
/// overnight shift is visited twice — once per day it spans.
fn generate_daily_shift_map(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
) -> std::collections::HashMap<LocalDate, Vec<usize>> {
    work_week
        .dates()
        .into_iter()
        .map(|date| {
            let single_day = DateRange::new(date, date);
            let mut indices = time_card.shift_indices_with_distributions_for_period(&single_day);
            indices.retain(|&index| {
                time_card.shift_is_not_salaried_exempt(&time_card.shifts()[index])
            });
            indices.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());
            (date, indices)
        })
        .collect()
}

/// The shift's non-premium distributions dated `date`.
fn regular_distributions_on(
    time_card: &dyn TimeCard,
    shift_index: usize,
    date: LocalDate,
) -> Vec<usize> {
    time_card.shifts()[shift_index]
        .hours_distributions()
        .iter()
        .enumerate()
        .filter(|(_, distribution)| {
            !time_card.distribution_is_premium(distribution) && distribution.date() == date
        })
        .map(|(index, _)| index)
        .collect()
}

/// `createNewDistribution` — accumulate, split, write, accumulate again.
#[allow(clippy::too_many_arguments)]
fn create_new_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    buckets: &Buckets,
    pay_dt: bool,
    rule_item_id: i32,
    weekly: &mut WeeklyAccumulator,
    daily: &mut DailyAccumulator,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let original_hours = distribution.original_hours();
    let date = distribution.date();

    daily.add_hours(original_hours);
    weekly.add_hours(original_hours);
    weekly.set_original_hours(original_hours);

    // `computePremiumHours` — the larger of the two overtime measures, and the
    // daily double time carved out of it.
    let shift_ot = weekly
        .compute_weekly_ot()
        .max(daily.compute_daily_overtime(weekly));
    let shift_dt = daily.compute_daily_double_time(weekly);

    if time_card.is_open_for_editing_on(date) && shift_ot > 0.0 {
        let shift = &mut time_card.shifts_mut()[shift_index];
        let mut premium = |hours: f64, bucket: i32| {
            let row = create_premium_distribution_at_rate(
                &shift.hours_distributions()[distribution_index],
                hours,
                0.0,
                bucket,
                Some(rule_item_id),
                None,
            );
            shift.add_hours_distribution(row);
        };

        if pay_dt {
            if shift_dt > 0.0 {
                premium(shift_dt, buckets.double_time);
            }
            if shift_ot - shift_dt > 0.0 {
                premium(round_hours(shift_ot - shift_dt), buckets.overtime);
            }
        } else {
            premium(shift_ot, buckets.overtime);
        }

        let reduced =
            round_hours(shift.hours_distributions()[distribution_index].hours() - shift_ot);
        shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
    }

    // Outside the guard, deliberately — see the module note.
    daily.add_overtime(shift_ot);
    daily.add_double_time(shift_dt);
    weekly.add_weekly_ot(shift_ot);
}

/// `convertAllOtHoursToDTHours` — empty the date's overtime row into its
/// double-time row.
fn convert_all_ot_hours_to_dt_hours(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    buckets: &Buckets,
    rule_item_id: i32,
    date: LocalDate,
) {
    let shift = &mut time_card.shifts_mut()[shift_index];

    let Some(ot_index) = shift
        .hours_distributions()
        .iter()
        .position(|d| d.is_of_type(buckets.overtime) && d.date() == date)
    else {
        return;
    };
    let dt_index = shift
        .hours_distributions()
        .iter()
        .position(|d| d.is_of_type(buckets.double_time) && d.date() == date);

    let ot_hours = shift.hours_distributions()[ot_index].hours();

    match dt_index {
        Some(index) => {
            // Java adds without rounding here.
            let hours = shift.hours_distributions()[index].hours();
            shift.hours_distributions_mut()[index].set_hours(hours + ot_hours);
        }
        None => {
            let row = create_premium_distribution_at_rate(
                &shift.hours_distributions()[ot_index],
                ot_hours,
                0.0,
                buckets.double_time,
                Some(rule_item_id),
                None,
            );
            shift.add_hours_distribution(row);
        }
    }

    shift.hours_distributions_mut()[ot_index].set_hours(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    pub(super) const JOB: i32 = 11;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// The DAO, answering a fixed prior-days count.
    pub(super) struct PriorDays(pub i32);

    impl EmployeeShiftConsecutiveDaysPort for PriorDays {
        fn prior_consecutive_days_worked(&self, _time_card: &dyn TimeCard) -> i32 {
            self.0
        }
        fn prior_consecutive_days_worked_including_earnings(
            &self,
            _time_card: &dyn TimeCard,
            _earning_type_ids: &[i32],
        ) -> i32 {
            self.0
        }
    }

    pub(super) fn employee(pay_type: EmployeePayType) -> Employee {
        Employee::new(
            1,
            1,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                LocalDate::of(2009, 1, 1),
                LocalDate::of(2099, 12, 31),
                pay_type,
                7.50,
                true,
            )],
        )
    }

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    /// `newEmployeeShift(netHours, date)` — one regular distribution on the
    /// shift date carrying the whole shift.
    pub(super) fn shift(id: i32, hours: f64, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_net_hours(hours)
            .with_hours_distributions(vec![regular(date, hours)])
    }

    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(99, 1, "", RuleClass::CaExtHdr, rule_params)
    }

    /// The hours on a shift's row of a given bucket, or zero if it has none.
    pub(super) fn hours_of(card: &TimeCardData, shift_index: usize, type_id: i32) -> f64 {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .find(|d| d.is_of_type(type_id))
            .map_or(0.0, |d| d.hours())
    }

    fn day(day_of_month: u32) -> LocalDate {
        LocalDate::of(2010, 11, day_of_month.try_into().unwrap())
    }

    /// The Java fixture: nine shifts, 2010-11-21 through 2010-11-29, with the
    /// week 11-22..11-28 and the calculation opening on 11-22.
    fn fixture() -> TimeCardData {
        card(vec![
            shift(1, 12.0, day(21)),
            shift(2, 8.0, day(22)),
            shift(3, 6.32, day(23)),
            shift(4, 8.0, day(24)),
            shift(5, 8.5, day(25)),
            shift(6, 12.54, day(26)),
            shift(7, 8.0, day(27)),
            shift(8, 8.5, day(28)),
            shift(9, 12.0, day(29)),
        ])
        .with_calculation_start_date(day(22))
        .with_dataset_start_date(day(21))
    }

    fn week() -> DateRange {
        DateRange::new(day(22), day(28))
    }

    fn rule() -> CaliforniaExtendedOTHrsRule<PriorDays> {
        CaliforniaExtendedOTHrsRule::new(PriorDays(3))
    }

    #[test]
    fn the_premium_rows_record_the_rule_item_and_no_rate() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[]));

        for shift in card.shifts() {
            for row in shift.hours_distributions() {
                if !row.is_of_type(REGULAR) {
                    assert_eq!(row.hours_rule_item_id(), Some(99));
                    assert_eq!(row.premium_rate(), 0.0);
                    assert_eq!(row.rate_rule_item_id(), None);
                    assert_eq!(row.original_hours(), 0.0);
                }
            }
        }
    }

    #[test]
    fn double_time_is_carved_out_of_the_overtime_not_added_to_it() {
        // 12.54 hours on one day: 4.54 over the daily limit, of which 0.54 is
        // past the double-time limit. The regular row loses 4.54 in total.
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 5, REGULAR), 8.0);
        assert_eq!(hours_of(&card, 5, OVERTIME), 4.0);
        assert_eq!(hours_of(&card, 5, DOUBLE_TIME), 0.54);
    }

    #[test]
    fn convert_all_ot_to_dt_empties_the_overtime_row() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(CONVERT_ALL_OT_TO_DT, "true")]));

        assert_eq!(hours_of(&card, 5, REGULAR), 8.0);
        assert_eq!(hours_of(&card, 5, OVERTIME), 0.0, "left in place at zero");
        assert_eq!(hours_of(&card, 5, DOUBLE_TIME), 4.54);
    }

    #[test]
    fn convert_all_ot_to_dt_does_nothing_without_pay_dt() {
        let mut card = fixture();

        rule().execute(
            &mut card,
            &week(),
            &item(&[(CONVERT_ALL_OT_TO_DT, "true"), (PAY_DT_PROP, "false")]),
        );

        assert_eq!(hours_of(&card, 5, OVERTIME), 4.54);
        assert_eq!(hours_of(&card, 5, DOUBLE_TIME), 0.0);
    }

    #[test]
    fn a_property_with_no_double_time_bucket_writes_nothing() {
        // Java unboxes a null Integer inside addDistributions and throws.
        let mut card = fixture().with_hours_distribution_types(vec![
            HoursDistributionType::new(REGULAR, "Regular", false),
            HoursDistributionType::new(OVERTIME, "Overtime", true),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 5, REGULAR), 12.54);
        assert_eq!(hours_of(&card, 5, OVERTIME), 0.0);
    }
}

/// `CaliforniaExtendedOTHrsRuleImplTest.groovy`, transcribed.
///
/// All twelve cases, each asserting net, regular, overtime and double-time
/// hours on every one of its shifts — 100 assertions in total, and by far the
/// strongest spec in the family.
///
/// The Groovy's `hasNetRegOTDT` helper checks
/// `(actual - expected) <= 0.01`, which is **one-sided**: an actual far *below*
/// the expected value passes it. These transcriptions assert equality instead,
/// which is strictly stronger and still passes — so a rounding divergence in
/// either direction would show up here where it would not in Java.
///
/// The pay group is replaced by the card's calculation start date
/// (divergence 24), and the spec's `getDatasetStartDate()` override by
/// `with_dataset_start_date`.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        DOUBLE_TIME, JOB, OVERTIME, PriorDays, REGULAR, card, employee, hours_of, item, regular,
        shift,
    };
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use rstest::rstest;

    fn nov(day_of_month: u32) -> LocalDate {
        LocalDate::of(2010, 11, day_of_month.try_into().unwrap())
    }

    /// The `setup()` fixture: nine shifts either side of the week.
    fn fixture() -> TimeCardData {
        card(vec![
            shift(1, 12.0, nov(21)),
            shift(2, 8.0, nov(22)),
            shift(3, 6.32, nov(23)),
            shift(4, 8.0, nov(24)),
            shift(5, 8.5, nov(25)),
            shift(6, 12.54, nov(26)),
            shift(7, 8.0, nov(27)),
            shift(8, 8.5, nov(28)),
            shift(9, 12.0, nov(29)),
        ])
        // `currentPayPeriodEndDate: 2010-11-21.plusWeeks(1)` weekly, so the
        // period runs 11-22..11-28 and 11-21 is closed.
        .with_calculation_start_date(nov(22))
        // The anonymous subclass overrides this.
        .with_dataset_start_date(nov(21))
    }

    fn week() -> DateRange {
        DateRange::new(nov(22), nov(28))
    }

    fn rule() -> CaliforniaExtendedOTHrsRule<PriorDays> {
        // `getPriorConsecutiveDayWorkedWithCache(_) >> 3`.
        CaliforniaExtendedOTHrsRule::new(PriorDays(3))
    }

    /// `hasNetRegOTDT(shift, net, reg, ot, dt)`.
    fn assert_shift(card: &TimeCardData, shift_index: usize, net: f64, reg: f64, ot: f64, dt: f64) {
        assert_eq!(
            (
                card.shifts()[shift_index].net_hours(),
                hours_of(card, shift_index, REGULAR),
                hours_of(card, shift_index, OVERTIME),
                hours_of(card, shift_index, DOUBLE_TIME),
            ),
            (net, reg, ot, dt),
            "shift {shift_index}"
        );
    }

    #[test]
    fn test_default_config() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0); // closed, before the week
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 4.0, 0.54);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0); // after the week
    }

    #[test]
    fn test_consecutive_days_cross_week() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        // Primed to 4 from the prior day plus the DAO's 3, so the seventh
        // consecutive day falls on 11-24 and the daily limits stop applying.
        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 4, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 5, 12.54, 0.0, 8.0, 4.54);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_no_dt() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(PAY_DT_PROP, "false")]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 4.54, 0.0);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.5, 0.0);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_daily_ot_7() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(DAILY_OT_LIMIT_PROP, "7")]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 7.0, 1.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 7.0, 1.0, 0.0);
        assert_shift(&card, 4, 8.5, 7.0, 1.5, 0.0);
        assert_shift(&card, 5, 12.54, 7.0, 5.0, 0.54);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 7.0, 1.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_daily_dt_11() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(DAILY_DT_LIMIT_PROP, "11")]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 3.0, 1.54);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_weekly_45() {
        let mut card = fixture();

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_LIMIT_PROP, "45")]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 4.0, 0.54);
        assert_shift(&card, 6, 8.0, 1.64, 6.36, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_salaried() {
        let mut card = fixture().with_employee(employee(EmployeePayType::SalariedExempt));

        rule().execute(&mut card, &week(), &item(&[]));

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.5, 0.0, 0.0);
        assert_shift(&card, 5, 12.54, 12.54, 0.0, 0.0);
        assert_shift(&card, 6, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 7, 8.5, 8.5, 0.0, 0.0);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }

    #[test]
    fn test_consecutive_days_cross_week_2() {
        // A card built with **no** distribution types, so every distribution
        // reads as premium and the rule's filter excludes all of them.
        let may = |day: u32| LocalDate::of(2011, 5, day.try_into().unwrap());
        let mut card = TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(vec![
                shift(1, 12.0, may(9)),
                shift(2, 12.0, may(10)),
                shift(3, 12.0, may(11)),
                shift(4, 12.0, may(12)),
                shift(5, 12.0, may(13)),
                shift(6, 12.0, may(14)),
                shift(7, 8.0, may(16)),
                shift(8, 8.0, may(17)),
                shift(9, 8.0, may(18)),
            ])
            .with_calculation_start_date(may(16))
            .with_dataset_start_date(nov(21));

        rule().execute(
            &mut card,
            &DateRange::new(may(15), may(21)),
            &item(&[(CONSEC_DAYS_IN_WEEK, "false")]),
        );

        for index in 0..6 {
            assert_shift(&card, index, 12.0, 12.0, 0.0, 0.0);
        }
        for index in 6..9 {
            assert_shift(&card, index, 8.0, 8.0, 0.0, 0.0);
        }
    }

    /// A shift spanning midnight, with its hours split across two days.
    fn overnight(
        id: i32,
        net: f64,
        first: (LocalDate, f64),
        second: (LocalDate, f64),
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, first.0, ShiftType::Actual, Vec::new())
            .with_net_hours(net)
            .with_hours_distributions(vec![regular(first.0, first.1), regular(second.0, second.1)])
    }

    #[test]
    fn test_consecutive_days_with_ot_and_dt() {
        let day = |d: u32| LocalDate::of(2011, 11, d.try_into().unwrap());
        let mut card = card(vec![
            shift(1, 8.0, day(22)),
            shift(2, 8.0, day(23)),
            shift(3, 1.0, day(24)),
            shift(4, 1.0, day(25)),
            shift(5, 1.0, day(26)),
            overnight(6, 16.0, (day(27), 14.0), (day(28), 2.0)),
        ])
        .with_calculation_start_date(nov(22))
        .with_dataset_start_date(nov(21));

        rule().execute(&mut card, &DateRange::new(day(22), day(28)), &item(&[]));

        assert_shift(&card, 0, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 1.0, 1.0, 0.0, 0.0);
        assert_shift(&card, 3, 1.0, 1.0, 0.0, 0.0);
        assert_shift(&card, 4, 1.0, 1.0, 0.0, 0.0);

        // "ot and dt was created for hours distributions on both days that the
        // shift was on" — the overnight shift is visited once per day it spans.
        let row = |date: LocalDate, type_id: i32| {
            card.shifts()[5]
                .hours_distributions()
                .iter()
                .find(|d| d.date() == date && d.is_of_type(type_id))
                .map(|d| d.hours())
        };

        assert_eq!(row(day(27), REGULAR), Some(8.0));
        assert_eq!(row(day(27), OVERTIME), Some(4.0));
        assert_eq!(row(day(27), DOUBLE_TIME), Some(2.0));
        assert_eq!(row(day(28), REGULAR), Some(0.0));
        assert_eq!(row(day(28), OVERTIME), Some(2.0));
    }

    #[test]
    fn ot_in_prior_pay_period_is_not_double_counted_toward_weekly_ot_threshold() {
        let day = |d: u32| LocalDate::of(2011, 11, d.try_into().unwrap());

        // The first shift already carries an overtime row from a prior run:
        // eight regular hours out of nine original, plus one overtime hour.
        let mut carried = HoursDistribution::new(1, day(22), Some(REGULAR), 8.0, 0.0);
        carried.set_original_hours(9.0);
        let first = EmployeeShift::new(1, 1, JOB, day(22), ShiftType::Actual, Vec::new())
            .with_net_hours(9.0)
            .with_hours_distributions(vec![
                carried,
                HoursDistribution::new(1, day(22), Some(OVERTIME), 1.0, 0.0),
            ]);

        let mut card = card(vec![
            first,
            shift(2, 8.0, day(23)),
            shift(3, 8.0, day(24)),
            shift(4, 8.0, day(25)),
            shift(5, 9.0, day(26)),
            shift(6, 2.0, day(27)),
        ])
        // `currentPayPeriodEndDate: 2011-11-23.plusWeeks(1)` weekly, so the
        // period opens on 11-24 and the first two days are closed.
        .with_calculation_start_date(day(24))
        .with_dataset_start_date(nov(21));

        rule().execute(&mut card, &DateRange::new(day(22), day(27)), &item(&[]));

        assert_shift(&card, 0, 9.0, 8.0, 1.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 9.0, 7.0, 2.0, 0.0);
        assert_shift(&card, 5, 2.0, 0.0, 2.0, 0.0);
    }

    #[test]
    fn when_both_consec_and_weekly_ot_is_checked_it_applies_both_parameters() {
        // Named for BOTH_CONSECUTIVE_AND_WEEKLY_OT, which sets a field nothing
        // reads. The numbers below follow from CONSEC_DAYS_IN_WEEK alone —
        // `both_consec_and_weekly_ot_changes_nothing` shows it directly.
        let day = |d: u32| LocalDate::of(2011, 11, d.try_into().unwrap());
        let mut card = card(vec![
            shift(1, 8.0, day(22)),
            shift(2, 8.0, day(23)),
            shift(3, 8.0, day(24)),
            shift(4, 8.0, day(25)),
            shift(5, 5.0, day(26)),
            overnight(6, 16.0, (day(27), 14.0), (day(28), 2.0)),
        ])
        .with_calculation_start_date(nov(22))
        .with_dataset_start_date(nov(21));

        rule().execute(
            &mut card,
            &DateRange::new(day(22), day(28)),
            &item(&[
                (CONSEC_DAYS_IN_WEEK, "false"),
                (BOTH_CONSECUTIVE_AND_WEEKLY_OT, "true"),
            ]),
        );

        assert_shift(&card, 0, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 5.0, 5.0, 0.0, 0.0);

        let row = |date: LocalDate, type_id: i32| {
            card.shifts()[5]
                .hours_distributions()
                .iter()
                .find(|d| d.date() == date && d.is_of_type(type_id))
                .map(|d| d.hours())
        };

        assert_eq!(row(day(27), REGULAR), Some(3.0));
        assert_eq!(row(day(27), OVERTIME), Some(9.0));
        assert_eq!(row(day(27), DOUBLE_TIME), Some(2.0));
        assert_eq!(row(day(28), REGULAR), Some(0.0));
        assert_eq!(row(day(28), OVERTIME), Some(2.0));
    }

    #[rstest]
    #[case("true")]
    #[case("false")]
    fn both_consec_and_weekly_ot_changes_nothing(#[case] flag: &str) {
        let mut card = fixture();

        rule().execute(
            &mut card,
            &week(),
            &item(&[(BOTH_CONSECUTIVE_AND_WEEKLY_OT, flag)]),
        );

        // Identical to test_default_config either way.
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 4.0, 0.54);
        assert_shift(&card, 6, 8.0, 0.0, 8.0, 0.0);
    }

    #[test]
    fn when_premium_hours_count_towards_weekly_ot_is_false_the_conservative_formula_is_used() {
        let mut card = fixture();

        rule().execute(
            &mut card,
            &week(),
            &item(&[(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false")]),
        );

        assert_shift(&card, 0, 12.0, 12.0, 0.0, 0.0);
        assert_shift(&card, 1, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 2, 6.32, 6.32, 0.0, 0.0);
        assert_shift(&card, 3, 8.0, 8.0, 0.0, 0.0);
        assert_shift(&card, 4, 8.5, 8.0, 0.5, 0.0);
        assert_shift(&card, 5, 12.54, 8.0, 4.0, 0.54);
        assert_shift(&card, 6, 8.0, 1.68, 6.32, 0.0);
        assert_shift(&card, 7, 8.5, 0.0, 8.0, 0.5);
        assert_shift(&card, 8, 12.0, 12.0, 0.0, 0.0);
    }
}
