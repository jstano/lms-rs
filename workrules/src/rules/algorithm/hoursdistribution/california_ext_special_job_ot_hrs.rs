//! Port of `CaliforniaExtSpecialJobOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/CaliforniaExtSpecialJobOTHrsRuleImpl.java`.
//!
//! `CA_SPJOB_HDR`. `CaliforniaExtendedOTHrs` with a second set of limits for a
//! **special job worked on a special day of the week** — and one idea that
//! exists nowhere else in the family: when a week mixes ordinary and special
//! work, the limits are not chosen, they are **averaged**.
//!
//! # Every limit is an hours-weighted average
//!
//! `HourLimits` walks the whole work week before anything is paid. For each
//! distribution it picks the special limits if the day is a special day of week
//! **and** the shift's job is in the special list, the default limits
//! otherwise, and accumulates `hours × limit` against `hours`. The weekly limit
//! the accumulator then runs on is
//!
//! ```java
//! TDouble.roundHours(totalWeeklyLimits / totalWeeklyHours)
//! ```
//!
//! and each day's overtime and double-time limits are the same quotient taken
//! over that day's hours alone. So an employee who works six hours at a
//! 35-hour-week special job and the rest of the week ordinarily gets a weekly
//! limit somewhere between 35 and 40, proportional to how much of the week was
//! special. The Java spec pins it: with one 8-hour and one 6-hour special shift
//! in a 59.86-hour week, the limit comes out **38.83**.
//!
//! Two consequences worth holding on to:
//!
//! - the weekly limit is computed from the **whole** week, including days the
//!   walk has not reached yet, so a Sunday shift changes what Monday is paid;
//! - a day with no shifts divides zero by zero. Java gets `NaN`, every
//!   comparison against it is false, and nothing is written — which is also
//!   what happens here, because such a day has no distributions to process
//!   either. The port answers `0.0` rather than `NaN`; see the note on
//!   `weighted_limit`.
//!
//! # It declares its own `DailyData`, and a third pair of accumulators
//!
//! The audit has warned about this file since the shared `DailyData` was
//! ported: the inner class of that name here carries `(date, shifts,
//! specialDay)` and **no earnings**, so the shared struct does not fit. Its
//! `WeeklyAccumulator` and `DailyAccumulator` are shadowed too:
//!
//! | | the shared class | this rule's inner class |
//! |---|---|---|
//! | `WeeklyAccumulator(…)` | three arguments | four — the fourth seeds the counter |
//! | `computeWeeklyOT` | two formulas behind a flag, unrounded | one formula, **rounded** |
//! | daily double time | always computed | zero unless `payDT` |
//! | on a consecutive-day-limit day | overtime drops its limit; double time measures from the **overtime** limit | the same, and this is the one place the three agree |
//!
//! Ported private as `WeekTotals` and `DayTotals`.
//!
//! # The special-day lists are parsed by the unchecked reader
//!
//! `specDaysOfWeek` and `specJobs` go through `JSONUtils.getIdsFromJSON`, which
//! **throws** — not `getIdsListForKey`, which swallows and returns nothing. So
//! a corrupt list here aborts the calculation where the same mistake in
//! `holidayTypes` would silently select nothing. See
//! [`crate::common::json_ids::ids_from_json`].
//!
//! The day numbers are **Sunday-based**, not ISO: the rule compares the list
//! against `DateUtil.translateDOWFromISO(date.getDayOfWeek())`.
//!
//! # A dead predicate
//!
//! `employeeJobStatusIsNotSalariedExemptForEarning` is declared and never used
//! — this rule does not read earnings at all. Not ported. The fourth dead
//! member found in the family.
//!

use crate::common::dates::translate_dow_from_iso;
use crate::common::json_ids::ids_from_json;
use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    CONSEC_DAY_LIMIT, CONSEC_DAYS_IN_WEEK, CaliforniaExtSpecialJobOTHrsRuleConfig,
    DAILY_DT_LIMIT_PROP, DAILY_OT_LIMIT_PROP, MAX_CONSEC_DAYS_PD, PAY_DT_PROP,
    SPEC_DAILY_DT_LIMIT_PROP, SPEC_DAILY_OT_LIMIT_PROP, SPEC_DOW_PROP, SPEC_JOBS_PROP,
    SPEC_WEEKLY_LIMIT_PROP, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::prior_days_calculator::calculate_prior_consecutive_days;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::EmployeeShiftConsecutiveDaysPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// California overtime with a second set of limits for special jobs on special
/// days. `CaliforniaExtSpecialJobOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaliforniaExtSpecialJobOTHrsRule<P: EmployeeShiftConsecutiveDaysPort> {
    consecutive_days: P,
}

impl<P: EmployeeShiftConsecutiveDaysPort> CaliforniaExtSpecialJobOTHrsRule<P> {
    /// Build the rule over the lookup that primes a week's consecutive-day
    /// count from days before the time card starts.
    pub fn new(consecutive_days: P) -> Self {
        Self { consecutive_days }
    }
}

/// This rule's **private** `DailyData`: a date, the day's shifts, and whether
/// the date is one of the configured special days of week. No earnings.
struct DailyData {
    shifts: Vec<usize>,
    special_day: bool,
}

/// This rule's **private** `WeeklyAccumulator`.
#[derive(Debug, Default)]
struct WeekTotals {
    weekly_limit: f64,
    consecutive_days_limit: i32,
    hours: f64,
    weekly_ot: f64,
    consecutive_days_counter: i32,
    consec_days_modifier: i32,
}

impl WeekTotals {
    fn new(
        weekly_limit: f64,
        consecutive_days_limit: i32,
        max_consecutive_days: i32,
        initial_consecutive_days: i32,
    ) -> Self {
        Self {
            weekly_limit,
            consecutive_days_limit,
            consecutive_days_counter: initial_consecutive_days,
            consec_days_modifier: consecutive_days_limit + max_consecutive_days - 1,
            ..Self::default()
        }
    }

