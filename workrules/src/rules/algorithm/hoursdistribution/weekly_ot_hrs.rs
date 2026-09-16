//! Port of `WeeklyOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/WeeklyOTHrsRuleImpl.java`.
//!
//! `WEEKLY_OT_HDR`. Plain weekly overtime: walk the week's shifts in start-time
//! order, accumulate regular hours as you go, and the moment the running total
//! passes `weeklyOtLimit` every further hour is overtime.
//!
//! The same job as
//! [`WeeklyOTSecJobHrs`](super::weekly_ot_sec_job_hrs), arrived at from the
//! opposite direction. That rule measures the whole week first and then decides
//! *which* hours to charge — secondary job, latest first. This one charges the
//! hours it reaches last, in chronological order, with no opinion about the
//! job. Three differences follow from it, and none of them is cosmetic:
//!
//! | | this | `WeeklyOTSecJobHrs` |
//! |---|---|---|
//! | order | shifts earliest first, distributions earliest date first | secondary job first, then latest first throughout |
//! | overtime already paid | seeded from **earnings** of configured types | always starts at zero |
//! | a second OT row on one date | **merged** into the existing one | always appended |
//!
//! # Running it twice charges the week twice, as the sibling does
//!
//! Both accumulators are fields reset at the top of `execute`, and both rules
//! measure `originalHours` while writing `hours`. Premium rows carry zero
//! original hours, so a second pass rebuilds the same week total, computes the
//! same overtime, and subtracts it again from hours already reduced.
//!
//! The merge below hides it a little — the second pass adds to the existing
//! overtime row instead of leaving a visible duplicate — but the regular hours
//! still fall. Pinned by `a_second_pass_reduces_the_regular_hours_again`, so
//! the family's shared "run once per week" constraint is recorded on both rules
//! rather than only on the one where it happened to be noticed.
//!
//! # The 7(i) exemption
//!
//! `check7I` gates a test for the FLSA §7(i) retail-and-service commission
//! exemption: an employee paid mostly on commission, at more than
//! time-and-a-half of minimum wage, is exempt from overtime entirely. All three
//! of these must hold, and the rule then does **nothing at all** for the week:
//!
//! 1. `check7I` is set;
//! 2. the week's **end** date is open for editing;
//! 3. the FLSA regular rate exceeds `minWage * 1.5`, **and** 7(i) commissions
//!    exceed half the week's straight-time pay.
//!
//! Java reads `timeCard.getFlsaDataMap().get(workWeek.getEndDate())` and
//! dereferences it — a week with no FLSA row throws. Here a missing row means
//! the exemption cannot be established, so the week is calculated normally.
//!
//! The minimum wage comes through [`MinWagePort`]; see divergence 38 for why it
//! is a port rather than an entity walk.

use crate::common::json_ids::ids_for_key;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    CHECK_7I, OT_HOURS_EARNINGS_PROP, WEEKLY_LIMIT_PROP, WeeklyOTHrsRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// Weekly overtime, charged to the latest hours of the week.
/// `WeeklyOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeeklyOTHrsRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> WeeklyOTHrsRule<M> {
    /// Build the rule over its minimum-wage lookup, which only the 7(i) check
    /// consults.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    /// `qualifiesFor7iExemption`.
    fn qualifies_for_7i_exemption(
        &self,
        time_card: &dyn TimeCard,
        work_week: &DateRange,
        check_7i: bool,
    ) -> bool {
        check_7i
            && time_card.is_open_for_editing_on(work_week.end_date())
            && self.passes_7i_exemption_tests(time_card, work_week)
    }

    /// `passes7iExemptionTests`.
    ///
    /// Java dereferences the FLSA row and the home job status unguarded; both
    /// are `Option` here, and a missing one means the exemption is not
    /// established.
    fn passes_7i_exemption_tests(&self, time_card: &dyn TimeCard, work_week: &DateRange) -> bool {
        let Some(flsa_data) = time_card.flsa_data_map().get(&work_week.end_date()) else {
            return false;
        };
        let Some(employee) = time_card.employee() else {
            return false;
        };

        let half_flsa_dollars = round_currency(flsa_data.total_regular_rate_pay() * 0.5);
        let job_id = employee
            .first_home_employee_job_status_for_period(work_week)
            .map(|status| status.job_id());
        let min_wage =
            self.min_wage
                .min_wage(employee.property_id(), job_id, work_week.start_date());
        let min_wage_ot = round_currency(min_wage * 1.5);

        flsa_data.regular_rate() > min_wage_ot
            && flsa_data.seven_i_commissions() > half_flsa_dollars
    }
}

