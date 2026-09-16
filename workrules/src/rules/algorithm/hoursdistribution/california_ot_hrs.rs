//! Port of `CaliforniaOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/CaliforniaOTHrsRuleImpl.java`.
//!
//! `CAL_HDR`. California overtime: a daily limit, a daily double-time limit, a
//! weekly limit, and the seventh consecutive day. The simpler sibling of
//! [`CaliforniaExtendedOTHrs`](super::california_extended_ot_hrs) — and the
//! first rule in the family to rewrite **earnings** as well as hours
//! distributions, and the only user of the shared
//! [`DailyData`](super::daily_data).
//!
//! # The shape
//!
//! For each date of the work week, in order:
//!
//! 1. a fresh [`DailyAccumulator`];
//! 2. advance the consecutive-day counter from whether the day was worked;
//! 3. the day's **standalone** earnings — those attached to no shift;
//! 4. then each shift worked that day, in start-time order: that shift's own
//!    earnings first, then its non-premium distributions dated that day.
//!
//! Earnings and distributions run through identical arithmetic against the same
//! two accumulators; only the write differs. An earning contributes
//! `getHours()`, a distribution its `getOriginalHours()`.
//!
//! # Weekly overtime is unreachable under the default configuration
//!
//! This rule never calls `weeklyAccumulator.setOriginalHours(...)`, where the
//! extended sibling calls it before every `computeWeeklyOT`. So `originalHours`
//! sits at zero inside the accumulator, and the formula
//! `premiumHoursCountTowardsWeeklyOT` selects —
//! `min(hours - weeklyLimit, originalHours)` — can only ever return zero.
//!
//! That flag **defaults to `"true"`**. So out of the box the weekly limit pays
//! nothing at all, and a property gets weekly overtime only by setting the flag
//! `false`, which switches to `hours - weeklyLimit - weeklyOT`. The Java spec
//! agrees: its one weekly case sets the flag explicitly, and would assert
//! nothing without it. Pinned in both directions by
//! `the_weekly_limit_pays_nothing_under_the_default_formula`.
//!
//! Daily overtime and double time are unaffected — they read the daily
//! accumulator, which this rule does feed.
//!
//! # The consecutive-day limits are hardcoded, and prior weeks are not seeded
//!
//! `consecDaysLimit` and `maxConsecDays` are `final` fields — 7 and 36500 — not
//! rule parameters, so [`CaliforniaOTHrsRuleConfig`] declares no key for
//! either. And the rule calls
//! [`update_consecutive_days_for_day`], the overload that resets on an unworked
//! day and increments otherwise but **never wraps at the modifier** — so
//! `maxConsecDays` only ever feeds a modifier nothing consults.
//!
//! There is no [`prior_days_calculator`](super::prior_days_calculator) call
//! either: the counter starts at zero on every `execute`, so the seventh-day
//! rule fires only for seven days worked inside this work week. The extended
//! sibling primes the counter from the days before the card; this one cannot.
//!
//! [`DailyAccumulator`]: super::daily_accumulator::DailyAccumulator
//! [`update_consecutive_days_for_day`]:
//!     super::weekly_accumulator::WeeklyAccumulator::update_consecutive_days_for_day
//!
//! # Earnings are opt-in, and silent by default
//!
//! Only earnings whose type appears in the `earningTypePaySet` parameter are
//! considered, and that parameter defaults to a pay set with no mappings at
//! all. So a property that has not configured one gets the distribution half of
//! this rule and none of the earning half. See
//! [`EarningTypePaySet`](crate::rules::types::earning_type_pay_set).
//!
//! # A premium earning is paid by adding three rows, not by editing one
//!
//! The source earning is left exactly as it was. Instead the rule writes a
//! **negative earning at the original type** cancelling the premium hours, and
//! then the premium hours back at the overtime and double-time types. The three
//! sum to zero hours at the regular type, which is how the reader can still see
//! where the hours came from — the same instinct as `HolidayDTHrs` zeroing its
//! regular row rather than removing it.
//!
//! Double time is carved **out of** the overtime total here too, not added to
//! it: the double-time row takes `shiftDT` and the overtime row the remainder.
//!
//! # The accumulators advance even when nothing is written
//!
//! Both `addDistributions` and `addEarnings` are gated on the date being open
//! for editing and on `premiumHours > 0`; the three `add*` calls that follow
//! each of them are not. So a distribution or earning in a closed pay period
//! still counts toward the day and the week, and its computed overtime is still
//! booked as already paid. Same shape as the extended sibling.
//!
//! [`CaliforniaOTHrsRuleConfig`]: super::config::CaliforniaOTHrsRuleConfig

use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_hours;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    CaliforniaOTHrsRuleConfig, DAILY_DT_LIMIT_PROP, DAILY_OT_LIMIT_PROP, EARNING_TYPE_PAY_SET,
    PAY_DT_PROP, PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::daily_accumulator::DailyAccumulator;