    /// `updateConsecutiveDays(DailyData)` — reset, or increment and wrap.
    ///
    /// The `% != 0` guard is the shared accumulator's: landing exactly on the
    /// modifier leaves the counter *at* it, so the limit day still tests as
    /// over the limit and only the day after resets.
    fn update_consecutive_days(&mut self, day_was_worked: bool) {
        if !day_was_worked {
            self.consecutive_days_counter = 0;
            return;
        }

        self.consecutive_days_counter += 1;
        if self.consecutive_days_counter % self.consec_days_modifier != 0 {
            self.consecutive_days_counter %= self.consec_days_modifier;
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// `computeWeeklyOT` — rounded, unlike the shared accumulator's.
    fn compute_weekly_ot(&self) -> f64 {
        round_hours(self.hours - self.weekly_limit - self.weekly_ot).max(0.0)
    }

    /// `isDuringOTConsecDaysRange`.
    fn is_during_ot_consec_days_range(&self) -> bool {
        self.consecutive_days_counter >= self.consecutive_days_limit
    }
}

/// This rule's **private** `DailyAccumulator`, carrying the day's averaged
/// limits.
#[derive(Debug, Default, Clone, Copy)]
struct DayTotals {
    daily_ot_limit: f64,
    daily_dt_limit: f64,
    hours: f64,
    overtime: f64,
    double_time: f64,
}

impl DayTotals {
    fn new(daily_ot_limit: f64, daily_dt_limit: f64) -> Self {
        Self {
            daily_ot_limit,
            daily_dt_limit,
            ..Self::default()
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_overtime(&mut self, add_overtime: f64) {
        self.overtime = round_hours(self.overtime + add_overtime);
    }

    fn add_double_time(&mut self, add_double_time: f64) {
        self.double_time = round_hours(self.double_time + add_double_time);
    }

    /// `computeDailyOvertime` — inside the consecutive-day range the daily
    /// limit is dropped entirely and every hour is overtime.
    fn compute_daily_overtime(&self, week: &WeekTotals) -> f64 {
        let daily_ot = if week.is_during_ot_consec_days_range() {
            self.hours - self.overtime
        } else {
            self.hours - self.daily_ot_limit - self.overtime
        };

        round_hours(daily_ot).max(0.0)
    }

    /// `computeDailyDoubleTime` — nothing at all without `payDT`; inside the
    /// range double time starts measuring at the **overtime** limit.
    fn compute_daily_double_time(&self, week: &WeekTotals, pay_dt: bool) -> f64 {
        if !pay_dt {
            return 0.0;
        }

        let daily_dt = if week.is_during_ot_consec_days_range() {
            self.hours - self.daily_ot_limit - self.double_time
        } else {
            self.hours - self.daily_dt_limit - self.double_time
        };

        round_hours(daily_dt).max(0.0)
    }
}

/// The two bucket ids this rule writes into.
struct Buckets {
    overtime: i32,
    double_time: i32,
}

/// The six limits and the two lists that say which apply. `HourLimits`'s fields.
struct Limits {
    default_weekly: f64,
    default_daily_ot: f64,
    default_daily_dt: f64,
    special_weekly: f64,
    special_daily_ot: f64,
    special_daily_dt: f64,
    special_job_ids: Vec<i32>,
}

impl<P: EmployeeShiftConsecutiveDaysPort> HoursDistributionRule
    for CaliforniaExtSpecialJobOTHrsRule<P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&CaliforniaExtSpecialJobOTHrsRuleConfig.default_values());

        let pay_dt = params.bool_at(PAY_DT_PROP);
        let consec_day_limit = params.int_at(CONSEC_DAY_LIMIT);
        let max_consec_days = params.int_at(MAX_CONSEC_DAYS_PD);
        let this_week_only = params.bool_at(CONSEC_DAYS_IN_WEEK);

        let special_day_of_week_ids = ids_from_json(params.get(SPEC_DOW_PROP).unwrap_or("[]"));
        let limits = Limits {
            default_weekly: params.double_at(WEEKLY_LIMIT_PROP),
            default_daily_ot: params.double_at(DAILY_OT_LIMIT_PROP),
            default_daily_dt: params.double_at(DAILY_DT_LIMIT_PROP),
            special_weekly: params.double_at(SPEC_WEEKLY_LIMIT_PROP),
            special_daily_ot: params.double_at(SPEC_DAILY_OT_LIMIT_PROP),
            special_daily_dt: params.double_at(SPEC_DAILY_DT_LIMIT_PROP),
            special_job_ids: ids_from_json(params.get(SPEC_JOBS_PROP).unwrap_or("[]")),
        };

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

        let daily_data_map = build_daily_data_map(time_card, work_week, &special_day_of_week_ids);

        // "subtract 1, because the day the consecutive limit starts is included"
        let consec_days_modifier = consec_day_limit + max_consec_days - 1;
        let initial_consecutive_days = if this_week_only {
            0
        } else {
            calculate_prior_consecutive_days(
                time_card,
                &self.consecutive_days,
                work_week,
                &[],
                consec_days_modifier,
                false,
            ) as i32
        };

        let (weekly_limit, daily_limits) =
            compute_hour_limits(time_card, work_week, &daily_data_map, &limits);

        let mut week = WeekTotals::new(
            weekly_limit,
            consec_day_limit,
            max_consec_days,
            initial_consecutive_days,
        );

        for date in work_week.dates() {
            let daily_data = &daily_data_map[&date];
            let mut day = daily_limits[&date];

            week.update_consecutive_days(!daily_data.shifts.is_empty());

            for &shift_index in &daily_data.shifts {
                compute_distribution_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    date,
                    shift_index,
                    &buckets,
                    rule_item.id(),
                    pay_dt,
                );
            }
        }
    }
}

/// `dailyDataProducer` — this rule's, which carries a special-day flag instead
/// of earnings.
fn build_daily_data_map(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    special_day_of_week_ids: &[i32],
) -> HashMap<LocalDate, DailyData> {
    work_week
        .dates()
        .into_iter()
        .map(|date| {
            let single_day = DateRange::new(date, date);
            let mut shifts = time_card.shift_indices_with_distributions_for_period(&single_day);
            shifts.retain(|&index| {
                time_card.shift_is_not_salaried_exempt(&time_card.shifts()[index])
            });
            shifts.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

            // Sunday-based, not ISO.
            let day_number = translate_dow_from_iso(date.day_of_week().value());
            let special_day = special_day_of_week_ids.contains(&day_number);

            (
                date,
                DailyData {
                    shifts,
                    special_day,
                },
            )
        })
        .collect()
}

