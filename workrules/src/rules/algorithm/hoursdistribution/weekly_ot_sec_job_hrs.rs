//! Port of `WeeklyOTSecJobHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/WeeklyOTSecJobHrsRuleImpl.java`.
//!
//! `WEEKLY_OT_SEC_JOB_HDR`. Weekly overtime for an employee who works more than
//! one job, with a deliberate opinion about *which* hours become overtime: the
//! ones worked at a **secondary** job, latest first.
//!
//! The week's total is measured across every non-exempt shift, the excess over
//! `weeklyOtLimit` is the overtime owed, and that amount is then taken out of
//! the regular distributions in this order:
//!
//! 1. secondary-job shifts, latest shift first;
//! 2. home-job shifts, latest shift first;
//! 3. within a shift, its distributions latest **date** first.
//!
//! So a second job absorbs the overtime before the home job does, and the
//! employee's home-job pay stays whole. That ordering is the whole point of the
//! rule; `WeeklyOTHrsRuleImpl` is the same idea without it.
//!
//! # `totalOT` is a budget, spent down
//!
//! Java holds it in a field across the whole `execute` and each distribution
//! takes `min(originalHours, totalOT)` from it. Once it reaches zero the
//! remaining distributions are visited and change nothing — `distributeOT` is
//! still called for each, and still recomputes `totalOT` as
//! `roundHours(totalOT - 0)`. Reproduced as a local that the loop threads
//! through.
//!
//! # It measures `originalHours` and writes `hours`, and **is not idempotent**
//!
//! The week's total sums `getOriginalHours()`, and so does each distribution's
//! share — but what gets reduced is `getHours()`:
//!
//! ```java
//! double distributionOT = Math.min(distribution.getOriginalHours(), totalOT);
//! ...
//! distribution.setHours(roundHours(distribution.getHours() - distributionOT));
//! ```
//!
//! Premium rows carry zero original hours (see
//! [`hours_distribution_factory`](crate::rules::algorithm::utility::hours_distribution_factory)),
//! so a second `execute` over an already-calculated week measures the **same**
//! week total and computes the same overtime — and then subtracts it from
//! `hours` that were already reduced. Eight regular hours cut to two become
//! **minus four**, and a second premium row is added beside the first.
//!
//! So this rule is safe to run once and only once per week. Unlike the
//! punch-rounding family, which resets every punch before rounding and is
//! idempotent by construction, nothing here restores `hours` from
//! `originalHours` first. Pinned by
//! `a_second_pass_drives_the_regular_hours_negative`, so a future "tidy-up"
//! that makes the rule idempotent is a visible behaviour change rather than a
//! silent one.
//!
//! # Buckets by name here, unlike `HolidayDTHrs`
//!
//! This rule selects with `timeCard.distributionIsRegular()` — the
//! non-premium bucket list — and creates with
//! `timeCard.getOTHoursDistributionTypeId()`, the lookup on the literal name
//! `"Overtime"`. `HolidayDTHrsRuleImpl` in the same family uses the `REGULAR_ID`
//! and `DT_ID` constants instead. Both mechanisms are ported as written.
//!
//! Java's `getOTHoursDistributionTypeId()` unboxes a possibly-null `Integer`;
//! a property with no bucket named `"Overtime"` throws there. Here the rule
//! returns without paying anything, which is the only other reading — there is
//! no bucket to put the hours in.

use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    WEEKLY_LIMIT_PROP, WeeklyOTSecJobHrsRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// Weekly overtime taken out of secondary-job hours first.
/// `WeeklyOTSecJobHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyOTSecJobHrsRule;

impl HoursDistributionRule for WeeklyOTSecJobHrsRule {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&WeeklyOTSecJobHrsRuleConfig.default_values());
        let weekly_limit = params.double_at(WEEKLY_LIMIT_PROP);

        let shifts = non_salaried_exempt_shifts(time_card);

        let mut total_ot =
            round_hours(sum_total_worked_hours(time_card, &shifts, work_week) - weekly_limit);
        if total_ot <= 0.0 {
            return;
        }

        let Some(ot_type_id) = time_card.ot_hours_distribution_type_id() else {
            return;
        };

        for shift_index in ot_priority_order(time_card, &shifts) {
            for distribution_index in
                regular_distributions_latest_first(time_card, shift_index, work_week)
            {
                total_ot = distribute_ot(
                    time_card,
                    shift_index,
                    distribution_index,
                    ot_type_id,
                    rule_item.id(),
                    total_ot,
                );
            }
        }
    }
}

/// `nonSalariedExemptShiftsProducer` over `timeCard.employeeShiftStream()`.
///
/// Java's predicate null-checks the job status explicitly, unlike the two
/// helpers that share the expression; see divergence 32.
fn non_salaried_exempt_shifts(time_card: &dyn TimeCard) -> Vec<usize> {
    time_card
        .shifts()
        .iter()
        .enumerate()
        .filter(|(_, shift)| time_card.shift_is_not_salaried_exempt(shift))
        .map(|(index, _)| index)
        .collect()
}

