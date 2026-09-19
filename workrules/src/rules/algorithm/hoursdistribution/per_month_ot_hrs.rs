//! Port of `PerMonthOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/PerMonthOTHrsRuleImpl.java`.
//!
//! `PER_MONTH_OT_HDR`. Overtime against a **monthly contract**: each calendar
//! month has its own configured hours, and everything past it is overtime.
//! Twelve parameters, one per month, keyed by the `Month` enum's own names —
//! `JAN` through `DEC`, not the full month names.
//!
//! # The first [`ContractHrsRule`](super::ContractHrsRule)
//!
//! This is the family's first implementation of `ContractHrsRuleImpl`, the
//! marker interface `RuleUtils.getContractHours` reaches for by `instanceof`.
//! It contributes [`contract_period`](super::ContractHrsRule::contract_period)
//! (always monthly) and
//! [`contract_hours`](super::ContractHrsRule::contract_hours), which other
//! parts of the engine ask for without knowing which rule answers.
//!
//! # A mid-month hire is pro-rated by **calendar** days
//!
//! ```java
//! double hoursPerDay = TDouble.roundHours(monthlyContract / monthPeriod.getNumberOfDaysInRange());
//! monthlyContract = TDouble.roundHours(hoursPerDay * monthsWorkingDays.getNumberOfDaysInRange());
//! ```
//!
//! Not working days — every day of the month, weekends included. And the
//! per-day figure is rounded to two places *before* being multiplied back up,
//! so a 160-hour January gives 5.16 hours a day and a hire on the 16th gets
//! 82.56 rather than 82.58.
//!
//! # A month's running total is seeded from before the week, two ways
//!
//! `createOrGetMonthlyAccumulator` compares the month's first day against
//! `getDatasetStartDate()`: at or after it the totals come off the card, before
//! it from `employeeEarningDAO` and `employeeShiftDAO`. Same split as
//! [`rolling_x_weeks_ot_hrs`](super::rolling_x_weeks_ot_hrs), and the same
//! disagreement between the two sides — the card path treats every non-regular
//! bucket as overtime while the DAO's SQL counts only bucket 2.
//!
//! **The card path's shift window is routinely inverted.** It is
//! `[firstOfMonth, workWeek.getStartDate() - 1]`, so for any week that starts
//! on or before the first of the month — which is most first-week-of-month
//! calculations — the range runs backwards and `containsDate` answers `false`
//! for everything. The seeding silently contributes nothing. Reproduced; the
//! Java spec's last case depends on it.
//!
//! Note also that only the DAO path consults the earning DAO, and only under
//! [`EmployeeCalculationMode::Ta`](crate::common::enums::employee_calculation_mode::EmployeeCalculationMode::Ta);
//! the card path reads earnings regardless of mode.
//!
//! # `homeDeptOnly` compares **parent** assignments
//!
//! `shiftJob.getParentAssignment() == homeJob.getParentAssignment()` — Java
//! reference equality on the entity, so two jobs under no parent at all compare
//! equal (both null) and two distinct objects holding the same id would not.
//! Here it is an `Option<i32>` comparison, which agrees on the first case and
//! differs only for a graph with duplicate entity instances — something a
//! materialised card does not produce.
//!
//! # Its own JSON parser, with a different failure mode
//!
//! `getEarningTypeIdsList` inlines the array parse instead of calling
//! `JSONUtils.getIdsListForKey`, and its `ids` list is declared **outside** the
//! `try`. So a malformed element leaves the partially built list intact, where
//! [`json_ids`](crate::common::json_ids) — reproducing `JSONUtils` — discards
//! the whole thing. Two spellings of one operation, two answers.