/// `HourLimits.calculate` — the weighted weekly limit, and each day's weighted
/// daily limits.
fn compute_hour_limits(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    daily_data_map: &HashMap<LocalDate, DailyData>,
    limits: &Limits,
) -> (f64, HashMap<LocalDate, DayTotals>) {
    let mut total_weekly_limits = 0.0;
    let mut total_weekly_hours = 0.0;
    let mut daily_limits = HashMap::new();

    for date in work_week.dates() {
        let daily_data = &daily_data_map[&date];

        let mut total_ot_limits = 0.0;
        let mut total_dt_limits = 0.0;
        let mut total_daily_hours = 0.0;

        for &shift_index in &daily_data.shifts {
            let shift = &time_card.shifts()[shift_index];
            let special =
                daily_data.special_day && limits.special_job_ids.contains(&shift.job_id());

            let (weekly_limit, daily_ot_limit, daily_dt_limit) = if special {
                (
                    limits.special_weekly,
                    limits.special_daily_ot,
                    limits.special_daily_dt,
                )
            } else {
                (
                    limits.default_weekly,
                    limits.default_daily_ot,
                    limits.default_daily_dt,
                )
            };

            // `HourLimits.addHours` — this day's distributions only.
            for distribution in shift
                .hours_distributions()
                .iter()
                .filter(|d| d.date() == date)
            {
                let hours = distribution.original_hours();

                total_weekly_hours = round_hours(total_weekly_hours + hours);
                total_weekly_limits =
                    round_hours(total_weekly_limits + round_hours(hours * weekly_limit));

                total_daily_hours = round_hours(total_daily_hours + hours);
                total_ot_limits =
                    round_hours(total_ot_limits + round_hours(hours * daily_ot_limit));
                total_dt_limits =
                    round_hours(total_dt_limits + round_hours(hours * daily_dt_limit));
            }
        }

        daily_limits.insert(
            date,
            DayTotals::new(
                weighted_limit(total_ot_limits, total_daily_hours),
                weighted_limit(total_dt_limits, total_daily_hours),
            ),
        );
    }

    (
        weighted_limit(total_weekly_limits, total_weekly_hours),
        daily_limits,
    )
}

/// `roundHours(totalLimits / totalHours)`.
///
/// **Divergence:** Java divides unguarded, so a day or a week with no hours
/// yields `NaN`. Every later comparison against `NaN` is false, so nothing is
/// written — and nothing *can* be, because a day with no hours has no
/// distributions to walk. Answering `0.0` reaches the same place by a route a
/// reader can follow, and keeps a `NaN` from escaping into an accumulator if
/// this is ever called from somewhere new.
fn weighted_limit(total_limits: f64, total_hours: f64) -> f64 {
    if total_hours == 0.0 {
        return 0.0;
    }
    round_hours(total_limits / total_hours)
}

/// `computeDistributionHours`.
#[allow(clippy::too_many_arguments)]
fn compute_distribution_hours(
    time_card: &mut dyn TimeCard,
    week: &mut WeekTotals,
    day: &mut DayTotals,
    date: LocalDate,
    shift_index: usize,
    buckets: &Buckets,
    rule_item_id: i32,
    pay_dt: bool,
) {
    let distribution_indices: Vec<usize> = time_card.shifts()[shift_index]
        .hours_distributions()
        .iter()
        .enumerate()
        .filter(|(_, distribution)| {
            !time_card.distribution_is_premium(distribution) && distribution.date() == date
        })
        .map(|(index, _)| index)
        .collect();

    for distribution_index in distribution_indices {
        create_distribution(
            time_card,
            shift_index,
            distribution_index,
            buckets,
            rule_item_id,
            week,
            day,
            pay_dt,
        );
    }
}

