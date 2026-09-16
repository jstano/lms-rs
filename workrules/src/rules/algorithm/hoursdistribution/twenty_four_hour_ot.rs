//! Port of `TwentyFourHourOTRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/TwentyFourHourOTRuleImpl.java`.
//!
//! `TFH_HDR`. Overtime measured against a **rolling 24-hour work day** rather
//! than a calendar day, and against the work week, whichever pays more. The
//! employee's work day starts when they first clock in that date and runs
//! twenty-four hours from there, so a shift early the next morning can still
//! fall inside yesterday's work day and push it over the limit.
//!
//! # The two measures, and the maximum between them
//!
//! Each shift gets a weekly figure and a work-day figure, and the accumulator
//! keeps `max(shiftWeeklyOT, shiftWorkDayOT)` — an hour that is both is paid
//! once. Same instinct as the California rules' `shiftOT`, reached differently:
//! there the two measures are daily and weekly against one distribution, here
//! they are work-day and weekly against a whole shift.
//!
//! The two are accumulated asymmetrically, and it is deliberate.
//! `workDayOTAccountedFor` is advanced **inside**
//! `accumulateWorkDayHoursAndComputeOTForWorkDay`, as soon as the figure is
//! computed; `weeklyOTAccountedFor` is advanced later, in the caller, and by
//! the **capped** `shiftOT` rather than by the weekly figure. So weekly
//! overtime is only ever booked as paid to the extent a shift could actually
//! absorb it.
//!
//! # A shift is visited twice, and paid on the second visit
//!
//! `getShiftsForWorkDay` walks the work day's own date and then appends the
//! shifts of the **following** date whose start falls strictly inside the
//! 24-hour window. Those carry `currentDayShift == false`: they contribute
//! work-day hours and bank overtime into `otAccumulator`, but nothing is
//! written and no weekly hours are counted for them. The next work day reaches
//! the same shift with `currentDayShift == true`, adds to what was banked, and
//! writes the total.
//!
//! That is why the accumulator is keyed by shift rather than reset per day,
//! and why `weeklyHrsAccumulated` is only ever advanced on the current-day
//! visit — otherwise a spanning shift would count toward the week twice.
//!
//! # The rate gate turns off work-day overtime only
//!
//! `overRateThreshold` compares either the FLSA regular rate for the week or
//! the employee's last home job status rate against `rateThreshold`, whose
//! default is 99,999 — so the gate is off unless a property turns it on. When
//! it is on, **work-day** overtime becomes zero and weekly overtime is
//! untouched. A highly paid employee still earns overtime past forty hours.
//!
//! # Adjustment-only shifts count for the week and not for the day
//!
//! `isShiftAdjustmentOnly` is `!hasErrors() && !hasBothTimes()` — a shift with
//! no punched times. Those are excluded from the work-day measure, because
//! there is no interval to intersect with the window, but their distributions
//! still feed `weeklyHrsAccumulated`. Four of the Java spec's cases turn on
//! exactly this.
//!
//! # `createAndDistributeOT` sorts the shift's own list in place
//!
//! `Collections.sort(shift.getHoursDistributions(), reverseOrder(comparing(getDate)))`
//! reorders the **live** list, latest date first, so the shift is left
//! reordered whether or not any overtime was paid. Second case of this in the
//! family, after `RollingXWeeksOTHrs`; reproduced, and pinned.
//!
//! Overtime is then spent against the latest date first, capped per row by that
//! row's remaining `hours` — so a shift that spans midnight pays its overtime
//! out of the second day before the first.

use crate::common::numbers::round_hours;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    EMPLOYEE_WORK_DAY_LIMIT_PROP, RATE_THRESHOLD_PROP, TwentyFourHourOTRuleConfig,
    USE_FLSA_REGULAR_RATE_PROP, WEEKLY_LIMIT_PROP,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDate;
use std::cmp::Reverse;
use std::collections::HashMap;

/// Hours in a work day. `DateTimeConstants.HOURS_PER_DAY`.
const HOURS_PER_DAY: i64 = 24;

/// Overtime against a rolling 24-hour work day and against the work week.
/// `TwentyFourHourOTRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TwentyFourHourOTRule;

/// The rule's four parameters. Java's private inner `RuleParams`, which shadows
/// the engine's [`RuleParams`](crate::rules::params::RuleParams) by name inside
/// that one file.
struct Settings {
    work_day_hours_before_ot: f64,
    weekly_hours_before_ot: f64,
    rate_threshold: f64,
    use_flsa_rate: bool,
}

/// The running totals. Java holds these as fields on a `@Scope("prototype")`
/// bean, initialised once per instance and never reset inside `execute`; a
/// fresh one per call says the same thing without depending on how the
/// container hands the rule out.
#[derive(Default)]
struct Accumulators {
    /// `otAccumulator`, keyed by shift **index** where Java keys by entity
    /// identity. It survives across work days on purpose — see the module note.
    ot_by_shift: HashMap<usize, f64>,
    weekly_hrs: f64,
    work_day_hrs: f64,
    weekly_ot_accounted_for: f64,
    work_day_ot_accounted_for: f64,
}

impl HoursDistributionRule for TwentyFourHourOTRule {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&TwentyFourHourOTRuleConfig.default_values());
        let settings = Settings {
            work_day_hours_before_ot: params.double_at(EMPLOYEE_WORK_DAY_LIMIT_PROP),
            weekly_hours_before_ot: params.double_at(WEEKLY_LIMIT_PROP),
            rate_threshold: params.double_at(RATE_THRESHOLD_PROP),
            use_flsa_rate: params.bool_at(USE_FLSA_REGULAR_RATE_PROP),
        };

        // Divergence 37: no "Overtime" bucket, nothing to pay into.
        let Some(overtime_bucket) = time_card.ot_hours_distribution_type_id() else {
            return;
        };
        let regular_buckets = time_card.regular_hours_distribution_type_ids();

        let shifts_by_date = group_shifts_by_date(time_card, work_week);
        let over_rate_threshold = is_rate_over_threshold(time_card, work_week, &settings);

        let mut accumulators = Accumulators::default();

        for date in work_week.dates() {
            if shifts_by_date[&date].is_empty() {
                continue;
            }

            accumulators.work_day_hrs = 0.0;
            accumulators.work_day_ot_accounted_for = 0.0;

            let work_day = work_day_date_time_range(time_card, &shifts_by_date, date);
            let shifts_for_work_day = shifts_for_work_day(&shifts_by_date, &work_day);

            for shift_index in shifts_for_work_day {
                compute_overtime_for_shift(
                    time_card,
                    shift_index,
                    &work_day,
                    &settings,
                    &mut accumulators,
                    over_rate_threshold,
                    rule_item.id(),
                    overtime_bucket,
                    &regular_buckets,
                );
            }
        }
    }
}

