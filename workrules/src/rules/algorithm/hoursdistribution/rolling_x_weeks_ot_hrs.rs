//! Port of `RollingXWeeksOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/RollingXWeeksOTHrsRuleImpl.java`.
//!
//! `ROLLING_X_WEEKS_OT_HDR`. Weekly overtime measured over a window of several
//! weeks rather than one: seed the running totals from the `weeks - 1` weeks
//! before this one, then walk this week's shifts charging everything past the
//! limit.
//!
//! Structurally [`weekly_ot_hrs`](super::weekly_ot_hrs) with a head start —
//! same `hours - limit - alreadyPaid` arithmetic, same earliest-first walk —
//! except that where that rule seeds its overtime accumulator from *earnings*,
//! this one seeds both accumulators from prior weeks' distributions.
//!
//! # Where the prior weeks come from depends on one date
//!
//! ```java
//! private boolean needMoreData(DateRange priorWeeksPeriod) {
//!    return priorWeeksPeriod.getStartDate().isBefore(dataset.getDatasetStartDate());
//! }
//! ```
//!
//! Inside the dataset, the totals are summed off the card. Outside it, they
//! come from [`EmployeeShiftPort::net_and_ot_hours_for_period`]. **The two
//! paths do not count the same things:**
//!
//! | | off the card | from the port |
//! |---|---|---|
//! | net hours | `originalHours` of every **regular bucket** | `originalHours` where `HoursDistributionTypeID = 1` |
//! | overtime | `hours` of every **non-regular** bucket, double time included | `hours` where `HoursDistributionTypeID = 2` |
//!
//! So a property with a second non-premium bucket, or any double time in the
//! window, gets different answers on either side of the dataset boundary. Both
//! are reproduced as written.
//!
//! # It sorts the shift's own distribution list in place
//!
//! `sortHoursDistributions` does `Collections.sort(shift.getHoursDistributions(), …)`
//! — not a copy. The shift's stored order is left sorted by date, which a
//! later rule reading "the first regular distribution" (as
//! [`pay_period_ot_hrs`](super::pay_period_ot_hrs) does) would see. Reproduced,
//! because dropping it would make rule order matter differently.
//!
//! # The regular row is reduced without rounding
//!
//! `hoursDistribution.setHours(hoursDistribution.getHours() - shiftRollingOT)`
//! — bare subtraction, where every sibling rule wraps this in
//! `TDouble.roundHours`. The overtime it subtracts *is* rounded, so the result
//! is usually clean; it is the one unrounded write in the family.

use crate::common::enums::shift_type::ShiftType;
use crate::common::numbers::round_hours;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    RollingXWeeksOTHrsRuleConfig, WEEKLY_LIMIT_PROP, WEEKS,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::ports::EmployeeShiftPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// Overtime over a rolling multi-week window. `RollingXWeeksOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RollingXWeeksOTHrsRule<P: EmployeeShiftPort> {
    shifts: P,
}

impl<P: EmployeeShiftPort> RollingXWeeksOTHrsRule<P> {
    /// Build the rule over the lookup that reaches back past the dataset.
    pub fn new(shifts: P) -> Self {
        Self { shifts }
    }

    /// `initializeRollingAccumulators`.
    fn initial_totals(
        &self,
        time_card: &dyn TimeCard,
        work_week: &DateRange,
        weeks: i32,
    ) -> Rolling {
        let mut rolling = Rolling {
            hours: 0.0,
            ot: 0.0,
        };

        if weeks <= 1 {
            return rolling;
        }

        let additional_weeks = i64::from(weeks - 1);
        let prior_weeks = DateRange::new(
            work_week.start_date().minus_days(7 * additional_weeks),
            work_week.start_date().minus_days(1),
        );

        if prior_weeks.start_date() < time_card.dataset_start_date() {
            let shift_type = if time_card.is_run_from_scheduling() {
                ShiftType::Schedule
            } else {
                ShiftType::Actual
            };

            if let Some(totals) = self.shifts.net_and_ot_hours_for_period(
                time_card.employee_id(),
                &prior_weeks,
                shift_type,
                false,
            ) {
                rolling.hours += totals.net_hours;
                rolling.ot += totals.ot_hours;
            }
        } else {
            let regular_type_ids = time_card.regular_hours_distribution_type_ids();
            for shift in time_card.shifts_with_distributions_for_period(&prior_weeks) {
                for distribution in shift.hours_distributions() {
                    if !prior_weeks.contains_date(distribution.date()) {
                        continue;
                    }
                    let is_regular = distribution
                        .hours_distribution_type_id()
                        .is_some_and(|id| regular_type_ids.contains(&id));

                    // `getShiftNetHours` reads originalHours, `getShiftOTHours`
                    // reads hours — and everything not regular counts as
                    // overtime, double time included.
                    if is_regular {
                        rolling.hours += distribution.original_hours();
                    } else {
                        rolling.ot += distribution.hours();
                    }
                }
            }
        }

        rolling
    }
}