use crate::rules::algorithm::hoursdistribution::daily_data::{DailyData, ShiftWithEarnings};
use crate::rules::algorithm::hoursdistribution::weekly_accumulator::WeeklyAccumulator;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use crate::rules::types::earning_type_pay_set::EarningTypePaySet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::HashMap;

/// Consecutive days worked before the daily limits change.
/// `CaliforniaOTHrsRuleImpl.consecDaysLimit`, a `final` field.
const CONSEC_DAYS_LIMIT: i32 = 7;

/// `CaliforniaOTHrsRuleImpl.maxConsecDays`, a `final` field. It reaches only
/// the weekly accumulator's wrap modifier, which the overload this rule calls
/// never consults — a hundred years of days, so nothing wraps in practice
/// either.
const MAX_CONSEC_DAYS: i32 = 36500;

/// The premium level of overtime within an `EarningTypePayMap`.
const OVERTIME_LEVEL: usize = 0;

/// The premium level of double time within an `EarningTypePayMap`.
const DOUBLE_TIME_LEVEL: usize = 1;

/// California overtime over hours distributions and earnings alike.
/// `CaliforniaOTHrsRuleImpl`.
#[derive(Debug, Default)]
pub struct CaliforniaOTHrsRule<P: EmployeeEarningPort> {
    earnings: RefCell<P>,
}

impl<P: EmployeeEarningPort> CaliforniaOTHrsRule<P> {
    /// Build the rule over the port it persists new earnings through.
    ///
    /// `EmployeeEarningPort::save` needs `&mut self` and
    /// [`HoursDistributionRule::execute`] offers only `&self`, so the port is
    /// held behind a [`RefCell`]; the borrow lives no longer than the one call.
    pub fn new(earnings: P) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

/// The two bucket ids this rule writes into.
struct Buckets {
    overtime: i32,
    double_time: i32,
}

/// Everything `addEarnings` needs that is not on the accumulators.
struct EarningContext<'a> {
    pay_set: &'a EarningTypePaySet,
    pay_dt: bool,
    rule_item_id: i32,
    note: &'a str,
}

impl<P: EmployeeEarningPort> HoursDistributionRule for CaliforniaOTHrsRule<P> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&CaliforniaOTHrsRuleConfig.default_values());

        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt_limit = params.double_at(DAILY_DT_LIMIT_PROP);
        let weekly_limit = params.double_at(WEEKLY_LIMIT_PROP);
        let pay_dt = params.bool_at(PAY_DT_PROP);
        let premium_hours_count_towards_weekly_ot =
            params.bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT);

        // `paySet.setJSONString(params.get(EARNING_TYPE_PAY_SET))`, which throws
        // out of the rule on a malformed parameter.
        let pay_set =
            EarningTypePaySet::from_json_string(params.get(EARNING_TYPE_PAY_SET).unwrap_or(""));

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
        let context = EarningContext {
            pay_set: &pay_set,
            pay_dt,
            rule_item_id: rule_item.id(),
            note: rule_item.name(),
        };

        let daily_data_map = generate_daily_data_map(time_card, work_week, &pay_set);

        let mut weekly = WeeklyAccumulator::new(weekly_limit, CONSEC_DAYS_LIMIT, MAX_CONSEC_DAYS);
        weekly.set_premium_hours_count_towards_weekly_ot(premium_hours_count_towards_weekly_ot);

        for date in work_week.dates() {
            let mut daily = DailyAccumulator::new(date, daily_ot_limit, daily_dt_limit);
            let daily_data = &daily_data_map[&date];

            weekly.update_consecutive_days_for_day(daily_data.was_worked());

            self.compute_earning_hours(
                time_card,
                &mut weekly,
                &mut daily,
                daily_data.earnings(),
                &context,
            );

            // `computeShifts` — each shift's earnings, then its distributions.
            for shift_to_earnings in daily_data.shifts() {
                self.compute_earning_hours(
                    time_card,
                    &mut weekly,
                    &mut daily,
                    shift_to_earnings.earnings(),
                    &context,
                );
                compute_distribution_hours(
                    time_card,
                    &mut weekly,
                    &mut daily,
                    shift_to_earnings.shift(),
                    &buckets,
                    pay_dt,
                    rule_item.id(),
                );
            }
        }
    }
}