/// `getShiftsGroupedByDate` over `collectIncludedShiftsGroupedByShiftDate`.
///
/// Every date of the work week gets an entry, empty or not — which matters
/// later, because `endDateIsDifferentAndHasShifts` tests the map's **key set**
/// and so really asks "is the next day inside the work week".
///
/// Java's `Optional.of(...).filter(shiftsAreNotEmpty)` can therefore never
/// fail: the map has one entry per date of a non-empty range however few shifts
/// the card holds. The guard is dead and is not ported.
fn group_shifts_by_date(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
) -> HashMap<LocalDate, Vec<usize>> {
    let run_from_scheduling = time_card.is_run_from_scheduling();

    let mut included = time_card.shift_indices_for_period(work_week);
    included.retain(|&index| {
        let shift = &time_card.shifts()[index];
        // `shiftIsNotInErrorOrRunFromScheduling`, then
        // `employeeJobStatusForJobAndDateIsNotSalariedExempt` — which Java
        // dereferences unguarded; divergence 32 excludes instead.
        (shift.errors().is_empty() || run_from_scheduling)
            && time_card.shift_is_not_salaried_exempt(shift)
    });
    included.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

    let mut by_date: HashMap<LocalDate, Vec<usize>> = work_week
        .dates()
        .into_iter()
        .map(|d| (d, Vec::new()))
        .collect();
    for index in included {
        // `groupingBy` keeps encounter order, so each day stays start-time sorted.
        by_date
            .entry(time_card.shifts()[index].shift_date())
            .or_default()
            .push(index);
    }
    by_date
}

/// `isRateOverThreshold`.
fn is_rate_over_threshold(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    settings: &Settings,
) -> bool {
    let rate = if settings.use_flsa_rate {
        // `getFlsaRegularRateForDateRange` dereferences the map entry for the
        // week's end date. Divergence 39's case: a week the calc pipeline did
        // not fill reads as zero, so the gate stays off and the week is
        // calculated normally, rather than throwing inside the rule.
        time_card
            .flsa_data_map()
            .get(&work_week.end_date())
            .map_or(0.0, |flsa| flsa.regular_rate())
    } else {
        // `getRegularRateForLastEmployeeHomeJobStatus`, also unguarded in Java.
        time_card
            .employee()
            .and_then(|employee| employee.last_home_employee_job_status_for_period(work_week))
            .map_or(0.0, |status| status.hourly_rate())
    };

    rate > settings.rate_threshold
}

/// `getWorkDateTimeRange` — twenty-four hours from the employee's first clock-in
/// that date, or from midnight if nothing on it has both times.
fn work_day_date_time_range(
    time_card: &dyn TimeCard,
    shifts_by_date: &HashMap<LocalDate, Vec<usize>>,
    date: LocalDate,
) -> DateTimeRange {
    let start = shifts_by_date[&date]
        .iter()
        .filter_map(|&index| {
            let shift = &time_card.shifts()[index];
            shift.has_both_times().then(|| shift.start_date_time())?
        })
        .next()
        .unwrap_or_else(|| date.at_start_of_day());

    DateTimeRange::of(start, start.plus_hours(HOURS_PER_DAY))
}

/// `getShiftsForWorkDay` — the work day's own shifts, plus the next date's
/// shifts that begin before the window closes.
///
/// `endDateIsDifferentAndHasShifts` guards the second half with
/// `!start.equals(end)`, which is never true for two datetimes twenty-four
/// hours apart, and with the key set containing the end date — so in practice
/// it asks only whether the next day is still inside the work week.
fn shifts_for_work_day(
    shifts_by_date: &HashMap<LocalDate, Vec<usize>>,
    work_day: &DateTimeRange,
) -> Vec<(usize, LocalDate)> {
    let start_date = work_day.start().to_local_date();
    let end_date = work_day.end().to_local_date();

    let mut shifts: Vec<(usize, LocalDate)> = shifts_by_date
        .get(&start_date)
        .map(|indices| indices.iter().map(|&i| (i, start_date)).collect())
        .unwrap_or_default();

    if start_date != end_date && shifts_by_date.contains_key(&end_date) {
        shifts.extend(
            shifts_by_date[&end_date]
                .iter()
                .map(|&index| (index, end_date)),
        );
    }

    shifts
}