/// The running totals `execute` carries across the week. Java keeps these as
/// fields on the request-scoped bean and resets them at the top of `execute`.
struct Accumulators {
    weekly_hours: f64,
    weekly_ot: f64,
    weekly_limit: f64,
}

impl Accumulators {
    /// `calculateOTHours` — nothing until the limit is passed, and then only
    /// what has not already been paid.
    fn calculate_ot_hours(&self) -> f64 {
        if self.weekly_hours > self.weekly_limit {
            round_hours(self.weekly_hours - self.weekly_limit - self.weekly_ot).max(0.0)
        } else {
            0.0
        }
    }
}

impl<M: MinWagePort> HoursDistributionRule for WeeklyOTHrsRule<M> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&WeeklyOTHrsRuleConfig.default_values());

        if self.qualifies_for_7i_exemption(time_card, work_week, params.bool_at(CHECK_7I)) {
            return;
        }

        let paid_ot_earning_type_ids = ids_for_key(OT_HOURS_EARNINGS_PROP, &params);
        let mut accumulators = Accumulators {
            weekly_hours: 0.0,
            // Hours already paid as overtime through an earning of a configured
            // type count against the week before any shift does.
            weekly_ot: time_card
                .earnings_for_period(work_week)
                .into_iter()
                .filter(|earning| paid_ot_earning_type_ids.contains(&earning.earning_type_id()))
                .map(|earning| earning.hours())
                .sum(),
            weekly_limit: params.double_at(WEEKLY_LIMIT_PROP),
        };

        let Some(ot_type_id) = time_card.ot_hours_distribution_type_id() else {
            return;
        };

        for shift_index in shifts_earliest_first(time_card, work_week) {
            for distribution_index in
                regular_distributions_earliest_first(time_card, shift_index, work_week)
            {
                compute_ot_for_distribution(
                    time_card,
                    shift_index,
                    distribution_index,
                    ot_type_id,
                    rule_item.id(),
                    &mut accumulators,
                );
            }
        }
    }
}

/// The week's shifts that can be charged overtime, in start-time order.
///
/// `getShiftsWithDistributionsForPeriod(workWeek)` filtered by
/// `canCalculateShiftOT` and sorted with `ShiftStartTimeComparator`.
fn shifts_earliest_first(time_card: &dyn TimeCard, work_week: &DateRange) -> Vec<usize> {
    let mut indices = time_card.shift_indices_with_distributions_for_period(work_week);
    indices.retain(|&index| time_card.shift_is_not_salaried_exempt(&time_card.shifts()[index]));
    indices.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());
    indices
}

/// A shift's regular distributions inside the week, earliest date first.
fn regular_distributions_earliest_first(
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
            work_week.contains_date(distribution.date())
                && time_card.distribution_is_regular(distribution)
        })
        .map(|(index, _)| index)
        .collect();

    indices.sort_by_key(|&index| shift.hours_distributions()[index].date());
    indices
}