impl<P: EmployeeEarningPort> CaliforniaOTHrsRule<P> {
    /// `computeEarningHours` — the earning half of the same arithmetic the
    /// distributions get.
    fn compute_earning_hours(
        &self,
        time_card: &mut dyn TimeCard,
        weekly: &mut WeeklyAccumulator,
        daily: &mut DailyAccumulator,
        earning_indices: &[usize],
        context: &EarningContext<'_>,
    ) {
        for &earning_index in earning_indices {
            let hours = time_card.earnings()[earning_index].hours();

            daily.add_hours(hours);
            weekly.add_hours(hours);

            let (premium_hours, double_time) = compute_premium_hours(weekly, daily);

            self.add_earnings(
                time_card,
                earning_index,
                premium_hours,
                double_time,
                context,
            );

            // Outside the guard, deliberately — see the module note.
            daily.add_overtime(premium_hours);
            daily.add_double_time(double_time);
            weekly.add_weekly_ot(premium_hours);
        }
    }

    /// `addEarnings` — cancel the premium hours at the regular type, then pay
    /// them back at the premium types.
    fn add_earnings(
        &self,
        time_card: &mut dyn TimeCard,
        earning_index: usize,
        premium_hours: f64,
        double_time_hours: f64,
        context: &EarningContext<'_>,
    ) {
        let earning = &time_card.earnings()[earning_index];
        if !(time_card.is_open_for_editing_for_earning(earning) && premium_hours > 0.0) {
            return;
        }

        let regular_type_id = earning.earning_type_id();
        let premium_type_id = |level: usize| {
            context
                .pay_set
                .premium_earning_type_id(regular_type_id, level)
        };

        self.create_earning(
            time_card,
            earning_index,
            // `roundHours(premiumHours * -1)`.
            round_hours(-premium_hours),
            regular_type_id,
            context,
        );

        if context.pay_dt && double_time_hours > 0.0 {
            self.create_earning(
                time_card,
                earning_index,
                double_time_hours,
                premium_type_id(DOUBLE_TIME_LEVEL),
                context,
            );

            if premium_hours - double_time_hours > 0.0 {
                self.create_earning(
                    time_card,
                    earning_index,
                    round_hours(premium_hours - double_time_hours),
                    premium_type_id(OVERTIME_LEVEL),
                    context,
                );
            }
        } else {
            self.create_earning(
                time_card,
                earning_index,
                premium_hours,
                premium_type_id(OVERTIME_LEVEL),
                context,
            );
        }
    }

    /// `createEarning` — a copy of the source earning at a new type and hours.
    ///
    /// Java persists through `employeeEarningDAO.makePersistent` and then adds
    /// the same object to the card; here the card takes it and the port takes a
    /// copy. The id is zero, which is what Java's `int` holds before the DAO
    /// assigns one.
    fn create_earning(
        &self,
        time_card: &mut dyn TimeCard,
        source_index: usize,
        hours: f64,
        earning_type_id: i32,
        context: &EarningContext<'_>,
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
            EarningSource::Rule,
        )
        .from_rule(context.rule_item_id, source.shift_id());

        // `setPayDate(earning.getEarningDate())` — the pay date is the earning
        // date, not the source's own pay date.
        earning.set_pay_date(Some(source.earning_date()));
        earning.set_note(context.note);
        earning.calc_and_set_total_dollars();

        self.earnings.borrow_mut().save(earning.clone());
        time_card.earnings_mut().push(earning);
    }
}

/// `dailyDataProducer` — the week split into days, each with its standalone
/// earnings and its shifts with theirs.
///
/// The earnings are partitioned on whether they name a shift, so an earning
/// reaches the arithmetic through exactly one of the two paths. That is what
/// the Java spec's `an earning is not processed twice` turns on.
fn generate_daily_data_map(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    pay_set: &EarningTypePaySet,
) -> HashMap<LocalDate, DailyData> {
    let configured = pay_set.configured_earning_type_ids();

    // The index form of `getEarningsForPeriod`, filtered by
    // `earningIsIncludedInRegularTypes.and(employeeJobStatusIsNotSalariedExemptForEarning)`.
    // Java dereferences the job status unguarded; divergence 32 settled that a
    // missing one excludes the record.
    let mut standalone: HashMap<LocalDate, Vec<usize>> = HashMap::new();
    let mut by_shift: HashMap<i32, Vec<usize>> = HashMap::new();

    for (index, earning) in time_card.earnings().iter().enumerate() {
        if !work_week.contains_date(earning.earning_date())
            || !configured.contains(&earning.earning_type_id())
            || !time_card.earning_is_not_salaried_exempt(earning)
        {
            continue;
        }

        match earning.shift_id() {
            Some(shift_id) => by_shift.entry(shift_id).or_default().push(index),
            None => standalone
                .entry(earning.earning_date())
                .or_default()
                .push(index),
        }
    }

    work_week
        .dates()
        .into_iter()
        .map(|date| {
            let shifts = build_shifts_for_date(time_card, date, &by_shift);
            let data = DailyData::new(
                date,
                standalone.get(&date).cloned().unwrap_or_default(),
                shifts,
            );
            (date, data)
        })
        .collect()
}

