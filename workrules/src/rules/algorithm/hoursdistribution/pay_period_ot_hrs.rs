//! Port of `PayPeriodOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/PayPeriodOTHrsRuleImpl.java`.
//!
//! `PAY_PERIOD_OT_HDR`. Overtime measured over the **pay period** rather than
//! the work week: walk the period's shifts in start-time order, accumulate
//! until the limit is passed, and from that shift onward every hour is
//! overtime.
//!
//! It shares almost no machinery with the two weekly rules, and three of the
//! differences will surprise a reader who comes to it from them.
//!
//! # 1. It ignores the work week it is handed
//!
//! `execute` takes `workWeek` and uses it only to locate the period:
//! `employee.getProperty().getPayPeriod().getDateRangeContainingDate(workWeek.getStartDate())`.
//! Everything after that is scoped to the pay period. So the rule recalculates
//! the **whole period** once per week of it, which is why it overwrites rather
//! than accumulates — see below.
//!
//! # 2. It accumulates `shift.getWorkedHours()`, not distribution hours
//!
//! The two weekly rules sum `HoursDistribution.getOriginalHours()`. This one
//! sums the shift's own worked hours, which come from its punches. The two are
//! independent: a shift carrying distributions but no punches has worked hours
//! of **zero** and contributes nothing to the period total, however many hours
//! its distributions claim.
//!
//! That is not hypothetical. Every fixture in `PayPeriodOTHrsRuleImplTest`
//! builds shifts with distributions and no punches, so **no case in the Java
//! spec ever reaches the overtime branch** — see the parity module.
//!
//! # 3. The overtime row is **set**, not added to
//!
//! `updateOTHoursDistribution` assigns `setHours(totalOTHours)` to an existing
//! overtime row and re-stamps its rule item, where `WeeklyOTHrs` adds to it and
//! `WeeklyOTSecJobHrs` appends a second row. Assignment is what makes running
//! this rule repeatedly safe for the overtime row — and it has to be, given
//! point 1.
//!
//! The regular rows are a different story. `convertHoursDistributionToOT`
//! rewrites each from its **`originalHours`**:
//!
//! ```java
//! for (HoursDistribution distribution : shift.getHoursDistributions()) {
//!    if (otHoursToAdd < 0.0 || distribution.getHoursDistributionTypeID().equals(otHoursDistributionTypeId)) {
//!       continue;
//!    }
//!    double originalHours = distribution.getOriginalHours();
//!    distribution.setHours(TDouble.roundHours(Math.max(0.0, originalHours - otHoursToAdd)));
//!    otHoursToAdd -= originalHours;
//! }
//! ```
//!
//! Recomputing from `originalHours` rather than decrementing `hours` is what
//! makes this rule — alone in the family so far — **idempotent**. Pinned by
//! `running_it_twice_changes_nothing`.
//!
//! Two quirks in that loop, both kept:
//!
//! - **The budget goes negative and then short-circuits.** `otHoursToAdd` is
//!   reduced by each row's *whole* `originalHours`, not by what that row
//!   actually absorbed, so a first row larger than the budget drives it below
//!   zero and every later row is skipped — left at whatever `hours` it already
//!   had, rather than being restored from `originalHours`.
//! - **It skips only the overtime bucket.** Any other premium row — double
//!   time, a shift differential — is treated as absorbable and rewritten from
//!   its own original hours, which for a premium row created by
//!   [`hours_distribution_factory`](crate::rules::algorithm::utility::hours_distribution_factory)
//!   is zero. So a double-time row the loop reaches while the budget is still
//!   non-negative is silently zeroed. Whether it is reached depends on where it
//!   sits in the list, since a large regular row ahead of it ends the loop.
//!
//! # A dead collaborator
//!
//! The class `@Resource`-injects an `EmployeeShiftDAO` and never calls it. Not
//! ported; there is nothing to port.

use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    PERIOD_OT_LIMIT_PROP, PayPeriodOTHrsRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// Overtime measured across the pay period. `PayPeriodOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PayPeriodOTHrsRule;

