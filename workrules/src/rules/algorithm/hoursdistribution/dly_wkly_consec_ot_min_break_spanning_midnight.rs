//! Port of `DlyWklyConsecOTMinBreakSpanningMidnightRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DlyWklyConsecOTMinBreakSpanningMidnightRuleImpl.java`.
//!
//! `DWCOT_MIN_BR_SPAN_MN_HDR`. The nineteenth and last rule of the family, and
//! the sibling of
//! [`DlyWklyOffConsecOTMinBreak`](super::dly_wkly_off_consec_ot_min_break).
//! Same minimum-break idea, three things different:
//!
//! 1. the break rule fires **only across midnight**;
//! 2. consecutive-day overtime is measured in **hours**, not days;
//! 3. there is no unscheduled-day rule, so no schedule list at all.
//!
//! # The break must span midnight
//!
//! ```java
//! priorShift.getEndDateTime().toLocalDate().equals(employeeShift.getStartDateTime().toLocalDate().minusDays(1))
//!    && actualTimeBetween > dailySplitShiftLimit
//!    && actualTimeBetween < timeBetweenShifts
//! ```
//!
//! The sibling's first clause is `!priorShift.getShiftDate().equals(shift.getShiftDate()) || gap > splitLimit`
//! — any pair of shifts on different **shift dates**, or a long enough gap on
//! one. Here it is the narrower question: did the prior shift *end* on the
//! calendar day before this one *starts*. Two shifts on one date never qualify,
//! and neither does a gap of more than a day.
//!
//! `minTimeBetweenPayFullShift` then chooses what a violation costs: the
//! shortfall, as the sibling always pays, or the **whole shift**.
//!
//! # Consecutive-day overtime is hours-based
//!
//! ```java
//! consecDayOt = isDuringOTConsecDays()
//!    ? hours + min(priorConsecutiveDaysHours, consecDaysHrsLimit) - consecDaysHrsLimit - overtime
//!    : 0.0;
//! ```
//!
//! So reaching the consecutive-day *count* only opens the question; what is
//! paid is everything past `consecDayHrsLimit` **hours** across the run. This
//! is the only rule in the family with such a limit, and it is the one thing
//! that makes the case shared with the sibling's spec attributable — see below.
//!
//! `priorConsecutiveDaysHours` is seeded before the week from the days the
//! prior-days calculator counted, and then advanced **one day late**:
//!
//! ```java
//! priorConsecutiveDaysHours = consecutiveDaysCounter == 0 ? 0 : priorConsecutiveDaysHours + currentDailyData.hoursForConsecDays();
//! currentDailyData = dailyData;
//! ```
//!
//! `currentDailyData` still holds *yesterday* when the line runs, so the total
//! never includes the day being processed — which is what makes
//! `hours + min(prior, limit) - limit` the right shape. And `hoursForConsecDays`
//! reads `getHours()`, the **current** value, so a day already reduced by an
//! earlier rule pass contributes less.
//!
//! # The case this spec shares with its sibling's belongs here
//!
//! `when both consec and weekly OT is checked…` appears in both specs with the
//! same eight shifts, the same four parameters and the same expectations. It is
//! right here and wrong there, for two reasons that are both config:
//!
//! - `minTimeBetweenShifts` defaults to **10.0** here and `7.0` there. The
//!   fixture's first gap is 3.25 hours, so the shortfall is `min(10 - 3.25, 4)`
//!   = **4** — the whole shift, as asserted — where the sibling gets
//!   `min(7 - 3.25, 4)` = 3.75.
//! - `consecDayHrsLimit` exists only here. The case sets it to 35, and the
//!   fifth consecutive day then pays `8 + min(28, 35) - 35` = **1** hour, as
//!   asserted. The sibling has no such parameter and pays the whole day.
//!
//! The sibling's copy is recorded as unreconciled in its own module and in
//! `PARITY_AUDIT.md`; this is the resolution.

use crate::common::json_ids::ids_from_json;
use crate::common::numbers::round_hours;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    BOTH_CONSECUTIVE_AND_WEEKLY_OT, CONSEC_DAY_HRS_LIMIT, CONSEC_DAY_OT_LIMIT, DAILY_DT_LIMIT_PROP,
    DAILY_OT_LIMIT_PROP, DAILY_SPLIT_SHIFT_LIMIT,
    DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig, EARNING_TYPES, MAX_CONSEC_DAYS_PD,
    MIN_TIME_BETWEEN_PAY_FULL_SHIFT, MIN_TIME_BETWEEN_SHIFTS,
    PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, THIS_WEEK_ONLY, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::hoursdistribution::prior_days_calculator::calculate_prior_consecutive_days;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::EmployeeShiftConsecutiveDaysPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Daily, weekly and hours-based consecutive-day overtime, with a
/// midnight-spanning minimum-break rule.
/// `DlyWklyConsecOTMinBreakSpanningMidnightRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DlyWklyConsecOTMinBreakSpanningMidnightRule<P: EmployeeShiftConsecutiveDaysPort> {
    consecutive_days: P,
}