/// `computeOvertimeForShift`.
#[allow(clippy::too_many_arguments)]
fn compute_overtime_for_shift(
    time_card: &mut dyn TimeCard,
    shift_and_date: (usize, LocalDate),
    work_day: &DateTimeRange,
    settings: &Settings,
    accumulators: &mut Accumulators,
    over_rate_threshold: bool,
    rule_item_id: i32,
    overtime_bucket: i32,
    regular_buckets: &[i32],
) {
    let (shift_index, grouped_under) = shift_and_date;
    let shift = &time_card.shifts()[shift_index];

    // The next-day shifts appended above are filtered here in Java, by
    // `containsDateTimeExclusive(shift.getStartDateTime())`. A shift with no
    // start time answers false rather than throwing.
    if grouped_under != work_day.start().to_local_date()
        && !shift
            .start_date_time()
            .is_some_and(|start| work_day.contains_exclusive(start))
    {
        return;
    }

    let current_day_shift = shift.shift_date() == work_day.start().to_local_date();

    let shift_weekly_ot = if current_day_shift {
        accumulate_work_week_hours(shift, regular_buckets, settings, accumulators)
    } else {
        0.0
    };

    let shift_work_day_ot = if over_rate_threshold || is_shift_adjustment_only(shift) {
        0.0
    } else {
        accumulate_work_day_hours(shift, work_day, settings, accumulators)
    };

    let banked = accumulators.ot_by_shift.entry(shift_index).or_insert(0.0);
    *banked += shift_weekly_ot.max(shift_work_day_ot);
    let accumulated = *banked;

    if current_day_shift {
        let shift_ot = accumulated.min(time_card.shifts()[shift_index].net_hours());
        accumulators.weekly_ot_accounted_for += shift_ot;

        if time_card.is_open_for_editing_for_shift(&time_card.shifts()[shift_index]) {
            create_and_distribute_ot(
                time_card,
                shift_index,
                rule_item_id,
                shift_ot,
                overtime_bucket,
                regular_buckets,
            );
        }
    }
}

/// `accumulateWorkWeekHoursAndComputeOTForWorkWeek`.
///
/// The week is measured on the shift's **regular** distributions' original
/// hours, not on its net hours — so an adjustment-only shift contributes
/// whatever its distributions claim.
fn accumulate_work_week_hours(
    shift: &EmployeeShift,
    regular_buckets: &[i32],
    settings: &Settings,
    accumulators: &mut Accumulators,
) -> f64 {
    let shift_hours: f64 = shift
        .hours_distributions()
        .iter()
        .filter(|d| {
            d.hours_distribution_type_id()
                .is_some_and(|id| regular_buckets.contains(&id))
        })
        .map(|d| d.original_hours())
        .sum();

    accumulators.weekly_hrs += shift_hours;

    if accumulators.weekly_hrs > settings.weekly_hours_before_ot {
        round_hours(
            accumulators.weekly_hrs
                - settings.weekly_hours_before_ot
                - accumulators.weekly_ot_accounted_for,
        )
    } else {
        0.0
    }
}

/// `isShiftAdjustmentOnly` — no saved errors and no punched times.
fn is_shift_adjustment_only(shift: &EmployeeShift) -> bool {
    !shift.has_errors() && !shift.has_both_times()
}

/// `accumulateWorkDayHoursAndComputeOTForWorkDay`.
///
/// Unlike its weekly twin this one advances `workDayOTAccountedFor` itself,
/// immediately.
fn accumulate_work_day_hours(
    shift: &EmployeeShift,
    work_day: &DateTimeRange,
    settings: &Settings,
    accumulators: &mut Accumulators,
) -> f64 {
    accumulators.work_day_hrs =
        round_hours(accumulators.work_day_hrs + shift.net_hours_in_range(work_day));

    if accumulators.work_day_hrs > settings.work_day_hours_before_ot {
        let shift_work_day_ot = round_hours(
            accumulators.work_day_hrs
                - settings.work_day_hours_before_ot
                - accumulators.work_day_ot_accounted_for,
        );
        accumulators.work_day_ot_accounted_for =
            round_hours(accumulators.work_day_ot_accounted_for + shift_work_day_ot);
        shift_work_day_ot
    } else {
        0.0
    }
}

