//! Port of `ScheduledShiftOTRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/ScheduledShiftOTRuleImpl.java`.
//!
//! `SCHEDULED_SHIFT_OT_HDR`. Overtime for working longer than you were
//! scheduled to: match each actual shift to the schedule that starts nearest
//! it, and pay the difference in net hours as overtime — regardless of any
//! daily or weekly limit.
//!
//! The only rule in the family so far that reads
//! [`schedules`](crate::entity::time_card::TimeCard::schedules), and the only
//! one that measures a shift against something other than a number.
//!
//! # It does nothing when a schedule is being built
//!
//! ```java
//! if (timeCard instanceof ScheduleCalcDataSet) {
//!    return;
//! }
//! ```
//!
//! An `instanceof` against the time card's *class*, which is the first time the
//! two implementations have had to be told apart — see divergence 41 for what
//! that becomes here and why it is not a free translation.
//!
//! # Four gates before a shift is even considered
//!
//! `hasBothTimes()`, no persisted errors, open for editing, and net hours at or
//! above `minShiftLength`. Then a schedule must be found within `threshold`
//! minutes of the shift's start, **inclusive at both ends** — the default
//! threshold is `0`, so out of the box only a schedule starting at the same
//! instant matches.
//!
//! Ties are broken by absolute distance from the shift's start, and Java's
//! `sorted().findFirst()` is stable, so two schedules equidistant either side
//! resolve to whichever the card lists first.
//!
//! # Overtime is charged to the **latest** day first
//!
//! `createDistributions` walks the shift's regular distributions in reverse
//! date order, taking `min(distribution.getHours(), remainingOT)` from each. An
//! overnight shift therefore has the overtime taken out of the day it ended on,
//! which is the "backfill" the Java spec names.
//!
//! Note it reads `getHours()`, not `getOriginalHours()` as the weekly rules do,
//! and the deduction is applied afterwards in a second pass over the pairs it
//! built. Nothing in between changes those rows, so the two-pass shape is
//! reproduced as one loop.
//!
//! # Buckets by constant, not by name
//!
//! `HoursDistribution.isRegularType` compares against `REGULAR_ID`, and the
//! premium rows are created with `OVERTIME_ID`. Like
//! [`HolidayDTHrs`](super::holiday_dt_hrs) and unlike the two weekly rules,
//! this one never consults the property's configured buckets.

use crate::common::numbers::round_hours;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution_type::HoursDistributionType;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    MIN_SHIFT_LENGTH_PROP, ScheduledShiftOTRuleConfig, THRESHOLD_PROP,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDateTime;

/// Overtime for working past the matching schedule. `ScheduledShiftOTRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScheduledShiftOTRule;

impl HoursDistributionRule for ScheduledShiftOTRule {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        if time_card.is_run_from_scheduling() {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&ScheduledShiftOTRuleConfig.default_values());
        let min_shift_length = params.double_at(MIN_SHIFT_LENGTH_PROP);
        let threshold = params.int_at(THRESHOLD_PROP);

        // Every selection decided up front, so the writes below can take the
        // card mutably (divergence 22).
        let work: Vec<(usize, Vec<(usize, f64)>)> = time_card
            .shift_indices_for_period(work_week)
            .into_iter()
            .filter(|&index| {
                shift_meets_rule_requirements(
                    time_card,
                    &time_card.shifts()[index],
                    min_shift_length,
                )
            })
            .filter_map(|index| {
                let schedule = matching_schedule_for_shift(time_card, index, threshold)?;
                let overtime = ot_distributions_for_shift(time_card, index, schedule);
                (!overtime.is_empty()).then_some((index, overtime))
            })
            .collect();

        for (shift_index, overtime) in work {
            let shift = &mut time_card.shifts_mut()[shift_index];
            for (distribution_index, ot_hours) in overtime {
                let premium = create_premium_distribution(
                    &shift.hours_distributions()[distribution_index],
                    HoursDistributionType::OVERTIME_ID,
                    ot_hours,
                    Some(rule_item.id()),
                );
                // `deductPremiumHoursFromRegularDistributionOnDistributionDate`.
                let reduced =
                    round_hours(shift.hours_distributions()[distribution_index].hours() - ot_hours)
                        .max(0.0);
                shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
                shift.add_hours_distribution(premium);
            }
        }
    }
}

/// `shiftMeetsRuleRequirements` — the three-predicate `and` chain.
fn shift_meets_rule_requirements(
    time_card: &dyn TimeCard,
    shift: &EmployeeShift,
    min_shift_length: f64,
) -> bool {
    shift.has_both_times()
        && !shift.has_errors()
        && time_card.is_open_for_editing_for_shift(shift)
        && shift.net_hours() >= min_shift_length
}