/// `createDistribution` — accumulate, split, write, accumulate again.
#[allow(clippy::too_many_arguments)]
fn create_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    buckets: &Buckets,
    rule_item_id: i32,
    week: &mut WeekTotals,
    day: &mut DayTotals,
    pay_dt: bool,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let original_hours = distribution.original_hours();
    let date = distribution.date();

    day.add_hours(original_hours);
    week.add_hours(original_hours);

    let premium_hours = week
        .compute_weekly_ot()
        .max(day.compute_daily_overtime(week));
    let double_time = day.compute_daily_double_time(week, pay_dt);

    // `addDistributions`.
    if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
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
            if double_time > 0.0 {
                premium(double_time, buckets.double_time);
            }
            if premium_hours - double_time > 0.0 {
                premium(round_hours(premium_hours - double_time), buckets.overtime);
            }
        } else {
            premium(premium_hours, buckets.overtime);
        }

        let reduced =
            round_hours(shift.hours_distributions()[distribution_index].hours() - premium_hours);
        shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
    }

    // Outside the guard, as everywhere else in the family.
    day.add_overtime(premium_hours);
    day.add_double_time(double_time);
    week.add_weekly_ot(premium_hours);
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

    pub(super) const HOME_JOB: i32 = 11;
    pub(super) const SPECIAL_JOB: i32 = 22;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// The `EmployeeShiftConsecutiveDaysDAO` stub.
    ///
    /// The Groovy mocks `PriorDaysCalculator` itself, one level up, to answer
    /// **4**. That calculator is ported here rather than stubbed, so the stub
    /// sits below it — and the calculator adds the 11-21 shift it finds between
    /// the dataset start date and the week, so a DAO answer of 3 reaches the
    /// rule as the 4 the spec intends.
    #[derive(Debug, Clone, Copy, Default)]
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

    pub(super) fn rule() -> CaliforniaExtSpecialJobOTHrsRule<PriorDays> {
        CaliforniaExtSpecialJobOTHrsRule::new(PriorDays(3))
    }

    pub(super) fn nov(day: u32) -> LocalDate {
        LocalDate::of(2010, 11, day.try_into().unwrap())
    }

    pub(super) fn week() -> DateRange {
        DateRange::new(nov(22), nov(28))
    }

    pub(super) fn employee(pay_type: EmployeePayType) -> Employee {
        Employee::new(
            1,
            1,
            "",
            vec![
                EmployeeJobStatus::new(
                    1,
                    1,
                    HOME_JOB,
                    LocalDate::of(2009, 1, 1),
                    LocalDate::of(2099, 12, 31),
                    pay_type,
                    7.50,
                    true,
                ),
                EmployeeJobStatus::new(
                    2,
                    1,
                    SPECIAL_JOB,
                    LocalDate::of(2009, 1, 1),
                    LocalDate::of(2099, 12, 31),
                    pay_type,
                    7.50,
                    false,
                ),
            ],
        )
    }

    pub(super) fn shift(id: i32, job_id: i32, hours: f64, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 1, job_id, date, ShiftType::Actual, Vec::new())
            .with_net_hours(hours)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                date,
                Some(REGULAR),
                hours,
                0.0,
            )])
    }

    /// The `setup()` fixture: ten shifts, two of them on 2010-11-27, and two of
    /// them worked in the secondary job. The week is 11-22..11-28, the
    /// calculation opens on 11-22 (divergence 24), and the dataset start date is
    /// the anonymous subclass's 11-21.
    pub(super) fn card() -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(vec![
                shift(1, HOME_JOB, 8.0, nov(21)),
                shift(2, HOME_JOB, 8.0, nov(22)),
                shift(3, HOME_JOB, 6.32, nov(23)),
                shift(4, SPECIAL_JOB, 8.0, nov(24)),
                shift(5, HOME_JOB, 8.5, nov(25)),
                shift(6, HOME_JOB, 12.54, nov(26)),
                shift(7, HOME_JOB, 2.0, nov(27)),
                shift(8, SPECIAL_JOB, 6.0, nov(27)),
                shift(9, HOME_JOB, 8.5, nov(28)),
                shift(10, HOME_JOB, 8.0, nov(29)),
            ])
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(nov(22))
            .with_dataset_start_date(nov(21))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::CaExtSpJobHdr, rule_params)
    }

    /// `shiftHasCorrectHoursDistributions(shift, reg, ot, dt)` — bucket totals.
    pub(super) fn assert_hours(card: &TimeCardData, shift_index: usize, expected: (f64, f64, f64)) {
        let total = |type_id: i32| -> f64 {
            card.shifts()[shift_index]
                .hours_distributions()
                .iter()
                .filter(|d| d.is_of_type(type_id))
                .map(|d| d.hours())
                .sum()
        };

        assert_eq!(
            (total(REGULAR), total(OVERTIME), total(DOUBLE_TIME)),
            expected,
            "shift {}",
            shift_index + 1
        );
    }

    /// All seven days of the week, every day special.
    pub(super) const ALL_DAYS: &str = "[1,2,3,4,5,6,7]";

    #[test]
    fn the_weekly_limit_is_an_hours_weighted_average_of_the_two_limits() {
        // Two special shifts of 8 and 6 hours at a 35-hour week, the rest of a
        // 59.86-hour week at 40: (2324.4 / 59.86) rounds to 38.83.
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (SPEC_WEEKLY_LIMIT_PROP, "35"),
                (SPEC_DOW_PROP, ALL_DAYS),
                (SPEC_JOBS_PROP, "[22]"),
            ]),
        );

        // 11-27's first shift is paid 45.36 - 38.83 - 5.04 of weekly overtime.
        assert_hours(&card, 6, (0.51, 1.49, 0.0));
    }

    #[test]
    fn a_day_with_no_special_job_still_averages_over_both_shifts() {
        // 11-27 mixes a default job and a special one at different daily
        // limits: (2 * 8 + 6 * 7) / 8 = 7.25.
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (SPEC_DAILY_OT_LIMIT_PROP, "7"),
                (SPEC_DOW_PROP, ALL_DAYS),
                (SPEC_JOBS_PROP, "[22]"),
            ]),
        );

        // The 2-hour shift is under 7.25 and pays nothing; the 6-hour one
        // carries the day past it.
        assert_hours(&card, 6, (2.0, 0.0, 0.0));
        assert_hours(&card, 7, (0.68, 5.32, 0.0));
    }

    #[test]
    fn a_special_job_on_an_ordinary_day_gets_the_default_limits() {
        // The same special job and limit, but no day of week is configured.
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[(SPEC_DAILY_OT_LIMIT_PROP, "7"), (SPEC_JOBS_PROP, "[22]")]),
        );

        assert_hours(&card, 3, (8.0, 0.0, 0.0));
    }

    #[test]
    fn an_ordinary_job_on_a_special_day_gets_the_default_limits() {
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (SPEC_DAILY_OT_LIMIT_PROP, "7"),
                (SPEC_DOW_PROP, ALL_DAYS),
                (SPEC_JOBS_PROP, "[99]"),
            ]),
        );

        assert_hours(&card, 3, (8.0, 0.0, 0.0));
    }

    #[test]
    fn the_special_days_are_sunday_based_not_iso() {
        // 2010-11-24 is a Wednesday: ISO 3, which translates to 4.
        let special_job_daily_ot = |days: &str| {
            let mut card = card();
            rule().execute(
                &mut card,
                &week(),
                &item(&[
                    (SPEC_DAILY_OT_LIMIT_PROP, "7"),
                    (SPEC_DOW_PROP, days),
                    (SPEC_JOBS_PROP, "[22]"),
                ]),
            );
            card.shifts()[3]
                .hours_distributions()
                .iter()
                .filter(|d| d.is_of_type(OVERTIME))
                .map(|d| d.hours())
                .sum::<f64>()
        };

        assert_eq!(special_job_daily_ot("[4]"), 1.0, "Sunday-based Wednesday");
        assert_eq!(special_job_daily_ot("[3]"), 0.0, "the ISO number misses");
    }

    #[test]
    fn a_property_with_no_double_time_bucket_writes_nothing() {
        let mut card = card().with_hours_distribution_types(vec![
            HoursDistributionType::new(REGULAR, "Regular", false),
            HoursDistributionType::new(OVERTIME, "Overtime", true),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_hours(&card, 5, (12.54, 0.0, 0.0));
    }

    #[test]
    #[should_panic(expected = "not a JSON array")]
    fn a_malformed_special_day_list_aborts() {
        // getIdsFromJSON throws where getIdsListForKey would have swallowed.
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(SPEC_DOW_PROP, "nonsense")]));
    }
}

