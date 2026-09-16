//! Port of `DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl.java`.
//!
//! `DW_6OT_7DT_NCS_HDR`. Daily and weekly overtime, plus a rule about how many
//! days of the week were worked: from the sixth worked day the daily limit
//! stops applying and every hour is overtime, and from the seventh every hour
//! is **double time**.
//!
//! The "NonConsec" in the name is the whole point — the days need not be
//! consecutive. A week of Mon, Tue, Wed, Fri, Sat, Sun reaches six worked days
//! on the Sunday even though the run was broken.
//!
//! # It declares its own accumulators, and they are not the shared ones
//!
//! Both [`DailyAccumulator`](super::daily_accumulator::DailyAccumulator) and
//! [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator) are
//! shadowed by **private inner classes of the same names**, the second case of
//! this in the family after `CaliforniaExtSpecialJobOTHrsRuleImpl`'s inner
//! `DailyData`. They are materially different:
//!
//! | | the shared class | this rule's inner class |
//! |---|---|---|
//! | `WeeklyAccumulator(a, b, c)` | `(weeklyLimit, consecDayLimit, maxConsecDays)` | `(weeklyLimit, workedDayOTLimit, workedDayDTLimit)` |
//! | day counter | resets to zero on an unworked day, wraps at a modifier | **only ever increments** |
//! | `computeWeeklyOT` | two formulas, selected by a flag; unrounded | one formula, unrounded |
//! | `DailyAccumulator(a, …)` | `(date, dailyOTLimit, dailyDTLimit)` | `(date, dailyOTLimit)` |
//! | daily double time | `computeDailyDoubleTime` | none — the inner class has a `doubleTime` field that is never written or read |
//!
//! The constructors take the same *number* of arguments with different
//! meanings, so mistaking one for the other compiles in Java and silently
//! changes which day the limits move on. Both inner classes are ported private
//! to this module, under names that cannot be confused with the shared ones.
//!
//! # Double time is a bucket choice, not an amount
//!
//! There is no separate double-time computation. `addDistributions` computes
//! one `premiumHours` figure and then asks `isDuringDTConsecDaysRange()` which
//! bucket to put it in — all of it, either way. So on the seventh worked day
//! the whole premium is double time and the overtime bucket gets nothing, where
//! [`california_extended_ot_hrs`](super::california_extended_ot_hrs) splits the
//! same total between the two.
//!
//! # The daily limit is suspended, not lowered
//!
//! `computeDailyOvertime` drops `dailyOTLimit` from its subtraction once either
//! range is active, so from the sixth worked day the first hour is already
//! overtime.

use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    DAILY_OT_LIMIT_PROP, DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig, WEEKLY_LIMIT_PROP,
    WORKED_DAY_DT_LIMIT_PROP, WORKED_DAY_OT_LIMIT_PROP,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Overtime from the sixth worked day, double time from the seventh.
/// `DailyWeekly6thDayOT7thDayDTNonConsecRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DailyWeekly6thDayOT7thDayDTNonConsecRule;

/// This rule's **private** weekly accumulator — not
/// [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator).
struct WorkedDayAccumulator {
    weekly_limit: f64,
    worked_day_ot_limit: i32,
    worked_day_dt_limit: i32,
    hours: f64,
    weekly_ot: f64,
    worked_day_counter: i32,
}

impl WorkedDayAccumulator {
    fn new(weekly_limit: f64, worked_day_ot_limit: i32, worked_day_dt_limit: i32) -> Self {
        Self {
            weekly_limit,
            worked_day_ot_limit,
            worked_day_dt_limit,
            hours: 0.0,
            weekly_ot: 0.0,
            worked_day_counter: 0,
        }
    }

