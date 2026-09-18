//! Port of `DailyWeekly6thOT7thDTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyWeekly6thOT7thDTHrsRuleImpl.java`.
//!
//! `DW6OT7DT_HDR`. Daily and weekly overtime with **two** consecutive-day
//! thresholds rather than one: the sixth consecutive day pays overtime from the
//! first hour, the seventh pays double time. Fourteen parameters, the most of
//! any rule in the family.
//!
//! The day/shift/earning walk and the earning rewrite are
//! [`DailyWeekly7thDTHrs`](super::daily_weekly_7th_dt_hrs)'s, down to the pay
//! level read out of the pay set before the hours are counted. The
//! `dailyDataProducer` is the shared one — this is the **third** rule to
//! declare it identically; see
//! [`super::daily_data::build_daily_data_map`].
//!
//! # Two thresholds, and the gap between them
//!
//! The weekly accumulator carries `consecDayOtLimit` (6) and
//! `consecDayDtLimit` (7), and the two ranges it answers are disjoint:
//!
//! ```java
//! isDuringOTConsecDaysRange() -> counter >= otLimit && counter <  dtLimit
//! isDuringDTConsecDaysRange() -> counter >= dtLimit
//! ```
//!
//! So the sixth day is "OT range" and the seventh is "DT range", and a day is
//! never both. What each means:
//!
//! - **OT range** (the sixth day): the daily overtime limit is dropped, so
//!   every hour is overtime. Double time is paid only if `paySixthDayDT`, and
//!   then only past the daily **overtime** limit — `hours - dailyOTLimit`.
//! - **DT range** (the seventh): the daily limit is dropped for overtime *and*
//!   the whole day is double time, `hours - doubleTime`. Since `shiftDT` then
//!   equals `shiftOT`, the overtime row gets nothing.
//! - **Neither**: the ordinary daily limits apply.
//!
//! # `payDailyOT` suppresses the row but not the arithmetic
//!
//! `addDistributions` reassigns its **local** `premiumHours` when the flag is
//! clear — to `doubleTime` inside the consecutive-day branch, to `0` outside it
//! — so the regular row loses only what was actually written. The caller's copy
//! is untouched, so `addOvertime` and `addWeeklyOT` still see the whole figure
//! and the week still counts it as paid. The same shape as
//! [`DlyWklyOffConsecOTMinBreak`](super::dly_wkly_off_consec_ot_min_break)'s
//! `premiumHours -= dtHours`.
//!
//! # Double time is computed even when it cannot be written
//!
//! `computeDailyDoubleTime`'s last branch returns `hours - dailyDTLimit -
//! doubleTime` whether or not `payDailyDT` is set; the flag is only consulted
//! in `addDistributions`. So with the flag clear the figure is computed,
//! discarded, and then **still added to the day's double-time total** by the
//! unguarded `addDoubleTime` below. Pinned by
//! `double_time_accumulates_even_when_pay_daily_dt_is_clear`.
//!
//! # `isDuringDaysInWeekOtRange` mixes its two counters
//!
//! ```java
//! return daysInWeekCounter >= otConsecDaysLimit && consecutiveDaysCounter < dtConsecDayLimit;
//! ```
//!
//! The first half counts days worked **in this week**, the second consecutive
//! days. `overrideConsecDayOt` selects it in place of
//! `isDuringOTConsecDaysRange`, so a week with six non-consecutive worked days
//! reaches the sixth-day rule — but is still shut off by the seventh
//! *consecutive* day. `daysInWeekCounter` is never reset and never wrapped,
//! where the consecutive counter is both.
//!
//! # `bothConsecutiveAndWeeklyOt` is read and never used
//!
//! `initParams` assigns the field; nothing reads it. Fifth dead member in the
//! family, and the second for this parameter — `CaliforniaExtendedOTHrs` hands
//! it to an accumulator whose field nothing reads. Only `MinHrsForFullTimeOT`
//! does anything with it. The weekly formula here is selected by
//! `premiumHoursCountTowardsWeeklyOT` instead, whose default is `"true"`.