/// `computeOTForDistribution`.
fn compute_ot_for_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    ot_type_id: i32,
    rule_item_id: i32,
    accumulators: &mut Accumulators,
) {
    let distribution = &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
    let date = distribution.date();

    accumulators.weekly_hours =
        round_hours(accumulators.weekly_hours + distribution.original_hours());
    let distribution_ot = accumulators.calculate_ot_hours();

    if distribution_ot > 0.0 && time_card.is_open_for_editing_on(date) {
        let shift = &mut time_card.shifts_mut()[shift_index];
        let reduced =
            round_hours(shift.hours_distributions()[distribution_index].hours() - distribution_ot);
        shift.hours_distributions_mut()[distribution_index].set_hours(reduced);

        // An overtime row already on this date takes the hours; only a date
        // with none gets a new row. `WeeklyOTSecJobHrsRuleImpl` always appends.
        let existing = shift
            .hours_distributions()
            .iter()
            .position(|d| d.date() == date && d.is_of_type(ot_type_id));

        match existing {
            Some(index) => {
                let hours = shift.hours_distributions()[index].hours();
                shift.hours_distributions_mut()[index]
                    .set_hours(round_hours(hours + distribution_ot));
            }
            None => {
                let premium = create_premium_distribution(
                    &shift.hours_distributions()[distribution_index],
                    ot_type_id,
                    distribution_ot,
                    Some(rule_item_id),
                );
                shift.add_hours_distribution(premium);
            }
        }
    }

    accumulators.weekly_ot = round_hours(accumulators.weekly_ot + distribution_ot);
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
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    const JOB: i32 = 1;
    const PROPERTY: i32 = 1;
    const REGULAR: i32 = 1;
    const OVERTIME: i32 = 2;

    /// The property's own minimum wage, with no job pay rates configured —
    /// `Property.getMinWage()` reached through `getEffectiveMinWage`'s zero.
    struct PropertyMinWage(f64);

    impl MinWagePort for PropertyMinWage {
        fn min_wage(
            &self,
            _property_id: i32,
            _job_id: Option<i32>,
            _effective_date: LocalDate,
        ) -> f64 {
            self.0
        }
    }

    fn rule() -> WeeklyOTHrsRule<PropertyMinWage> {
        WeeklyOTHrsRule::new(PropertyMinWage(5.0))
    }

    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn week() -> DateRange {
        DateRange::new(today(), today().plus_days(5))
    }

    fn employee() -> Employee {
        Employee::new(
            1,
            PROPERTY,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                today().minus_days(365),
                today().plus_days(365),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }

    fn distribution(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(PROPERTY, date, Some(REGULAR), hours, 0.0)
    }

    fn shift(
        id: i32,
        shift_date: LocalDate,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(distributions)
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(today())
    }

    fn rule_item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = crate::rules::params::RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "", RuleClass::WeeklyOtHdr, rule_params)
    }

    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.date(), d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn nothing_happens_under_the_limit() {
        let mut card = card(vec![shift(1, today(), vec![distribution(today(), 8.0)])]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "40.0")]),
        );

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn the_hours_reached_last_become_overtime() {
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![(today(), Some(REGULAR), 10.0)],
            "the earlier shift is whole"
        );
        assert_eq!(
            rows(&card, 1),
            vec![
                (today().plus_days(1), Some(REGULAR), 5.0),
                (today().plus_days(1), Some(OVERTIME), 5.0)
            ]
        );
    }

    #[test]
    fn a_second_overtime_row_on_one_date_is_merged_into_the_first() {
        // Two regular rows on the same date, both crossing the limit. Java
        // looks for an existing overtime row before creating one, which
        // WeeklyOTSecJobHrs does not.
        let mut card = card(vec![shift(
            1,
            today(),
            vec![distribution(today(), 6.0), distribution(today(), 6.0)],
        )]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "4.0")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (today(), Some(REGULAR), 4.0),
                (today(), Some(REGULAR), 0.0),
                (today(), Some(OVERTIME), 8.0)
            ],
            "one overtime row carrying both contributions"
        );
    }

    #[test]
    fn a_second_pass_reduces_the_regular_hours_again() {
        // The family's shared hazard: the accumulators reset, the premium rows
        // carry zero original hours, so the same overtime is computed and
        // subtracted from already-reduced hours.
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ]);
        let item = rule_item(&[(WEEKLY_LIMIT_PROP, "15.0")]);

        rule().execute(&mut card, &week(), &item);
        rule().execute(&mut card, &week(), &item);

        assert_eq!(
            rows(&card, 1),
            vec![
                (today().plus_days(1), Some(REGULAR), 0.0),
                (today().plus_days(1), Some(OVERTIME), 10.0)
            ],
            "5 - 5 = 0, merged into the existing overtime row"
        );
    }

    #[test]
    fn a_salaried_exempt_shift_neither_counts_nor_pays() {
        let exempt_job = 9;
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 40.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 4.0)],
            ),
        ]);
        card.shifts_mut()[0] =
            EmployeeShift::new(1, 1, exempt_job, today(), ShiftType::Actual, Vec::new())
                .with_hours_distributions(vec![distribution(today(), 40.0)]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0")]),
        );

        assert_eq!(
            rows(&card, 1),
            vec![(today().plus_days(1), Some(REGULAR), 4.0)]
        );
    }

    #[test]
    fn the_seven_i_exemption_skips_the_whole_week() {
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ])
        .with_flsa_data(HashMap::from([(
            week().end_date(),
            // Rate well above 5.00 * 1.5, commissions above half of 400.
            FlsaData::new(
                week().end_date(),
                0.0,
                0.0,
                0.0,
                400.0,
                500.0,
                0.0,
                0.0,
                20.0,
                50.0,
                0.0,
            ),
        )]));

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0"), (CHECK_7I, "true")]),
        );

        assert_eq!(
            rows(&card, 1),
            vec![(today().plus_days(1), Some(REGULAR), 10.0)]
        );
    }

    #[test]
    fn a_rate_at_or_below_time_and_a_half_of_minimum_wage_is_not_exempt() {
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ])
        .with_flsa_data(HashMap::from([(
            week().end_date(),
            // 7.50 is exactly 5.00 * 1.5, and the test is strict.
            FlsaData::new(
                week().end_date(),
                0.0,
                0.0,
                0.0,
                400.0,
                500.0,
                0.0,
                0.0,
                20.0,
                7.5,
                0.0,
            ),
        )]));

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0"), (CHECK_7I, "true")]),
        );

        assert_eq!(
            rows(&card, 1),
            vec![
                (today().plus_days(1), Some(REGULAR), 5.0),
                (today().plus_days(1), Some(OVERTIME), 5.0)
            ]
        );
    }

    #[test]
    fn a_week_with_no_flsa_row_is_calculated_normally() {
        // Java dereferences the map's null and throws.
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0"), (CHECK_7I, "true")]),
        );

        assert_eq!(rows(&card, 1).len(), 2);
    }

    #[test]
    fn configured_earnings_count_as_overtime_already_paid() {
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ])
        .with_earnings(vec![EmployeeEarning::new(
            1,
            1,
            JOB,
            98,
            today(),
            4.0,
            0.0,
            EarningSource::Manual,
        )]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[
                (WEEKLY_LIMIT_PROP, "15.0"),
                (OT_HOURS_EARNINGS_PROP, "[98]"),
            ]),
        );

        assert_eq!(
            rows(&card, 1),
            vec![
                (today().plus_days(1), Some(REGULAR), 9.0),
                (today().plus_days(1), Some(OVERTIME), 1.0)
            ],
            "five hours over, less the four already paid"
        );
    }

    #[test]
    fn an_earning_of_an_unconfigured_type_does_not_count() {
        let mut card = card(vec![
            shift(1, today(), vec![distribution(today(), 10.0)]),
            shift(
                2,
                today().plus_days(1),
                vec![distribution(today().plus_days(1), 10.0)],
            ),
        ])
        .with_earnings(vec![EmployeeEarning::new(
            1,
            1,
            JOB,
            33,
            today(),
            4.0,
            0.0,
            EarningSource::Manual,
        )]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[
                (WEEKLY_LIMIT_PROP, "15.0"),
                (OT_HOURS_EARNINGS_PROP, "[98]"),
            ]),
        );

        assert_eq!(
            rows(&card, 1),
            vec![
                (today().plus_days(1), Some(REGULAR), 5.0),
                (today().plus_days(1), Some(OVERTIME), 5.0)
            ]
        );
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        let mut card = card(vec![shift(1, today(), vec![distribution(today(), 20.0)])])
            .with_hours_distribution_types(vec![HoursDistributionType::new(1, "Regular", false)]);

        rule().execute(
            &mut card,
            &week(),
            &rule_item(&[(WEEKLY_LIMIT_PROP, "15.0")]),
        );

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 20.0)]);
    }
}