impl<P: EmployeeShiftConsecutiveDaysPort> DlyWklyConsecOTMinBreakSpanningMidnightRule<P> {
    /// Build the rule over the lookup that primes the consecutive-day count.
    pub fn new(consecutive_days: P) -> Self {
        Self { consecutive_days }
    }
}

/// This rule's **private** `DailyData` — no schedule list, unlike the
/// sibling's.
struct DailyData {
    actual_shifts: Vec<usize>,
    earning_hours: f64,
}

impl DailyData {
    fn was_worked(&self) -> bool {
        !self.actual_shifts.is_empty() || self.earning_hours > 0.0
    }
}

/// `DailyData.hoursForConsecDays` — the day's earning hours plus the **current**
/// hours of its shifts' regular distributions dated that day.
fn hours_for_consec_days(
    time_card: &dyn TimeCard,
    daily_data: &DailyData,
    date: LocalDate,
    regular_buckets: &[i32],
) -> f64 {
    let shift_hours: f64 = daily_data
        .actual_shifts
        .iter()
        .flat_map(|&index| time_card.shifts()[index].hours_distributions())
        .filter(|d| d.date() == date)
        .filter(|d| {
            d.hours_distribution_type_id()
                .is_some_and(|id| regular_buckets.contains(&id))
        })
        .map(|d| d.hours())
        .sum();

    daily_data.earning_hours + shift_hours
}

/// This rule's **private** `WeeklyAccumulator`.
#[derive(Debug, Default)]
struct WeekTotals {
    hours: f64,
    weekly_ot: f64,
    weekly_limit: f64,
    consec_days_ot_limit: i32,
    consec_days_modifier: i32,
    consecutive_days_counter: i32,
    prior_consecutive_days_hours: f64,
    premium_hours_count_towards_weekly_ot: bool,
}

impl WeekTotals {
    /// `updateConsecutiveDays`. `yesterdays_hours` is
    /// `currentDailyData.hoursForConsecDays()` — the day *before* the one being
    /// started, because Java advances the total before reassigning the field.
    fn update_consecutive_days(&mut self, day_was_worked: bool, yesterdays_hours: f64) {
        if day_was_worked {
            self.consecutive_days_counter += 1;
            if self.consecutive_days_counter % self.consec_days_modifier != 0 {
                self.consecutive_days_counter %= self.consec_days_modifier;
            }
        } else {
            self.consecutive_days_counter = 0;
        }

        self.prior_consecutive_days_hours = if self.consecutive_days_counter == 0 {
            0.0
        } else {
            self.prior_consecutive_days_hours + yesterdays_hours
        };
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_weekly_ot(&mut self, add_weekly_ot: f64) {
        self.weekly_ot = round_hours(self.weekly_ot + add_weekly_ot);
    }

    /// `computeWeeklyOT(originalHours)` — zero inside the consecutive-day
    /// range, where the hours-based rule takes over.
    fn compute_weekly_ot(&self, original_hours: f64) -> f64 {
        if self.is_during_ot_consec_days() {
            return 0.0;
        }

        let pay_ot = self.premium_hours_count_towards_weekly_ot && self.hours > self.weekly_limit;
        let ot = if pay_ot {
            (self.hours - self.weekly_limit).min(original_hours)
        } else {
            self.hours - self.weekly_limit - self.weekly_ot
        };

        ot.max(0.0)
    }

    fn is_during_ot_consec_days(&self) -> bool {
        self.consecutive_days_counter >= self.consec_days_ot_limit
    }
}

/// This rule's **private** `DailyAccumulator`. No `addDoubleTime`, as the
/// sibling's has none.
#[derive(Debug, Default)]
struct DayTotals {
    hours: f64,
    overtime: f64,
    daily_ot_limit: f64,
    daily_dt_limit: f64,
}

impl DayTotals {
    fn new(daily_ot_limit: f64, daily_dt_limit: f64, earning_hours: f64) -> Self {
        Self {
            hours: earning_hours,
            overtime: round_hours(earning_hours - daily_ot_limit).max(0.0),
            daily_ot_limit,
            daily_dt_limit,
        }
    }

    fn add_hours(&mut self, add_hours: f64) {
        self.hours = round_hours(self.hours + add_hours);
    }

    fn add_overtime(&mut self, add_overtime: f64) {
        self.overtime = round_hours(self.overtime + add_overtime);
    }

    /// `calculateNormallyDistributedOT` — the larger of the ordinary daily
    /// figure and the hours-based consecutive-day one.
    fn normally_distributed_ot(&self, week: &WeekTotals, consec_days_hrs_limit: f64) -> f64 {
        let daily_ot = self.hours - self.daily_ot_limit - self.overtime;

        let consec_day_ot = if week.is_during_ot_consec_days() {
            self.hours + week.prior_consecutive_days_hours.min(consec_days_hrs_limit)
                - consec_days_hrs_limit
                - self.overtime
        } else {
            0.0
        };

        round_hours(daily_ot)
            .max(round_hours(consec_day_ot))
            .max(0.0)
    }