use crate::common::numbers::round_hours;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    BOTH_CONSECUTIVE_AND_WEEKLY_OT, CONSEC_DAY_DT_LIMIT, CONSEC_DAY_OT_LIMIT, CONSEC_DAYS_IN_WEEK,
    DAILY_DT_LIMIT_PROP, DAILY_OT_LIMIT_PROP, DailyWeekly6thOT7thDTHrsRuleConfig,
    EARNING_TYPE_PAY_SET, INCLUDE_EARNING_TYPE_PAY_MAP, MAX_CONSEC_DAYS_PD, OVERRIDE_CONSEC_DAY_OT,
    PAY_6TH_DAY_DT, PAY_DAILY_DT, PAY_DAILY_OT, PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT,
    WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::daily_data::build_daily_data_map;
use crate::rules::algorithm::hoursdistribution::prior_days_calculator::calculate_prior_consecutive_days;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::{EmployeeEarningPort, EmployeeShiftConsecutiveDaysPort};
use crate::rules::rule_config::RuleConfig;
use crate::rules::types::earning_type_pay_set::EarningTypePaySet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// The premium level of overtime within an `EarningTypePayMap`.
const OVERTIME_LEVEL: usize = 0;

/// The premium level of double time within an `EarningTypePayMap`.
const DOUBLE_TIME_LEVEL: usize = 1;

/// Sixth-day overtime and seventh-day double time.
/// `DailyWeekly6thOT7thDTHrsRuleImpl`.
#[derive(Debug, Default)]
pub struct DailyWeekly6thOT7thDTHrsRule<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort>
{
    consecutive_days: C,
    earnings: RefCell<P>,
}

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort>
    DailyWeekly6thOT7thDTHrsRule<C, P>
{
    /// Build the rule over the consecutive-day lookup and the earning port.
    pub fn new(consecutive_days: C, earnings: P) -> Self {
        Self {
            consecutive_days,
            earnings: RefCell::new(earnings),
        }
    }
}

/// The rule's fourteen parameters, less the two limits the day carries.
struct Settings {
    pay_daily_ot: bool,
    pay_daily_dt: bool,
    pay_6th_day_dt: bool,
    include_earning_type_pay_map: bool,
    override_consec_day_ot: bool,
    premium_hours_count_towards_weekly_ot: bool,
}

/// This rule's **private** `WeeklyAccumulator`.
#[derive(Debug, Default)]
struct WeekTotals {
    weekly_limit: f64,
    ot_consec_days_limit: i32,
    dt_consec_day_limit: i32,
    consecutive_days_modifier: i32,
    hours: f64,
    weekly_ot: f64,
    consecutive_days_counter: i32,
    /// Days worked in this week. Never reset, never wrapped.
    days_in_week_counter: i32,
    premium_hours_count_towards_weekly_ot: bool,
}

