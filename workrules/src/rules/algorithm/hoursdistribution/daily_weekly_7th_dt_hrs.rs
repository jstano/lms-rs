//! Port of `DailyWeekly7thDTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyWeekly7thDTHrsRuleImpl.java`.
//!
//! `DW7DT_HDR`. Daily overtime, daily double time and weekly overtime, with one
//! rule on top of all three: **the seventh consecutive day worked is double
//! time from the first hour**.
//!
//! Structurally it is [`CaliforniaOTHrs`](super::california_ot_hrs) — the same
//! day/shift/earning walk, the same three-row earning rewrite, the same
//! `dailyDataProducer` (shared now; see
//! [`super::daily_data::build_daily_data_map`]). What
//! differs is the arithmetic, and it differs because this rule **declares its
//! own accumulators**.
//!
//! # It shadows both shared accumulators
//!
//! Third shadowing case in the family, after
//! `CaliforniaExtSpecialJobOTHrsRuleImpl`'s inner `DailyData` and
//! `DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl`'s inner accumulators. As
//! there, the inner classes take the shared names and behave differently:
//!
//! | | the shared class | this rule's inner class |
//! |---|---|---|
//! | `WeeklyAccumulator(…)` | `(weeklyLimit, consecDayLimit, maxConsecDays)` | `(weeklyLimit)` alone |
//! | the consecutive-day test | `counter >= consecDayLimit` | `counter == 7`, exactly |
//! | `computeWeeklyOT` | two formulas behind a flag, **unrounded** | one formula, **rounded**, and zero on the seventh day |
//! | daily double time | always computed | zero unless `payDailyDT`, or it is the seventh day |
//!
//! So they are ported private to this module, as `WeekTotals` and
//! `DayTotals` — names that cannot be mistaken for the shared ones.
//!
//! Its inner `DailyData`, by contrast, is a field-for-field copy of the shared
//! one, so `DailyData`(super::daily_data::DailyData) is used directly.
//!
//! # The seventh day overrides everything
//!
//! On `counter == 7`:
//!
//! - weekly overtime is **zero**, whatever the week's total;
//! - daily overtime is `hours - overtime`, so every hour is overtime from the
//!   first;
//! - daily double time is `hours - doubleTime`, so every hour is *also* double
//!   time.
//!
//! Since `shiftDT` then equals `shiftOT`, `premiumHours - doubleTime` is zero
//! and no overtime row is written at all — the whole day lands in double time.
//! The Java spec asserts exactly that: its seventh shift of 8.5 hours comes out
//! `(0 regular, 0 overtime, 8.5 double time)` in all seven cases.
//!
//! The test is `== 7`, not `>= 7`. A work week is seven days, so an eighth
//! consecutive day inside one `execute` is not reachable; a longer period
//! handed in as a work week would silently stop paying it.
//!
//! # An earning already at a premium level is counted as premium
//!
//! `computeEarningHours` reads the earning's pay level out of the pay set and,
//! **before** accumulating its hours, books them as overtime already paid
//! (level 1 or 2) and as double time already paid (level 2). `CaliforniaOTHrs`
//! does none of this — it treats every configured earning as regular hours. So
//! the same pay set means different things to the two rules.
//!
//! `payLevelMap.get(...)` unboxes an `Integer`, but the producer has already
//! filtered earnings to the pay set's configured types and the level map covers
//! exactly those, so the lookup cannot miss.
//!
//! # `payDailyDT` does not gate the premium split
//!
//! Unlike the California rules, `addDistributions` here branches on
//! `doubleTime > 0` alone; the flag has already done its work inside
//! `computeDailyDoubleTime`. Clearing it removes ordinary daily double time and
//! leaves the seventh day's untouched.

use crate::common::numbers::round_hours;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    DAILY_DT_LIMIT_PROP, DAILY_OT_LIMIT_PROP, DailyWeekly7thDTHrsRuleConfig, EARNING_TYPE_PAY_SET,
    PAY_DAILY_DT, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::daily_data::build_daily_data_map;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use crate::rules::types::earning_type_pay_set::EarningTypePaySet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// The consecutive day on which everything becomes double time.
/// `WeeklyAccumulator.SEVENTH_DAY`.
const SEVENTH_DAY: i32 = 7;

/// The premium level of overtime within an `EarningTypePayMap`.
const OVERTIME_LEVEL: usize = 0;