/// `createAndDistributeOT` — spend the shift's overtime against its regular
/// rows, latest date first.
fn create_and_distribute_ot(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    rule_item_id: i32,
    shift_ot: f64,
    overtime_bucket: i32,
    regular_buckets: &[i32],
) {
    let shift = &mut time_card.shifts_mut()[shift_index];

    // Java sorts the live list, so the shift is left reordered either way.
    shift
        .hours_distributions_mut()
        .sort_by_key(|distribution| Reverse(distribution.date()));

    let mut remaining = shift_ot;
    let mut premium_rows = Vec::new();

    for index in 0..shift.hours_distributions().len() {
        let distribution = &shift.hours_distributions()[index];
        if !distribution
            .hours_distribution_type_id()
            .is_some_and(|id| regular_buckets.contains(&id))
        {
            continue;
        }

        let ot_to_distribute = remaining.min(distribution.hours());
        if ot_to_distribute > 0.0 {
            premium_rows.push(create_premium_distribution(
                distribution,
                overtime_bucket,
                ot_to_distribute,
                Some(rule_item_id),
            ));

            let reduced = round_hours(distribution.hours() - ot_to_distribute);
            shift.hours_distributions_mut()[index].set_hours(reduced);
            remaining -= ot_to_distribute;
        }
    }

    for row in premium_rows {
        shift.add_hours_distribution(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    pub(super) const JOB: i32 = 12;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;

    /// `workWeekStartDate`, plus an offset in days.
    pub(super) fn day(offset: i64) -> LocalDate {
        LocalDate::of(2014, 9, 8).plus_days(offset)
    }

    /// `new LegacyDatePeriod(workWeekStartDate, workWeekEndDate)`.
    pub(super) fn week() -> DateRange {
        DateRange::new(day(0), day(6))
    }

    pub(super) fn at(date: LocalDate, hour: i64, minute: i64) -> LocalDateTime {
        date.at_start_of_day().plus_hours(hour).plus_minutes(minute)
    }

    pub(super) fn employee_with(job_statuses: Vec<EmployeeJobStatus>) -> Employee {
        Employee::new(1, 1, "", job_statuses)
    }

    /// The spec's one home job status: hourly, 8.50, covering the work week.
    pub(super) fn job_status(
        job_id: i32,
        rate: f64,
        home: bool,
        pay_type: EmployeePayType,
    ) -> EmployeeJobStatus {
        EmployeeJobStatus::new(1, 1, job_id, day(0), day(6), pay_type, rate, home)
    }

    pub(super) fn employee() -> Employee {
        employee_with(vec![job_status(JOB, 8.50, true, EmployeePayType::Hourly)])
    }

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    /// `empShiftWithPunches(netHours, shiftDate, startTime)`.
    pub(super) fn shift_with_punches(
        id: i32,
        net_hours: f64,
        shift_date: LocalDate,
        start_hour: i64,
        start_minute: i64,
    ) -> EmployeeShift {
        let start = at(shift_date, start_hour, start_minute);
        let end = start.plus_seconds((net_hours * 3600.0) as i64);

        EmployeeShift::new(
            id,
            1,
            JOB,
            shift_date,
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, start),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, end),
            ],
        )
        .with_times(Some(start), Some(end))
        .with_net_hours(net_hours)
        .with_hours_distributions(vec![regular(shift_date, net_hours)])
    }

    /// `empShiftWithPunchesSplitByDayShift` — the hours split across midnight
    /// into two regular rows.
    pub(super) fn shift_split_by_day(
        id: i32,
        net_hours: f64,
        shift_date: LocalDate,
        start_hour: i64,
        start_minute: i64,
    ) -> EmployeeShift {
        let start = at(shift_date, start_hour, start_minute);
        let midnight = shift_date.plus_days(1).at_start_of_day();
        let first_day_hours = DateTimeRange::of(start, midnight)
            .duration()
            .fractional_hours();

        shift_with_punches(id, net_hours, shift_date, start_hour, start_minute)
            .with_hours_distributions(vec![
                regular(shift_date, first_day_hours),
                regular(shift_date.plus_days(1), net_hours - first_day_hours),
            ])
    }

    /// `empShiftWithAdjustmentOnly(adjHours, shiftDate)` — no times and no
    /// punches, its hours carried only by a distribution.
    ///
    /// The Groovy attaches a mocked `EmployeeShiftAdjustment` of type `WORKED`,
    /// which is what makes Java's derived `getNetHours()` report those hours.
    /// The adjustments entity is not ported (see `employee_shift.rs`), so the
    /// resulting net hours are set on the shift directly — the rule reads
    /// `getNetHours()` and never the adjustment list.
    pub(super) fn shift_adjustment_only(
        id: i32,
        adj_hours: f64,
        shift_date: LocalDate,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_net_hours(adj_hours)
            .with_hours_distributions(vec![regular(shift_date, adj_hours)])
    }

    /// `setup()`: the employee, one FLSA row on the week's end date, the
    /// system-generated buckets, and a pay period opening on the work week's
    /// first day (divergence 24 — the mocked `PayGroup` is not ported).
    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_flsa_data(HashMap::from([(day(6), flsa_row())]))
            .with_calculation_start_date(day(0))
    }

    /// An FLSA row whose regular rate is zero — the Groovy uses `Mock(FlsaData)`,
    /// whose unstubbed `getRegularRate()` also answers zero.
    pub(super) fn flsa_row() -> FlsaData {
        FlsaData::new(day(6), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    }

    /// `shift.punches << BREAK; shift.punches << BACK` — an unpaid break inside
    /// an already-built shift, which splits its worked interval in two.
    pub(super) fn with_break(
        shift: EmployeeShift,
        break_at: LocalDateTime,
        back_at: LocalDateTime,
    ) -> EmployeeShift {
        shift
            .with_punch(EmployeeShiftPunch::new(
                3,
                PunchType::Break,
                PunchSource::Clock,
                break_at,
            ))
            .with_punch(EmployeeShiftPunch::new(
                4,
                PunchType::Back,
                PunchSource::Clock,
                back_at,
            ))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::TwentyFourHourHdr, rule_params)
    }

    /// The hours on a shift's first overtime row, or zero if it has none.
    /// `validateOvertimeHoursInListOfShifts` in the Groovy.
    pub(super) fn ot_hours(card: &TimeCardData, shift_index: usize) -> f64 {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .find(|d| d.is_of_type(OVERTIME))
            .map_or(0.0, |d| d.hours())
    }

    /// Assert each shift's overtime, the way the Groovy's helper does.
    pub(super) fn assert_ot(card: &TimeCardData, expected: &[f64]) {
        let actual: Vec<f64> = (0..card.shifts().len())
            .map(|index| ot_hours(card, index))
            .collect();
        assert_eq!(actual, expected);
    }

    fn run(shifts: Vec<EmployeeShift>) -> TimeCardData {
        let mut card = card(shifts);
        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));
        card
    }

    #[test]
    fn a_shift_inside_the_limits_is_left_alone() {
        let card = run(vec![shift_with_punches(1, 8.0, day(0), 10, 0)]);

        assert_ot(&card, &[0.0]);
        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
    }

    #[test]
    fn the_work_day_starts_at_the_first_clock_in_not_at_midnight() {
        // 8 hours from 10:00, then 5 hours from 09:00 the next morning: one of
        // those five falls inside the first work day and is its ninth hour.
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 5.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 1.0]);
    }

    #[test]
    fn overtime_banked_on_one_work_day_is_paid_on_the_next() {
        // The second shift earns its hour while being visited as part of day
        // one's window, and is written on day two.
        let mut card = card(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 5.0, day(1), 9, 0),
        ]);

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_eq!(ot_hours(&card, 1), 1.0);
        assert_eq!(
            card.shifts()[1].hours_distributions().len(),
            2,
            "the regular row stays, reduced"
        );
    }

    #[test]
    fn the_rate_gate_stops_work_day_overtime_and_leaves_weekly_alone() {
        let shifts = || vec![shift_with_punches(1, 9.0, day(0), 10, 0)];

        let mut ungated = card(shifts());
        TwentyFourHourOTRule.execute(&mut ungated, &week(), &item(&[]));
        assert_eq!(ot_hours(&ungated, 0), 1.0);

        let mut gated = card(shifts());
        TwentyFourHourOTRule.execute(&mut gated, &week(), &item(&[(RATE_THRESHOLD_PROP, "8.0")]));
        assert_eq!(ot_hours(&gated, 0), 0.0, "8.50 is over the threshold");
    }

    #[test]
    fn a_missing_flsa_row_leaves_the_gate_off() {
        // Java dereferences the map entry for the week's end date; divergence 39
        // reads an absent row as establishing nothing.
        let mut card =
            card(vec![shift_with_punches(1, 9.0, day(0), 10, 0)]).with_flsa_data(HashMap::new());

        TwentyFourHourOTRule.execute(
            &mut card,
            &week(),
            &item(&[
                (USE_FLSA_REGULAR_RATE_PROP, "true"),
                (RATE_THRESHOLD_PROP, "0.0"),
            ]),
        );

        assert_eq!(ot_hours(&card, 0), 1.0, "a zero rate is not over zero");
    }

    #[test]
    fn a_property_with_no_overtime_bucket_writes_nothing() {
        let mut card = card(vec![shift_with_punches(1, 9.0, day(0), 10, 0)])
            .with_hours_distribution_types(vec![HoursDistributionType::new(
                REGULAR, "Regular", false,
            )]);

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
        assert_eq!(card.shifts()[0].hours_distributions()[0].hours(), 9.0);
    }

    #[test]
    fn the_shifts_own_distribution_list_is_left_sorted_latest_date_first() {
        // Collections.sort on the live list, whether or not overtime was paid.
        let card = run(vec![shift_split_by_day(1, 8.0, day(0), 20, 0)]);

        let dates: Vec<LocalDate> = card.shifts()[0]
            .hours_distributions()
            .iter()
            .map(|d| d.date())
            .collect();

        assert_eq!(dates, vec![day(1), day(0)], "reverse order by date");
    }

    #[test]
    fn a_salaried_exempt_shift_is_excluded_entirely() {
        let mut card =
            card(vec![shift_with_punches(1, 9.0, day(0), 10, 0)]).with_employee(employee_with(
                vec![job_status(JOB, 8.50, true, EmployeePayType::SalariedExempt)],
            ));

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_ot(&card, &[0.0]);
    }

    #[test]
    fn an_adjustment_only_shift_feeds_the_week_but_not_the_work_day() {
        // Nine hours punched plus eight adjustment-only on the same date: the
        // work-day measure sees only the nine.
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_adjustment_only(2, 8.0, day(0)),
        ]);

        assert_ot(&card, &[1.0, 0.0]);
    }

    #[test]
    fn net_hours_in_range_measures_punches_against_a_window() {
        // The ShiftUtil half, directly: a 10:00-18:00 shift against a window
        // opening at 14:00 contributes four hours.
        let shift = shift_with_punches(1, 8.0, day(0), 10, 0);
        let window = DateTimeRange::of(at(day(0), 14, 0), at(day(1), 14, 0));

        assert_eq!(shift.net_hours_in_range(&window), 4.0);
    }

    #[test]
    fn a_positive_adjustment_lengthens_the_last_worked_range() {
        let shift = shift_with_punches(1, 8.0, day(0), 10, 0).with_adj_hours(1.0);
        let window = DateTimeRange::of(at(day(0), 0, 0), at(day(1), 0, 0));

        assert_eq!(shift.net_hours_in_range(&window), 9.0);
    }

    #[test]
    fn a_negative_adjustment_eats_back_from_the_end() {
        let shift = shift_with_punches(1, 8.0, day(0), 10, 0).with_adj_hours(-1.0);
        let window = DateTimeRange::of(at(day(0), 0, 0), at(day(1), 0, 0));

        assert_eq!(shift.net_hours_in_range(&window), 7.0);
    }

    #[test]
    fn a_shift_with_no_times_contributes_no_hours_to_a_window() {
        let shift = shift_adjustment_only(1, 8.0, day(0));
        let window = DateTimeRange::of(at(day(0), 0, 0), at(day(1), 0, 0));

        assert_eq!(shift.net_hours_in_range(&window), 0.0);
    }

    #[test]
    fn an_errored_actual_shift_is_excluded_but_a_scheduled_one_is_not() {
        let errored = || {
            vec![
                shift_with_punches(1, 9.0, day(0), 9, 0)
                    .with_errors(vec![ShiftErrorType::MissingOut]),
            ]
        };

        let mut actuals = card(errored());
        TwentyFourHourOTRule.execute(&mut actuals, &week(), &item(&[]));
        assert_ot(&actuals, &[0.0]);

        let mut scheduling =
            card(errored()).with_calculation_mode(EmployeeCalculationMode::EditSchedule);
        TwentyFourHourOTRule.execute(&mut scheduling, &week(), &item(&[]));
        assert_ot(&scheduling, &[1.0]);
    }
}