impl HoursDistributionRule for PayPeriodOTHrsRule {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&PayPeriodOTHrsRuleConfig.default_values());
        // Java parses before reading the bucket id, so a malformed limit throws
        // before the unboxing below can. `double_at` panics here (divergence 9),
        // which the Java spec asserts as a `NumberFormatException`.
        let period_ot_limit = params.double_at(PERIOD_OT_LIMIT_PROP);

        let Some(ot_type_id) = time_card.ot_hours_distribution_type_id() else {
            return;
        };

        let pay_period = time_card.pay_period_containing(work_week.start_date());
        let sorted_shifts = sorted_shifts_for_pay_period(time_card, &pay_period);

        let mut accumulated_worked_hours = 0.0;
        let mut is_ot_limit_reached = false;

        for shift_index in sorted_shifts {
            if !time_card.shift_is_not_salaried_exempt(&time_card.shifts()[shift_index]) {
                continue;
            }

            let worked_hours = time_card.shifts()[shift_index].worked_hours();

            if is_ot_limit_reached {
                convert_hours_distribution_to_ot(
                    time_card,
                    shift_index,
                    ot_type_id,
                    rule_item.id(),
                    worked_hours,
                );
            } else {
                accumulated_worked_hours += worked_hours;
                if accumulated_worked_hours > period_ot_limit {
                    is_ot_limit_reached = true;
                    convert_hours_distribution_to_ot(
                        time_card,
                        shift_index,
                        ot_type_id,
                        rule_item.id(),
                        accumulated_worked_hours - period_ot_limit,
                    );
                }
            }
        }
    }
}

/// `initializeSortedShiftsForPayPeriod`.
///
/// Note the salaried-exempt filter is **not** applied here — Java applies it
/// inside the loop, so an exempt shift is skipped without advancing anything.
fn sorted_shifts_for_pay_period(time_card: &dyn TimeCard, pay_period: &DateRange) -> Vec<usize> {
    let mut indices = time_card.shift_indices_with_distributions_for_period(pay_period);
    indices.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());
    indices
}