/// The premium level of double time within an `EarningTypePayMap`.
const DOUBLE_TIME_LEVEL: usize = 1;

/// Daily and weekly overtime with the seventh consecutive day at double time.
/// `DailyWeekly7thDTHrsRuleImpl`.
#[derive(Debug, Default)]
pub struct DailyWeekly7thDTHrsRule<P: EmployeeEarningPort> {
    earnings: RefCell<P>,
}

impl<P: EmployeeEarningPort> DailyWeekly7thDTHrsRule<P> {
    /// Build the rule over the port it persists new earnings through.
    pub fn new(earnings: P) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

/// This rule's **private** `WeeklyAccumulator`. Not
/// [`the shared one`](super::weekly_accumulator::WeeklyAccumulator).
#[derive(Debug, Default)]
struct WeekTotals {
    hours: f64,
    weekly_ot: f64,
    consecutive_days_counter: i32,
    weekly_limit: f64,
}

impl WeekTotals {
    fn new(weekly_limit: f64) -> Self {
        Self {
            weekly_limit,
            ..Self::default()
        }
    }

    /// `updateConsecutiveDays(DailyData)` — reset on an unworked day,
    /// otherwise increment. No modifier, no wrap.
    fn update_consecutive_days(&mut self, day_was_worked: bool) {
        self.consecutive_days_counter = if day_was_worked {
            self.consecutive_days_counter + 1
        } else {
            0
        };
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// `computeWeeklyOT` — zero on the seventh day, where the daily rule takes
    /// over entirely. Note this one **rounds**; the shared accumulator's does
    /// not and leaves it to the caller.
    fn compute_weekly_ot(&self) -> f64 {
        if self.is_seventh_consecutive_day() {
            return 0.0;
        }
        round_hours(self.hours - self.weekly_limit - self.weekly_ot).max(0.0)
    }

    /// `isSeventhConsecutiveDay` — an equality test, not a threshold.
    fn is_seventh_consecutive_day(&self) -> bool {
        self.consecutive_days_counter == SEVENTH_DAY
    }
}

/// This rule's **private** `DailyAccumulator`. Not
/// [`the shared one`](super::daily_accumulator::DailyAccumulator).
#[derive(Debug, Default)]
struct DayTotals {
    date: Option<LocalDate>,
    hours: f64,
    overtime: f64,
    double_time: f64,
    daily_ot_limit: f64,
    daily_dt_limit: f64,
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

    /// `computeDailyOvertime` — the whole day on the seventh, otherwise
    /// everything past the daily limit.
    fn compute_daily_overtime(&self, week: &WeekTotals) -> f64 {
        let daily_ot = if week.is_seventh_consecutive_day() {
            self.hours - self.overtime
        } else {
            self.hours - self.daily_ot_limit - self.overtime
        };

        round_hours(daily_ot).max(0.0)
    }

    /// `computeDailyDoubleTime` — the whole day on the seventh; past the
    /// double-time limit when `payDailyDT` is set; otherwise **nothing at all**.
    fn compute_daily_double_time(&self, week: &WeekTotals, pay_daily_dt: bool) -> f64 {
        let daily_dt = if week.is_seventh_consecutive_day() {
            self.hours - self.double_time
        } else if pay_daily_dt {
            self.hours - self.daily_dt_limit - self.double_time
        } else {
            0.0
        };

        round_hours(daily_dt).max(0.0)
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
    rule_item_id: i32,
    note: &'a str,
}

impl<P: EmployeeEarningPort> HoursDistributionRule for DailyWeekly7thDTHrsRule<P> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&DailyWeekly7thDTHrsRuleConfig.default_values());

        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt_limit = params.double_at(DAILY_DT_LIMIT_PROP);
        let weekly_limit = params.double_at(WEEKLY_LIMIT_PROP);
        let pay_daily_dt = params.bool_at(PAY_DAILY_DT);

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
            rule_item_id: rule_item.id(),
            note: rule_item.name(),
        };

        let daily_data_map =
            build_daily_data_map(time_card, work_week, &pay_set.configured_earning_type_ids());

        let mut week = WeekTotals::new(weekly_limit);