/// `sumTotalWorkedHours` — the week's regular **original** hours.
fn sum_total_worked_hours(
    time_card: &dyn TimeCard,
    shifts: &[usize],
    work_week: &DateRange,
) -> f64 {
    shifts
        .iter()
        .flat_map(|&index| time_card.shifts()[index].hours_distributions())
        .filter(|distribution| {
            time_card.distribution_is_regular(distribution)
                && distribution.falls_within_period(work_week)
        })
        .map(|distribution| distribution.original_hours())
        .sum()
}

/// `otPriorityShiftStreamCreator` — latest shifts first, secondary jobs before
/// home jobs.
///
/// Java sorts the whole list with `new ShiftStartTimeComparator().reversed()`
/// and then `partitioningBy(homeJobShifts())`, concatenating the `false` half
/// ahead of the `true` half. `partitioningBy` preserves encounter order within
/// each half, so the reversed sort survives the partition — and because
/// `ShiftStartTimeComparator` calls two shifts on the same date with no start
/// time equal, shifts that tie keep the order the time card holds them in.
/// Both properties are reproduced with a stable sort.
fn ot_priority_order(time_card: &dyn TimeCard, shifts: &[usize]) -> Vec<usize> {
    let mut latest_first = shifts.to_vec();
    latest_first.sort_by(|&a, &b| {
        time_card.shifts()[b]
            .start_time_for_ordering()
            .cmp(&time_card.shifts()[a].start_time_for_ordering())
    });

    let is_home_job = |&index: &usize| {
        time_card
            .employee_job_status_for_shift(&time_card.shifts()[index])
            .is_some_and(|status| status.home())
    };

    let (home, secondary): (Vec<usize>, Vec<usize>) =
        latest_first.into_iter().partition(is_home_job);

    secondary.into_iter().chain(home).collect()
}

/// A shift's regular distributions inside the week, latest date first.
///
/// `sorted(comparing(HoursDistribution::getDate).reversed())`.
fn regular_distributions_latest_first(
    time_card: &dyn TimeCard,
    shift_index: usize,
    work_week: &DateRange,
) -> Vec<usize> {
    let shift = &time_card.shifts()[shift_index];
    let mut indices: Vec<usize> = shift
        .hours_distributions()
        .iter()
        .enumerate()
        .filter(|(_, distribution)| {
            distribution.falls_within_period(work_week)
                && time_card.distribution_is_regular(distribution)
        })
        .map(|(index, _)| index)
        .collect();

    indices.sort_by(|&a, &b| {
        shift.hours_distributions()[b]
            .date()
            .cmp(&shift.hours_distributions()[a].date())
    });
    indices
}