/// The two running totals, carried across the week.
struct Rolling {
    hours: f64,
    ot: f64,
}

impl Rolling {
    /// `calculateDistributionOT` together with `calculateOTHours`.
    fn take(&mut self, original_hours: f64, limit: f64) -> f64 {
        self.hours = round_hours(self.hours + original_hours);

        let overtime = if self.hours > limit {
            round_hours(self.hours - limit - self.ot).max(0.0)
        } else {
            0.0
        };

        self.ot = round_hours(self.ot + overtime);
        overtime
    }
}

impl<P: EmployeeShiftPort> HoursDistributionRule for RollingXWeeksOTHrsRule<P> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&RollingXWeeksOTHrsRuleConfig.default_values());
        let rolling_ot_limit = params.double_at(WEEKLY_LIMIT_PROP);
        let rolling_weeks = params.int_at(WEEKS);

        let mut rolling = self.initial_totals(time_card, work_week, rolling_weeks);

        let Some(ot_type_id) = time_card.ot_hours_distribution_type_id() else {
            return;
        };
        let regular_type_ids = time_card.regular_hours_distribution_type_ids();

        let mut sorted_shifts = time_card.shift_indices_with_distributions_for_period(work_week);
        sorted_shifts.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

        for shift_index in sorted_shifts {
            if !time_card.shift_is_not_salaried_exempt(&time_card.shifts()[shift_index]) {
                continue;
            }

            // `sortHoursDistributions` sorts the shift's own list; see the
            // module note.
            time_card.shifts_mut()[shift_index]
                .hours_distributions_mut()
                .sort_by_key(|distribution| distribution.date());

            let mut created = Vec::new();

            for index in 0..time_card.shifts()[shift_index].hours_distributions().len() {
                let distribution = &time_card.shifts()[shift_index].hours_distributions()[index];
                let is_regular = distribution
                    .hours_distribution_type_id()
                    .is_some_and(|id| regular_type_ids.contains(&id));

                if !is_regular || !work_week.contains_date(distribution.date()) {
                    continue;
                }

                let date = distribution.date();
                let overtime = rolling.take(distribution.original_hours(), rolling_ot_limit);

                // `createNewOtDistributionAndUpdateRegularHours`.
                if time_card.is_open_for_editing_on(date) && overtime > 0.0 {
                    let shift = &mut time_card.shifts_mut()[shift_index];
                    // Bare subtraction, as Java writes it.
                    let reduced = shift.hours_distributions()[index].hours() - overtime;
                    shift.hours_distributions_mut()[index].set_hours(reduced);

                    created.push(create_premium_distribution(
                        &shift.hours_distributions()[index],
                        ot_type_id,
                        overtime,
                        Some(rule_item.id()),
                    ));
                }
            }

            let shift = &mut time_card.shifts_mut()[shift_index];
            for distribution in created {
                shift.add_hours_distribution(distribution);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::ports::NetAndOtHours;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    pub(super) const JOB: i32 = 1;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;

    /// The DAO, answering fixed prior-window totals.
    pub(super) struct PriorWeeks(pub Option<NetAndOtHours>);

    impl EmployeeShiftPort for PriorWeeks {
        fn net_actual_hours_in_period(&self, _employee_id: i32, _period: &DateRange) -> f64 {
            0.0
        }
        fn is_scheduled(
            &self,
            _employee_id: i32,
            _date_time: LocalDateTime,
            _grace_pre_schedule: i32,
            _grace_post_schedule: i32,
        ) -> bool {
            false
        }
        fn net_and_ot_hours_for_period(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _shift_type: ShiftType,
            _home_dept_only: bool,
        ) -> Option<NetAndOtHours> {
            self.0
        }
    }

    /// No prior window held anywhere.
    pub(super) fn no_prior_weeks() -> RollingXWeeksOTHrsRule<PriorWeeks> {
        RollingXWeeksOTHrsRule::new(PriorWeeks(None))
    }

    pub(super) fn jan(day: u32) -> LocalDate {
        LocalDate::of(2016, 1, day.try_into().unwrap())
    }

    /// `new LegacyDatePeriod(2016-01-01, 2016-01-07)`.
    pub(super) fn work_week() -> DateRange {
        DateRange::new(jan(1), jan(7))
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
                work_week().start_date(),
                work_week().end_date(),
                pay_type,
                0.0,
                true,
            )],
        )
    }

    pub(super) fn distribution(date: LocalDate, type_id: i32, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(type_id), hours, 0.0)
    }

    pub(super) fn shift(
        id: i32,
        date: LocalDate,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(
            id,
            1,
            JOB,
            date,
            crate::common::enums::shift_type::ShiftType::Actual,
            Vec::new(),
        )
        .with_hours_distributions(distributions)
    }

    /// The weekly pay group ending 2016-01-07, so the calculation opens on
    /// 01-01 and the dataset starts ten days earlier, on 2015-12-22.
    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(EmployeePayType::Hourly))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(jan(1))
    }

    pub(super) fn item(limit: &str, weeks: &str) -> RuleItem {
        let mut params = RuleParams::new();
        params.set(WEEKLY_LIMIT_PROP, limit);
        params.set(WEEKS, weeks);
        RuleItem::new(1, 1, "", RuleClass::RollingXWeeksOtHdr, params)
    }

    pub(super) fn rows(
        card: &TimeCardData,
        shift_index: usize,
    ) -> Vec<(LocalDate, Option<i32>, f64)> {
        card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| (d.date(), d.hours_distribution_type_id(), d.hours()))
            .collect()
    }

    #[test]
    fn a_single_week_under_the_limit_pays_nothing() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 8.0)],
        )]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "1"));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn a_single_week_needs_no_prior_window() {
        // With weeks = 1 the port is never consulted, so a rule wired to a
        // panicking one would still work.
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 12.0)],
        )]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("10.0", "1"));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 10.0), (jan(1), Some(OVERTIME), 2.0)]
        );
    }

    #[test]
    fn the_prior_window_comes_off_the_card_when_it_starts_inside_the_dataset() {
        // Two weeks back from 01-01 is 2015-12-25, after the dataset start of
        // 2015-12-22 — so the card is summed and the port ignored.
        let mut card = card(vec![
            shift(
                1,
                LocalDate::of(2015, 12, 28),
                vec![distribution(LocalDate::of(2015, 12, 28), REGULAR, 30.0)],
            ),
            shift(2, jan(1), vec![distribution(jan(1), REGULAR, 12.0)]),
        ]);

        RollingXWeeksOTHrsRule::new(PriorWeeks(Some(NetAndOtHours {
            net_hours: 999.0,
            ot_hours: 0.0,
        })))
        .execute(&mut card, &work_week(), &item("40.0", "2"));

        assert_eq!(
            rows(&card, 1),
            vec![(jan(1), Some(REGULAR), 10.0), (jan(1), Some(OVERTIME), 2.0)],
            "30 held plus 12 is 42, two over the limit"
        );
    }

    #[test]
    fn a_window_reaching_before_the_dataset_uses_the_port() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 12.0)],
        )]);

        RollingXWeeksOTHrsRule::new(PriorWeeks(Some(NetAndOtHours {
            net_hours: 30.0,
            ot_hours: 0.0,
        })))
        .execute(&mut card, &work_week(), &item("40.0", "3"));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 10.0), (jan(1), Some(OVERTIME), 2.0)]
        );
    }

    #[test]
    fn a_port_that_answers_nothing_leaves_the_totals_at_zero() {
        // Java's `result != null && result[0] != null` guard.
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 12.0)],
        )]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "3"));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 12.0)]);
    }

    #[test]
    fn overtime_already_paid_in_the_prior_window_is_not_paid_again() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 12.0)],
        )]);

        RollingXWeeksOTHrsRule::new(PriorWeeks(Some(NetAndOtHours {
            net_hours: 45.0,
            ot_hours: 5.0,
        })))
        .execute(&mut card, &work_week(), &item("40.0", "3"));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 0.0), (jan(1), Some(OVERTIME), 12.0)],
            "57 - 40 - 5 = 12"
        );
    }

    #[test]
    fn the_card_path_counts_every_non_regular_bucket_as_overtime() {
        // Including double time — the port's SQL counts only bucket 2.
        let prior = LocalDate::of(2015, 12, 28);
        let mut card = card(vec![
            shift(
                1,
                prior,
                vec![
                    distribution(prior, REGULAR, 40.0),
                    distribution(prior, HoursDistributionType::DT_ID, 4.0),
                ],
            ),
            shift(2, jan(1), vec![distribution(jan(1), REGULAR, 8.0)]),
        ]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "2"));

        assert_eq!(
            rows(&card, 1),
            vec![(jan(1), Some(REGULAR), 4.0), (jan(1), Some(OVERTIME), 4.0)],
            "48 - 40 - 4 already paid, where the port would have said 8"
        );
    }

    #[test]
    fn a_salaried_exempt_shift_neither_counts_nor_pays() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 50.0)],
        )])
        .with_employee(employee(EmployeePayType::SalariedExempt));

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "1"));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 50.0)]);
    }

    #[test]
    fn the_shifts_own_distribution_list_is_left_sorted_by_date() {
        // `sortHoursDistributions` sorts in place, not a copy.
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![
                distribution(jan(3), REGULAR, 2.0),
                distribution(jan(1), REGULAR, 2.0),
            ],
        )]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "1"));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 2.0), (jan(3), Some(REGULAR), 2.0)],
            "reordered even though no overtime was paid"
        );
    }

    #[test]
    fn a_distribution_outside_the_work_week_is_not_counted() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![
                distribution(jan(1), REGULAR, 8.0),
                distribution(LocalDate::of(2016, 1, 20), REGULAR, 50.0),
            ],
        )]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("10.0", "1"));

        assert_eq!(
            rows(&card, 0),
            vec![
                (jan(1), Some(REGULAR), 8.0),
                (LocalDate::of(2016, 1, 20), Some(REGULAR), 50.0)
            ]
        );
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        let mut card = card(vec![shift(
            1,
            jan(1),
            vec![distribution(jan(1), REGULAR, 50.0)],
        )])
        .with_hours_distribution_types(vec![HoursDistributionType::new(REGULAR, "Regular", false)]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("40.0", "1"));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 50.0)]);
    }
}