        for date in work_week.dates() {
            let mut day = DayTotals::new(date, daily_ot_limit, daily_dt_limit);
            let daily_data = &daily_data_map[&date];

            week.update_consecutive_days(daily_data.was_worked());

            self.compute_earning_hours(
                time_card,
                &mut week,
                &mut day,
                daily_data.earnings(),
                &context,
                pay_daily_dt,
            );

            for shift_to_earnings in daily_data.shifts() {
                self.compute_earning_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    shift_to_earnings.earnings(),
                    &context,
                    pay_daily_dt,
                );
                compute_distribution_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    shift_to_earnings.shift(),
                    &buckets,
                    rule_item.id(),
                    pay_daily_dt,
                );
            }
        }
    }
}

impl<P: EmployeeEarningPort> DailyWeekly7thDTHrsRule<P> {
    /// `computeEarningHours`.
    fn compute_earning_hours(
        &self,
        time_card: &mut dyn TimeCard,
        week: &mut WeekTotals,
        day: &mut DayTotals,
        earning_indices: &[usize],
        context: &EarningContext<'_>,
        pay_daily_dt: bool,
    ) {
        for &earning_index in earning_indices {
            let earning = &time_card.earnings()[earning_index];
            let hours = earning.hours();
            let pay_level = context
                .pay_set
                .pay_level_map()
                .get(&earning.earning_type_id())
                .copied()
                .unwrap_or(0);

            // Hours that arrive already at a premium level are booked as
            // premium before they are counted — `CaliforniaOTHrs` does not.
            if pay_level >= 1 {
                day.add_overtime(hours);
                week.add_weekly_ot(hours);
            }
            if pay_level == 2 {
                day.add_double_time(hours);
            }

            day.add_hours(hours);
            week.add_hours(hours);

            let (premium_hours, double_time) = compute_premium_hours(week, day, pay_daily_dt);

            self.add_earnings(
                time_card,
                earning_index,
                premium_hours,
                double_time,
                context,
            );

            day.add_overtime(premium_hours);
            day.add_double_time(double_time);
            week.add_weekly_ot(premium_hours);
        }
    }

    /// `addEarnings` — cancel the premium hours at the regular type, then pay
    /// them back at the premium types. Note the double-time branch is gated on
    /// `doubleTimeHours > 0` alone, with no `payDT` alongside it.
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
            round_hours(-premium_hours),
            regular_type_id,
            context,
        );

        if double_time_hours > 0.0 {
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
    /// Divergence 42: the note is the rule item's name alone.
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
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule(context.rule_item_id, source.shift_id());

        earning.set_pay_date(Some(source.earning_date()));
        earning.set_note(context.note);
        earning.calc_and_set_total_dollars();

        self.earnings.borrow_mut().save(earning.clone());
        time_card.earnings_mut().push(earning);
    }
}