/// `WeeklyOTHrsRuleImplSpec.groovy`, transcribed.
///
/// All seven cases. `LocalDate.now()` pinned; the pay period, which the Groovy
/// mocks as `WeeklyDateRange.withStartDate(today)`, set on the card as its
/// calculation start date (divergence 24); and the property's `State(minWage:
/// 5.00)` supplied through [`MinWagePort`] (divergence 38) — the spec's job has
/// no pay rates, so `getEffectiveMinWage` returns zero and Java falls through
/// to the property's figure.
///
/// **One case does not test what it says.** `should not create OT if
/// i7earnings and rate are greater than FLSA min wage` builds its rule item
/// with `CHECK_7I` alone, so `weeklyOtLimit` takes its default of 40 — and the
/// fixture only has 20 hours. The week produces no overtime whether or not the
/// exemption fires. Transcribed as written, with a second assertion added
/// alongside it that does isolate the branch.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    /// `new Assignment(id: 1, property: property)`.
    const JOB: i32 = 1;
    /// `new Property(id: 1, state: new State(minWage: 5.00), ...)`.
    const PROPERTY: i32 = 1;
    const REGULAR: i32 = 1;
    const OVERTIME: i32 = 2;

    /// `new State(minWage: 5.00)`, reached because the job has no pay rates.
    struct StateMinWage;

    impl MinWagePort for StateMinWage {
        fn min_wage(
            &self,
            _property_id: i32,
            _job_id: Option<i32>,
            _effective_date: LocalDate,
        ) -> f64 {
            5.00
        }
    }

    fn rule() -> WeeklyOTHrsRule<StateMinWage> {
        WeeklyOTHrsRule::new(StateMinWage)
    }

    /// `def today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn day(offset: i64) -> LocalDate {
        today().plus_days(offset)
    }

    /// `new LegacyDatePeriod(today, today.plusDays(5))`.
    fn work_week() -> DateRange {
        DateRange::new(today(), day(5))
    }

    fn employee() -> Employee {
        Employee::new(
            1,
            PROPERTY,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                today().minus_days(365),
                today().plus_days(365),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }

    fn distribution(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(PROPERTY, date, Some(REGULAR), hours, 0.0)
    }

    fn shift(shift_date: LocalDate, distributions: Vec<HoursDistribution>) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(distributions)
    }

    /// `singleDayShifts` — ten hours today, ten tomorrow.
    fn single_day_shifts() -> Vec<EmployeeShift> {
        vec![
            shift(today(), vec![distribution(today(), 10.0)]),
            shift(day(1), vec![distribution(day(1), 10.0)]),
        ]
    }

    /// `splitDayShifts` — the second shift's hours land on two days.
    fn split_day_shifts() -> Vec<EmployeeShift> {
        vec![
            shift(today(), vec![distribution(today(), 10.0)]),
            shift(
                day(1),
                vec![distribution(day(1), 5.0), distribution(day(2), 5.0)],
            ),
        ]
    }

    /// `splitWeekShifts` — the first shift straddles the week boundary.
    fn split_week_shifts() -> Vec<EmployeeShift> {
        vec![
            shift(
                day(-1),
                vec![distribution(day(-1), 5.0), distribution(today(), 5.0)],
            ),
            shift(day(1), vec![distribution(day(1), 10.0)]),
        ]
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(today())
    }

    /// The spec's shared rule item: limit 15, no 7(i), earning type 98.
    fn rule_item() -> RuleItem {
        item(&[
            (WEEKLY_LIMIT_PROP, "15.0"),
            (CHECK_7I, "false"),
            (OT_HOURS_EARNINGS_PROP, "[98]"),
        ])
    }

    fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(1, 1, "WeeklyOTHrsRule", RuleClass::WeeklyOtHdr, rule_params)
    }

    fn earning(id: i32, date: LocalDate, hours: f64, earning_type_id: i32) -> EmployeeEarning {
        EmployeeEarning::new(
            id,
            1,
            JOB,
            earning_type_id,
            date,
            hours,
            0.0,
            EarningSource::Manual,
        )
    }

    /// Every distribution on a shift, as `(date, type id, hours)`.
    fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.date(), d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn rule_should_distribute_ot_hours_to_shifts_that_exceed_the_weekly_limit() {
        let mut card = card(single_day_shifts());

        rule().execute(&mut card, &work_week(), &rule_item());

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(day(1), Some(REGULAR), 5.0), (day(1), Some(OVERTIME), 5.0)]
        );
    }

    #[test]
    fn should_create_a_premium_hours_distribution_for_the_day_the_hours_were_worked() {
        let mut card = card(split_day_shifts());

        rule().execute(&mut card, &work_week(), &rule_item());

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (day(1), Some(REGULAR), 5.0),
                (day(2), Some(REGULAR), 0.0),
                (day(2), Some(OVERTIME), 5.0)
            ]
        );
    }

    #[test]
    fn should_create_multiple_premium_distributions_if_work_is_split_across_multiple_days() {
        let mut card = card(split_day_shifts());

        // Only the limit is set, so CHECK_7I and OT_HOURS_EARNINGS take their
        // defaults.
        rule().execute(
            &mut card,
            &work_week(),
            &item(&[(WEEKLY_LIMIT_PROP, "12.0")]),
        );

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (day(1), Some(REGULAR), 2.0),
                (day(2), Some(REGULAR), 0.0),
                (day(1), Some(OVERTIME), 3.0),
                (day(2), Some(OVERTIME), 5.0)
            ]
        );
    }

    #[test]
    fn should_not_create_ot_if_i7earnings_and_rate_are_greater_than_flsa_min_wage() {
        let flsa = HashMap::from([(
            day(5),
            FlsaData::new(day(5), 0.0, 0.0, 0.0, 400.0, 0.0, 0.0, 0.0, 0.0, 50.0, 0.0),
        )]);
        let mut as_written = card(single_day_shifts()).with_flsa_data(flsa.clone());

        // As the Groovy writes it: CHECK_7I alone, so the limit defaults to 40
        // and twenty hours produce no overtime either way.
        rule().execute(&mut as_written, &work_week(), &item(&[(CHECK_7I, "true")]));

        assert_eq!(rows(&as_written, 0), vec![(today(), Some(REGULAR), 10.0)]);
        assert_eq!(rows(&as_written, 1), vec![(day(1), Some(REGULAR), 10.0)]);

        // The same fixture with a limit the week actually crosses, which is
        // what the case means to assert.
        let mut crossing = card(single_day_shifts()).with_flsa_data(flsa);
        rule().execute(
            &mut crossing,
            &work_week(),
            &item(&[(CHECK_7I, "true"), (WEEKLY_LIMIT_PROP, "15.0")]),
        );

        assert_eq!(rows(&crossing, 1), vec![(day(1), Some(REGULAR), 10.0)]);
    }

    #[test]
    fn should_not_include_hours_distributions_towards_ot_if_not_in_the_week() {
        let mut card = card(split_week_shifts());

        rule().execute(&mut card, &work_week(), &rule_item());

        assert_eq!(
            rows(&card, 0),
            vec![(day(-1), Some(REGULAR), 5.0), (today(), Some(REGULAR), 5.0)],
            "the day before the week does not count toward the limit"
        );
        assert_eq!(rows(&card, 1), vec![(day(1), Some(REGULAR), 10.0)]);
    }

    #[test]
    fn rule_should_count_configured_earnings_as_already_paid_ot() {
        let mut card = card(single_day_shifts()).with_earnings(vec![
            earning(1, today(), 4.25, 98),
            earning(2, today(), 4.25, 33),
            earning(3, day(-3), 4.25, 98),
        ]);

        rule().execute(&mut card, &work_week(), &rule_item());

        assert_eq!(rows(&card, 0), vec![(today(), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (day(1), Some(REGULAR), 9.25),
                (day(1), Some(OVERTIME), 0.75)
            ],
            "only the in-week earning of a configured type counts"
        );
    }

    #[test]
    fn test_zachs_scenario() {
        let mut card = card(vec![
            shift(day(1), vec![distribution(day(1), 8.0)]),
            shift(day(2), vec![distribution(day(2), 8.0)]),
            shift(day(4), vec![distribution(day(4), 4.0)]),
            shift(day(5), vec![distribution(day(5), 4.0)]),
        ])
        .with_earnings(vec![
            earning(1, day(5), 4.0, 98),
            earning(2, day(5), 2.0, 98),
        ]);

        rule().execute(
            &mut card,
            &DateRange::new(today(), day(6)),
            &item(&[
                (WEEKLY_LIMIT_PROP, "18.0"),
                (OT_HOURS_EARNINGS_PROP, "[98]"),
            ]),
        );

        // Twenty-four hours against a limit of eighteen is six over, and six
        // hours of earnings have already paid it.
        assert_eq!(rows(&card, 0), vec![(day(1), Some(REGULAR), 8.0)]);
        assert_eq!(rows(&card, 1), vec![(day(2), Some(REGULAR), 8.0)]);
        assert_eq!(rows(&card, 2), vec![(day(4), Some(REGULAR), 4.0)]);
        assert_eq!(rows(&card, 3), vec![(day(5), Some(REGULAR), 4.0)]);
    }
}