    /// `calculateDT` — unrounded, and never net of double time already paid.
    fn calculate_dt(&self) -> f64 {
        (self.hours - self.daily_dt_limit).max(0.0)
    }
}

/// The parameters the break rule needs.
struct BreakSettings {
    daily_split_shift_limit: f64,
    time_between_shifts: f64,
    pay_full_shift: bool,
}

impl<P: EmployeeShiftConsecutiveDaysPort> HoursDistributionRule
    for DlyWklyConsecOTMinBreakSpanningMidnightRule<P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig.default_values());

        let (Some(overtime_bucket), Some(double_time_bucket)) = (
            time_card.ot_hours_distribution_type_id(),
            time_card.dt_hours_distribution_type_id(),
        ) else {
            return;
        };
        let regular_buckets = time_card.regular_hours_distribution_type_ids();

        let earning_type_ids = ids_from_json(params.get(EARNING_TYPES).unwrap_or("[]"));
        let breaks = BreakSettings {
            daily_split_shift_limit: params.double_at(DAILY_SPLIT_SHIFT_LIMIT),
            time_between_shifts: params.double_at(MIN_TIME_BETWEEN_SHIFTS),
            pay_full_shift: params.bool_at(MIN_TIME_BETWEEN_PAY_FULL_SHIFT),
        };
        let consec_days_hrs_limit = params.double_at(CONSEC_DAY_HRS_LIMIT);
        // Read into a field in Java, where nothing reads it back.
        let _both_consecutive_and_weekly_ot = params.bool_at(BOTH_CONSECUTIVE_AND_WEEKLY_OT);

        let consec_days_ot_limit = params.int_at(CONSEC_DAY_OT_LIMIT);
        let consec_days_modifier = consec_days_ot_limit + params.int_at(MAX_CONSEC_DAYS_PD) - 1;

        // Built a day wider than it is read, as the sibling's is.
        let producer_range =
            DateRange::new(work_week.start_date().minus_days(1), work_week.end_date());
        let daily_data_map = build_daily_data_map(time_card, &producer_range, &earning_type_ids);

        let initial_consecutive_days = if params.bool_at(THIS_WEEK_ONLY) {
            0
        } else {
            calculate_prior_consecutive_days(
                time_card,
                &self.consecutive_days,
                work_week,
                &earning_type_ids,
                consec_days_modifier,
                true,
            ) as i32
        };

        // The hours worked over the days the calculator counted, seeded before
        // the week starts.
        let consec_days_hours = if initial_consecutive_days > 0 {
            prior_run_hours(
                time_card,
                work_week,
                initial_consecutive_days,
                &earning_type_ids,
                &regular_buckets,
            )
        } else {
            0.0
        };

        let mut week = WeekTotals {
            weekly_limit: params.double_at(WEEKLY_LIMIT_PROP),
            consec_days_ot_limit,
            consec_days_modifier,
            consecutive_days_counter: initial_consecutive_days,
            prior_consecutive_days_hours: consec_days_hours,
            premium_hours_count_towards_weekly_ot: params
                .bool_at(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT),
            ..WeekTotals::default()
        };

        let daily_ot_limit = params.double_at(DAILY_OT_LIMIT_PROP);
        let daily_dt_limit = params.double_at(DAILY_DT_LIMIT_PROP);

        // `currentDailyData`, which lags the loop by one day.
        let mut yesterday: Option<LocalDate> = None;

        for date in work_week.dates() {
            let yesterdays_hours = yesterday.map_or(0.0, |previous| {
                hours_for_consec_days(
                    time_card,
                    &daily_data_map[&previous],
                    previous,
                    &regular_buckets,
                )
            });

            let daily_data = &daily_data_map[&date];
            let mut day = DayTotals::new(daily_ot_limit, daily_dt_limit, daily_data.earning_hours);

            week.update_consecutive_days(daily_data.was_worked(), yesterdays_hours);
            week.add_hours(daily_data.earning_hours);

            for &shift_index in &daily_data_map[&date].actual_shifts.clone() {
                compute_distribution_hours(
                    time_card,
                    &mut week,
                    &mut day,
                    daily_data_map[&date].earning_hours,
                    date,
                    shift_index,
                    &breaks,
                    consec_days_hrs_limit,
                    &regular_buckets,
                    overtime_bucket,
                    double_time_bucket,
                    rule_item.id(),
                );
            }

            yesterday = Some(date);
        }
    }
}