/// `RollingXWeeksOTHrsRuleImplTest.groovy`, transcribed.
///
/// All three cases. The mocked `PayGroup` becomes the card's calculation start
/// date (divergence 24), which puts the dataset start at 2015-12-22.
///
/// **The Groovy's `any {}` blocks assert far less than they appear to.** Each
/// is four bare comparisons on consecutive lines:
///
/// ```groovy
/// shift2.hoursDistributions.any { hd ->
///    hd.hours == 5
///    hd.date == new LocalDate(2016, 1, 3)
///    hd.originalHours == 5
///    hd.hoursDistributionTypeID == HoursDistributionTestUtils.overtime.id
/// }
/// ```
///
/// Only the **last** line is the closure's return value; the first three are
/// evaluated and discarded. So the predicate is "some distribution is of the
/// overtime type" — the hours, date and original hours are never checked. These
/// transcriptions assert the whole row instead, which is what the case is
/// plainly meant to say and is strictly stronger.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        OVERTIME, REGULAR, card, distribution, item, jan, no_prior_weeks, rows, shift, work_week,
    };
    use super::*;
    use crate::rules::ports::NetAndOtHours;

    #[test]
    fn for_a_single_week_shifts_over_the_rolling_limit_will_be_added_as_ot() {
        let mut card = card(vec![
            shift(1, jan(1), vec![distribution(jan(1), REGULAR, 10.0)]),
            shift(2, jan(3), vec![distribution(jan(3), REGULAR, 5.0)]),
        ]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("10", "1"));

        // Ten hours is not *over* ten, so the first shift is untouched.
        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 0.0), (jan(3), Some(OVERTIME), 5.0)]
        );
    }

    #[test]
    fn for_multiple_weeks_the_prior_window_counts_toward_the_limit() {
        let mut card = card(vec![
            shift(1, jan(1), vec![distribution(jan(1), REGULAR, 10.0)]),
            shift(2, jan(3), vec![distribution(jan(3), REGULAR, 5.0)]),
        ]);

        // `getNetAndOTHoursForPeriod(employee, _, ACTUAL, false) >> [16.0d, 0.0d]`
        // — three weeks back reaches 2015-12-18, before the dataset start.
        RollingXWeeksOTHrsRule::new(super::tests::PriorWeeks(Some(NetAndOtHours {
            net_hours: 16.0,
            ot_hours: 0.0,
        })))
        .execute(&mut card, &work_week(), &item("25", "3"));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 9.0), (jan(1), Some(OVERTIME), 1.0)],
            "16 + 10 is one over the limit of 25"
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 0.0), (jan(3), Some(OVERTIME), 5.0)]
        );
    }

    #[test]
    fn rule_correctly_backfills_shift_ot_on_spanning_distributions() {
        let mut card = card(vec![
            shift(
                1,
                jan(1),
                vec![
                    distribution(jan(1), REGULAR, 5.0),
                    distribution(jan(2), REGULAR, 5.0),
                ],
            ),
            shift(2, jan(3), vec![distribution(jan(3), REGULAR, 5.0)]),
        ]);

        no_prior_weeks().execute(&mut card, &work_week(), &item("4", "1"));

        assert_eq!(
            rows(&card, 0),
            vec![
                (jan(1), Some(REGULAR), 4.0),
                (jan(2), Some(REGULAR), 0.0),
                (jan(1), Some(OVERTIME), 1.0),
                (jan(2), Some(OVERTIME), 5.0)
            ],
            "the overnight shift's two days are charged in date order"
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 0.0), (jan(3), Some(OVERTIME), 5.0)]
        );
    }
}