/// `buildShiftForDateMap` — the day's non-exempt shifts in start-time order,
/// each paired with the earnings attached to it.
///
/// A shift appears under **every** date it has a distribution for, so an
/// overnight shift is visited once per day it spans — and so are its earnings.
fn build_shifts_for_date(
    time_card: &dyn TimeCard,
    date: LocalDate,
    by_shift: &HashMap<i32, Vec<usize>>,
) -> Vec<ShiftWithEarnings> {
    let single_day = DateRange::new(date, date);

    let mut indices = time_card.shift_indices_with_distributions_for_period(&single_day);
    indices.retain(|&index| time_card.shift_is_not_salaried_exempt(&time_card.shifts()[index]));
    // `ShiftStartTimeComparator`, which calls two shifts on the same date with
    // no start time equal — so ties keep the card's own order, and the sort
    // has to be stable.
    indices.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

    indices
        .into_iter()
        .map(|index| {
            let shift_id = time_card.shifts()[index].id();
            ShiftWithEarnings::new(index, by_shift.get(&shift_id).cloned().unwrap_or_default())
        })
        .collect()
}

/// `computeDistributionHours` — the shift's non-premium distributions dated
/// this day, in the order the shift holds them.
fn compute_distribution_hours(
    time_card: &mut dyn TimeCard,
    weekly: &mut WeeklyAccumulator,
    daily: &mut DailyAccumulator,
    shift_index: usize,
    buckets: &Buckets,
    pay_dt: bool,
    rule_item_id: i32,
) {
    let date = daily.date();

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
            pay_dt,
            rule_item_id,
            weekly,
            daily,
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

    let (premium_hours, double_time) = compute_premium_hours(weekly, daily);

    // `addDistributions`.
    if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
        let shift = &mut time_card.shifts_mut()[shift_index];
        let mut premium = |hours: f64, bucket: i32| {
            let row = create_premium_distribution(
                &shift.hours_distributions()[distribution_index],
                bucket,
                hours,
                Some(rule_item_id),
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

    // Outside the guard, deliberately — see the module note.
    daily.add_overtime(premium_hours);
    daily.add_double_time(double_time);
    weekly.add_weekly_ot(premium_hours);
}

/// `computePremiumHours` — the larger of the two overtime measures, and the
/// daily double time carved out of it.
///
/// An hour that is both daily and weekly overtime is paid once. Note that the
/// weekly half is dead under the default configuration; see the module note.
fn compute_premium_hours(weekly: &WeeklyAccumulator, daily: &DailyAccumulator) -> (f64, f64) {
    let shift_ot = weekly
        .compute_weekly_ot()
        .max(daily.compute_daily_overtime(weekly));
    let shift_dt = daily.compute_daily_double_time(weekly);

    (shift_ot, shift_dt)
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
    use crate::rules::types::earning_type_pay_map::EarningTypePayMap;

    pub(super) const JOB: i32 = 11;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// The Groovy's three earning types, which share their ids with the
    /// distribution buckets above without being the same thing.
    pub(super) const EARNING_REGULAR: i32 = 1;
    pub(super) const EARNING_OVERTIME: i32 = 2;
    pub(super) const EARNING_DOUBLE_TIME: i32 = 3;

    /// `Mock(EmployeeEarningDAO)` — it collects what `makePersistent` is given.
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

    pub(super) fn rule() -> CaliforniaOTHrsRule<SavedEarnings> {
        CaliforniaOTHrsRule::new(SavedEarnings::default())
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

    /// `new EarningTypePaySet(defaultRegularEarningTypeID: 1,
    /// earningTypePayMapSet: [new EarningTypePayMap(regularEarningTypeID: 1,
    /// premiumEarningTypeIDs: [2, 3])])`.
    pub(super) fn pay_set_json() -> String {
        EarningTypePaySet::new()
            .with_default_regular_earning_type_id(EARNING_REGULAR)
            .with_pay_maps(vec![EarningTypePayMap::new(
                EARNING_REGULAR,
                vec![EARNING_OVERTIME, EARNING_DOUBLE_TIME],
            )])
            .to_json_string()
    }

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    /// A shift starting at `hour` on its date, carrying all its hours as one
    /// regular distribution.
    pub(super) fn shift_at(id: i32, date: LocalDate, hour: i64, hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_times(Some(date.at_start_of_day().plus_hours(hour)), None)
            .with_net_hours(hours)
            .with_hours_distributions(vec![regular(date, hours)])
    }

    /// A standalone earning — one attached to no shift.
    pub(super) fn earning(id: i32, date: LocalDate, type_id: i32, hours: f64) -> EmployeeEarning {
        EmployeeEarning::new(
            id,
            1,
            JOB,
            type_id,
            date,
            hours,
            10.0,
            crate::common::enums::earning_source::EarningSource::Auto,
        )
    }

    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
    }

    /// The rule item, always carrying the pay set the Groovy configures.
    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        rule_params.set(EARNING_TYPE_PAY_SET, pay_set_json());
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(99, 1, "ruleItem", RuleClass::CaliforniaHdr, rule_params)
    }

    /// The hours on a shift's row of a given bucket, or zero if it has none.
    pub(super) fn hours_of(card: &TimeCardData, shift_index: usize, type_id: i32) -> f64 {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .find(|d| d.is_of_type(type_id))
            .map_or(0.0, |d| d.hours())
    }

    /// Every earning of a type, by hours, in the order the card holds them.
    pub(super) fn earning_hours_of(card: &TimeCardData, type_id: i32) -> Vec<f64> {
        card.earnings()
            .iter()
            .filter(|e| e.earning_type_id() == type_id)
            .map(EmployeeEarning::hours)
            .collect()
    }

    pub(super) fn day(offset: i64) -> LocalDate {
        LocalDate::of(2010, 11, 22).plus_days(offset)
    }

    pub(super) fn week() -> DateRange {
        DateRange::new(day(0), day(6))
    }

    /// A card whose calculation opens on the first day of the week.
    fn open_card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        card(shifts).with_calculation_start_date(day(0))
    }

    #[test]
    fn the_weekly_limit_pays_nothing_under_the_default_formula() {
        // The rule never calls setOriginalHours, so the formula
        // premiumHoursCountTowardsWeeklyOT selects is min(hours - limit, 0).
        // Fifty hours against a forty-hour week, with the daily limit raised
        // out of the way so only the weekly measure can fire.
        let shifts = || {
            (0..5)
                .map(|d| shift_at(d as i32 + 1, day(d), 9, 10.0))
                .collect()
        };
        let params: &[(&str, &str)] = &[(DAILY_OT_LIMIT_PROP, "24")];

        let mut defaulted = open_card(shifts());
        rule().execute(&mut defaulted, &week(), &item(params));

        assert_eq!(hours_of(&defaulted, 4, REGULAR), 10.0);
        assert_eq!(
            hours_of(&defaulted, 4, OVERTIME),
            0.0,
            "the default is `true`, and that formula caps at zero original hours"
        );

        let mut flag_cleared = open_card(shifts());
        rule().execute(
            &mut flag_cleared,
            &week(),
            &item(&[
                (DAILY_OT_LIMIT_PROP, "24"),
                (PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false"),
            ]),
        );

        assert_eq!(hours_of(&flag_cleared, 4, REGULAR), 0.0);
        assert_eq!(hours_of(&flag_cleared, 4, OVERTIME), 10.0, "50 - 40");
    }

    #[test]
    fn seven_consecutive_days_suspend_the_daily_overtime_limit() {
        // The limit day pays every hour as overtime, and double time starts at
        // the *overtime* limit rather than the double-time one.
        let mut shifts: Vec<EmployeeShift> = (0..6)
            .map(|d| shift_at(d as i32 + 1, day(d), 9, 4.0))
            .collect();
        shifts.push(shift_at(7, day(6), 9, 10.0));
        let mut card = open_card(shifts);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 6, REGULAR), 0.0);
        assert_eq!(hours_of(&card, 6, OVERTIME), 8.0);
        assert_eq!(hours_of(&card, 6, DOUBLE_TIME), 2.0);
    }

    #[test]
    fn the_consecutive_day_counter_resets_on_an_unworked_day() {
        // The same week with the fourth day off: six worked days, so the
        // seventh-day rule never fires and the last day is ordinary overtime.
        let mut shifts: Vec<EmployeeShift> = (0..6)
            .filter(|d| *d != 3)
            .map(|d| shift_at(d as i32 + 1, day(d), 9, 4.0))
            .collect();
        shifts.push(shift_at(7, day(6), 9, 10.0));
        let mut card = open_card(shifts);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 5, REGULAR), 8.0);
        assert_eq!(hours_of(&card, 5, OVERTIME), 2.0, "10 - the daily limit");
        assert_eq!(hours_of(&card, 5, DOUBLE_TIME), 0.0);
    }

    #[test]
    fn an_earning_only_day_breaks_the_consecutive_day_run() {
        // `was_worked()` reads the shifts and not the earnings, so a day
        // carrying a holiday earning and no shift still resets the counter —
        // even though the earning's hours do count toward the week.
        let mut shifts: Vec<EmployeeShift> = (0..6)
            .filter(|d| *d != 3)
            .map(|d| shift_at(d as i32 + 1, day(d), 9, 4.0))
            .collect();
        shifts.push(shift_at(7, day(6), 9, 10.0));
        let mut card =
            open_card(shifts).with_earnings(vec![earning(1, day(3), EARNING_REGULAR, 4.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 5, OVERTIME), 2.0);
        assert_eq!(hours_of(&card, 5, DOUBLE_TIME), 0.0, "the run was broken");
    }

    #[test]
    fn prior_weeks_do_not_seed_the_consecutive_day_counter() {
        // Unlike CaliforniaExtendedOTHrs there is no PriorDaysCalculator here:
        // the counter starts at zero every execute, so seven days worked before
        // the week count for nothing.
        let mut shifts: Vec<EmployeeShift> = (1..=7)
            .map(|d| shift_at(d as i32, day(-d), 9, 8.0))
            .collect();
        shifts.extend((1..6).map(|d| shift_at(d as i32 + 10, day(d), 9, 4.0)));
        shifts.push(shift_at(20, day(6), 9, 10.0));
        let mut card = open_card(shifts).with_calculation_start_date(day(-7));

        rule().execute(&mut card, &week(), &item(&[]));

        let last = card.shifts().len() - 1;
        assert_eq!(hours_of(&card, last, OVERTIME), 2.0);
        assert_eq!(
            hours_of(&card, last, DOUBLE_TIME),
            0.0,
            "six worked days inside the week, not thirteen"
        );
    }

    #[test]
    fn an_empty_pay_set_processes_no_earnings() {
        // The default from HoursDistributionRuleWithPayMappingsConfig.
        let mut card =
            open_card(Vec::new()).with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(
                EARNING_TYPE_PAY_SET,
                &EarningTypePaySet::new()
                    .with_premium_levels(2)
                    .to_json_string(),
            )]),
        );

        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.earnings()[0].hours(), 14.0);
    }

    #[test]
    fn an_earning_of_an_unconfigured_type_is_left_alone() {
        let mut card = open_card(Vec::new()).with_earnings(vec![earning(1, day(0), 99, 14.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.earnings().len(), 1);
    }

    #[test]
    fn a_closed_day_is_left_alone_but_still_counts_toward_the_week() {
        // The open-for-editing guard wraps only the write; the accumulators
        // advance regardless. Weekly limit 5 with the daily limit out of the
        // way, so the second day's overtime is 20 - 5 - 5 and not 20 - 5.
        let mut card = card(vec![
            shift_at(1, day(0), 9, 10.0),
            shift_at(2, day(1), 9, 10.0),
        ])
        .with_calculation_start_date(day(1));

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (WEEKLY_LIMIT_PROP, "5.0"),
                (DAILY_OT_LIMIT_PROP, "24"),
                (PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false"),
            ]),
        );

        assert_eq!(hours_of(&card, 0, REGULAR), 10.0, "closed, untouched");
        assert_eq!(hours_of(&card, 0, OVERTIME), 0.0);
        assert_eq!(hours_of(&card, 1, REGULAR), 0.0);
        assert_eq!(
            hours_of(&card, 1, OVERTIME),
            10.0,
            "the closed day's five hours of overtime were already booked"
        );
    }

    #[test]
    fn a_property_with_no_double_time_bucket_writes_nothing() {
        // Java unboxes a null Integer into an int field in initParameters and
        // throws before the rule does anything; divergence 37.
        let mut card =
            open_card(vec![shift_at(1, day(0), 9, 14.0)]).with_hours_distribution_types(vec![
                HoursDistributionType::new(REGULAR, "Regular", false),
                HoursDistributionType::new(OVERTIME, "Overtime", true),
            ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 0, REGULAR), 14.0);
        assert_eq!(hours_of(&card, 0, OVERTIME), 0.0);
    }

    #[test]
    fn the_premium_earnings_record_the_rule_item_and_net_to_the_original() {
        let mut card =
            open_card(Vec::new()).with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        let created: Vec<&EmployeeEarning> =
            card.earnings().iter().filter(|e| e.id() == 0).collect();
        assert_eq!(created.len(), 3);

        for row in &created {
            assert_eq!(row.rule_item_id(), Some(99));
            assert_eq!(row.source(), EarningSource::Rule);
            // Divergence 42: Java writes "<ruleSet> - <ruleItem>".
            assert_eq!(row.note(), "ruleItem");
            assert_eq!(row.pay_date(), Some(day(0)));
            assert_eq!(row.rate(), 10.0, "the source earning's rate");
            assert_eq!(
                row.shift_id(),
                None,
                "a standalone earning stays standalone"
            );
        }

        assert_eq!(
            earning_hours_of(&card, EARNING_REGULAR).iter().sum::<f64>(),
            8.0,
            "14 paid, 6 cancelled — the hours left at the regular type"
        );
    }

    #[test]
    fn the_created_earnings_are_persisted_through_the_port() {
        let mut card =
            open_card(Vec::new()).with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);
        let rule = rule();

        rule.execute(&mut card, &week(), &item(&[]));

        assert_eq!(rule.earnings.borrow().0.len(), 3);
    }

    #[test]
    fn a_premium_earning_carries_its_money() {
        // createEarning ends in calcAndSetTotalDollars.
        let mut card =
            open_card(Vec::new()).with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        let overtime = card
            .earnings()
            .iter()
            .find(|e| e.earning_type_id() == EARNING_OVERTIME)
            .unwrap();

        assert_eq!(overtime.hours(), 4.0);
        assert_eq!(overtime.total_dollars(), 40.0);
    }

    #[test]
    fn a_salaried_exempt_employee_is_skipped_entirely() {
        let mut card = card(vec![shift_at(1, day(0), 9, 14.0)])
            .with_employee(employee(EmployeePayType::SalariedExempt))
            .with_calculation_start_date(day(0))
            .with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(hours_of(&card, 0, REGULAR), 14.0);
        assert_eq!(card.earnings().len(), 1);
    }
}