/// `matchingScheduleForShift` — the schedule starting nearest the shift, within
/// `threshold` minutes either side.
///
/// Returns the schedule's position in [`TimeCard::schedules`].
fn matching_schedule_for_shift(
    time_card: &dyn TimeCard,
    shift_index: usize,
    threshold: i32,
) -> Option<usize> {
    let shift_start = time_card.shifts()[shift_index].start_date_time()?;
    let range_start = shift_start.minus_minutes(i64::from(threshold));
    let range_end = shift_start.plus_minutes(i64::from(threshold));

    time_card
        .schedules()
        .iter()
        .enumerate()
        .filter_map(|(index, schedule)| Some((index, schedule.start_date_time()?)))
        // `containsDateTimeInclusiveOfEndDateTime` — closed at both ends.
        .filter(|&(_, start)| start >= range_start && start <= range_end)
        .min_by_key(|&(_, start)| seconds_between(shift_start, start).abs())
        .map(|(index, _)| index)
}

/// `TimeUtil.durationInSeconds(start, end)`.
fn seconds_between(start: LocalDateTime, end: LocalDateTime) -> i64 {
    end.epoch_seconds() - start.epoch_seconds()
}

/// `createOTDistributionsForShift` and `createDistributions`, as
/// `(distribution index, overtime hours)` pairs against the shift.
fn ot_distributions_for_shift(
    time_card: &dyn TimeCard,
    shift_index: usize,
    schedule_index: usize,
) -> Vec<(usize, f64)> {
    let shift = &time_card.shifts()[shift_index];
    let total_ot =
        round_hours(shift.net_hours() - time_card.schedules()[schedule_index].net_hours()).max(0.0);

    if total_ot == 0.0 {
        return Vec::new();
    }

    // `createReverseSortedHoursDistributionList` — regular rows, latest first.
    let mut candidates: Vec<usize> = shift
        .hours_distributions()
        .iter()
        .enumerate()
        .filter(|(_, distribution)| distribution.is_of_type(HoursDistributionType::REGULAR_ID))
        .map(|(index, _)| index)
        .collect();
    candidates.sort_by(|&a, &b| {
        shift.hours_distributions()[b]
            .date()
            .cmp(&shift.hours_distributions()[a].date())
    });

    let mut remaining = total_ot;
    let mut created = Vec::new();

    for index in candidates {
        let distribution = &shift.hours_distributions()[index];
        if !time_card.is_open_for_editing_on(distribution.date()) {
            continue;
        }

        let to_distribute = distribution.hours().min(remaining);
        if to_distribute > 0.0 {
            created.push((index, to_distribute));
            remaining -= to_distribute;
        }
    }

    created
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const REGULAR: i32 = HoursDistributionType::REGULAR_ID;
    const OVERTIME: i32 = HoursDistributionType::OVERTIME_ID;

    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn at(day_offset: i64, hour: u32) -> LocalDateTime {
        today()
            .plus_days(day_offset)
            .at_start_of_day()
            .plus_hours(i64::from(hour))
    }

    fn week() -> DateRange {
        DateRange::new(today(), today())
    }

    fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    fn shift(
        id: i32,
        start: LocalDateTime,
        end: LocalDateTime,
        net_hours: f64,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, 1, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(start), Some(end))
            .with_net_hours(net_hours)
            .with_hours_distributions(distributions)
    }

    fn card(shifts: Vec<EmployeeShift>, schedules: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_schedules(schedules)
            .with_calculation_start_date(today())
    }

    fn rule_item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = crate::rules::params::RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::ScheduledShiftOtHdr, rule_params)
    }

    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.date(), d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn working_past_the_matching_schedule_pays_the_difference() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 10),
                10.0,
                vec![regular(today(), 10.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 8.0),
                (today(), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn a_schedule_of_the_same_length_pays_nothing() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 8),
                8.0,
                vec![regular(today(), 8.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn a_shorter_shift_than_its_schedule_pays_nothing() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 8),
                8.0,
                vec![regular(today(), 8.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 12), 12.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn the_default_threshold_matches_only_an_exact_start() {
        let one_minute_late = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 10),
                10.0,
                vec![regular(today(), 10.0)],
            )],
            vec![shift(
                2,
                at(0, 0).plus_minutes(1),
                at(0, 8),
                8.0,
                Vec::new(),
            )],
        );
        let mut one_minute_late = one_minute_late;

        ScheduledShiftOTRule.execute(&mut one_minute_late, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&one_minute_late, 0),
            vec![(today(), Some(REGULAR), 10.0)]
        );
    }

    #[test]
    fn the_threshold_window_is_inclusive_at_both_ends() {
        for offset in [-60, 60] {
            let mut card = card(
                vec![shift(
                    1,
                    at(0, 4),
                    at(0, 14),
                    10.0,
                    vec![regular(today(), 10.0)],
                )],
                vec![shift(
                    2,
                    at(0, 4).plus_minutes(offset),
                    at(0, 12),
                    8.0,
                    Vec::new(),
                )],
            );

            ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[(THRESHOLD_PROP, "60")]));

            assert_eq!(
                rows(&card, 0),
                vec![
                    (today(), Some(REGULAR), 8.0),
                    (today(), Some(OVERTIME), 2.0)
                ],
                "a schedule exactly on the window edge still matches"
            );
        }
    }

    #[test]
    fn the_nearest_schedule_wins() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 4),
                at(0, 14),
                10.0,
                vec![regular(today(), 10.0)],
            )],
            vec![
                shift(2, at(0, 3), at(0, 9), 6.0, Vec::new()),
                shift(3, at(0, 4).plus_minutes(30), at(0, 12), 8.0, Vec::new()),
            ],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[(THRESHOLD_PROP, "120")]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 8.0),
                (today(), Some(OVERTIME), 2.0)
            ],
            "the 04:30 schedule is nearer than the 03:00 one"
        );
    }

    #[test]
    fn equidistant_schedules_resolve_to_the_first_listed() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 4),
                at(0, 14),
                10.0,
                vec![regular(today(), 10.0)],
            )],
            vec![
                shift(2, at(0, 3), at(0, 9), 6.0, Vec::new()),
                shift(3, at(0, 5), at(0, 13), 8.0, Vec::new()),
            ],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[(THRESHOLD_PROP, "120")]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 6.0),
                (today(), Some(OVERTIME), 4.0)
            ],
            "the six-hour schedule listed first"
        );
    }

    #[test]
    fn a_shift_without_both_times_is_skipped() {
        let mut card = card(
            vec![
                EmployeeShift::new(1, 1, 1, today(), ShiftType::Actual, Vec::new())
                    .with_net_hours(10.0)
                    .with_hours_distributions(vec![regular(today(), 10.0)]),
            ],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
    }

    #[test]
    fn a_shift_in_error_is_skipped() {
        let mut card = card(
            vec![
                shift(1, at(0, 0), at(0, 10), 10.0, vec![regular(today(), 10.0)])
                    .with_errors(vec![ShiftErrorType::MissingOut]),
            ],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
    }

    #[test]
    fn a_shift_below_the_minimum_length_is_skipped() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 5),
                5.0,
                vec![regular(today(), 5.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 2), 2.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(today(), Some(REGULAR), 5.0)],
            "five hours is under the default minimum of eight"
        );
    }

    #[test]
    fn the_minimum_length_test_is_inclusive() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 8),
                8.0,
                vec![regular(today(), 8.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 6), 6.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 6.0),
                (today(), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn nothing_runs_when_a_schedule_is_being_built() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 10),
                10.0,
                vec![regular(today(), 10.0)],
            )],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        )
        .with_calculation_mode(EmployeeCalculationMode::EditSchedule);

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
    }

    #[test]
    fn overtime_is_taken_out_of_the_latest_day_first() {
        // An overnight shift: two hours on the first day, eight on the second.
        let mut card = card(
            vec![shift(
                1,
                at(0, 22),
                at(1, 8),
                10.0,
                vec![regular(today(), 2.0), regular(today().plus_days(1), 8.0)],
            )],
            vec![shift(2, at(0, 22), at(1, 6), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 2.0),
                (today().plus_days(1), Some(REGULAR), 6.0),
                (today().plus_days(1), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn overtime_spills_back_onto_the_earlier_day() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 16),
                at(1, 2),
                10.0,
                vec![regular(today(), 8.0), regular(today().plus_days(1), 2.0)],
            )],
            vec![shift(2, at(0, 20), at(1, 2), 6.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[(THRESHOLD_PROP, "240")]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 6.0),
                (today().plus_days(1), Some(REGULAR), 0.0),
                (today().plus_days(1), Some(OVERTIME), 2.0),
                (today(), Some(OVERTIME), 2.0)
            ],
            "four hours over: two from the second day, then two from the first"
        );
    }

    #[test]
    fn a_distribution_in_a_closed_period_is_skipped_without_spending_the_budget() {
        // Unlike WeeklyOTSecJobHrs, the `continue` here happens before the
        // budget is touched, so the overtime lands wholly on the open day.
        //
        // The shift is dated the second day so it passes the shift-level
        // `isOpenForEditingFor` gate, which tests the shift date; only its
        // first day's distribution is closed.
        let overnight =
            EmployeeShift::new(1, 1, 1, today().plus_days(1), ShiftType::Actual, Vec::new())
                .with_times(Some(at(0, 22)), Some(at(1, 8)))
                .with_net_hours(10.0)
                .with_hours_distributions(vec![
                    regular(today(), 2.0),
                    regular(today().plus_days(1), 8.0),
                ]);

        let mut card = card(
            vec![overnight],
            vec![shift(2, at(0, 22), at(1, 6), 6.0, Vec::new())],
        )
        .with_calculation_start_date(today().plus_days(1));

        ScheduledShiftOTRule.execute(
            &mut card,
            &DateRange::new(today(), today().plus_days(1)),
            &rule_item(&[]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 2.0),
                (today().plus_days(1), Some(REGULAR), 4.0),
                (today().plus_days(1), Some(OVERTIME), 4.0)
            ],
            "the closed first day contributes nothing and keeps its hours"
        );
    }

    #[test]
    fn a_premium_row_is_not_a_candidate() {
        let mut card = card(
            vec![shift(
                1,
                at(0, 0),
                at(0, 10),
                10.0,
                vec![
                    regular(today(), 6.0),
                    HoursDistribution::new(1, today(), Some(OVERTIME), 4.0, 0.0),
                ],
            )],
            vec![shift(2, at(0, 0), at(0, 8), 8.0, Vec::new())],
        );

        ScheduledShiftOTRule.execute(&mut card, &week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 4.0),
                (today(), Some(OVERTIME), 4.0),
                (today(), Some(OVERTIME), 2.0)
            ],
            "only the regular row absorbs, and the existing overtime row is untouched"
        );
    }
}