impl WeekTotals {
    /// `updateConsecutiveDays` — a day counts if it was worked, or carried a
    /// configured earning while `includeEarningTypePayMap` is set.
    fn update_consecutive_days(&mut self, day_was_worked: bool, has_earnings: bool, include: bool) {
        if day_was_worked || (include && has_earnings) {
            self.consecutive_days_counter += 1;
            self.days_in_week_counter += 1;

            if self.consecutive_days_counter % self.consecutive_days_modifier != 0 {
                self.consecutive_days_counter %= self.consecutive_days_modifier;
            }
        } else {
            self.consecutive_days_counter = 0;
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// `computeWeeklyOT(originalHours)` — two formulas behind
    /// `premiumHoursCountTowardsWeeklyOT`, rounded either way.
    fn compute_weekly_ot(&self, original_hours: f64) -> f64 {
        let pay_ot = self.premium_hours_count_towards_weekly_ot && self.hours > self.weekly_limit;
        let ot = if pay_ot {
            (self.hours - self.weekly_limit).min(original_hours)
        } else {
            self.hours - self.weekly_limit - self.weekly_ot
        };

        round_hours(ot).max(0.0)
    }

    /// The seventh day and beyond. `isDuringDTConsecDaysRange`.
    fn is_during_dt_consec_days_range(&self) -> bool {
        self.consecutive_days_counter >= self.dt_consec_day_limit
    }

    /// The sixth day only — the two ranges are disjoint.
    /// `isDuringOTConsecDaysRange`.
    fn is_during_ot_consec_days_range(&self) -> bool {
        self.consecutive_days_counter >= self.ot_consec_days_limit
            && self.consecutive_days_counter < self.dt_consec_day_limit
    }

    /// `isDuringDaysInWeekOtRange` — days worked in the week against the OT
    /// limit, but still shut off by the seventh **consecutive** day.
    fn is_during_days_in_week_ot_range(&self) -> bool {
        self.days_in_week_counter >= self.ot_consec_days_limit
            && self.consecutive_days_counter < self.dt_consec_day_limit
    }
}

/// This rule's **private** `DailyAccumulator`.
#[derive(Debug, Default)]
struct DayTotals {
    date: Option<LocalDate>,
    daily_ot_limit: f64,
    daily_dt_limit: f64,
    hours: f64,
    overtime: f64,
    double_time: f64,
}

impl DayTotals {
    fn new(date: LocalDate, daily_ot_limit: f64, daily_dt_limit: f64) -> Self {
        Self {
            date: Some(date),
            daily_ot_limit,
            daily_dt_limit,
            ..Self::default()
        }
    }

    fn date(&self) -> LocalDate {
        self.date.expect("a day's totals always carry their date")
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

    /// `isSixthDayOt` — which of the two sixth-day tests applies.
    fn is_sixth_day_ot(&self, week: &WeekTotals, override_consec_day_ot: bool) -> bool {
        if override_consec_day_ot {
            week.is_during_days_in_week_ot_range()
        } else {
            week.is_during_ot_consec_days_range()
        }
    }

    /// `computeDailyOvertime` — the whole day on the sixth or seventh, past the
    /// daily limit otherwise.
    fn compute_daily_overtime(&self, week: &WeekTotals, settings: &Settings) -> f64 {
        let pay_daily_ot = self.is_sixth_day_ot(week, settings.override_consec_day_ot)
            || week.is_during_dt_consec_days_range();

        let daily_ot = if pay_daily_ot {
            self.hours - self.overtime
        } else {
            self.hours - self.daily_ot_limit - self.overtime
        };

        round_hours(daily_ot).max(0.0)
    }

    /// `computeDailyDoubleTime` — three branches, and the last ignores
    /// `payDailyDT`; see the module note.
    fn compute_daily_double_time(&self, week: &WeekTotals, settings: &Settings) -> f64 {
        let daily_dt = if self.is_sixth_day_ot(week, settings.override_consec_day_ot) {
            if settings.pay_6th_day_dt {
                // Measured from the **overtime** limit, not the double-time one.
                self.hours - self.daily_ot_limit - self.double_time
            } else {
                0.0
            }
        } else if week.is_during_dt_consec_days_range() {
            self.hours - self.double_time
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

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort> HoursDistributionRule
    for DailyWeekly6thOT7thDTHrsRule<C, P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&DailyWeekly6thOT7thDTHrsRuleConfig.default_values());

        let settings = Settings {
            pay_daily_ot: params.bool_at(PAY_DAILY_OT),
            pay_daily_dt: params.bool_at(PAY_DAILY_DT),
            pay_6th_day_dt: params.bool_at(PAY_6TH_DAY_DT),
            include_earning_type_pay_map: params.bool_at(INCLUDE_EARNING_TYPE_PAY_MAP),
            override_consec_day_ot: params.bool_at(OVERRIDE_CONSEC_DAY_OT),
            premium_hours_count_towards_weekly_ot: params
                .bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT),
        };
        // Read into a field in Java, where nothing reads it back.
        let _both_consecutive_and_weekly_ot = params.bool_at(BOTH_CONSECUTIVE_AND_WEEKLY_OT);

        let pay_set =
            EarningTypePaySet::from_json_string(params.get(EARNING_TYPE_PAY_SET).unwrap_or(""));
        let configured: Vec<i32> = pay_set.configured_earning_type_ids().into_iter().collect();

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

        let ot_consec_days_limit = params.int_at(CONSEC_DAY_OT_LIMIT);
        let max_consec_days = params.int_at(MAX_CONSEC_DAYS_PD);
        let consecutive_days_modifier = ot_consec_days_limit + max_consec_days - 1;

        let initial_consecutive_days = if params.bool_at(CONSEC_DAYS_IN_WEEK) {
            0
        } else {
            calculate_prior_consecutive_days(
                time_card,
                &self.consecutive_days,
                work_week,
                &configured,
                consecutive_days_modifier,
                settings.include_earning_type_pay_map,
            ) as i32
        };

        let mut week = WeekTotals {
            weekly_limit: params.double_at(WEEKLY_LIMIT_PROP),
            ot_consec_days_limit,
            dt_consec_day_limit: params.int_at(CONSEC_DAY_DT_LIMIT),
            consecutive_days_modifier,
            consecutive_days_counter: initial_consecutive_days,
            premium_hours_count_towards_weekly_ot: settings.premium_hours_count_towards_weekly_ot,
            ..WeekTotals::default()
        };

        let daily_data_map =
            build_daily_data_map(time_card, work_week, &pay_set.configured_earning_type_ids());
        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt_limit = params.double_at(DAILY_DT_LIMIT_PROP);

        for date in work_week.dates() {
            let mut day = DayTotals::new(date, daily_ot_limit, daily_dt_limit);
            let daily_data = &daily_data_map[&date];

            week.update_consecutive_days(
                daily_data.was_worked(),
                !daily_data.earnings().is_empty(),
                settings.include_earning_type_pay_map,
            );

            self.compute_earning_hours(
                time_card,
                &mut week,
                &mut day,
                daily_data.earnings(),
                &pay_set,
                &settings,
                rule_item,
            );

            for shift_to_earnings in daily_data.shifts() {
                self.compute_earning_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    shift_to_earnings.earnings(),
                    &pay_set,
                    &settings,
                    rule_item,
                );
                compute_distribution_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    shift_to_earnings.shift(),
                    &buckets,
                    &settings,
                    rule_item.id(),
                );
            }
        }
    }
}

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort>
    DailyWeekly6thOT7thDTHrsRule<C, P>
{
    /// `computeEarningHours` — `DailyWeekly7thDTHrs`'s, pay level and all.
    #[allow(clippy::too_many_arguments)]
    fn compute_earning_hours(
        &self,
        time_card: &mut dyn TimeCard,
        week: &mut WeekTotals,
        day: &mut DayTotals,
        earning_indices: &[usize],
        pay_set: &EarningTypePaySet,
        settings: &Settings,
        rule_item: &RuleItem,
    ) {
        for &earning_index in earning_indices {
            let earning = &time_card.earnings()[earning_index];
            let hours = earning.hours();
            let pay_level = pay_set
                .pay_level_map()
                .get(&earning.earning_type_id())
                .copied()
                .unwrap_or(0);

            if pay_level >= 1 {
                day.add_overtime(hours);
                week.add_weekly_ot(hours);
            }
            if pay_level == 2 {
                day.add_double_time(hours);
            }

            day.add_hours(hours);
            week.add_hours(hours);

            let premium_hours = week
                .compute_weekly_ot(hours)
                .max(day.compute_daily_overtime(week, settings));
            let double_time = day.compute_daily_double_time(week, settings);

            self.add_earnings(
                time_card,
                earning_index,
                premium_hours,
                double_time,
                pay_set,
                rule_item,
            );

            day.add_overtime(premium_hours);
            day.add_double_time(double_time);
            week.add_weekly_ot(premium_hours);
        }
    }

    /// `addEarnings` — cancel at the regular type, pay back at the premium
    /// types. Note it consults neither `payDailyOT` nor `payDailyDT`.
    fn add_earnings(
        &self,
        time_card: &mut dyn TimeCard,
        earning_index: usize,
        premium_hours: f64,
        double_time_hours: f64,
        pay_set: &EarningTypePaySet,
        rule_item: &RuleItem,
    ) {
        let earning = &time_card.earnings()[earning_index];
        if !(time_card.is_open_for_editing_for_earning(earning) && premium_hours > 0.0) {
            return;
        }

        let regular_type_id = earning.earning_type_id();
        let premium_type_id =
            |level: usize| pay_set.premium_earning_type_id(regular_type_id, level);

        self.create_earning(
            time_card,
            earning_index,
            round_hours(-premium_hours),
            regular_type_id,
            rule_item,
        );

        if double_time_hours > 0.0 {
            self.create_earning(
                time_card,
                earning_index,
                double_time_hours,
                premium_type_id(DOUBLE_TIME_LEVEL),
                rule_item,
            );

            if premium_hours - double_time_hours > 0.0 {
                self.create_earning(
                    time_card,
                    earning_index,
                    round_hours(premium_hours - double_time_hours),
                    premium_type_id(OVERTIME_LEVEL),
                    rule_item,
                );
            }
        } else {
            self.create_earning(
                time_card,
                earning_index,
                premium_hours,
                premium_type_id(OVERTIME_LEVEL),
                rule_item,
            );
        }
    }

    /// `createEarning`. Divergence 42: the note is the rule item's name alone.
    fn create_earning(
        &self,
        time_card: &mut dyn TimeCard,
        source_index: usize,
        hours: f64,
        earning_type_id: i32,
        rule_item: &RuleItem,
    ) {
        let source = &time_card.earnings()[source_index];

        let mut earning = EmployeeEarning::new(
            0,
            source.employee_id(),
            source.job_id(),
            earning_type_id,
            source.earning_date(),
            hours,
            source.rate(),
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule(rule_item.id(), source.shift_id());

        earning.set_pay_date(Some(source.earning_date()));
        earning.set_note(rule_item.name());
        earning.calc_and_set_total_dollars();

        self.earnings.borrow_mut().save(earning.clone());
        time_card.earnings_mut().push(earning);
    }
}

/// `computeDistributionHours` — the shift's **regular** distributions dated
/// this day.
///
/// Note `distributionIsRegular()`, not `!distributionIsPremium()`: a
/// distribution with no bucket, or one naming a bucket the property has not
/// configured, is premium under that definition (see the `TimeCard` finding)
/// and is skipped here where the two California rules would have included it.
fn compute_distribution_hours(
    time_card: &mut dyn TimeCard,
    week: &mut WeekTotals,
    day: &mut DayTotals,
    shift_index: usize,
    buckets: &Buckets,
    settings: &Settings,
    rule_item_id: i32,
) {
    let date = day.date();

    let distribution_indices: Vec<usize> = time_card.shifts()[shift_index]
        .hours_distributions()
        .iter()
        .enumerate()
        .filter(|(_, distribution)| {
            time_card.distribution_is_regular(distribution) && distribution.date() == date
        })
        .map(|(index, _)| index)
        .collect();

    for distribution_index in distribution_indices {
        create_distribution(
            time_card,
            shift_index,
            distribution_index,
            buckets,
            settings,
            rule_item_id,
            week,
            day,
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
    settings: &Settings,
    rule_item_id: i32,
    week: &mut WeekTotals,
    day: &mut DayTotals,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let original_hours = distribution.original_hours();
    let date = distribution.date();

    day.add_hours(original_hours);
    week.add_hours(original_hours);

    let premium_hours = week
        .compute_weekly_ot(original_hours)
        .max(day.compute_daily_overtime(week, settings));
    let double_time = day.compute_daily_double_time(week, settings);

    // `addDistributions`.
    if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
        // Java reassigns its local copy when a row is suppressed; the
        // accumulators below still see the whole figure.
        let mut written = premium_hours;

        let shift = &mut time_card.shifts_mut()[shift_index];
        let premium =
            |hours: f64, bucket: i32, shift: &mut crate::entity::employee_shift::EmployeeShift| {
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

        if settings.pay_daily_dt
            || week.is_during_dt_consec_days_range()
            || week.is_during_ot_consec_days_range()
        {
            if double_time > 0.0 {
                premium(double_time, buckets.double_time, shift);
            }
            if premium_hours - double_time > 0.0 {
                if settings.pay_daily_ot {
                    premium(
                        round_hours(premium_hours - double_time),
                        buckets.overtime,
                        shift,
                    );
                } else {
                    written = double_time;
                }
            }
        } else if settings.pay_daily_ot {
            premium(premium_hours, buckets.overtime, shift);
        } else {
            written = 0.0;
        }

        let reduced =
            round_hours(shift.hours_distributions()[distribution_index].hours() - written);
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

    pub(super) const JOB: i32 = 11;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    #[derive(Debug, Default, Clone, Copy)]
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

    #[derive(Debug, Default)]
    pub(super) struct SavedEarnings(pub Vec<EmployeeEarning>);

    impl EmployeeEarningPort for SavedEarnings {
        fn save(&mut self, earning: EmployeeEarning) {
            self.0.push(earning);
        }
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            None
        }
    }

    pub(super) fn rule() -> DailyWeekly6thOT7thDTHrsRule<PriorDays, SavedEarnings> {
        DailyWeekly6thOT7thDTHrsRule::new(PriorDays(0), SavedEarnings::default())
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

    pub(super) fn shift(id: i32, hours: f64, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_net_hours(hours)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                date,
                Some(REGULAR),
                hours,
                0.0,
            )])
    }

    /// The `setup()` fixture: nine shifts, 2010-11-21 through 2010-11-29, the
    /// week 11-22..11-28, and the calculation opening on 11-22 — the pay period
    /// the mocked `PayGroup` returns (divergence 24).
    pub(super) fn card() -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(vec![
                shift(1, 8.0, nov(21)),
                shift(2, 8.0, nov(22)),
                shift(3, 6.32, nov(23)),
                shift(4, 8.0, nov(24)),
                shift(5, 8.5, nov(25)),
                shift(6, 12.54, nov(26)),
                shift(7, 8.5, nov(27)),
                shift(8, 8.5, nov(28)),
                shift(9, 8.0, nov(29)),
            ])
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(nov(22))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::Dw6ot7dtHdr, rule_params)
    }

    /// The **sum** of each bucket's rows on a shift.
    pub(super) fn hours_of(card: &TimeCardData, shift_index: usize) -> (f64, f64, f64) {
        let total = |type_id: i32| -> f64 {
            card.shifts()[shift_index]
                .hours_distributions()
                .iter()
                .filter(|d| d.is_of_type(type_id))
                .map(|d| d.hours())
                .sum()
        };
        (total(REGULAR), total(OVERTIME), total(DOUBLE_TIME))
    }

    pub(super) fn assert_hours(card: &TimeCardData, shift_index: usize, expected: (f64, f64, f64)) {
        assert_eq!(
            hours_of(card, shift_index),
            expected,
            "shift {}",
            shift_index + 1
        );
    }

    #[test]
    fn the_sixth_day_is_overtime_and_the_seventh_is_double_time() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[]));

        // 11-27 is the sixth consecutive day, 11-28 the seventh.
        assert_hours(&card, 6, (0.0, 8.0, 0.5));
        assert_hours(&card, 7, (0.0, 0.0, 8.5));
    }

    #[test]
    fn the_two_consecutive_day_ranges_are_disjoint() {
        // The seventh day is in the DT range, so `isDuringOTConsecDaysRange`
        // is false there and no overtime row is written at all.
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[]));

        assert!(
            !card.shifts()[7]
                .hours_distributions()
                .iter()
                .any(|d| d.is_of_type(OVERTIME)),
            "the seventh day writes no overtime row"
        );
    }

    #[test]
    fn double_time_accumulates_even_when_pay_daily_dt_is_clear() {
        // computeDailyDoubleTime's last branch ignores payDailyDT, and the
        // addDoubleTime below the write guard is unconditional — so the day's
        // double-time total advances although no row was written. On 11-26 that
        // shows up as the overtime row taking the whole 4.54 rather than 4.0.
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(PAY_DAILY_DT, "false")]));

        assert_hours(&card, 5, (8.0, 4.54, 0.0));
    }

    #[test]
    fn an_unworked_day_resets_the_consecutive_counter_but_not_the_week_counter() {
        // Drop 11-25. 11-28 is then only the **third** consecutive day, so
        // neither consecutive-day range applies: it is paid as overtime rather
        // than the seventh day's double time. The week is over 40 hours by
        // then, so the weekly measure claims the whole 8.5 where the daily one
        // would have claimed 0.5.
        let mut card = card();
        card.shifts_mut().retain(|s| s.shift_date() != nov(25));

        rule().execute(&mut card, &week(), &item(&[]));

        let last = card.shifts().len() - 2;
        assert_hours(&card, last, (0.0, 8.5, 0.0));
        assert!(
            !card.shifts()[last]
                .hours_distributions()
                .iter()
                .any(|d| d.is_of_type(DOUBLE_TIME)),
            "no double-time row, where the seventh day would write one"
        );
    }

    #[test]
    fn override_consec_day_ot_counts_days_in_the_week_not_consecutive_days() {
        // The same gap, with the override set: six days worked in the week
        // reaches the sixth-day rule even though they are not consecutive.
        let mut card = card();
        card.shifts_mut().retain(|s| s.shift_date() != nov(25));

        rule().execute(
            &mut card,
            &week(),
            &item(&[(OVERRIDE_CONSEC_DAY_OT, "true")]),
        );

        let last = card.shifts().len() - 2;
        assert_hours(&card, last, (0.0, 8.0, 0.5));
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
    fn an_untyped_distribution_is_not_regular_and_is_skipped() {
        // The filter is `distributionIsRegular()`, not `!distributionIsPremium()`.
        let mut card = card();
        card.shifts_mut()[1] =
            EmployeeShift::new(2, 1, JOB, nov(22), ShiftType::Actual, Vec::new())
                .with_net_hours(20.0)
                .with_hours_distributions(vec![HoursDistribution::new(
                    1,
                    nov(22),
                    None,
                    20.0,
                    0.0,
                )]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.shifts()[1].hours_distributions().len(), 1);
        assert_eq!(card.shifts()[1].hours_distributions()[0].hours(), 20.0);
    }
}