use crate::common::enums::pay_period_type::PayPeriodType;
use crate::common::enums::shift_type::ShiftType;
use crate::common::numbers::round_hours;
use crate::entity::employee::Employee;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::config::{
    EARNING_TYPES, HOME_DEPT_ONLY, PerMonthOTHrsRuleConfig, month_key,
};
use crate::rules::algorithm::hoursdistribution::{ContractHrsRule, HoursDistributionRule};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::params::RuleParams;
use crate::rules::ports::{AssignmentPort, EmployeeEarningPort, EmployeeShiftPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Overtime against a monthly contract. `PerMonthOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PerMonthOTHrsRule<A: AssignmentPort, E: EmployeeEarningPort, S: EmployeeShiftPort> {
    jobs: A,
    earnings: E,
    shifts: S,
}

/// One month's running hours and overtime. Java's private inner
/// `MonthlyAccumulator`.
#[derive(Debug, Clone, Copy, Default)]
struct MonthlyAccumulator {
    hours: f64,
    ot: f64,
}

impl<A: AssignmentPort, E: EmployeeEarningPort, S: EmployeeShiftPort> PerMonthOTHrsRule<A, E, S> {
    /// Build the rule over the three lookups it needs.
    pub fn new(jobs: A, earnings: E, shifts: S) -> Self {
        Self {
            jobs,
            earnings,
            shifts,
        }
    }

    /// Whether the shift's job sits under the same parent as the employee's
    /// home job on that date. `homeDeptOnly`'s test.
    fn same_department(
        &self,
        time_card: &dyn TimeCard,
        shift_job_id: i32,
        date: LocalDate,
    ) -> bool {
        let home_job_id = time_card
            .employee()
            .and_then(|employee| employee.home_employee_job_status(date))
            .map(|status| status.job_id());

        let parent_of = |job_id: Option<i32>| {
            job_id
                .and_then(|id| self.jobs.find_by_id(id))
                .and_then(|job| job.parent_assignment_id())
        };

        parent_of(Some(shift_job_id)) == parent_of(home_job_id)
    }

    /// `createOrGetMonthlyAccumulator`'s seeding half.
    fn seed_month(
        &self,
        time_card: &dyn TimeCard,
        work_week: &DateRange,
        first_of_month: LocalDate,
        earning_type_ids: &[i32],
        home_dept_only: bool,
    ) -> MonthlyAccumulator {
        let mut accumulator = MonthlyAccumulator::default();
        let month = DateRange::new(first_of_month, end_of_month(first_of_month));
        // Runs backwards whenever the week starts on or before the first of
        // the month — see the module note.
        let shift_period = DateRange::new(first_of_month, work_week.start_date().minus_days(1));

        if time_card.dataset_start_date() <= first_of_month {
            self.seed_from_card(
                time_card,
                &mut accumulator,
                &month,
                &shift_period,
                earning_type_ids,
                home_dept_only,
            );
        } else {
            self.seed_from_ports(
                time_card,
                &mut accumulator,
                &month,
                &shift_period,
                earning_type_ids,
                home_dept_only,
            );
        }

        accumulator
    }

    /// `accumulateUsingDataset`.
    fn seed_from_card(
        &self,
        time_card: &dyn TimeCard,
        accumulator: &mut MonthlyAccumulator,
        month: &DateRange,
        shift_period: &DateRange,
        earning_type_ids: &[i32],
        home_dept_only: bool,
    ) {
        for earning in time_card.earnings() {
            if earning_type_ids.contains(&earning.earning_type_id())
                && month.contains_date(earning.earning_date())
            {
                accumulator.hours = round_hours(accumulator.hours + earning.hours());
            }
        }

        let regular_type_ids = time_card.regular_hours_distribution_type_ids();

        for shift in time_card.shifts() {
            for distribution in shift.hours_distributions() {
                if !shift_period.contains_date(distribution.date()) {
                    continue;
                }
                if home_dept_only
                    && !self.same_department(time_card, shift.job_id(), distribution.date())
                {
                    continue;
                }

                let is_regular = distribution
                    .hours_distribution_type_id()
                    .is_some_and(|id| regular_type_ids.contains(&id));

                if is_regular {
                    accumulator.hours =
                        round_hours(accumulator.hours + distribution.original_hours());
                } else {
                    accumulator.ot = round_hours(accumulator.ot + distribution.hours());
                }
            }
        }
    }

    /// `accumulateUsingDAOs`.
    fn seed_from_ports(
        &self,
        time_card: &dyn TimeCard,
        accumulator: &mut MonthlyAccumulator,
        month: &DateRange,
        shift_period: &DateRange,
        earning_type_ids: &[i32],
        home_dept_only: bool,
    ) {
        // Only time and attendance folds earnings in on this path.
        if !time_card.is_run_from_scheduling()
            && let Some(hours) = self.earnings.earning_hours_for_period_and_types(
                time_card.employee_id(),
                month,
                earning_type_ids,
            )
        {
            accumulator.hours = round_hours(accumulator.hours + hours);
        }

        let shift_type = if time_card.is_run_from_scheduling() {
            ShiftType::Schedule
        } else {
            ShiftType::Actual
        };

        if let Some(totals) = self.shifts.net_and_ot_hours_for_period(
            time_card.employee_id(),
            shift_period,
            shift_type,
            home_dept_only,
        ) {
            accumulator.hours = round_hours(accumulator.hours + totals.net_hours);
            accumulator.ot = round_hours(accumulator.ot + totals.ot_hours);
        }
    }
}

impl<A: AssignmentPort, E: EmployeeEarningPort, S: EmployeeShiftPort> ContractHrsRule
    for PerMonthOTHrsRule<A, E, S>
{
    fn contract_period(&self) -> PayPeriodType {
        PayPeriodType::Monthly
    }

    /// `getContractHours`, including the mid-month hire pro-ration.
    fn contract_hours(&self, employee: &Employee, date: LocalDate, params: &RuleParams) -> f64 {
        let monthly_contract = params.double_at(month_key(date));

        let first = date.with_day_of_month(1);
        let month = DateRange::new(first, end_of_month(first));

        let Some(hire_date) = employee.hire_date() else {
            return monthly_contract;
        };
        if !month.contains_date(hire_date) {
            return monthly_contract;
        }

        // Calendar days, and the per-day figure is rounded before it is
        // multiplied back up.
        let hours_per_day = round_hours(monthly_contract / month.len() as f64);
        let working_days = DateRange::new(hire_date, month.end_date());
        round_hours(hours_per_day * working_days.len() as f64)
    }
}

impl<A: AssignmentPort, E: EmployeeEarningPort, S: EmployeeShiftPort> HoursDistributionRule
    for PerMonthOTHrsRule<A, E, S>
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());
        let home_dept_only = params.bool_at(HOME_DEPT_ONLY);
        let earning_type_ids = earning_type_ids(&params);

        let Some(ot_type_id) = time_card.ot_hours_distribution_type_id() else {
            return;
        };
        let regular_type_ids = time_card.regular_hours_distribution_type_ids();

        let mut sorted_shifts = time_card.shift_indices_with_distributions_for_period(work_week);
        sorted_shifts.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

        let mut months: HashMap<LocalDate, MonthlyAccumulator> = HashMap::new();

        for shift_index in sorted_shifts {
            let shift_job_id = time_card.shifts()[shift_index].job_id();
            let mut created = Vec::new();

            for index in 0..time_card.shifts()[shift_index].hours_distributions().len() {
                let distribution = &time_card.shifts()[shift_index].hours_distributions()[index];
                let date = distribution.date();
                let original_hours = distribution.original_hours();

                // Java dereferences both job-status lookups unguarded; a
                // missing status is treated as exempt here (divergence 32).
                let not_exempt = time_card
                    .employee()
                    .and_then(|employee| employee.employee_job_status(shift_job_id, date))
                    .is_some_and(|status| status.pay_type().is_not_salaried_exempt());

                let in_scope = not_exempt
                    && (!home_dept_only || self.same_department(time_card, shift_job_id, date))
                    && distribution
                        .hours_distribution_type_id()
                        .is_some_and(|id| regular_type_ids.contains(&id))
                    && work_week.contains_date(date);

                if !in_scope {
                    continue;
                }

                let Some(employee) = time_card.employee() else {
                    continue;
                };
                let contract_hours = self.contract_hours(employee, date, &params);

                let first_of_month = date.with_day_of_month(1);
                let accumulator = match months.get(&first_of_month) {
                    Some(existing) => *existing,
                    None => {
                        let seeded = self.seed_month(
                            time_card,
                            work_week,
                            first_of_month,
                            &earning_type_ids,
                            home_dept_only,
                        );
                        months.insert(first_of_month, seeded);
                        seeded
                    }
                };
                let mut accumulator = accumulator;

                accumulator.hours = round_hours(accumulator.hours + original_hours);

                let mut distribution_ot = 0.0;
                if accumulator.hours > contract_hours {
                    distribution_ot = round_hours(
                        round_hours(accumulator.hours - contract_hours) - accumulator.ot,
                    );
                }

                if time_card.is_open_for_editing_on(date) && distribution_ot > 0.0 {
                    let shift = &mut time_card.shifts_mut()[shift_index];
                    created.push(create_premium_distribution(
                        &shift.hours_distributions()[index],
                        ot_type_id,
                        distribution_ot,
                        Some(rule_item.id()),
                    ));
                    let reduced =
                        round_hours(shift.hours_distributions()[index].hours() - distribution_ot);
                    shift.hours_distributions_mut()[index].set_hours(reduced);
                }

                accumulator.ot = round_hours(accumulator.ot + distribution_ot);
                months.insert(first_of_month, accumulator);
            }

            let shift = &mut time_card.shifts_mut()[shift_index];
            for distribution in created {
                shift.add_hours_distribution(distribution);
            }
        }
    }
}

/// The last day of the month `first_of_month` opens.
fn end_of_month(first_of_month: LocalDate) -> LocalDate {
    first_of_month.plus_months(1).minus_days(1)
}

/// `getEarningTypeIdsList` — the rule's own parse, which keeps whatever it read
/// before a bad element rather than discarding the list.
fn earning_type_ids(params: &RuleParams) -> Vec<i32> {
    let Some(value) = params.get(EARNING_TYPES) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(value) else {
        return Vec::new();
    };

    let mut ids = Vec::with_capacity(parsed.len());
    for element in parsed {
        match element.as_f64() {
            Some(number) => ids.push(number as i32),
            // The throw is caught outside the loop, so what is already in the
            // list survives.
            None => break,
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::ports::NetAndOtHours;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    pub(super) const HOME_JOB: i32 = 1;
    pub(super) const SECONDARY_JOB: i32 = 7;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;

    /// Jobs by id — the home job has no parent, the secondary one sits under
    /// assignment 45.
    pub(super) struct Jobs;

    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            match id {
                HOME_JOB => Some(Assignment::new(HOME_JOB, 1, "home", "H", None)),
                SECONDARY_JOB => Some(Assignment::new(SECONDARY_JOB, 1, "sec", "S", Some(45))),
                _ => None,
            }
        }
    }

    /// The two lookups that only matter before the dataset starts.
    pub(super) struct NoHistory;

    impl EmployeeEarningPort for NoHistory {
        fn save(&mut self, _earning: EmployeeEarning) {}
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            None
        }

        fn banked_rate_for_rule(
            &self,
            _employee_id: i32,
            _hours_earning_type_id: i32,
            _cost_earning_type_id: i32,
        ) -> Option<f64> {
            None
        }
    }

    impl EmployeeShiftPort for NoHistory {
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
            None
        }
    }

    pub(super) fn rule() -> PerMonthOTHrsRule<Jobs, NoHistory, NoHistory> {
        PerMonthOTHrsRule::new(Jobs, NoHistory, NoHistory)
    }

    pub(super) fn jan(day: u32) -> LocalDate {
        LocalDate::of(2016, 1, day.try_into().unwrap())
    }

    /// `new LegacyDatePeriod(2016-01-01, 2016-01-07)`.
    pub(super) fn work_week() -> DateRange {
        DateRange::new(jan(1), jan(7))
    }

    pub(super) fn employee() -> Employee {
        let status = |id: i32, job: i32, home: bool| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                jan(1).minus_days(365),
                jan(7),
                EmployeePayType::Hourly,
                0.0,
                home,
            )
        };

        Employee::new(
            1,
            1,
            "",
            vec![status(1, HOME_JOB, true), status(2, SECONDARY_JOB, false)],
        )
    }

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, date, Some(REGULAR), hours, 0.0)
    }

    pub(super) fn shift(
        id: i32,
        job_id: i32,
        shift_date: LocalDate,
        distributions: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        EmployeeShift::new(
            id,
            1,
            job_id,
            shift_date,
            crate::common::enums::shift_type::ShiftType::Actual,
            Vec::new(),
        )
        .with_hours_distributions(distributions)
    }

    /// The weekly pay group ending 2016-01-07, so the calculation opens on
    /// 01-01 and the dataset starts on 2015-12-22.
    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(jan(1))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(5, 1, "", RuleClass::PerMonthOtHdr, rule_params)
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

    pub(super) fn earning(hours: f64, date: LocalDate, type_id: i32) -> EmployeeEarning {
        EmployeeEarning::new(
            1,
            1,
            HOME_JOB,
            type_id,
            date,
            hours,
            0.0,
            EarningSource::Manual,
        )
    }

    #[test]
    fn the_contract_period_is_monthly() {
        assert_eq!(rule().contract_period(), PayPeriodType::Monthly);
    }

    #[test]
    fn a_full_month_contract_is_the_configured_figure() {
        let params = item(&[("JAN", "160.0")])
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());

        assert_eq!(rule().contract_hours(&employee(), jan(15), &params), 160.0);
    }

    #[test]
    fn a_mid_month_hire_is_pro_rated_by_calendar_days() {
        // 160 / 31 rounds to 5.16 an hour a day, and 16 days remain from the
        // 16th — so 82.56, not the 82.58 an unrounded rate would give.
        let hired = employee().with_dates(Some(jan(16)), None, None);
        let params = item(&[("JAN", "160.0")])
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());

        assert_eq!(rule().contract_hours(&hired, jan(20), &params), 82.56);
        assert_ne!(rule().contract_hours(&hired, jan(20), &params), 82.58);
    }

    #[test]
    fn a_hire_in_another_month_does_not_pro_rate() {
        let hired = employee().with_dates(Some(LocalDate::of(2015, 6, 1)), None, None);
        let params = item(&[("JAN", "160.0")])
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());

        assert_eq!(rule().contract_hours(&hired, jan(20), &params), 160.0);
    }

    #[test]
    fn each_month_reads_its_own_parameter() {
        let params = item(&[("JAN", "100.0"), ("FEB", "90.0")])
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());

        assert_eq!(rule().contract_hours(&employee(), jan(5), &params), 100.0);
        assert_eq!(
            rule().contract_hours(&employee(), LocalDate::of(2016, 2, 5), &params),
            90.0
        );
        assert_eq!(
            rule().contract_hours(&employee(), LocalDate::of(2016, 3, 5), &params),
            160.0,
            "the default"
        );
    }

    #[test]
    fn a_month_under_its_contract_pays_nothing() {
        let mut card = card(vec![shift(1, HOME_JOB, jan(1), vec![regular(jan(1), 8.0)])]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "160.0")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 8.0)]);
    }

    #[test]
    fn a_distribution_outside_the_work_week_is_skipped() {
        let mut card = card(vec![shift(
            1,
            HOME_JOB,
            jan(1),
            vec![regular(jan(1), 8.0), regular(jan(20), 50.0)],
        )]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 8.0), (jan(20), Some(REGULAR), 50.0)]
        );
    }

    #[test]
    fn the_seeding_window_is_inverted_for_a_week_starting_the_month() {
        // A prior shift inside the month contributes nothing, because the
        // seeding range runs from 01-01 back to 2015-12-31.
        let mut card = card(vec![
            shift(1, HOME_JOB, jan(1), vec![regular(jan(1), 9.0)]),
            shift(2, HOME_JOB, jan(2), vec![regular(jan(2), 9.0)]),
        ]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 9.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(2), Some(REGULAR), 1.0), (jan(2), Some(OVERTIME), 8.0)],
            "eighteen hours against a ten hour contract"
        );
    }

    #[test]
    fn a_salaried_exempt_shift_is_skipped() {
        let exempt = Employee::new(
            1,
            1,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                HOME_JOB,
                jan(1).minus_days(365),
                jan(7),
                EmployeePayType::SalariedExempt,
                0.0,
                true,
            )],
        );
        let mut card = card(vec![shift(
            1,
            HOME_JOB,
            jan(1),
            vec![regular(jan(1), 50.0)],
        )])
        .with_employee(exempt);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 50.0)]);
    }

    #[test]
    fn a_closed_day_is_left_alone_but_still_counts() {
        let mut card = card(vec![
            shift(1, HOME_JOB, jan(1), vec![regular(jan(1), 12.0)]),
            shift(2, HOME_JOB, jan(2), vec![regular(jan(2), 4.0)]),
        ])
        .with_calculation_start_date(jan(2));

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 12.0)],
            "closed"
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(2), Some(REGULAR), 0.0), (jan(2), Some(OVERTIME), 4.0)],
            "the two closed hours were still booked as owed"
        );
    }

    #[test]
    fn the_rules_own_json_parser_keeps_what_it_read_before_a_bad_element() {
        // Where `json_ids`, reproducing JSONUtils, would return nothing.
        let params = item(&[(EARNING_TYPES, "[3,4,{},5]")])
            .params()
            .fixed(&PerMonthOTHrsRuleConfig.default_values());

        assert_eq!(earning_type_ids(&params), vec![3, 4]);
        assert_eq!(
            crate::common::json_ids::id_list("[3,4,{},5]"),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn a_property_with_no_overtime_bucket_pays_nothing() {
        let mut card = card(vec![shift(
            1,
            HOME_JOB,
            jan(1),
            vec![regular(jan(1), 50.0)],
        )])
        .with_hours_distribution_types(vec![HoursDistributionType::new(REGULAR, "Regular", false)]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 50.0)]);
    }
}

/// `PerMonthOTHrsRuleImplTest.groovy`, transcribed.
///
/// All five cases. The mocked `PayGroup` becomes the card's calculation start
/// date (divergence 24). As in `RollingXWeeksOTHrsRuleImplTest`, the spec's
/// `any {}` blocks are four bare comparisons of which only the last is the
/// closure's return value — two of the five cases use `&&` and actually assert
/// the whole row, the other three do not. All are transcribed against the whole
/// distribution list.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        HOME_JOB, OVERTIME, REGULAR, SECONDARY_JOB, card, earning, item, jan, regular, rows, rule,
        shift, work_week,
    };
    use super::*;

    #[test]
    fn rule_gives_ot_when_hours_worked_in_excess_of_the_months_contract_are_worked() {
        let mut card = card(vec![
            shift(1, HOME_JOB, jan(1), vec![regular(jan(1), 10.0)]),
            shift(2, HOME_JOB, jan(3), vec![regular(jan(3), 5.0)]),
        ]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 0.0), (jan(3), Some(OVERTIME), 5.0)]
        );
    }

    #[test]
    fn rule_backfills_on_the_second_day_for_spanning_shifts() {
        let mut card = card(vec![
            shift(1, HOME_JOB, jan(2), vec![regular(jan(2), 8.0)]),
            shift(
                2,
                HOME_JOB,
                jan(3),
                vec![regular(jan(3), 4.0), regular(jan(4), 4.0)],
            ),
        ]);

        rule().execute(&mut card, &work_week(), &item(&[("JAN", "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(2), Some(REGULAR), 8.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (jan(3), Some(REGULAR), 2.0),
                (jan(4), Some(REGULAR), 0.0),
                (jan(3), Some(OVERTIME), 2.0),
                (jan(4), Some(OVERTIME), 4.0)
            ]
        );
    }

    #[test]
    fn home_dept_only_excludes_hours_worked_under_another_parent_assignment() {
        let mut card = card(vec![
            shift(1, HOME_JOB, jan(2), vec![regular(jan(2), 10.0)]),
            shift(2, SECONDARY_JOB, jan(3), vec![regular(jan(3), 5.0)]),
        ]);

        rule().execute(
            &mut card,
            &work_week(),
            &item(&[("JAN", "10"), (HOME_DEPT_ONLY, "true")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(2), Some(REGULAR), 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 5.0)],
            "the secondary job sits under a different parent"
        );
    }

    #[test]
    fn earnings_of_a_configured_type_count_toward_the_contract() {
        let mut card = card(vec![shift(
            1,
            HOME_JOB,
            jan(1),
            vec![regular(jan(1), 10.0)],
        )])
        .with_earnings(vec![earning(10.0, jan(1), 3)]);

        rule().execute(
            &mut card,
            &work_week(),
            &item(&[("JAN", "10"), (EARNING_TYPES, "[3]")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![(jan(1), Some(REGULAR), 0.0), (jan(1), Some(OVERTIME), 10.0)],
            "the ten earning hours fill the contract on their own"
        );
    }

    #[test]
    fn shift_hours_only_count_toward_the_month_their_distribution_falls_in() {
        let december = LocalDate::of(2015, 12, 31);
        let mut card = card(vec![
            shift(
                1,
                HOME_JOB,
                december,
                vec![regular(december, 5.0), regular(jan(1), 6.0)],
            ),
            shift(2, HOME_JOB, jan(3), vec![regular(jan(3), 8.0)]),
        ])
        // `workWeek = 2015-12-30..2016-01-05`, the pay period ending 01-05.
        .with_calculation_start_date(LocalDate::of(2015, 12, 30));

        rule().execute(
            &mut card,
            &DateRange::new(LocalDate::of(2015, 12, 30), jan(5)),
            &item(&[("DEC", "10"), ("JAN", "10"), (EARNING_TYPES, "[3]")]),
        );

        // Five December hours against a ten hour December contract, and six
        // January hours against a separate ten hour January one.
        assert_eq!(
            rows(&card, 0),
            vec![(december, Some(REGULAR), 5.0), (jan(1), Some(REGULAR), 6.0)]
        );
        // Fourteen January hours now, four of them over.
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), Some(REGULAR), 4.0), (jan(3), Some(OVERTIME), 4.0)]
        );
    }
}