/// `CaliforniaOTHrsRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/CaliforniaOTHrsRuleImplTest.groovy`
/// — the file is named `…Test` and the class inside it is
/// `CaliforniaOTHrsRuleImplSpec`. All six cases.
///
/// Three adjustments, none touching what is asserted:
///
/// - The spec anchors every date on `LocalDate.now()`; these pin it to
///   2010-11-22, as `punchvalidation`'s and `EarningMapperTest`'s
///   transcriptions did and for the same reason.
/// - Its work week is `ArbitraryDateRange.of(today, today.plusWeeks(1))` —
///   **eight** days, not seven. Kept, because the consecutive-day counter runs
///   over every date the week holds.
/// - The mocked `PayGroup` supplying the calculation start date is
///   divergence 24's derivation, so the resulting date is set on the card
///   directly; it is the pay period's start, which is `today`.
///
/// The two DAO mocks are replaced by the `SavedEarnings` port stub and, for
/// `earningTypeDAO.findByID`, by nothing at all: an earning here carries its
/// type as an id, so the lookup the Java needs to attach an entity has no work
/// to do.
///
/// # Two of the six assert only collection sizes
///
/// `spanning shifts hours in the previous period are not counted to the ot
/// threshold` and `an earning is not processed twice` check
/// `hoursDistributions.size()` and `earnings.size()` and nothing else. The
/// first turns on a week totalling exactly forty hours against a forty-hour
/// limit — that is, on `hours - weeklyLimit` being strict — so these
/// transcriptions assert the hours beside the counts, as the family's other
/// transcriptions do wherever the original was weak.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        DOUBLE_TIME, EARNING_DOUBLE_TIME, EARNING_OVERTIME, EARNING_REGULAR, JOB, OVERTIME,
        REGULAR, card, earning, earning_hours_of, hours_of, item, regular, rule, shift_at,
    };
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::time_card::TimeCardData;

    /// `static def today = LocalDate.now()`, pinned.
    fn day(offset: i64) -> LocalDate {
        LocalDate::of(2010, 11, 22).plus_days(offset)
    }

    /// `ArbitraryDateRange.of(today, today.plusWeeks(1))` — eight days.
    fn week() -> DateRange {
        DateRange::new(day(0), day(7))
    }

    /// `setup()`: an `ActualsTimeCard` with no shifts or earnings and the
    /// system-generated distribution types, opened by the pay period the mocked
    /// pay group returns.
    fn fixture(shifts: Vec<EmployeeShift>) -> TimeCardData {
        card(shifts)
            .with_current_pay_period(week())
            .with_calculation_start_date(day(0))
    }

    /// A shift whose hours are split across two dates, as an overnight one is.
    fn spanning(id: i32, date: LocalDate, hour: i64, first: f64, second: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_times(Some(date.at_start_of_day().plus_hours(hour)), None)
            .with_hours_distributions(vec![
                regular(date, first),
                regular(date.plus_days(1), second),
            ])
    }

    #[test]
    fn earning_hours_are_reallocated_to_premium_earnings_if_the_hours_cross_the_ot_dt_threshold() {
        let mut card =
            fixture(Vec::new()).with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 14.0)]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.earnings().len(), 4);

        // "negative earning is created to be reallocated to premium earnings",
        // beside the original, which is left untouched.
        let mut regular_hours = earning_hours_of(&card, EARNING_REGULAR);
        regular_hours.sort_by(f64::total_cmp);
        assert_eq!(regular_hours, vec![-6.0, 14.0]);

        // "an overtime earning is created for hours above the daily ot
        // threshold and below the dt threshold"
        assert_eq!(earning_hours_of(&card, EARNING_OVERTIME), vec![4.0]);

        // "a double time earning is created for hours above the dt threshold"
        assert_eq!(earning_hours_of(&card, EARNING_DOUBLE_TIME), vec![2.0]);
    }

    #[test]
    fn working_over_the_daily_ot_dt_limits_distributes_ot_dt() {
        // The day runs 4 (the earning) + 6 (otShift) + 6 (dtShift) = 16.
        let mut card = fixture(vec![
            shift_at(1, day(0), 0, 6.0),
            shift_at(2, day(0), 9, 6.0),
        ])
        .with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 4.0).with_shift(1)]);

        rule().execute(&mut card, &week(), &item(&[]));

        // "an overtime distribution should be created"
        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);
        assert_eq!(hours_of(&card, 0, REGULAR), 4.0);
        assert_eq!(hours_of(&card, 0, OVERTIME), 2.0);

        // "a double time earning should be created" — a distribution, in fact.
        assert_eq!(card.shifts()[1].hours_distributions().len(), 3);
        assert_eq!(hours_of(&card, 1, REGULAR), 0.0);
        assert_eq!(hours_of(&card, 1, OVERTIME), 2.0);
        assert_eq!(hours_of(&card, 1, DOUBLE_TIME), 4.0);

        // "No additional earnings are created" — at four hours the day had not
        // yet reached the daily limit.
        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.earnings()[0].hours(), 4.0);
    }

    #[test]
    fn overtime_is_payed_if_the_employee_works_over_the_week_threshold() {
        // 10 + 10 + 10 (the earning) + 8 + 6 = 44 against a forty-hour week,
        // with the daily limit raised to 24 so only the weekly measure fires.
        let mut card = fixture(vec![
            shift_at(1, day(0), 9, 10.0),
            shift_at(2, day(1), 9, 10.0),
            shift_at(3, day(3), 9, 8.0),
            shift_at(4, day(4), 9, 6.0),
        ])
        .with_earnings(vec![
            earning(1, day(0), EARNING_REGULAR, 10.0).with_shift(1),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (DAILY_OT_LIMIT_PROP, "24"),
                (PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false"),
            ]),
        );

        assert_eq!(card.shifts()[3].hours_distributions().len(), 2);
        assert_eq!(hours_of(&card, 3, OVERTIME), 4.0);
    }

    #[test]
    fn working_7_days_consecutively_distributes_all_hours_under_the_day_threshold_as_ot_and_over_as_dt()
     {
        let mut shifts: Vec<EmployeeShift> = (0..6)
            .map(|offset| shift_at(offset as i32 + 1, day(offset), 9, 4.0))
            .collect();
        shifts.push(shift_at(7, day(6), 9, 10.0));
        let mut card = fixture(shifts);

        rule().execute(&mut card, &week(), &item(&[]));

        // "there are 3 distributions on the shift"
        assert_eq!(card.shifts()[6].hours_distributions().len(), 3);
        assert_eq!(hours_of(&card, 6, REGULAR), 0.0);
        // "an overtime distribution is created"
        assert_eq!(hours_of(&card, 6, OVERTIME), 8.0);
        // "a double time distribution is created"
        assert_eq!(hours_of(&card, 6, DOUBLE_TIME), 2.0);
    }

    #[test]
    fn spanning_shifts_hours_in_the_previous_period_are_not_counted_to_the_ot_threshold() {
        // The overnight shift is dated the day before the week and distributes
        // 4 hours there and 5 into the first day of it. The week therefore sees
        // 5 + 8 + 8 + 8 + 8 + 3 = 40 — exactly the weekly limit, and `hours -
        // weeklyLimit` is strict, so nothing is paid.
        let mut card = fixture(vec![
            spanning(1, day(-1), 20, 4.0, 5.0),
            shift_at(2, day(1), 9, 8.0),
            shift_at(3, day(2), 9, 8.0),
            shift_at(4, day(3), 9, 8.0),
            shift_at(5, day(4), 9, 8.0),
            shift_at(6, day(5), 9, 3.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);
        assert_eq!(card.shifts()[5].hours_distributions().len(), 1);

        // Asserted here and not in the Groovy: the hours are untouched, and the
        // four hours dated outside the week never entered the count.
        let spanning_rows = card.shifts()[0].hours_distributions();
        assert_eq!(
            (spanning_rows[0].hours(), spanning_rows[1].hours()),
            (4.0, 5.0)
        );
        assert_eq!(hours_of(&card, 5, REGULAR), 3.0);
    }

    #[test]
    fn an_earning_is_not_processed_twice() {
        // The earning names a shift, so it reaches the arithmetic through that
        // shift's list and not through the day's standalone earnings. Four plus
        // the shift's three in-week hours is under the daily limit.
        let mut card = fixture(vec![spanning(1, day(-1), 20, 4.0, 3.0)])
            .with_earnings(vec![earning(1, day(0), EARNING_REGULAR, 4.0).with_shift(1)]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);

        // Asserted here and not in the Groovy.
        let rows = card.shifts()[0].hours_distributions();
        assert_eq!((rows[0].hours(), rows[1].hours()), (4.0, 3.0));
    }
}