/// `DailyWeekly6thOT7thDTHrsRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyWeekly6thOT7thDTHrsRuleImplTest.groovy`
/// — all thirteen cases. They are written `def 'name'()` with single quotes, so
/// a search for `def "` finds none of them.
///
/// The fixture is `CaliforniaExtendedOTHrs`'s and `DailyWeekly7thDTHrs`'s: nine
/// shifts either side of the week 2010-11-22..28, with 11-27 raised from 8 to
/// 8.5 hours. The mocked `PayGroup` is divergence 24's derivation, so the
/// calculation start date is set on the card; 11-21 falls outside it.
///
/// # `shiftMatches` asserts nothing for a zero, and ignores its first argument
///
/// ```groovy
/// void shiftMatches(EmployeeShift shift, netHours, regHours, otHours, dtHours) {
///    if (regHours > 0) { assert … == regHours }
///    if (otHours  > 0) { assert … == otHours  }
///    if (dtHours  > 0) { assert … == dtHours  }
/// }
/// ```
///
/// So `shiftMatches(shift1, 8, 0, 0, 0)` — the line every case uses for the
/// two shifts outside the week, commented "should not calc" — asserts
/// **nothing at all**, and could not detect the rule paying them. `netHours` is
/// never read in the body either, so the first number is decorative in all 117
/// calls. The transcriptions assert the whole `(regular, overtime, double
/// time)` triple, zeros included.
///
/// That is the eighth spec-weakness recorded in this family, and the second
/// where a helper silently drops assertions — after `MinHrsForFullTimeOT`'s
/// `(0..N).each` closures.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{assert_hours, card, item, nov, rule, shift, week};
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::time_card::TimeCardData;

    /// The nine `shiftMatches` lines of a case, in shift order.
    fn assert_all(card: &TimeCardData, expected: [(f64, f64, f64); 9]) {
        for (index, hours) in expected.into_iter().enumerate() {
            assert_hours(card, index, hours);
        }
    }

    #[test]
    fn default_config_test() {
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
                (0.0, 8.0, 0.5), // the sixth consecutive day
                (0.0, 0.0, 8.5), // the seventh
                (8.0, 0.0, 0.0), // after the week
            ],
        );
    }

    #[test]
    fn no_daily_dt() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(PAY_DAILY_DT, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.54, 0.0),
                (0.0, 8.0, 0.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn no_daily_ot() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(PAY_DAILY_OT, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (12.0, 0.0, 0.54),
                (8.0, 0.0, 0.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn no_sixth_day_dt() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(PAY_6TH_DAY_DT, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (8.0, 4.0, 0.54),
                (0.0, 8.5, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn daily_ot_7() {
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
                (0.0, 7.0, 1.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn daily_dt_11() {
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
                (0.0, 8.0, 0.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn weekly_35() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_LIMIT_PROP, "35")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (4.18, 7.82, 0.54),
                (0.0, 8.0, 0.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn salaried() {
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
                (8.5, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn cross_workweek() {
        // `calculatePriorConsecutiveDays(...) >> 1`. The ported calculator
        // reaches 1 from the 11-21 shift the fixture already carries, so the
        // DAO stub stays at zero.
        let mut card = card().with_dataset_start_date(nov(21));

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (8.0, 0.5, 0.0),
                (0.0, 8.0, 4.54),
                (0.0, 0.0, 8.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn early_limits() {
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[(CONSEC_DAY_OT_LIMIT, "4"), (CONSEC_DAY_DT_LIMIT, "6")]),
        );

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (0.0, 8.0, 0.5),
                (0.0, 8.0, 4.54),
                (0.0, 0.0, 8.5),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn consec_days_resets_correctly() {
        let mut card = card();

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAY_OT_LIMIT, "2"),
                (CONSEC_DAY_DT_LIMIT, "3"),
                (MAX_CONSEC_DAYS_PD, "2"),
            ]),
        );

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (0.0, 6.32, 0.0),
                (0.0, 0.0, 8.0),
                (8.0, 0.5, 0.0),
                (0.0, 8.0, 4.54),
                (0.0, 0.0, 8.5),
                (0.0, 8.5, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn override_consec_day_ot_for_days_in_week() {
        // An extra 1-hour shift on 11-19, and the week moved to 11-19..11-25.
        let mut card = card();
        card.shifts_mut().push(shift(10, 1.0, nov(19)));
        let early_week = DateRange::new(nov(19), nov(25));

        rule().execute(
            &mut card,
            &early_week,
            &item(&[(OVERRIDE_CONSEC_DAY_OT, "true")]),
        );

        // The Groovy asserts the extra shift first; here it is last, since the
        // card holds it where it was pushed.
        assert_hours(&card, 9, (1.0, 0.0, 0.0));
        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (0.0, 8.0, 0.5),
                (12.54, 0.0, 0.0), // after this week, not calculated
                (8.5, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn when_both_consec_and_weekly_ot_is_checked_it_applies_both_parameters() {
        // An extra 8-hour shift on 11-20, and the week moved to 11-20..11-26.
        let mut card = card().with_dataset_start_date(nov(20));
        card.shifts_mut().push(shift(10, 8.0, nov(20)));
        let early_week = DateRange::new(nov(20), nov(26));

        rule().execute(
            &mut card,
            &early_week,
            &item(&[
                (CONSEC_DAYS_IN_WEEK, "false"),
                (BOTH_CONSECUTIVE_AND_WEEKLY_OT, "true"),
                (MAX_CONSEC_DAYS_PD, "2"),
                (PAY_DAILY_DT, "false"),
                (PAY_6TH_DAY_DT, "false"),
            ]),
        );

        assert_hours(&card, 9, (8.0, 0.0, 0.0));
        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (0.0, 8.5, 0.0),
                (0.0, 0.0, 12.54),
                (8.5, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }
}