    /// `updatedWorkedDays` — increments on a worked day and **never resets**,
    /// which is what makes the count non-consecutive.
    fn updated_worked_days(&mut self, day_was_worked: bool) {
        if day_was_worked {
            self.worked_day_counter += 1;
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// `computeWeeklyOT` — one formula, and unrounded like the shared class's.
    fn compute_weekly_ot(&self) -> f64 {
        (self.hours - self.weekly_limit - self.weekly_ot).max(0.0)
    }

    /// `isDuringDTConsecDaysRange`.
    fn is_during_dt_range(&self) -> bool {
        self.worked_day_counter >= self.worked_day_dt_limit
    }

    /// `isDuringOTConsecDaysRange` — note it is **exclusive** of the
    /// double-time range, so the two never overlap.
    fn is_during_ot_range(&self) -> bool {
        self.worked_day_counter >= self.worked_day_ot_limit
            && self.worked_day_counter < self.worked_day_dt_limit
    }
}

/// This rule's **private** daily accumulator — not
/// [`DailyAccumulator`](super::daily_accumulator::DailyAccumulator).
///
/// Java's carries an unused `doubleTime` field; there is nothing to write it.
struct DailyHours {
    daily_ot_limit: f64,
    hours: f64,
    overtime: f64,
}

impl DailyHours {
    fn new(daily_ot_limit: f64) -> Self {
        Self {
            daily_ot_limit,
            hours: 0.0,
            overtime: 0.0,
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_overtime(&mut self, add_overtime: f64) {
        self.overtime = round_hours(self.overtime + add_overtime);
    }

    /// `computeDailyOvertime` — the daily limit drops out of the subtraction
    /// once either worked-day range is active.
    fn compute_daily_overtime(&self, weekly: &WorkedDayAccumulator) -> f64 {
        let daily_ot = if weekly.is_during_ot_range() || weekly.is_during_dt_range() {
            self.hours - self.overtime
        } else {
            self.hours - self.daily_ot_limit - self.overtime
        };
        round_hours(daily_ot).max(0.0)
    }
}

impl HoursDistributionRule for DailyWeekly6thDayOT7thDayDTNonConsecRule {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig.default_values());

        let daily_shift_map = daily_shift_map(time_card, work_week);

        let (Some(overtime_type), Some(double_time_type)) = (
            time_card.ot_hours_distribution_type_id(),
            time_card.dt_hours_distribution_type_id(),
        ) else {
            return;
        };

        let mut weekly = WorkedDayAccumulator::new(
            params.double_at(WEEKLY_LIMIT_PROP),
            params.int_at(WORKED_DAY_OT_LIMIT_PROP),
            params.int_at(WORKED_DAY_DT_LIMIT_PROP),
        );
        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);

        for date in work_week.dates() {
            let shifts = daily_shift_map.get(&date).cloned().unwrap_or_default();
            let mut daily = DailyHours::new(daily_ot_limit);

            weekly.updated_worked_days(!shifts.is_empty());

            for shift_index in shifts {
                for distribution_index in regular_distributions_on(time_card, shift_index, date) {
                    create_distribution(
                        time_card,
                        shift_index,
                        distribution_index,
                        overtime_type,
                        double_time_type,
                        rule_item.id(),
                        &mut weekly,
                        &mut daily,
                    );
                }
            }
        }
    }
}

/// `dailyDataProducer` — the week's non-exempt shifts, grouped by the dates
/// their distributions fall on.
///
/// A shift spanning midnight is listed under both days, and counts as a worked
/// day on each.
fn daily_shift_map(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
) -> HashMap<LocalDate, Vec<usize>> {
    let mut map: HashMap<LocalDate, Vec<usize>> = HashMap::new();

    for index in time_card.shift_indices_with_distributions_for_period(work_week) {
        let shift = &time_card.shifts()[index];
        if !time_card.shift_is_not_salaried_exempt(shift) {
            continue;
        }
        // Java groups over *every* distribution the shift owns, not only those
        // inside the week, so a date outside it can pick up an entry — which
        // the date loop then never asks for.
        for date in shift.dates_with_hours_distributions() {
            map.entry(date).or_default().push(index);
        }
    }

    map
}

/// The shift's regular distributions dated `date`.
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
            time_card.distribution_is_regular(distribution) && distribution.date() == date
        })
        .map(|(index, _)| index)
        .collect()
}