/// The hours worked across the consecutive-day run the calculator counted, over
/// `[weekStart - consecutiveDays, weekStart - 1]`.
///
/// Note this half reads `getHours()` of **regular** distributions by
/// `HoursDistribution.isRegularType` — the static predicate, which tests the
/// `REGULAR_ID` constant rather than the property's configured buckets. The
/// in-week half goes through `getRegularHoursDistributionTypeIds()`. Two ways
/// of asking in one rule, the same split the family has recorded before.
fn prior_run_hours(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    consecutive_days: i32,
    earning_type_ids: &[i32],
    regular_buckets: &[i32],
) -> f64 {
    let range = DateRange::new(
        work_week
            .start_date()
            .minus_days(i64::from(consecutive_days)),
        work_week.start_date().minus_days(1),
    );

    let earning_hours: f64 = time_card
        .earnings()
        .iter()
        .filter(|earning| earning_type_ids.contains(&earning.earning_type_id()))
        .filter(|earning| range.contains_date(earning.earning_date()))
        .map(|earning| earning.hours())
        .sum();

    let shift_hours: f64 = time_card
        .shifts()
        .iter()
        .flat_map(EmployeeShift::hours_distributions)
        .filter(|d| range.contains_date(d.date()))
        .filter(|d| {
            d.hours_distribution_type_id()
                .is_some_and(|id| regular_buckets.contains(&id))
        })
        .map(|d| d.hours())
        .sum();

    round_hours(earning_hours + shift_hours)
}

/// `dailyDataProducer`. In schedule mode the day's shifts are the schedules.
fn build_daily_data_map(
    time_card: &dyn TimeCard,
    range: &DateRange,
    earning_type_ids: &[i32],
) -> HashMap<LocalDate, DailyData> {
    let is_ta_calculation_mode = !time_card.is_run_from_scheduling();

    range
        .dates()
        .into_iter()
        .map(|date| {
            let single_day = DateRange::new(date, date);

            let mut shifts = if is_ta_calculation_mode {
                time_card.shift_indices_with_distributions_for_period(&single_day)
            } else {
                (0..time_card
                    .schedules_with_distributions_for_period(&single_day)
                    .len())
                    .collect()
            };
            shifts.retain(|&index| {
                time_card.shift_is_not_salaried_exempt(&time_card.shifts()[index])
            });
            shifts.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

            let earning_hours = if is_ta_calculation_mode {
                time_card
                    .earnings()
                    .iter()
                    .filter(|earning| earning_type_ids.contains(&earning.earning_type_id()))
                    .filter(|earning| earning.earning_date() == date)
                    .map(|earning| earning.hours())
                    .sum()
            } else {
                0.0
            };

            (
                date,
                DailyData {
                    actual_shifts: shifts,
                    earning_hours,
                },
            )
        })
        .collect()
}