/// `ScheduledShiftOTRuleImplTest.groovy`, transcribed.
///
/// All ten cases. `LocalDate.now()` pinned, and the mocked `PayGroup`'s current
/// pay period becomes the card's calculation start date (divergence 24).
///
/// **Two cases needed a shift date added.** `shifts without both times` and
/// `shifts that is in error` build their shift with no `shiftDate` at all, so
/// `getShiftsForPeriod` — which calls `period.containsDate(null)` — drops them
/// before either gate is reached. `shiftDate` is not nullable here, so each is
/// given a date inside the period, which makes the case actually exercise the
/// gate it is named for. The assertion is unchanged.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalTime};

    const REGULAR: i32 = HoursDistributionType::REGULAR_ID;
    const OVERTIME: i32 = HoursDistributionType::OVERTIME_ID;

    /// `static today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn day(offset: i64) -> LocalDate {
        today().plus_days(offset)
    }

    /// `today.toLocalDateTime(MIDNIGHT).plusHours(n)` and friends.
    fn at(day_offset: i64, hour: i32, minute: i32) -> LocalDateTime {
        day(day_offset).at_time(LocalTime::of(hour, minute, 0))
    }

    /// `new LegacyDatePeriod(today)` — a single day.
    fn work_week() -> DateRange {
        DateRange::new(today(), today())
    }

    fn distribution(date: LocalDate, type_id: i32, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(type_id), hours, 0.0)
    }

    fn shift(
        id: i32,
        shift_date: LocalDate,
        start: Option<LocalDateTime>,
        end: Option<LocalDateTime>,
        net_hours: f64,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_times(start, end)
            .with_net_hours(net_hours)
            .with_hours_distributions(distributions)
    }

    /// `currentPayPeriod() >> ArbitraryDateRange.of(today, today.plusWeeks(1))`,
    /// so the calculation opens on `today`.
    fn card(shifts: Vec<EmployeeShift>, schedules: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_schedules(schedules)
            .with_calculation_start_date(today())
    }

    /// `new RuleItem()` — no params, so `fixMap` supplies both defaults.
    fn rule_item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(0, 1, "", RuleClass::ScheduledShiftOtHdr, rule_params)
    }

    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.date(), d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn shifts_without_both_times_do_not_get_ot() {
        let mut card = card(
            vec![shift(0, today(), None, None, 0.0, Vec::new())],
            Vec::new(),
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert!(card.shifts()[0].hours_distributions().is_empty());
    }

    #[test]
    fn shifts_that_is_in_error_does_not_get_ot() {
        let mut card = card(
            vec![
                shift(
                    0,
                    today(),
                    Some(at(0, 0, 0)),
                    Some(at(0, 8, 0)),
                    0.0,
                    Vec::new(),
                )
                .with_errors(vec![ShiftErrorType::MissingOut]),
            ],
            Vec::new(),
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert!(card.shifts()[0].hours_distributions().is_empty());
    }

    #[test]
    fn shifts_that_are_not_open_for_edit_do_not_get_ot() {
        let mut card = card(
            vec![shift(
                0,
                day(-1),
                Some(at(-1, 0, 0)),
                Some(at(-1, 8, 0)),
                0.0,
                Vec::new(),
            )],
            Vec::new(),
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert!(card.shifts()[0].hours_distributions().is_empty());
    }

    #[test]
    fn shifts_that_have_less_hours_than_the_min_shift_length_do_not_get_ot() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 5, 0)),
                5.0,
                vec![distribution(today(), REGULAR, 5.0)],
            )],
            Vec::new(),
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 5.0)]);
    }

    #[test]
    fn shifts_with_no_matching_schedules_do_not_get_ot() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 8, 0)),
                8.0,
                vec![distribution(today(), REGULAR, 8.0)],
            )],
            Vec::new(),
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn shifts_with_matching_schedules_of_the_same_length_do_not_get_ot() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 8, 0)),
                8.0,
                vec![distribution(today(), REGULAR, 8.0)],
            )],
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 8, 0)),
                8.0,
                Vec::new(),
            )],
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn shifts_get_ot_distributed_for_the_difference_between_the_shift_and_the_schedule() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 10, 0)),
                10.0,
                vec![distribution(today(), REGULAR, 10.0)],
            )],
            vec![shift(
                0,
                today(),
                Some(at(0, 0, 0)),
                Some(at(0, 8, 0)),
                8.0,
                Vec::new(),
            )],
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 8.0),
                (today(), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn shifts_that_cross_days_get_ot_backfill_distributed() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 22, 0)),
                Some(at(1, 8, 0)),
                10.0,
                vec![
                    distribution(today(), REGULAR, 2.0),
                    distribution(day(1), REGULAR, 8.0),
                ],
            )],
            vec![shift(
                0,
                today(),
                Some(at(0, 22, 0)),
                Some(at(1, 6, 0)),
                8.0,
                Vec::new(),
            )],
        );

        ScheduledShiftOTRule.execute(&mut card, &work_week(), &rule_item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 2.0),
                (day(1), Some(REGULAR), 6.0),
                (day(1), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn shifts_distribute_ot_to_distributions_with_as_many_hours_as_worked_that_day() {
        let mut card = card(
            vec![shift(
                0,
                today(),
                Some(at(0, 16, 0)),
                Some(at(1, 2, 0)),
                10.0,
                vec![
                    distribution(today(), REGULAR, 8.0),
                    distribution(day(1), REGULAR, 2.0),
                ],
            )],
            vec![shift(
                0,
                today(),
                Some(at(0, 20, 0)),
                Some(at(1, 2, 0)),
                6.0,
                Vec::new(),
            )],
        );

        ScheduledShiftOTRule.execute(
            &mut card,
            &work_week(),
            &rule_item(&[(THRESHOLD_PROP, "240")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 6.0),
                (day(1), Some(REGULAR), 0.0),
                (day(1), Some(OVERTIME), 2.0),
                (today(), Some(OVERTIME), 2.0)
            ]
        );
    }

    #[test]
    fn the_matching_schedule_is_the_one_closest_to_the_shift() {
        let mut card = card(
            vec![shift(
                1,
                today(),
                Some(at(0, 1, 0)),
                Some(at(0, 9, 0)),
                8.0,
                vec![distribution(today(), REGULAR, 8.0)],
            )],
            vec![
                shift(
                    2,
                    today(),
                    Some(at(0, 1, 0)),
                    Some(at(0, 3, 0)),
                    2.0,
                    Vec::new(),
                ),
                shift(
                    3,
                    today(),
                    Some(at(0, 4, 0)),
                    Some(at(0, 8, 0)),
                    4.0,
                    Vec::new(),
                ),
            ],
        );

        ScheduledShiftOTRule.execute(
            &mut card,
            &work_week(),
            &rule_item(&[(THRESHOLD_PROP, "240")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 2.0),
                (today(), Some(OVERTIME), 6.0)
            ],
            "the two-hour schedule starts at the same instant as the shift"
        );
    }
}