/// `createDistribution` together with `addDistributions`.
#[allow(clippy::too_many_arguments)]
fn create_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    overtime_type: i32,
    double_time_type: i32,
    rule_item_id: i32,
    weekly: &mut WorkedDayAccumulator,
    daily: &mut DailyHours,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let original_hours = distribution.original_hours();
    let date = distribution.date();

    daily.add_hours(original_hours);
    weekly.add_hours(original_hours);

    // `computePremiumHours` — the larger of the two measures.
    let premium_hours = weekly
        .compute_weekly_ot()
        .max(daily.compute_daily_overtime(weekly));

    if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
        // One amount, and the worked-day count picks the bucket.
        let bucket = if weekly.is_during_dt_range() {
            double_time_type
        } else {
            overtime_type
        };

        let shift = &mut time_card.shifts_mut()[shift_index];
        let premium = create_premium_distribution_at_rate(
            &shift.hours_distributions()[distribution_index],
            premium_hours,
            0.0,
            bucket,
            Some(rule_item_id),
            None,
        );
        shift.add_hours_distribution(premium);

        let reduced =
            round_hours(shift.hours_distributions()[distribution_index].hours() - premium_hours);
        shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
    }

    // Outside the guard, as in every accumulator rule.
    daily.add_overtime(premium_hours);
    weekly.add_weekly_ot(premium_hours);
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

    pub(super) fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    pub(super) fn day(offset: i64) -> LocalDate {
        today().plus_days(offset)
    }

    /// `new LegacyDatePeriod(today, today.plusWeeks(1))` — eight dates.
    pub(super) fn work_week() -> DateRange {
        DateRange::new(today(), day(7))
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

    /// One shift on `date` carrying `hours` regular hours that day.
    pub(super) fn shift(id: i32, offset: i64, hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, day(offset), ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![regular(day(offset), hours)])
    }

    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(today())
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::Dw6ot7dtNcsHdr, rule_params)
    }

    pub(super) fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn under_every_limit_nothing_happens() {
        let mut card = card(vec![shift(1, 0, 5.0), shift(2, 1, 5.0)]);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 5.0)]);
        assert_eq!(rows(&card, 1), vec![(Some(REGULAR), 5.0)]);
    }

    #[test]
    fn the_daily_limit_pays_overtime_before_any_worked_day_limit() {
        let mut card = card(vec![shift(1, 0, 10.0)]);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(Some(REGULAR), 8.0), (Some(OVERTIME), 2.0)]
        );
    }

    #[test]
    fn the_whole_premium_goes_to_one_bucket() {
        // Seven worked days: the last day's premium is entirely double time,
        // with nothing in the overtime bucket.
        let shifts = (0..7)
            .map(|offset| shift(offset as i32 + 1, offset, 5.0))
            .collect();
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(
            rows(&card, 5),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 5.0)]
        );
        assert_eq!(
            rows(&card, 6),
            vec![(Some(REGULAR), 0.0), (Some(DOUBLE_TIME), 5.0)],
            "no overtime row at all on the seventh worked day"
        );
    }

    #[test]
    fn the_worked_day_counter_never_resets() {
        // Six worked days with two gaps still reaches the sixth-day rule.
        let shifts = vec![
            shift(1, 0, 5.0),
            shift(2, 2, 5.0),
            shift(3, 3, 5.0),
            shift(4, 5, 5.0),
            shift(5, 6, 5.0),
            shift(6, 7, 5.0),
        ];
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(rows(&card, 4), vec![(Some(REGULAR), 5.0)], "the fifth");
        assert_eq!(
            rows(&card, 5),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 5.0)],
            "the sixth, despite the gaps"
        );
    }

    #[test]
    fn the_worked_day_limits_are_configurable() {
        let shifts = (0..3)
            .map(|offset| shift(offset as i32 + 1, offset, 5.0))
            .collect();
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(
            &mut card,
            &work_week(),
            &item(&[
                (WORKED_DAY_OT_LIMIT_PROP, "2"),
                (WORKED_DAY_DT_LIMIT_PROP, "3"),
            ]),
        );

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 5.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 5.0)]
        );
        assert_eq!(
            rows(&card, 2),
            vec![(Some(REGULAR), 0.0), (Some(DOUBLE_TIME), 5.0)]
        );
    }

    #[test]
    fn the_weekly_limit_still_applies() {
        // Five days of nine hours: 45 for the week against a limit of 40, and
        // the daily limit of 8 has already paid five of those.
        let shifts = (0..5)
            .map(|offset| shift(offset as i32 + 1, offset, 9.0))
            .collect();
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        for index in 0..5 {
            assert_eq!(
                rows(&card, index),
                vec![(Some(REGULAR), 8.0), (Some(OVERTIME), 1.0)],
                "shift {index}"
            );
        }
    }

    #[test]
    fn a_closed_day_is_left_alone_but_still_counts() {
        let mut card =
            card(vec![shift(1, 0, 10.0), shift(2, 1, 10.0)]).with_calculation_start_date(day(1));

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 10.0)], "closed");
        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 8.0), (Some(OVERTIME), 2.0)]
        );
    }

    #[test]
    fn a_salaried_exempt_shift_is_skipped() {
        let mut card =
            card(vec![shift(1, 0, 12.0)]).with_employee(employee(EmployeePayType::SalariedExempt));

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 12.0)]);
    }

    #[test]
    fn an_overnight_shift_counts_as_a_worked_day_on_both_of_its_days() {
        let overnight = EmployeeShift::new(1, 1, JOB, today(), ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![regular(today(), 4.0), regular(day(1), 4.0)]);
        let mut card = card(vec![overnight]);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(
            &mut card,
            &work_week(),
            &item(&[(WORKED_DAY_OT_LIMIT_PROP, "2")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (Some(REGULAR), 4.0),
                (Some(REGULAR), 0.0),
                (Some(OVERTIME), 4.0)
            ],
            "the second day is already the second worked day"
        );
    }

    #[test]
    fn a_property_with_no_double_time_bucket_writes_nothing() {
        let mut card = card(vec![shift(1, 0, 12.0)]).with_hours_distribution_types(vec![
            HoursDistributionType::new(REGULAR, "Regular", false),
            HoursDistributionType::new(OVERTIME, "Overtime", true),
        ]);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 12.0)]);
    }
}