/// `computeDistributionHours`.
#[allow(clippy::too_many_arguments)]
fn compute_distribution_hours(
    time_card: &mut dyn TimeCard,
    week: &mut WeekTotals,
    day: &mut DayTotals,
    earning_hours: f64,
    date: LocalDate,
    shift_index: usize,
    breaks: &BreakSettings,
    consec_days_hrs_limit: f64,
    regular_buckets: &[i32],
    overtime_bucket: i32,
    double_time_bucket: i32,
    rule_item_id: i32,
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

    let break_ot = break_overtime(time_card, shift_index, date, breaks, regular_buckets);

    for distribution_index in distribution_indices {
        let original_hours = time_card.shifts()[shift_index].hours_distributions()
            [distribution_index]
            .original_hours();

        day.add_hours(original_hours);
        week.add_hours(original_hours);

        let normal_daily_ot = day.normally_distributed_ot(week, consec_days_hrs_limit);
        let daily_ot = match break_ot {
            Some(from_break) => normal_daily_ot.max(from_break),
            None => normal_daily_ot,
        };

        let premium_hours = week.compute_weekly_ot(original_hours).max(daily_ot);
        let dt_hours = day.calculate_dt();

        if time_card.is_open_for_editing_on(date) && premium_hours > 0.0 {
            let shift = &mut time_card.shifts_mut()[shift_index];
            let mut remaining = premium_hours;

            let premium = |hours: f64, bucket: i32, shift: &mut EmployeeShift| {
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

            if dt_hours > 0.0 {
                premium(dt_hours, double_time_bucket, shift);
                remaining -= dt_hours;
            }
            if remaining > 0.0 {
                premium(remaining, overtime_bucket, shift);
            }

            // The day's earning hours are folded in, as the sibling does.
            let hours = shift.hours_distributions()[distribution_index].hours();
            let reduced = round_hours((hours + earning_hours) - remaining - dt_hours);
            shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
        }

        day.add_overtime(premium_hours);
        week.add_weekly_ot(premium_hours);
    }
}

/// `calculateOTCausedByPriorShift`, guarded by
/// `shouldAccountForMinTimeBetweenShifts` — which here demands the prior shift
/// **ended the calendar day before** this one starts.
fn break_overtime(
    time_card: &dyn TimeCard,
    shift_index: usize,
    date: LocalDate,
    breaks: &BreakSettings,
    regular_buckets: &[i32],
) -> Option<f64> {
    let shift = &time_card.shifts()[shift_index];
    if shift.is_adjustment_only_shift() {
        return None;
    }

    let start = shift.start_date_time()?;

    let prior = time_card
        .shifts()
        .iter()
        .filter(|candidate| candidate.has_both_times())
        .filter(|candidate| candidate.end_date_time().is_some_and(|end| end < start))
        .max_by_key(|candidate| candidate.end_date_time())?;

    let prior_end = prior.end_date_time()?;
    let spans_midnight = prior_end.to_local_date() == start.to_local_date().minus_days(1);
    if !spans_midnight {
        return None;
    }

    let actual_time_between = DateTimeRange::of(prior_end, start)
        .duration()
        .fractional_hours();
    if actual_time_between <= breaks.daily_split_shift_limit
        || actual_time_between >= breaks.time_between_shifts
    {
        return None;
    }

    let total_shift_hours: f64 = shift
        .hours_distributions()
        .iter()
        .filter(|d| {
            d.hours_distribution_type_id()
                .is_some_and(|id| regular_buckets.contains(&id))
                && d.date() == date
        })
        .map(|d| d.original_hours())
        .sum();

    Some(if breaks.pay_full_shift {
        total_shift_hours
    } else {
        (breaks.time_between_shifts - actual_time_between).min(total_shift_hours)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    pub(super) const JOB: i32 = 1;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;
    pub(super) const INCLUDED_EARNING: i32 = 1;
    pub(super) const EXCLUDED_EARNING: i32 = 2;

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

    pub(super) fn rule() -> DlyWklyConsecOTMinBreakSpanningMidnightRule<PriorDays> {
        DlyWklyConsecOTMinBreakSpanningMidnightRule::new(PriorDays(0))
    }

    pub(super) fn day(offset: i64) -> LocalDate {
        LocalDate::of(2015, 6, 1).plus_days(offset)
    }

    pub(super) fn at(offset: i64, hour: i64, minute: i64) -> LocalDateTime {
        day(offset)
            .at_start_of_day()
            .plus_hours(hour)
            .plus_minutes(minute)
    }

    pub(super) fn shift(
        id: i32,
        date_offset: i64,
        start: LocalDateTime,
        end: LocalDateTime,
        hours: f64,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, day(date_offset), ShiftType::Actual, Vec::new())
            .with_times(Some(start), Some(end))
            .with_net_hours(hours)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                day(date_offset),
                Some(REGULAR),
                hours,
                0.0,
            )])
    }

    pub(super) fn earning(id: i32, offset: i64, type_id: i32, hours: f64) -> EmployeeEarning {
        EmployeeEarning::new(
            id,
            1,
            JOB,
            type_id,
            day(offset),
            hours,
            10.0,
            EarningSource::Auto,
        )
    }

    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(
                1,
                1,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    1,
                    JOB,
                    day(0),
                    day(365),
                    EmployeePayType::Hourly,
                    10.0,
                    true,
                )],
            ))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(day(0))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::DwcotMinBrSpanMnHdr, rule_params)
    }

    pub(super) fn one_day() -> DateRange {
        DateRange::new(day(0), day(0))
    }

    pub(super) fn week() -> DateRange {
        DateRange::new(day(0), day(7))
    }

    /// Every distribution on a shift as `(type, hours)`, sorted.
    pub(super) fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(i32, f64)> {
        let mut rows: Vec<(i32, f64)> = card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.hours_distribution_type_id().unwrap_or(0), d.hours()))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        rows
    }

    #[test]
    fn the_break_rule_needs_the_gap_to_span_midnight() {
        // Two shifts on ONE date, five hours apart: past the split-shift limit
        // and under the ten-hour minimum, but the prior shift did not end the
        // day before, so nothing is paid. The sibling rule pays this.
        let mut card = card(vec![
            shift(1, 0, at(0, 4, 0), at(0, 8, 0), 4.0),
            shift(2, 0, at(0, 13, 0), at(0, 17, 0), 4.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(rows(&card, 1), vec![(REGULAR, 4.0)]);
    }

    #[test]
    fn a_gap_across_midnight_inside_the_minimum_is_paid() {
        let mut card = card(vec![
            shift(1, 0, at(0, 17, 45), at(0, 21, 45), 4.0),
            shift(2, 1, at(1, 1, 0), at(1, 5, 0), 4.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        // The gap is 3.25 hours, so min(10 - 3.25, 4) is the whole shift.
        assert_eq!(rows(&card, 1), vec![(REGULAR, 0.0), (OVERTIME, 4.0)]);
    }

    #[test]
    fn min_time_between_pay_full_shift_pays_the_whole_shift() {
        // A gap of 9 hours leaves a shortfall of 1; with the flag set the whole
        // four-hour shift is paid instead.
        let shifts = || {
            vec![
                shift(1, 0, at(0, 12, 0), at(0, 16, 0), 4.0),
                shift(2, 1, at(1, 1, 0), at(1, 5, 0), 4.0),
            ]
        };

        let mut shortfall = card(shifts());
        rule().execute(&mut shortfall, &week(), &item(&[]));
        assert_eq!(rows(&shortfall, 1), vec![(REGULAR, 3.0), (OVERTIME, 1.0)]);

        let mut full = card(shifts());
        rule().execute(
            &mut full,
            &week(),
            &item(&[(MIN_TIME_BETWEEN_PAY_FULL_SHIFT, "true")]),
        );
        assert_eq!(rows(&full, 1), vec![(REGULAR, 0.0), (OVERTIME, 4.0)]);
    }

    #[test]
    fn consecutive_day_overtime_is_measured_in_hours() {
        // Six consecutive eight-hour days against a 40-hour consec-day limit:
        // the sixth day is in the range and pays 8 + min(40, 40) - 40 = 8.
        let shifts: Vec<EmployeeShift> = (0..6)
            .map(|d| shift(d as i32 + 1, d, at(d, 9, 0), at(d, 17, 0), 8.0))
            .collect();
        let mut card = card(shifts);

        rule().execute(&mut card, &week(), &item(&[(THIS_WEEK_ONLY, "true")]));

        assert_eq!(rows(&card, 5), vec![(REGULAR, 0.0), (OVERTIME, 8.0)]);
    }

    #[test]
    fn a_higher_consec_day_hours_limit_pays_less() {
        // The same six days against a 44-hour limit: 8 + 40 - 44 = 4.
        let shifts: Vec<EmployeeShift> = (0..6)
            .map(|d| shift(d as i32 + 1, d, at(d, 9, 0), at(d, 17, 0), 8.0))
            .collect();
        let mut card = card(shifts);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(THIS_WEEK_ONLY, "true"), (CONSEC_DAY_HRS_LIMIT, "44")]),
        );

        assert_eq!(rows(&card, 5), vec![(REGULAR, 4.0), (OVERTIME, 4.0)]);
    }

    #[test]
    fn a_property_with_no_double_time_bucket_writes_nothing() {
        let mut card = card(vec![shift(1, 0, at(0, 1, 0), at(0, 15, 0), 14.0)])
            .with_hours_distribution_types(vec![
                HoursDistributionType::new(REGULAR, "Regular", false),
                HoursDistributionType::new(OVERTIME, "Overtime", true),
            ]);

        rule().execute(&mut card, &one_day(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 14.0)]);
    }
}

/// `DlyWklyConsecOTMinBreakSpanningMidnightRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DlyWklyConsecOTMinBreakSpanningMidnightRuleImplTest.groovy`
/// — all thirteen cases. Each builds its own card; `today` is
/// `LocalDate.now()`, pinned here. The mocked `PayGroup` is divergence 24's
/// derivation, so the calculation start date is set on the card, and the mocked
/// `PriorDaysCalculator` answers 0 in every case.
///
/// The last case is the one the sibling rule's spec also carries; see the
/// module note for why it belongs here.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        DOUBLE_TIME, EXCLUDED_EARNING, INCLUDED_EARNING, OVERTIME, REGULAR, at, card, earning,
        item, one_day, rows, rule, shift, week,
    };
    use super::*;
    use crate::entity::employee_shift::EmployeeShift;

    #[test]
    fn daily_ot_is_given_if_you_work_more_than_the_daily_ot_threshold() {
        let mut card = card(vec![shift(1, 0, at(0, 0, 0), at(0, 4, 0), 10.0)]);

        rule().execute(&mut card, &one_day(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 8.0), (OVERTIME, 2.0)]);
    }

    #[test]
    fn weekly_ot_is_given_if_you_work_more_than_the_weekly_ot_threshold() {
        let mut card = card(vec![
            shift(1, 0, at(0, 0, 0), at(0, 4, 0), 10.0),
            shift(2, 1, at(1, 0, 0), at(1, 4, 0), 10.0),
            shift(3, 2, at(2, 0, 0), at(2, 4, 0), 10.0),
            shift(4, 3, at(3, 0, 0), at(3, 4, 0), 11.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[(DAILY_OT_LIMIT_PROP, "12")]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 10.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 10.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 10.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 10.0), (OVERTIME, 1.0)]);
    }

    #[test]
    fn consecutive_days_gives_overtime_over_the_consec_day_hrs_in_the_period() {
        let shifts: Vec<EmployeeShift> = (0..4)
            .map(|d| shift(d as i32 + 1, d, at(d, 0, 0), at(d, 4, 0), 5.0))
            .collect();
        let mut card = card(shifts);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(CONSEC_DAY_OT_LIMIT, "4"), (CONSEC_DAY_HRS_LIMIT, "17.0")]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 5.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 5.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 5.0)]);
        // The fourth day: 5 + min(15, 17) - 17 = 3.
        assert_eq!(rows(&card, 3), vec![(REGULAR, 2.0), (OVERTIME, 3.0)]);
    }

    #[test]
    fn weekly_ot_does_not_double_dip() {
        let mut card = card(vec![
            shift(1, 0, at(0, 0, 0), at(0, 4, 0), 12.0),
            shift(2, 1, at(1, 0, 0), at(1, 4, 0), 12.0),
            shift(3, 2, at(2, 0, 0), at(2, 4, 0), 12.0),
            shift(4, 3, at(3, 0, 0), at(3, 4, 0), 8.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false")]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 8.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 8.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 8.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 8.0)]);
    }

    #[test]
    fn split_shifts_are_not_given_minimum_break_ot() {
        let mut card = card(vec![
            shift(1, 0, at(0, 12, 0), at(0, 16, 0), 4.0),
            shift(2, 0, at(0, 17, 0), at(0, 21, 0), 4.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 4.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 4.0)]);
    }

    #[test]
    fn non_split_shifts_within_the_minimum_break_threshold_are_given_ot() {
        let mut card = card(vec![
            shift(1, 0, at(0, 4, 0), at(0, 8, 0), 4.0),
            shift(2, 0, at(0, 12, 0), at(0, 19, 0), 7.0),
            shift(3, 1, at(1, 2, 0), at(1, 6, 0), 4.0),
            shift(4, 1, at(1, 13, 0), at(1, 17, 0), 4.0),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 4.0)]);
        // Eleven hours on the day against an eight-hour limit.
        assert_eq!(rows(&card, 1), vec![(REGULAR, 4.0), (OVERTIME, 3.0)]);
        // 19:00 to 02:00 is 7 hours, across midnight and under the ten-hour
        // minimum: min(10 - 7, 4) = 3.
        assert_eq!(rows(&card, 2), vec![(REGULAR, 1.0), (OVERTIME, 3.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 4.0)]);
    }

    #[test]
    fn full_shift_is_paid_when_configured_for_full_shift() {
        let mut card = card(vec![
            shift(1, 0, at(0, 4, 0), at(0, 8, 0), 4.0),
            shift(2, 0, at(0, 12, 0), at(0, 19, 0), 7.0),
            shift(3, 1, at(1, 2, 0), at(1, 6, 0), 4.0),
            shift(4, 1, at(1, 13, 0), at(1, 17, 0), 4.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(MIN_TIME_BETWEEN_PAY_FULL_SHIFT, "true")]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 4.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 4.0), (OVERTIME, 3.0)]);
        // The whole shift, where the shortfall alone would have been 3.
        assert_eq!(rows(&card, 2), vec![(REGULAR, 0.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 4.0)]);
    }

    #[test]
    fn combine_all_rule_scenarios() {
        let mut card = card(vec![
            // 17:45, not the sibling spec's 20:00 — the gap to the next shift
            // is what the break rule measures.
            shift(1, 0, at(0, 17, 45), at(0, 21, 45), 4.0),
            shift(2, 1, at(1, 1, 0), at(1, 5, 0), 4.0),
            shift(3, 1, at(1, 7, 0), at(1, 16, 0), 9.0),
            shift(4, 2, at(2, 12, 0), at(2, 20, 0), 8.0),
            shift(5, 3, at(3, 12, 0), at(3, 22, 0), 10.0),
            shift(6, 4, at(4, 12, 0), at(4, 20, 0), 8.0),
            shift(7, 5, at(5, 12, 0), at(5, 20, 0), 8.0),
            shift(8, 6, at(6, 12, 0), at(6, 22, 0), 10.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAY_OT_LIMIT, "5"),
                (CONSEC_DAY_HRS_LIMIT, "35.0"),
                (MAX_CONSEC_DAYS_PD, "1"),
                (PREMIUM_HOURS_COUNT_TOWARDS_WEEKLY_OT, "false"),
            ]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 4.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 0.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 8.0), (DOUBLE_TIME, 1.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 8.0)]);
        assert_eq!(rows(&card, 4), vec![(REGULAR, 8.0), (OVERTIME, 2.0)]);
        assert_eq!(rows(&card, 5), vec![(REGULAR, 7.0), (OVERTIME, 1.0)]);
        assert_eq!(rows(&card, 6), vec![(REGULAR, 5.0), (OVERTIME, 3.0)]);
        assert_eq!(rows(&card, 7), vec![(REGULAR, 0.0), (OVERTIME, 10.0)]);
    }

    #[test]
    fn daily_ot_is_taken_into_account_even_if_there_is_a_min_break_violation() {
        let mut card = card(vec![
            shift(1, 0, at(0, 1, 0), at(0, 9, 0), 8.0),
            shift(2, 0, at(0, 15, 0), at(0, 23, 0), 8.0),
        ]);

        rule().execute(&mut card, &one_day(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 8.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(REGULAR, 0.0), (OVERTIME, 4.0), (DOUBLE_TIME, 4.0)]
        );
    }

    #[test]
    fn daily_ot_takes_into_account_configured_earning_types() {
        let mut card = card(vec![shift(1, 0, at(0, 0, 0), at(0, 8, 0), 8.0)]).with_earnings(vec![
            earning(1, 0, INCLUDED_EARNING, 2.0),
            earning(2, -1, INCLUDED_EARNING, 2.0),
            earning(3, 0, EXCLUDED_EARNING, 2.0),
        ]);

        rule().execute(&mut card, &one_day(), &item(&[(EARNING_TYPES, "[1]")]));

        assert_eq!(rows(&card, 0), vec![(REGULAR, 8.0), (OVERTIME, 2.0)]);
    }

    #[test]
    fn weekly_ot_accounts_for_configured_earning_types() {
        let mut card = card(vec![
            shift(1, 0, at(0, 0, 0), at(0, 4, 0), 5.0),
            shift(2, 1, at(1, 0, 0), at(1, 4, 0), 10.0),
            shift(3, 2, at(2, 0, 0), at(2, 4, 0), 4.0),
            shift(4, 3, at(3, 0, 0), at(3, 4, 0), 11.0),
        ])
        .with_earnings(vec![
            earning(1, 0, INCLUDED_EARNING, 5.0),
            earning(2, 2, INCLUDED_EARNING, 6.0),
            earning(3, 0, EXCLUDED_EARNING, 2.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(DAILY_OT_LIMIT_PROP, "12"), (EARNING_TYPES, "[1]")]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 5.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 10.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 4.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 10.0), (OVERTIME, 1.0)]);
    }

    #[test]
    fn consecutive_days_accounts_for_configured_earning_types() {
        let mut card = card(vec![
            shift(1, 0, at(0, 0, 0), at(0, 4, 0), 5.0),
            shift(2, 3, at(3, 0, 0), at(3, 4, 0), 5.0),
        ])
        .with_earnings(vec![
            earning(1, 1, INCLUDED_EARNING, 5.0),
            earning(2, 2, INCLUDED_EARNING, 6.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAY_OT_LIMIT, "4"),
                (CONSEC_DAY_HRS_LIMIT, "17.0"),
                (EARNING_TYPES, "[1]"),
            ]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 5.0)]);
        assert_eq!(rows(&card, 1), vec![(REGULAR, 1.0), (OVERTIME, 4.0)]);
    }

    /// The case this spec shares with `DlyWklyOffConsecOTMinBreak`'s, which is
    /// where it belongs: `minTimeBetweenShifts` defaults to 10 here and 7
    /// there, and `consecDayHrsLimit` exists only here.
    #[test]
    fn when_both_consec_and_weekly_ot_is_checked_it_applies_both_parameters() {
        let mut card = card(vec![
            shift(1, 0, at(0, 17, 45), at(0, 21, 45), 4.0),
            shift(2, 1, at(1, 1, 0), at(1, 5, 0), 4.0),
            shift(3, 1, at(1, 7, 0), at(1, 16, 0), 9.0),
            shift(4, 2, at(2, 12, 0), at(2, 20, 0), 8.0),
            shift(5, 3, at(3, 12, 0), at(3, 22, 0), 10.0),
            shift(6, 4, at(4, 12, 0), at(4, 20, 0), 8.0),
            shift(7, 5, at(5, 12, 0), at(5, 20, 0), 8.0),
            shift(8, 6, at(6, 12, 0), at(6, 22, 0), 10.0),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAY_OT_LIMIT, "5"),
                (CONSEC_DAY_HRS_LIMIT, "35.0"),
                (MAX_CONSEC_DAYS_PD, "1"),
                (BOTH_CONSECUTIVE_AND_WEEKLY_OT, "true"),
            ]),
        );

        assert_eq!(rows(&card, 0), vec![(REGULAR, 4.0)]);
        // The 3.25-hour gap across midnight: min(10 - 3.25, 4) is the whole
        // shift. The sibling's 7-hour default gives 3.75 here instead.
        assert_eq!(rows(&card, 1), vec![(REGULAR, 0.0), (OVERTIME, 4.0)]);
        assert_eq!(rows(&card, 2), vec![(REGULAR, 8.0), (DOUBLE_TIME, 1.0)]);
        assert_eq!(rows(&card, 3), vec![(REGULAR, 8.0)]);
        assert_eq!(rows(&card, 4), vec![(REGULAR, 8.0), (OVERTIME, 2.0)]);
        // The fifth consecutive day: 8 + min(28, 35) - 35 = 1.
        assert_eq!(rows(&card, 5), vec![(REGULAR, 7.0), (OVERTIME, 1.0)]);
        assert_eq!(rows(&card, 6), vec![(REGULAR, 0.0), (OVERTIME, 8.0)]);
        assert_eq!(rows(&card, 7), vec![(REGULAR, 0.0), (OVERTIME, 10.0)]);
    }
}