/// `distributeOT` — take this distribution's share out of the budget and
/// return what is left.
fn distribute_ot(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    ot_type_id: i32,
    rule_item_id: i32,
    total_ot: f64,
) -> f64 {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let distribution_ot = distribution.original_hours().min(total_ot);
    let date = distribution.date();

    if distribution_ot > 0.0 && time_card.is_open_for_editing_on(date) {
        let shift = &mut time_card.shifts_mut()[shift_index];
        let distribution = &shift.hours_distributions()[distribution_index];

        let premium = create_premium_distribution(
            distribution,
            ot_type_id,
            distribution_ot,
            Some(rule_item_id),
        );
        let reduced = round_hours(distribution.hours() - distribution_ot);

        shift.hours_distributions_mut()[distribution_index].set_hours(reduced);
        shift.add_hours_distribution(premium);
    }

    // Java subtracts unconditionally, even when the write above was skipped.
    round_hours(total_ot - distribution_ot)
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
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const HOME_JOB: i32 = 1;
    const SECONDARY_JOB: i32 = 2;
    const EXEMPT_JOB: i32 = 3;

    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn week() -> DateRange {
        DateRange::new(today(), today().plus_days(7))
    }

    fn employee() -> Employee {
        let start = today().minus_days(10);
        let end = today().plus_days(10);
        let status = |id: i32, job: i32, pay_type: EmployeePayType, home: bool| {
            EmployeeJobStatus::new(id, 1, job, start, end, pay_type, 10.0, home)
        };

        Employee::new(
            1,
            1,
            "",
            vec![
                status(1, HOME_JOB, EmployeePayType::Hourly, true),
                status(2, SECONDARY_JOB, EmployeePayType::Hourly, false),
                status(3, EXEMPT_JOB, EmployeePayType::SalariedExempt, false),
            ],
        )
    }

    fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(
            1,
            date,
            Some(HoursDistributionType::REGULAR_ID),
            hours,
            10.0,
        )
    }

    fn shift(id: i32, job_id: i32, date: LocalDate, hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, job_id, date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![regular(date, hours)])
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
    }

    fn rule_item(limit: &str) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::WeeklyOtSecJobHdr,
            rule_params! { WEEKLY_LIMIT_PROP => limit },
        )
    }

    /// Every distribution on a shift, as `(type id, hours)`.
    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn a_week_under_the_limit_pays_no_overtime() {
        let mut card = card(vec![shift(1, HOME_JOB, today(), 8.0)]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(1), 8.0)]);
    }

    #[test]
    fn a_week_exactly_on_the_limit_pays_no_overtime() {
        let mut card = card(vec![shift(1, HOME_JOB, today(), 8.0)]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("8.0"));

        assert_eq!(rows(&card, 0), vec![(Some(1), 8.0)]);
    }

    #[test]
    fn the_secondary_job_absorbs_the_overtime_first() {
        let mut card = card(vec![
            shift(1, HOME_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today(), 8.0),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("10.0"));

        assert_eq!(
            rows(&card, 0),
            vec![(Some(1), 8.0)],
            "the home job is whole"
        );
        assert_eq!(rows(&card, 1), vec![(Some(1), 2.0), (Some(2), 6.0)]);
    }

    #[test]
    fn overtime_beyond_the_secondary_job_spills_onto_the_home_job() {
        let mut card = card(vec![
            shift(1, HOME_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today(), 4.0),
        ]);

        // 12 hours against a limit of 5 is 7 hours of overtime; the secondary
        // job only has 4 to give.
        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("5.0"));

        assert_eq!(rows(&card, 1), vec![(Some(1), 0.0), (Some(2), 4.0)]);
        assert_eq!(rows(&card, 0), vec![(Some(1), 5.0), (Some(2), 3.0)]);
    }

    #[test]
    fn later_shifts_pay_the_overtime_before_earlier_ones() {
        let mut card = card(vec![
            shift(1, SECONDARY_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today().plus_days(1), 8.0),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("12.0"));

        assert_eq!(rows(&card, 0), vec![(Some(1), 8.0)], "the earlier shift");
        assert_eq!(rows(&card, 1), vec![(Some(1), 4.0), (Some(2), 4.0)]);
    }

    #[test]
    fn a_salaried_exempt_shift_neither_counts_nor_pays() {
        let mut card = card(vec![
            shift(1, EXEMPT_JOB, today(), 40.0),
            shift(2, SECONDARY_JOB, today(), 8.0),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("10.0"));

        assert_eq!(
            rows(&card, 0),
            vec![(Some(1), 40.0)],
            "the exempt shift's forty hours are not in the week's total"
        );
        assert_eq!(rows(&card, 1), vec![(Some(1), 8.0)], "so nothing is over");
    }

    #[test]
    fn a_distribution_in_a_closed_period_is_left_alone_but_still_spends_the_budget() {
        // Java's distributeOT subtracts outside the open-for-editing guard, so
        // a closed distribution consumes overtime that then cannot be paid
        // anywhere else.
        let mut card = card(vec![
            shift(1, SECONDARY_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today().plus_days(1), 8.0),
        ])
        .with_calculation_start_date(today().plus_days(1));

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("10.0"));

        assert_eq!(rows(&card, 1), vec![(Some(1), 2.0), (Some(2), 6.0)]);
        assert_eq!(rows(&card, 0), vec![(Some(1), 8.0)], "closed, so untouched");
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        // Java unboxes a null Integer and throws; there is no bucket to put
        // the hours in either way.
        let mut card = card(vec![shift(1, SECONDARY_JOB, today(), 8.0)])
            .with_hours_distribution_types(vec![HoursDistributionType::new(1, "Regular", false)]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("4.0"));

        assert_eq!(rows(&card, 0), vec![(Some(1), 8.0)]);
    }

    #[test]
    fn a_second_pass_drives_the_regular_hours_negative() {
        // The premium rows carry zero original hours, so the week's total is
        // unchanged by the first pass and the same overtime is computed again
        // — but it is then subtracted from `hours` that were already reduced.
        let mut card = card(vec![
            shift(1, HOME_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today(), 8.0),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("10.0"));

        assert_eq!(rows(&card, 1), vec![(Some(1), 2.0), (Some(2), 6.0)]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("10.0"));

        assert_eq!(
            rows(&card, 1),
            vec![(Some(1), -4.0), (Some(2), 6.0), (Some(2), 6.0)],
            "2 - 6 = -4, and a second premium row beside the first"
        );
    }

    #[test]
    fn distributions_outside_the_work_week_are_not_touched() {
        let mut card = card(vec![
            shift(1, SECONDARY_JOB, today(), 8.0),
            shift(2, SECONDARY_JOB, today().plus_days(30), 8.0),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &week(), &rule_item("4.0"));

        assert_eq!(rows(&card, 0), vec![(Some(1), 4.0), (Some(2), 4.0)]);
        assert_eq!(rows(&card, 1), vec![(Some(1), 8.0)]);
    }
}

/// `WeeklyOTSecJobHrsRuleImplSpec.groovy`, transcribed.
///
/// Both cases. `LocalDate.now()` pinned to a fixed date as elsewhere, and the
/// pay period — which the Groovy mocks as the work week itself — set on the
/// card directly (divergence 24).
///
/// The spec builds its shifts with `employee: employee` so that
/// `shift.getEmployeeJobStatus()` resolves; here that lookup goes through the
/// card, so the shifts only need their job ids.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    /// `new Assignment(id: 1, name: 'homeJob')`.
    const HOME_JOB: i32 = 1;
    /// `new Assignment(id: 2, name: 'secondaryJob')`.
    const SECONDARY_JOB: i32 = 2;
    const REGULAR: i32 = 1;
    const OVERTIME: i32 = 2;

    /// `static def today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    /// `new LegacyDatePeriod(today, today.plusWeeks(1))`.
    fn work_week() -> DateRange {
        DateRange::new(today(), today().plus_days(7))
    }

    /// Both jobs hourly, the first marked home, spanning ten days either side.
    fn employee() -> Employee {
        let start = today().minus_days(10);
        let end = today().plus_days(10);

        Employee::new(
            1,
            1,
            "",
            vec![
                EmployeeJobStatus::new(
                    1,
                    1,
                    HOME_JOB,
                    start,
                    end,
                    EmployeePayType::Hourly,
                    0.0,
                    true,
                ),
                EmployeeJobStatus::new(
                    2,
                    1,
                    SECONDARY_JOB,
                    start,
                    end,
                    EmployeePayType::Hourly,
                    0.0,
                    false,
                ),
            ],
        )
    }

    fn distribution(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    fn shift(
        id: i32,
        job_id: i32,
        shift_date: LocalDate,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, job_id, shift_date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(distributions)
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(today())
    }

    /// `new RuleItem(id: 1, params: [WEEKLY_LIMIT_PROP: "18.0"])`.
    fn rule_item() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::WeeklyOtSecJobHdr,
            rule_params! { WEEKLY_LIMIT_PROP => "18.0" },
        )
    }

    #[test]
    fn execute_redistributes_hours_properly() {
        let mut card = card(vec![
            shift(1, HOME_JOB, today(), vec![distribution(today(), 8.0)]),
            shift(2, SECONDARY_JOB, today(), vec![distribution(today(), 8.0)]),
            shift(
                3,
                HOME_JOB,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 8.0)],
            ),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &work_week(), &rule_item());

        // 24 hours against a limit of 18: six hours of overtime, all taken out
        // of the secondary job.
        assert_eq!(card.shifts()[0].hours_distributions()[0].hours(), 8.0);
        assert_eq!(card.shifts()[1].hours_distributions()[0].hours(), 2.0);
        assert_eq!(card.shifts()[1].hours_distributions()[1].hours(), 6.0);
        assert_eq!(card.shifts()[2].hours_distributions()[0].hours(), 8.0);
    }

    #[test]
    fn hours_distributions_are_created_properly_when_a_shift_is_split_over_days() {
        let tomorrow = today().plus_days(1);
        let mut card = card(vec![
            shift(1, HOME_JOB, today(), vec![distribution(today(), 8.0)]),
            shift(
                2,
                SECONDARY_JOB,
                today(),
                vec![distribution(today(), 4.0), distribution(tomorrow, 4.0)],
            ),
            shift(
                3,
                HOME_JOB,
                tomorrow,
                vec![distribution(today().plus_days(2), 8.0)],
            ),
        ]);

        WeeklyOTSecJobHrsRule.execute(&mut card, &work_week(), &rule_item());

        assert_eq!(card.shifts()[0].hours_distributions()[0].hours(), 8.0);

        // The split shift's later day is reduced first, then its earlier one.
        let split = card.shifts()[1].hours_distributions();
        let row = |date: LocalDate, type_id: i32| {
            split
                .iter()
                .find(|d| d.date() == date && d.hours_distribution_type_id() == Some(type_id))
                .map(|d| d.hours())
        };

        assert_eq!(row(today(), REGULAR), Some(2.0));
        assert_eq!(row(today(), OVERTIME), Some(2.0));
        assert_eq!(row(tomorrow, REGULAR), Some(0.0));
        assert_eq!(row(tomorrow, OVERTIME), Some(4.0));

        assert_eq!(card.shifts()[2].hours_distributions()[0].hours(), 8.0);
    }
}