/// `DailyWeekly6thDayOT7thDayDTNonConsecRuleImplTest.groovy`, transcribed.
///
/// All three cases. `LocalDate.now()` pinned, and the mocked `PayGroup` becomes
/// the card's calculation start date (divergence 24).
///
/// Its `assertListHasCorrectHoursDistribution` helper returns the result of a
/// three-way `find`, which Spock asserts as truthy — so unlike the `any {}`
/// blocks in `RollingXWeeksOTHrsRuleImplTest`, this one does check date, hours
/// and bucket together. It asserts only that a matching row **exists**, though,
/// so these transcriptions check the whole distribution list instead.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{DOUBLE_TIME, OVERTIME, REGULAR, card, item, rows, shift, work_week};
    use super::*;
    use crate::rules::algorithm::hoursdistribution::config::DAILY_OT_LIMIT_PROP;

    #[test]
    fn working_six_days_distributes_ot_seven_days_to_dt() {
        let shifts = (0..7)
            .map(|offset| shift(offset as i32 + 1, offset, 5.0))
            .collect();
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        for index in 0..5 {
            assert_eq!(
                rows(&card, index),
                vec![(Some(REGULAR), 5.0)],
                "shift {index}"
            );
        }
        assert_eq!(
            rows(&card, 5),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 5.0)]
        );
        assert_eq!(
            rows(&card, 6),
            vec![(Some(REGULAR), 0.0), (Some(DOUBLE_TIME), 5.0)]
        );
    }

    #[test]
    fn working_six_days_of_the_week_even_non_consecutively_distributes_ot_properly() {
        // Days 0, 1, 2, 4, 5, 6 — a gap on day 3 that the counter ignores.
        let shifts = [0, 1, 2, 4, 5, 6]
            .into_iter()
            .enumerate()
            .map(|(id, offset)| shift(id as i32 + 1, offset, 5.0))
            .collect();
        let mut card = card(shifts);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(&mut card, &work_week(), &item(&[]));

        for index in 0..5 {
            assert_eq!(
                rows(&card, index),
                vec![(Some(REGULAR), 5.0)],
                "shift {index}"
            );
        }
        assert_eq!(
            rows(&card, 5),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 5.0)],
            "the sixth worked day, reached across a gap"
        );
    }

    #[test]
    fn daily_ot_is_distributed_properly_and_does_not_double_dip_premium_hours() {
        let mut card = card(vec![
            shift(1, 0, 12.0),
            shift(2, 1, 12.0),
            shift(3, 2, 8.0),
            shift(4, 3, 5.0),
            shift(5, 4, 5.0),
        ]);

        DailyWeekly6thDayOT7thDayDTNonConsecRule.execute(
            &mut card,
            &work_week(),
            &item(&[(DAILY_OT_LIMIT_PROP, "5")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![(Some(REGULAR), 5.0), (Some(OVERTIME), 7.0)]
        );
        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 5.0), (Some(OVERTIME), 7.0)]
        );
        assert_eq!(
            rows(&card, 2),
            vec![(Some(REGULAR), 5.0), (Some(OVERTIME), 3.0)]
        );
        // Forty-two hours for the week against a limit of forty, but
        // seventeen have already been paid as daily overtime, so the weekly
        // measure claims nothing.
        assert_eq!(rows(&card, 3), vec![(Some(REGULAR), 5.0)]);
        assert_eq!(rows(&card, 4), vec![(Some(REGULAR), 5.0)]);
    }
}