/// `TwentyFourHourOTRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/TwentyFourHourOTRuleImplTest.groovy`
/// — 27 methods, four of them tabled, for 31 executions. The largest spec in
/// the family by some way, and the most valuable: the 24-hour window is
/// arithmetic no amount of reading the source pins down.
///
/// `TwentyFourHourOTRuleConfigTest.groovy` is transcribed alongside the other
/// configs in [`config`](super::config); its `testGetProperties` case is the
/// l2fprod UI half, which that module does not port.
///
/// Adjustments to the transcriptions, none touching what is asserted:
///
/// - The mocked `PayGroup` supplying the calculation start date is
///   divergence 24's derivation, so the resulting date is set on the card.
/// - `empShiftWithAdjustmentOnly` attaches a mocked `EmployeeShiftAdjustment`
///   of type `WORKED`, which is what makes Java's derived `getNetHours()`
///   report those hours. The adjustments entity is not ported, so the net hours
///   are set directly; the rule reads `getNetHours()` and never the list.
/// - `Mock(FlsaData)` answers zero for `getRegularRate()`, so the fixture row
///   carries an explicit zero.
///
/// # `A shift without punches should not get daily OT` cannot reach its gate
///
/// It builds its shift on `workWeekStartDate.minusDays(1)`, which
/// `getShiftsForPeriod` drops before anything looks at the punches — the same
/// shape as the two `ScheduledShiftOTRuleImplTest` cases already recorded in
/// the audit. Transcribed as written, with a second assertion beside it that
/// puts the punchless shift **inside** the week, so the case exercises what it
/// claims to.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        JOB, OVERTIME, REGULAR, assert_ot, at, card, day, employee_with, item, job_status,
        ot_hours, shift_adjustment_only, shift_split_by_day, shift_with_punches, week, with_break,
    };
    use super::*;
    use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use rstest::rstest;

    /// Run the default-configured rule over a card of shifts.
    /// `runRuleAndValidateOvertimeHoursInListOfShifts`.
    fn run(shifts: Vec<EmployeeShift>) -> TimeCardData {
        let mut card = card(shifts);
        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));
        card
    }

    /// The hours of every distribution of a type on a shift, by date.
    fn rows(card: &TimeCardData, shift_index: usize, type_id: i32) -> Vec<(LocalDate, f64)> {
        let mut rows: Vec<(LocalDate, f64)> = card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .filter(|d| d.is_of_type(type_id))
            .map(|d| (d.date(), d.hours()))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        rows
    }

    #[rstest]
    // useFLSARate | rateThreshold | expectedHomeHours | expectedSecHours
    #[case("false", "8.0", 1.0, 8.0)]
    #[case("true", "14.0", 1.0, 8.0)]
    fn overtime_should_be_paid_based_on_the_rate_threshold_and_the_flsa_regular_rate_flag(
        #[case] use_flsa_rate: &str,
        #[case] rate_threshold: &str,
        #[case] expected_home: f64,
        #[case] expected_secondary: f64,
    ) {
        const HOME_JOB: i32 = 1;
        const SECONDARY_JOB: i32 = 2;

        let home_shift = shift_with_punches(1, 9.0, day(0), 0, 0).with_job_id(HOME_JOB);
        let secondary_shift = shift_with_punches(2, 8.0, day(0), 10, 0).with_job_id(SECONDARY_JOB);

        let mut card = card(vec![home_shift, secondary_shift]).with_employee(employee_with(vec![
            job_status(HOME_JOB, 8.0, true, EmployeePayType::Hourly),
            job_status(SECONDARY_JOB, 20.0, false, EmployeePayType::Hourly),
        ]));

        TwentyFourHourOTRule.execute(
            &mut card,
            &week(),
            &item(&[
                (EMPLOYEE_WORK_DAY_LIMIT_PROP, "8"),
                (WEEKLY_LIMIT_PROP, "40"),
                (USE_FLSA_REGULAR_RATE_PROP, use_flsa_rate),
                (RATE_THRESHOLD_PROP, rate_threshold),
            ]),
        );

        assert_ot(&card, &[expected_home, expected_secondary]);
    }

    #[rstest]
    #[case(9.0, 1.0)]
    #[case(12.0, 4.0)]
    fn a_single_shift_over_the_work_day_limit_gets_ot(
        #[case] net_hours: f64,
        #[case] expected_ot: f64,
    ) {
        let card = run(vec![shift_with_punches(1, net_hours, day(0), 8, 0)]);

        assert_ot(&card, &[expected_ot]);
    }

    #[test]
    fn a_shift_without_punches_should_not_get_daily_ot() {
        // As written: the shift is dated the day before the week, so
        // getShiftsForPeriod drops it before the punches are consulted.
        let outside = EmployeeShift::new(1, 1, JOB, day(-1), ShiftType::Actual, Vec::new())
            .with_times(Some(at(day(-1), 8, 0)), Some(at(day(-1), 17, 0)))
            .with_net_hours(9.0)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                day(-1),
                Some(REGULAR),
                9.0,
                0.0,
            )]);

        assert_ot(&run(vec![outside]), &[0.0]);

        // The gate the case is named for, reached: a punchless shift inside the
        // week contributes no worked interval, so the work-day measure sees
        // nothing. It is not adjustment-only — it has both times — so it is not
        // excluded by that branch either.
        let inside = EmployeeShift::new(1, 1, JOB, day(0), ShiftType::Actual, Vec::new())
            .with_times(Some(at(day(0), 8, 0)), Some(at(day(0), 17, 0)))
            .with_net_hours(9.0)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                day(0),
                Some(REGULAR),
                9.0,
                0.0,
            )]);

        assert_ot(&run(vec![inside]), &[0.0]);
    }

    #[rstest]
    #[case(false, &[1.0, 1.0])]
    #[case(true, &[0.0, 0.0])]
    fn actual_shifts_with_errors_do_not_get_daily_overtime(
        #[case] has_error: bool,
        #[case] expected: &[f64],
    ) {
        let errors = if has_error {
            vec![ShiftErrorType::MissingOut]
        } else {
            Vec::new()
        };

        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 9, 0).with_errors(errors.clone()),
            shift_with_punches(2, 9.0, day(1), 9, 0).with_errors(errors),
        ]);

        assert_ot(&card, expected);
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn schedule_shifts_with_errors_do_get_daily_overtime(#[case] has_error: bool) {
        let errors = if has_error {
            vec![ShiftErrorType::MissingOut]
        } else {
            Vec::new()
        };

        let mut card = card(vec![
            shift_with_punches(1, 9.0, day(0), 9, 0)
                .with_shift_type(ShiftType::Schedule)
                .with_errors(errors.clone()),
            shift_with_punches(2, 9.0, day(1), 9, 0)
                .with_shift_type(ShiftType::Schedule)
                .with_errors(errors),
        ])
        .with_calculation_mode(EmployeeCalculationMode::EditSchedule);

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_ot(&card, &[1.0, 1.0]);
    }

    #[test]
    fn an_exempt_salaried_employee_does_not_get_daily_ot() {
        let mut card = card(vec![
            shift_with_punches(1, 9.0, day(0), 9, 0),
            shift_with_punches(2, 9.0, day(1), 9, 0),
        ])
        .with_employee(employee_with(vec![job_status(
            JOB,
            8.50,
            true,
            EmployeePayType::SalariedExempt,
        )]));

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn we_should_not_modify_shifts_in_a_closed_pay_period() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(-2), 9, 0),
            shift_with_punches(2, 9.0, day(-1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn we_shouldnt_modify_ot_for_shifts_that_are_before_the_calculation_start_date() {
        let mut existing_ot = HoursDistribution::new(1, day(0), Some(OVERTIME), 1.0, 0.0);
        existing_ot.set_original_hours(7.0);

        let mut card = card(vec![
            shift_with_punches(1, 7.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0).with_hours_distributions(vec![
                HoursDistribution::new(1, day(1), Some(REGULAR), 8.0, 0.0),
                existing_ot,
            ]),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(4), 10, 0),
            shift_adjustment_only(6, 8.0, day(5)),
            shift_with_punches(7, 8.0, day(5), 9, 0),
        ])
        // `currentPayPeriod() >> of(workWeekStartDate.plusDays(2))`.
        .with_calculation_start_date(day(2));

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_ot(&card, &[0.0, 1.0, 0.0, 0.0, 0.0, 7.0, 8.0]);
    }

    #[test]
    fn ot_gets_assigned_to_the_correct_shift_when_you_have_multiple_shifts_spanning_multiple_days()
    {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 5.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 1.0]);
    }

    #[test]
    fn three_shifts_with_two_in_the_same_24_hours_and_the_last_2_get_ot() {
        let card = run(vec![
            shift_with_punches(1, 4.0, day(0), 7, 0),
            shift_with_punches(2, 5.0, day(0), 13, 0),
            shift_with_punches(3, 7.0, day(1), 6, 0),
        ]);

        assert_ot(&card, &[0.0, 1.0, 1.0]);
    }

    #[test]
    fn three_shifts_with_two_in_the_same_24_hours_and_the_last_one_gets_ot() {
        let card = run(vec![
            shift_with_punches(1, 3.0, day(0), 8, 0),
            shift_with_punches(2, 5.0, day(0), 13, 0),
            shift_with_punches(3, 7.0, day(1), 7, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 1.0]);
    }

    #[test]
    fn ot_not_assigned_when_beginning_workday_shifts_are_split() {
        let card = run(vec![
            shift_with_punches(1, 3.0, day(0), 8, 0),
            shift_with_punches(2, 5.0, day(0), 13, 0),
            shift_with_punches(3, 3.0, day(1), 8, 0),
            shift_with_punches(4, 5.0, day(1), 12, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn ot_is_calculated_when_a_shift_the_next_day_puts_more_than_8_hours_in_the_24_hour_workday() {
        let card = run(vec![
            shift_with_punches(1, 4.0, day(0), 12, 0),
            shift_with_punches(2, 8.0, day(1), 7, 0),
        ]);

        assert_ot(&card, &[0.0, 1.0]);
    }

    #[test]
    fn ot_gets_assigned_to_the_correct_shifts_when_you_have_ot_on_the_same_day_and_on_the_next() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[1.0, 1.0]);
    }

    #[test]
    fn ot_gets_assigned_correctly_with_multiple_workday_overtime_shifts_on_the_next_day() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 4.0, day(1), 2, 0),
            shift_with_punches(3, 4.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 4.0, 1.0]);
    }

    #[test]
    fn ot_gets_assigned_correctly_with_daily_ot_from_the_previous_day_and_the_current_day() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 9.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 2.0]);
    }

    #[test]
    fn ot_assigned_to_beginning_of_shift_in_the_24_hour_work_day_with_a_break_in_the_first_shift() {
        let card = run(vec![
            with_break(
                shift_with_punches(1, 9.0, day(0), 10, 0),
                at(day(0), 12, 0),
                at(day(0), 13, 0),
            ),
            shift_with_punches(2, 8.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 1.0]);
    }

    #[test]
    fn ot_not_assigned_when_the_next_shift_falls_outside_the_24hr_workday_with_a_break() {
        let card = run(vec![
            with_break(
                shift_with_punches(1, 9.0, day(0), 10, 0),
                at(day(0), 12, 0),
                at(day(0), 13, 0),
            ),
            shift_with_punches(2, 7.0, day(1), 10, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn ot_not_assigned_when_a_24_hour_period_exceeds_8_hours_but_does_not_start_on_the_workday() {
        let card = run(vec![
            with_break(
                shift_with_punches(1, 9.0, day(0), 10, 0),
                at(day(0), 12, 0),
                at(day(0), 13, 0),
            ),
            with_break(
                shift_with_punches(2, 8.75, day(1), 10, 0),
                at(day(1), 12, 0),
                at(day(1), 12, 45),
            ),
        ]);

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn ot_should_only_be_assigned_to_shifts_in_the_work_week() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(7), 10, 0),
            shift_with_punches(2, 9.0, day(8), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn ot_only_when_hours_plus_adjustments_is_greater_than_the_24_hour_limit() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0).with_adj_hours(-1.0),
            shift_with_punches(2, 8.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0]);
    }

    #[test]
    fn ot_only_when_hours_plus_adjustments_at_the_end_of_shifts_is_greater_than_the_limit() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 9, 0).with_adj_hours(1.0),
        ]);

        assert_ot(&card, &[0.0, 2.0]);
    }

    #[test]
    fn overtime_should_not_be_double_dipped_over_two_sets_of_daily_overtime() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 1, 0),
            shift_with_punches(3, 8.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 8.0, 8.0]);
    }

    #[test]
    fn not_double_dipped_when_a_shift_wholly_exceeds_both_work_day_and_work_week_limits() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(3), 18, 0),
            shift_with_punches(6, 8.0, day(4), 2, 0),
        ]);

        assert_ot(&card, &[1.0, 0.0, 0.0, 0.0, 8.0, 8.0]);
    }

    #[test]
    fn overtime_when_a_shift_is_only_partially_in_overtime_by_both_limits() {
        let card = run(vec![
            shift_with_punches(1, 8.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 4.0, day(3), 10, 0),
            shift_with_punches(5, 4.0, day(4), 18, 0),
            shift_with_punches(6, 4.0, day(5), 8, 0),
            shift_with_punches(7, 8.0, day(5), 16, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 6.0]);
    }

    #[test]
    fn overtime_fully_applied_when_daily_and_weekly_together_cover_the_whole_shift() {
        let card = run(vec![
            shift_with_punches(1, 7.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(4), 10, 0),
            shift_with_punches(6, 8.0, day(5), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0, 0.0, 8.0]);
    }

    #[test]
    fn adjustment_only_shifts_should_not_affect_daily_overtime_for_the_current_day() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_adjustment_only(2, 8.0, day(0)),
            shift_with_punches(3, 8.0, day(1), 9, 0),
        ]);

        assert_ot(&card, &[1.0, 0.0, 1.0]);
    }

    #[test]
    fn adjustment_only_shifts_should_not_affect_daily_overtime_for_the_next_day() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 9, 0),
            shift_adjustment_only(3, 8.0, day(1)),
        ]);

        assert_ot(&card, &[1.0, 1.0, 0.0]);
    }

    #[test]
    fn adjustment_only_shifts_contribute_to_weekly_overtime() {
        let card = run(vec![
            shift_with_punches(1, 7.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_adjustment_only(5, 8.0, day(4)),
            shift_with_punches(6, 8.0, day(5), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0, 0.0, 7.0]);
    }

    #[test]
    fn adjustment_only_shifts_accumulate_weekly_overtime() {
        let card = run(vec![
            shift_with_punches(1, 7.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(4), 10, 0),
            shift_adjustment_only(6, 8.0, day(5)),
            shift_with_punches(7, 8.0, day(6), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0, 0.0, 7.0, 8.0]);
    }

    #[test]
    fn adjustment_only_shifts_accumulate_weekly_ot_before_punched_shifts_on_the_same_day() {
        // They sort to midnight, ahead of anything with a start time.
        let card = run(vec![
            shift_with_punches(1, 7.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(4), 10, 0),
            shift_adjustment_only(6, 8.0, day(5)),
            shift_with_punches(7, 8.0, day(5), 9, 0),
        ]);

        assert_ot(&card, &[0.0, 0.0, 0.0, 0.0, 0.0, 7.0, 8.0]);
    }

    #[test]
    fn not_double_dipped_when_an_earlier_shift_received_daily_ot_that_also_caused_weekly_ot() {
        let card = run(vec![
            shift_with_punches(1, 9.0, day(0), 10, 0),
            shift_with_punches(2, 8.0, day(1), 10, 0),
            shift_with_punches(3, 8.0, day(2), 10, 0),
            shift_with_punches(4, 8.0, day(3), 10, 0),
            shift_with_punches(5, 8.0, day(4), 10, 0),
        ]);

        assert_ot(&card, &[1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn split_by_day_correctly_backfills_shift_ot_to_the_second_day_first() {
        let card = run(vec![
            shift_split_by_day(1, 8.0, day(0), 20, 0),
            shift_split_by_day(2, 9.0, day(1), 19, 0),
        ]);

        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);
        assert_eq!(rows(&card, 0, REGULAR), vec![(day(0), 4.0), (day(1), 4.0)]);

        assert_eq!(card.shifts()[1].hours_distributions().len(), 3);
        assert_eq!(rows(&card, 1, REGULAR), vec![(day(1), 5.0), (day(2), 2.0)]);
        assert_eq!(rows(&card, 1, OVERTIME), vec![(day(2), 2.0)]);
    }

    #[test]
    fn weekly_ot_is_accounted_for_when_shifts_are_over_the_24_hour_threshold() {
        let card = run(vec![
            shift_split_by_day(1, 8.0, day(0), 23, 0),
            shift_split_by_day(2, 10.0, day(1), 23, 0),
            shift_split_by_day(3, 8.0, day(2), 23, 0),
            shift_split_by_day(4, 9.75, day(3), 23, 0),
            shift_split_by_day(5, 10.25, day(4), 23, 0),
            shift_split_by_day(6, 11.25, day(5), 21, 45),
            shift_split_by_day(7, 10.25, day(6), 21, 0),
        ]);

        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);
        assert_eq!(rows(&card, 0, REGULAR), vec![(day(0), 1.0), (day(1), 7.0)]);

        assert_eq!(card.shifts()[1].hours_distributions().len(), 3);
        assert_eq!(rows(&card, 1, REGULAR), vec![(day(1), 1.0), (day(2), 7.0)]);
        assert_eq!(rows(&card, 1, OVERTIME), vec![(day(2), 2.0)]);

        assert_eq!(card.shifts()[2].hours_distributions().len(), 2);
        assert_eq!(rows(&card, 2, REGULAR), vec![(day(2), 1.0), (day(3), 7.0)]);

        assert_eq!(card.shifts()[3].hours_distributions().len(), 3);
        assert_eq!(rows(&card, 3, REGULAR), vec![(day(3), 1.0), (day(4), 7.0)]);
        assert_eq!(rows(&card, 3, OVERTIME), vec![(day(4), 1.75)]);

        assert_eq!(card.shifts()[4].hours_distributions().len(), 3);
        assert_eq!(rows(&card, 4, REGULAR), vec![(day(4), 1.0), (day(5), 7.0)]);
        assert_eq!(rows(&card, 4, OVERTIME), vec![(day(5), 2.25)]);

        assert_eq!(card.shifts()[5].hours_distributions().len(), 4);
        assert_eq!(rows(&card, 5, REGULAR), vec![(day(5), 0.0), (day(6), 0.0)]);
        assert_eq!(
            rows(&card, 5, OVERTIME),
            vec![(day(5), 2.25), (day(6), 9.0)]
        );

        assert_eq!(card.shifts()[6].hours_distributions().len(), 4);
        assert_eq!(rows(&card, 6, REGULAR), vec![(day(6), 0.0), (day(7), 0.0)]);
        assert_eq!(
            rows(&card, 6, OVERTIME),
            vec![(day(6), 3.0), (day(7), 7.25)]
        );
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        let mut card = card(vec![shift_with_punches(1, 9.0, day(0), 8, 0)])
            .with_hours_distribution_types(vec![HoursDistributionType::new(
                REGULAR, "Regular", false,
            )]);

        TwentyFourHourOTRule.execute(&mut card, &week(), &item(&[]));

        assert_eq!(ot_hours(&card, 0), 0.0);
    }
}