/// `computeDistributionHours` — the shift's non-premium distributions dated
/// this day.
fn compute_distribution_hours(
    time_card: &mut dyn TimeCard,
    week: &mut WeekTotals,
    day: &mut DayTotals,
    shift_index: usize,
    buckets: &Buckets,
    rule_item_id: i32,
    pay_daily_dt: bool,
) {
    let date = day.date();

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
            pay_daily_dt,
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
    pay_daily_dt: bool,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let original_hours = distribution.original_hours();
    let date = distribution.date();

    day.add_hours(original_hours);
    week.add_hours(original_hours);

    let (premium_hours, double_time) = compute_premium_hours(week, day, pay_daily_dt);

    // `addDistributions`.
    if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
        let shift = &mut time_card.shifts_mut()[shift_index];
        let mut premium = |hours: f64, bucket: i32| {
            let row: HoursDistribution = create_premium_distribution_at_rate(
                &shift.hours_distributions()[distribution_index],
                hours,
                0.0,
                bucket,
                Some(rule_item_id),
                None,
            );
            shift.add_hours_distribution(row);
        };

        if double_time > 0.0 {
            premium(double_time, buckets.double_time);
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

/// `computePremiumHours` — the larger of the two overtime measures, and the
/// day's double time beside it.
///
/// On the seventh consecutive day both come back as the whole day, so the
/// double-time row takes everything and no overtime row is written.
fn compute_premium_hours(week: &WeekTotals, day: &DayTotals, pay_daily_dt: bool) -> (f64, f64) {
    let shift_ot = week
        .compute_weekly_ot()
        .max(day.compute_daily_overtime(week));
    let shift_dt = day.compute_daily_double_time(week, pay_daily_dt);

    (shift_ot, shift_dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use crate::rules::types::earning_type_pay_map::EarningTypePayMap;

    pub(super) const JOB: i32 = 11;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// The spec's earning types: 1 regular, escalating to 2 and 3.
    pub(super) const EARNING_REGULAR: i32 = 1;
    pub(super) const EARNING_OVERTIME: i32 = 2;

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

    pub(super) fn rule() -> DailyWeekly7thDTHrsRule<SavedEarnings> {
        DailyWeekly7thDTHrsRule::new(SavedEarnings::default())
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

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    pub(super) fn shift(id: i32, hours: f64, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_net_hours(hours)
            .with_hours_distributions(vec![regular(date, hours)])
    }

    pub(super) fn earning(
        id: i32,
        date: LocalDate,
        type_id: i32,
        hours: f64,
        shift_id: Option<i32>,
    ) -> EmployeeEarning {
        let earning =
            EmployeeEarning::new(id, 1, JOB, type_id, date, hours, 10.0, EarningSource::Auto);
        match shift_id {
            Some(id) => earning.with_shift(id),
            None => earning,
        }
    }

    /// `new EarningTypePaySet(premiumLevels: 1, defaultRegularEarningTypeID: 1,
    /// earningTypePayMapSet: [regular 1 -> [2, 3]])`.
    pub(super) fn pay_set_json() -> String {
        EarningTypePaySet::new()
            .with_premium_levels(1)
            .with_default_regular_earning_type_id(EARNING_REGULAR)
            .with_pay_maps(vec![EarningTypePayMap::new(EARNING_REGULAR, vec![2, 3])])
            .to_json_string()
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
                shift(7, 8.0, nov(27)),
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
        RuleItem::new(1, 1, "Rule", RuleClass::Dw7dtHdr, rule_params)
    }

    /// `correctHoursTotal(shift, regHours, otHours, dtHours)` — the **sum** of
    /// each bucket's rows, not the first row of each.
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

    #[test]
    fn the_seventh_consecutive_day_is_double_time_from_the_first_hour() {
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[]));

        // 11-28 is the seventh worked day of the week.
        assert_hours(&card, 7, (0.0, 0.0, 8.5));
    }

    #[test]
    fn the_seventh_day_pays_no_overtime_row_at_all() {
        // shiftDT equals shiftOT there, so `premiumHours - doubleTime` is zero
        // and the overtime branch is never taken.
        let mut card = card();

        rule().execute(&mut card, &week(), &item(&[]));

        assert!(
            !card.shifts()[7]
                .hours_distributions()
                .iter()
                .any(|d| d.is_of_type(OVERTIME)),
            "no overtime row is written on the seventh day"
        );
    }

    #[test]
    fn an_unworked_day_resets_the_consecutive_count() {
        // Drop 11-25. The counter resets there and 11-28 is only the third
        // consecutive day, so the seventh-day rule never fires: the day is paid
        // as ordinary weekly overtime (51.36 - 40 - 4.54) instead of landing
        // whole in double time.
        let mut card = card();
        card.shifts_mut().retain(|s| s.shift_date() != nov(25));

        let last = card.shifts().len() - 2;
        rule().execute(&mut card, &week(), &item(&[]));

        assert_hours(&card, last, (1.68, 6.82, 0.0));
        assert!(
            !card.shifts()[last]
                .hours_distributions()
                .iter()
                .any(|d| d.is_of_type(DOUBLE_TIME)),
            "no double-time row at all, where the seventh day would write one"
        );
    }

    #[test]
    fn pay_daily_dt_gates_the_ordinary_daily_limit_only() {
        let mut cleared = card();
        rule().execute(&mut cleared, &week(), &item(&[(PAY_DAILY_DT, "false")]));

        // 11-26's 12.54 hours: all the premium goes to overtime…
        assert_hours(&cleared, 5, (8.0, 4.54, 0.0));
        // …but the seventh day is untouched by the flag.
        assert_hours(&cleared, 7, (0.0, 0.0, 8.5));
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
    fn an_earning_already_at_a_premium_level_is_booked_as_premium_first() {
        // Type 2 is the overtime level, so its hours arrive already paid: the
        // day's overtime starts at nine and the shift that follows gets less.
        let mut card = card().with_earnings(vec![earning(1, nov(24), EARNING_OVERTIME, 9.0, None)]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(EARNING_TYPE_PAY_SET, &pay_set_json())]),
        );

        // 11-24: 9 premium-level hours then the 8-hour shift. The day's total
        // is 17, its overtime already 9, so the shift's own overtime is zero.
        assert_hours(&card, 3, (8.0, 0.0, 0.0));
    }

    #[test]
    fn the_created_earnings_are_persisted_through_the_port() {
        let mut card = card().with_earnings(vec![earning(1, nov(25), EARNING_REGULAR, 9.0, None)]);
        let rule = rule();

        rule.execute(
            &mut card,
            &week(),
            &item(&[(EARNING_TYPE_PAY_SET, &pay_set_json())]),
        );

        assert_eq!(
            rule.earnings.borrow().0.len(),
            2,
            "the offset and the OT row"
        );
    }
}

/// `DailyWeekly7thDTHrsRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyWeekly7thDTHrsRuleImplTest.groovy`
/// — all seven cases. Each asserts regular, overtime and double-time **totals**
/// on every one of the nine shifts, so a case is 27 assertions and the file is
/// close to two hundred.
///
/// The fixture is the one `CaliforniaExtendedOTHrsRuleImplTest` uses: nine
/// shifts either side of the week 2010-11-22..28. The mocked `PayGroup` is
/// divergence 24's derivation, so the calculation start date is set on the card
/// directly — 11-21 falls outside it and is left alone, which the spec's own
/// comments call out.
///
/// The Java helper sums each bucket rather than reading one row, so a day that
/// writes two overtime rows is asserted on the total; the transcription does
/// the same.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        EARNING_REGULAR, assert_hours, card, earning, item, nov, pay_set_json, rule, week,
    };
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::time_card::TimeCardData;

    /// The nine `correctHoursTotal` lines of a case, in shift order.
    fn assert_all(card: &TimeCardData, expected: [(f64, f64, f64); 9]) {
        for (index, hours) in expected.into_iter().enumerate() {
            assert_hours(card, index, hours);
        }
    }

    #[test]
    fn default_config_values_allocated_premium_hours_correctly() {
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
                (1.68, 6.32, 0.0),
                (0.0, 0.0, 8.5), // the seventh consecutive day
                (8.0, 0.0, 0.0), // after the week
            ],
        );
    }

    #[test]
    fn no_daily_dt_should_be_allocated() {
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
                (1.68, 6.32, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn seventh_day_ot_is_correctly_allocated() {
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
                (5.68, 2.32, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn daily_dt_is_distributed_when_worked_above_the_daily_dt_limit() {
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
                (1.68, 6.32, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn weekly_ot_is_given_when_the_weekly_limit_hours_prop_is_passed() {
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
                (6.68, 1.32, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn salaried_employees_should_not_be_given_premium_hours() {
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
                (8.0, 0.0, 0.0),
                (8.5, 0.0, 0.0),
                (8.0, 0.0, 0.0),
            ],
        );
    }

    #[test]
    fn earnings_are_counted_towards_premium_hours() {
        let mut card = card().with_earnings(vec![
            // Attached to the 11-24 shift.
            earning(1, nov(24), EARNING_REGULAR, 8.0, Some(4)),
            // Standing on its own on 11-25.
            earning(2, nov(25), EARNING_REGULAR, 9.0, None),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(EARNING_TYPE_PAY_SET, &pay_set_json())]),
        );

        assert_eq!(card.earnings().len(), 4);

        let has = |date: LocalDate, hours: f64, type_id: i32| {
            card.earnings().iter().any(|e| {
                e.earning_date() == date && e.hours() == hours && e.earning_type_id() == type_id
            })
        };
        assert!(has(nov(24), 8.0, 1), "the original attached earning");
        assert!(has(nov(25), 9.0, 1), "the original standalone earning");
        assert!(has(nov(25), -1.0, 1), "the offset at the regular type");
        assert!(has(nov(25), 1.0, 2), "the hour paid at the overtime type");

        assert_all(
            &card,
            [
                (8.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (6.32, 0.0, 0.0),
                (0.0, 4.0, 4.0),
                (0.0, 3.0, 5.5),
                (8.0, 4.0, 0.54),
                (1.68, 6.32, 0.0),
                (0.0, 0.0, 8.5),
                (8.0, 0.0, 0.0),
            ],
        );
    }
}