/// `convertHoursDistributionToOT`.
fn convert_hours_distribution_to_ot(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    ot_type_id: i32,
    rule_item_id: i32,
    ot_hours_to_add: f64,
) {
    let regular_type_ids = time_card.regular_hours_distribution_type_ids();
    let shift = &mut time_card.shifts_mut()[shift_index];

    // `getHoursDistributionFromShift(shift, getRegularHoursDistributionTypeIds())`
    // — the *first* non-premium row, whatever its date.
    let regular_index = shift.hours_distributions().iter().position(|distribution| {
        distribution
            .hours_distribution_type_id()
            .is_some_and(|id| regular_type_ids.contains(&id))
    });

    // `updateOTHoursDistribution` — assign, do not accumulate.
    let existing_ot = shift
        .hours_distributions()
        .iter()
        .position(|distribution| distribution.is_of_type(ot_type_id));

    match existing_ot {
        Some(index) => {
            let distribution = &mut shift.hours_distributions_mut()[index];
            distribution.set_hours(ot_hours_to_add);
            distribution.set_hours_rule_item_id(Some(rule_item_id));
        }
        None => {
            if let Some(index) = regular_index {
                let premium = create_premium_distribution(
                    &shift.hours_distributions()[index],
                    ot_type_id,
                    ot_hours_to_add,
                    Some(rule_item_id),
                );
                shift.add_hours_distribution(premium);
            }
        }
    }

    // Rewrite the absorbing rows from their original hours. The budget is
    // reduced by each row's whole original, so it can go negative and skip the
    // rest — see the module note.
    let mut remaining = ot_hours_to_add;
    for index in 0..shift.hours_distributions().len() {
        if remaining < 0.0 || shift.hours_distributions()[index].is_of_type(ot_type_id) {
            continue;
        }

        let original_hours = shift.hours_distributions()[index].original_hours();
        shift.hours_distributions_mut()[index]
            .set_hours(round_hours((original_hours - remaining).max(0.0)));
        remaining -= original_hours;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const JOB: i32 = 1;
    const EXEMPT_JOB: i32 = 9;
    const REGULAR: i32 = 1;
    const OVERTIME: i32 = 2;
    const DOUBLE_TIME: i32 = 3;

    fn day(offset: i64) -> LocalDate {
        LocalDate::of(2024, 1, 1).plus_days(offset)
    }

    /// The work week, and a two-week pay period containing it.
    fn work_week() -> DateRange {
        DateRange::new(day(0), day(6))
    }

    fn pay_period() -> DateRange {
        DateRange::new(day(0), day(13))
    }

    fn employee() -> Employee {
        let status = |id: i32, job: i32, pay_type: EmployeePayType| {
            EmployeeJobStatus::new(id, 1, job, day(-30), day(30), pay_type, 0.0, true)
        };

        Employee::new(
            1,
            1,
            "",
            vec![
                status(1, JOB, EmployeePayType::Hourly),
                status(2, EXEMPT_JOB, EmployeePayType::SalariedExempt),
            ],
        )
    }

    fn distribution(date: LocalDate, type_id: i32, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(type_id), hours, 0.0)
    }

    /// A shift whose punches give it `worked_hours`, carrying `distributions`.
    ///
    /// The punches matter: `getWorkedHours()` is computed from them, and this
    /// rule accumulates that rather than the distributions' hours.
    fn shift(
        id: i32,
        job_id: i32,
        date: LocalDate,
        worked_hours: f64,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        let punches = if worked_hours > 0.0 {
            let start = date.at_start_of_day();
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, start),
                EmployeeShiftPunch::new(
                    2,
                    PunchType::Out,
                    PunchSource::Clock,
                    start.plus_minutes((worked_hours * 60.0) as i64),
                ),
            ]
        } else {
            Vec::new()
        };

        let mut shift = EmployeeShift::new(id, 1, job_id, date, ShiftType::Actual, punches)
            .with_hours_distributions(distributions);
        shift.calc_worked_hours();
        shift
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_current_pay_period(pay_period())
            .with_calculation_start_date(day(0))
    }

    fn rule_item(limit: &str) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::PayPeriodOtHdr,
            rule_params! { PERIOD_OT_LIMIT_PROP => limit },
        )
    }

    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    /// A shift with punches, so its worked hours are non-zero.
    fn worked(id: i32, date: LocalDate, hours: f64) -> EmployeeShift {
        shift(
            id,
            JOB,
            date,
            hours,
            vec![distribution(date, REGULAR, hours)],
        )
    }

    #[test]
    fn a_shift_with_no_punches_contributes_no_worked_hours() {
        // The distributions claim fifty hours; the shift has no punches, so the
        // period total stays at zero and nothing crosses the limit.
        let mut card = card(vec![shift(
            1,
            JOB,
            day(0),
            0.0,
            vec![distribution(day(0), REGULAR, 50.0)],
        )]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 50.0)]);
    }

    #[test]
    fn nothing_happens_under_the_limit() {
        let mut card = card(vec![worked(1, day(0), 8.0), worked(2, day(1), 8.0)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 8.0)]);
        assert_eq!(rows(&card, 1), vec![(Some(REGULAR), 8.0)]);
    }

    #[test]
    fn the_shift_that_crosses_the_limit_pays_only_the_excess() {
        let mut card = card(vec![worked(1, day(0), 30.0), worked(2, day(1), 20.0)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 30.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 10.0), (Some(OVERTIME), 10.0)],
            "fifty hours over a limit of forty is ten"
        );
    }

    #[test]
    fn every_shift_after_the_limit_is_wholly_overtime() {
        let mut card = card(vec![
            worked(1, day(0), 45.0),
            worked(2, day(1), 8.0),
            worked(3, day(2), 6.0),
        ]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 8.0)]
        );
        assert_eq!(
            rows(&card, 2),
            vec![(Some(REGULAR), 0.0), (Some(OVERTIME), 6.0)]
        );
    }

    #[test]
    fn running_it_twice_changes_nothing() {
        // Alone in the family so far: the regular rows are recomputed from
        // originalHours and the overtime row is assigned, not accumulated.
        let mut card = card(vec![worked(1, day(0), 30.0), worked(2, day(1), 20.0)]);
        let item = rule_item("40.0");

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &item);
        let after_first = rows(&card, 1);
        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &item);

        assert_eq!(rows(&card, 1), after_first);
    }

    #[test]
    fn the_overtime_row_records_the_rule_item_each_time() {
        let mut card = card(vec![worked(1, day(0), 50.0)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        let overtime = &card.shifts()[0].hours_distributions()[1];
        assert_eq!(overtime.hours_rule_item_id(), Some(1));
        assert_eq!(overtime.original_hours(), 0.0);
    }

    #[test]
    fn a_salaried_exempt_shift_is_skipped_without_advancing_the_total() {
        let mut card = card(vec![
            shift(
                1,
                EXEMPT_JOB,
                day(0),
                100.0,
                vec![distribution(day(0), REGULAR, 100.0)],
            ),
            worked(2, day(1), 8.0),
        ]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 100.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 8.0)],
            "the exempt shift's hundred hours never entered the total"
        );
    }

    #[test]
    fn a_shift_outside_the_pay_period_is_not_considered() {
        let mut card = card(vec![worked(1, day(20), 50.0), worked(2, day(1), 8.0)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 50.0)]);
        assert_eq!(rows(&card, 1), vec![(Some(REGULAR), 8.0)]);
    }

    #[test]
    fn the_period_spans_more_than_the_work_week() {
        // The rule is handed the first week and still reaches the second.
        let mut card = card(vec![worked(1, day(1), 30.0), worked(2, day(9), 20.0)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(
            rows(&card, 1),
            vec![(Some(REGULAR), 10.0), (Some(OVERTIME), 10.0)]
        );
    }

    #[test]
    fn the_budget_goes_negative_and_skips_the_remaining_rows() {
        // Two regular rows; the first is bigger than the overtime being taken,
        // so the second is left where it was rather than restored.
        let mut card = card(vec![shift(
            1,
            JOB,
            day(0),
            50.0,
            vec![
                distribution(day(0), REGULAR, 40.0),
                distribution(day(1), REGULAR, 10.0),
            ],
        )]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(
            rows(&card, 0),
            vec![
                (Some(REGULAR), 30.0),
                (Some(REGULAR), 10.0),
                (Some(OVERTIME), 10.0)
            ],
            "40 - 10 = 30, then the budget is 10 - 40 = -30 and the rest is skipped"
        );
    }

    #[test]
    fn a_double_time_row_the_loop_reaches_is_zeroed() {
        // The loop skips only the overtime bucket. A double-time row created by
        // the factory has zero original hours, so it is rewritten to zero.
        let mut double_time = distribution(day(0), DOUBLE_TIME, 4.0);
        double_time.set_original_hours(0.0);

        let mut card = card(vec![shift(
            1,
            JOB,
            day(0),
            50.0,
            vec![double_time, distribution(day(0), REGULAR, 50.0)],
        )]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(
            rows(&card, 0),
            vec![
                (Some(DOUBLE_TIME), 0.0),
                (Some(REGULAR), 40.0),
                (Some(OVERTIME), 10.0)
            ],
            "the double time row absorbs nothing and is rewritten to its zero original"
        );
    }

    #[test]
    fn a_double_time_row_behind_a_large_regular_row_survives() {
        // Order decides it: the regular row's fifty original hours drive the
        // budget to -40, and the loop skips everything after.
        let mut double_time = distribution(day(0), DOUBLE_TIME, 4.0);
        double_time.set_original_hours(0.0);

        let mut card = card(vec![shift(
            1,
            JOB,
            day(0),
            50.0,
            vec![distribution(day(0), REGULAR, 50.0), double_time],
        )]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(
            rows(&card, 0),
            vec![
                (Some(REGULAR), 40.0),
                (Some(DOUBLE_TIME), 4.0),
                (Some(OVERTIME), 10.0)
            ]
        );
    }

    #[test]
    fn a_shift_with_no_regular_row_gets_no_overtime_row() {
        // `updateOTHoursDistribution` has nothing to derive a premium from.
        let mut card = card(vec![shift(
            1,
            JOB,
            day(0),
            50.0,
            vec![distribution(day(0), DOUBLE_TIME, 50.0)],
        )]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(DOUBLE_TIME), 40.0)]);
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        let mut card = card(vec![worked(1, day(0), 50.0)])
            .with_hours_distribution_types(vec![HoursDistributionType::new(1, "Regular", false)]);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(rows(&card, 0), vec![(Some(REGULAR), 50.0)]);
    }
}

/// `PayPeriodOTHrsRuleImplTest.groovy`, transcribed.
///
/// All eight cases — and they assert almost nothing. Six of them are
/// `noExceptionThrown()` plus `hoursDistributions.size() >= 1`; the arithmetic
/// is never checked.
///
/// The reason is worth recording: **every fixture builds shifts with
/// distributions and no punches**, so `getWorkedHours()` is zero for all of
/// them and the period total never crosses the limit. Not one case reaches
/// `convertHoursDistributionToOT`. Even `should handle zero OT limit` misses,
/// because the test is `accumulatedWorkedHours > periodOTLimit` and `0.0 > 0.0`
/// is false.
///
/// They are transcribed anyway: they pin the no-op behaviour, and the one case
/// that does assert something real — a malformed limit throwing
/// `NumberFormatException` — confirms divergence 9's choice to panic. The
/// arithmetic is covered by the tests above instead.
///
/// The Groovy's `PayGroup(payPeriodType: WEEKLY, currentPayPeriodEndDate:
/// 2024-01-14)` becomes the card's current pay period directly (divergence 24).
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

    const JOB: i32 = 1;
    const REGULAR: i32 = 1;

    /// `new LegacyDatePeriod(new LocalDate(2024, 1, 1), new LocalDate(2024, 1, 7))`.
    fn work_week() -> DateRange {
        DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7))
    }

    /// The weekly pay group ending 2024-01-14.
    fn pay_period() -> DateRange {
        DateRange::new(LocalDate::of(2024, 1, 8), LocalDate::of(2024, 1, 14))
    }

    fn employee(pay_type: EmployeePayType) -> Employee {
        Employee::new(
            1,
            1,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                work_week().start_date(),
                work_week().end_date(),
                pay_type,
                0.0,
                true,
            )],
        )
    }

    /// `new EmployeeShift(shiftDate: …, job: job, hoursDistributions: […])` —
    /// no punches, so no worked hours.
    fn shift(date: LocalDate, original_hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new()).with_hours_distributions(
            vec![HoursDistribution::new(
                1,
                date,
                Some(REGULAR),
                original_hours,
                0.0,
            )],
        )
    }

    fn card(shifts: Vec<EmployeeShift>, pay_type: EmployeePayType) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(pay_type))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_current_pay_period(pay_period())
            .with_calculation_start_date(work_week().start_date())
    }

    fn rule_item(limit: &str) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::PayPeriodOtHdr,
            rule_params! { PERIOD_OT_LIMIT_PROP => limit },
        )
    }

    fn day(day_of_month: u32) -> LocalDate {
        LocalDate::of(2024, 1, day_of_month.try_into().unwrap())
    }

    #[test]
    fn should_execute_without_errors_for_basic_scenario() {
        let mut card = card(
            vec![shift(day(1), 45.0), shift(day(2), 10.0)],
            EmployeePayType::Hourly,
        );

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert_eq!(card.shifts().len(), 2);
    }

    #[test]
    fn should_handle_shifts_within_pay_period_limit() {
        let mut card = card(
            vec![shift(day(1), 20.0), shift(day(2), 15.0)],
            EmployeePayType::Hourly,
        );

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        for index in 0..2 {
            let distributions = card.shifts()[index].hours_distributions();
            assert!(!distributions.is_empty());
            assert!(distributions.iter().any(|d| d.is_of_type(REGULAR)));
        }
    }

    #[test]
    fn should_handle_empty_shifts_list() {
        let mut card = card(Vec::new(), EmployeePayType::Hourly);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert!(card.shifts().is_empty());
    }

    #[test]
    fn should_handle_salaried_exempt_employees() {
        let mut card = card(vec![shift(day(1), 50.0)], EmployeePayType::SalariedExempt);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert!(!card.shifts()[0].hours_distributions().is_empty());
    }

    #[test]
    fn should_handle_multiple_distributions_per_shift() {
        // Named for more than one distribution; the fixture has one.
        let mut card = card(vec![shift(day(1), 25.0)], EmployeePayType::Hourly);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert!(!card.shifts()[0].hours_distributions().is_empty());
    }

    #[test]
    #[should_panic(expected = "periodOtLimit")]
    fn should_handle_invalid_parameters() {
        // `thrown(NumberFormatException)` — Java aborts the calculation here
        // and nothing catches it, so the port panics (divergence 9).
        let mut card = card(vec![shift(day(1), 10.0)], EmployeePayType::Hourly);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("invalid"));
    }

    #[test]
    fn should_handle_zero_ot_limit() {
        // Still no overtime: the test is strictly greater, and the shift's
        // worked hours are zero.
        let mut card = card(vec![shift(day(1), 5.0)], EmployeePayType::Hourly);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("0.0"));

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
    }

    #[test]
    fn should_handle_negative_ot_factor() {
        // Named for a negative factor; the fixture passes 40.0 like the rest.
        let mut card = card(vec![shift(day(1), 10.0)], EmployeePayType::Hourly);

        PayPeriodOTHrsRule.execute(&mut card, &work_week(), &rule_item("40.0"));

        assert!(!card.shifts()[0].hours_distributions().is_empty());
    }
}