/// `CaliforniaExtSpecialJobOTHrsRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/CaliforniaExtSpecialJobOTHrsRuleImplTest.groovy`
/// — all nine cases, each asserting regular, overtime and double-time totals on
/// every one of the ten shifts.
///
/// The mocked `PayGroup` is divergence 24's derivation, so the calculation
/// start date is set on the card; the spec's anonymous subclass overriding
/// `getDatasetStartDate()` becomes `with_dataset_start_date`. The mocked
/// `PriorDaysCalculator` answering 4 becomes the `PriorDays(4)` stub.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{ALL_DAYS, assert_hours, card, item, rule, week};
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::time_card::TimeCardData;

    /// The ten `shiftHasCorrectHoursDistributions` lines of a case.
    fn assert_all(card: &TimeCardData, expected: [(f64, f64, f64); 10]) {
        for (index, hours) in expected.into_iter().enumerate() {
            assert_hours(card, index, hours);
        }
    }

    #[test]
    fn premium_hours_should_be_correctly_distributed_using_the_default_config() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0), // before the week, in a closed period
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.0, 0.54),
                (1.68, 0.32, 0.0),
                (0.0, 6.0, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0), // after the week
            ],
        );
    }

    #[test]
    fn consecutive_days_in_a_prior_week_should_count_if_the_flag_is_off() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (0.0, 8.0, 0.0),
                (0.0, 8.0, 0.5),
                (0.0, 8.0, 4.54),
                (0.0, 2.0, 0.0),
                (0.0, 6.0, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn no_dt_is_given_if_the_pay_dt_flag_is_off() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(PAY_DT_PROP, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.54, 0.0),
                (1.68, 0.32, 0.0),
                (0.0, 6.0, 0.0),
                (0.0, 8.5, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn overtime_should_be_given_for_hours_over_7_and_under_12_in_a_day() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(DAILY_OT_LIMIT_PROP, "7")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (7.0, 1.0, 0.0),
                (6.32, 0.0, 0.0),
                (7.0, 1.0, 0.0),
                (7.0, 1.5, 0.0),
                (7.0, 5.0, 0.54),
                (2.0, 0.0, 0.0),
                (3.68, 2.32, 0.0),
                (0.0, 7.0, 1.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn dt_should_be_given_for_hours_worked_over_11_in_a_day() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(DAILY_DT_LIMIT_PROP, "11")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 3.0, 1.54),
                (1.68, 0.32, 0.0),
                (0.0, 6.0, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn weekly_ot_should_be_given_if_more_than_45_hours_are_worked() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_LIMIT_PROP, "45")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.0, 0.54),
                (2.0, 0.0, 0.0),
                (4.68, 1.32, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn salaried_exempt_employees_should_not_be_given_any_premium_hours() {
        let mut card =
            card().with_employee(super::tests::employee(EmployeePayType::SalariedExempt));

        rule().execute(&mut card, &week(), &item(&[]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (12.54, 0.0, 0.0),
                (2.0, 0.0, 0.0),
                (6.0, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn weekly_overtime_for_a_special_job_should_be_given_after_35_hours_worked() {
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (SPEC_WEEKLY_LIMIT_PROP, "35"),
                (SPEC_DOW_PROP, ALL_DAYS),
                (SPEC_JOBS_PROP, "[22]"),
            ]),
        );

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.0, 0.54),
                (0.51, 1.49, 0.0),
                (0.0, 6.0, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn daily_ot_should_be_given_if_a_special_job_has_more_than_7_hours_worked_in_a_day() {
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (SPEC_DAILY_OT_LIMIT_PROP, "7"),
                (SPEC_DOW_PROP, ALL_DAYS),
                (SPEC_JOBS_PROP, "[22]"),
            ]),
        );

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (7.0, 1.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.0, 0.54),
                (2.0, 0.0, 0.0),
                (0.68, 5.32, 0.0),
                (0.0, 8.0, 0.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }
}
