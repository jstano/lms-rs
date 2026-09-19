//! Port of `ContractOTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/ContractOTHrsRuleImpl.java`.
//!
//! `CONTRACT_OT_HDR`. Overtime past a **contracted** number of hours for the
//! pay period, rather than past a statutory weekly or daily limit: the property
//! sets weekly contract hours, a
//! [`ContractCalculator`](crate::common::contract) scales that to the period,
//! and everything above it is overtime. Worked holidays are carved out and paid
//! as double time instead.
//!
//! It is the only rule in the family that writes
//! [`CalcDataSetStat`](crate::entity::calc_data_set_stat) — three of the five
//! constants, one triple per pay period the week touches.
//!
//! # It is **not** a `ContractHrsRule`
//!
//! The name invites the assumption, and this file's own audit entry made it:
//! `ContractOTHrsRuleImpl implements HoursDistributionRuleImpl`, not
//! `ContractHrsRuleImpl`. `PerMonthOTHrsRuleImpl` is that interface's only
//! implementor, so `RuleUtils.getContractHours` still finds exactly one rule by
//! `instanceof`. This rule keeps its contract to itself.
//!
//! # The pay period is the unit, not the work week
//!
//! Each distribution is charged against an accumulator for **its own** pay
//! period, which is seeded with everything already worked in that period before
//! this work week. So the rule is run once per week of a period and each run
//! picks up where the last left off — the opposite of `PayPeriodOTHrs`, which
//! rescopes to the period and recalculates it whole.
//!
//! # Seeding reads two sources that disagree
//!
//! `createOrGetPayPeriodAccumulator` compares the period's start against
//! `getDatasetStartDate()`: inside the dataset the totals come off the card,
//! outside it from the DAOs. The same split `RollingXWeeksOTHrs` has, and the
//! two sides disagree here too:
//!
//! | | off the card | from the DAO |
//! |---|---|---|
//! | net hours | `hours` of **every** distribution, premium included | `originalHours` where type = 1 |
//! | overtime | `hours` of the overtime **and** double-time buckets | `hours` where type = 2, plus type = 3 |
//! | earnings | always folded in | only in `TA` mode |
//!
//! The card path counting premium rows into net hours is the sharp one: an
//! hour already paid as overtime is counted again toward the contract.
//!
//! # The seeding window is routinely inverted
//!
//! `shiftPeriod` is `[payPeriodStart, workWeek.getStartDate() - 1]`, which runs
//! backwards whenever the week starts on or before the period does — the first
//! week of every period. The card path's filters then match nothing and the
//! seeding silently contributes zero. Fourth inverted-range case in the family,
//! after `ConsecutiveDaysCalculator`'s, `PriorDaysCalculator`'s and
//! `PerMonthOTHrs`'s. The Java spec's earnings case depends on it: its shift
//! loop contributes nothing and only the earning seeds the period.
//!
//! # A worked holiday is paid whole, at double time
//!
//! A distribution dated on a configured holiday has **all** its hours moved to
//! double time and its own row zeroed — no limit is consulted, and the hours
//! are not tested against the contract. But they still *count*: the holiday
//! branch adds the original hours to the period total and the paid hours to the
//! period's overtime, so a holiday both fills the contract and is paid on top
//! of it.
//!
//! Note the branch is `hours > 0 && isEligibleForHoliday`, so a row already at
//! zero falls through to the overtime branch instead — which makes a second
//! pass over an already-calculated holiday behave differently from the first.
//!
//! # Both lists are sorted in place
//!
//! The shift list is sorted on a **copy** (`new ArrayList<>(...)`), but each
//! shift's own distribution list is sorted live, **ascending** by date — where
//! `TwentyFourHourOT` sorts the same list descending. Third in-place sort in the
//! family. Ascending is what makes a midnight-spanning shift fill its contract
//! from the earlier day first.

use crate::common::contract::ContractCalculator;
use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
use crate::common::enums::shift_type::ShiftType;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_hours;
use crate::entity::calc_data_set_stat::CalcDataSetStat;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    ContractOTHrsRuleConfig, EARNING_TYPES, HOLIDAY_TYPES_PROP, HOME_DEPT_ONLY,
    WEEKLY_CONTRACT_HOURS,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::ports::{AssignmentPort, EmployeeEarningPort, EmployeeShiftPort, HolidayPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Contract-based overtime, with worked holidays paid as double time.
/// `ContractOTHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContractOTHrsRule<S, E, H, A> {
    shifts: S,
    earnings: E,
    holidays: H,
    assignments: A,
}

impl<S, E, H, A> ContractOTHrsRule<S, E, H, A>
where
    S: EmployeeShiftPort,
    E: EmployeeEarningPort,
    H: HolidayPort,
    A: AssignmentPort,
{
    /// Build the rule over the four lookups it seeds and classifies with.
    ///
    /// Java autowires three DAOs and reaches assignments through the entity
    /// graph; the fourth is [`AssignmentPort`], which stands in for
    /// `shift.getJob().getParentAssignment()` — see the note on
    /// `same_department` below.
    pub fn new(shifts: S, earnings: E, holidays: H, assignments: A) -> Self {
        Self {
            shifts,
            earnings,
            holidays,
            assignments,
        }
    }
}

/// `ContractOTHrsRuleImpl.PayPeriodAccumulator`.
#[derive(Debug, Clone, Copy, Default)]
struct PayPeriodAccumulator {
    pay_period_hours: f64,
    pay_period_ot: f64,
    contract_hours: f64,
    earning_hours: f64,
}

/// The parameters and buckets every step needs.
struct Settings {
    home_dept_only: bool,
    earning_type_ids: Vec<i32>,
    holiday_type_ids: Vec<i32>,
    weekly_contract_hours: f64,
    overtime_bucket: Option<i32>,
    double_time_bucket: Option<i32>,
}

impl<S, E, H, A> HoursDistributionRule for ContractOTHrsRule<S, E, H, A>
where
    S: EmployeeShiftPort,
    E: EmployeeEarningPort,
    H: HolidayPort,
    A: AssignmentPort,
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        if time_card.calculation_mode() == EmployeeCalculationMode::AutoSchedule {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&ContractOTHrsRuleConfig.default_values());
        let settings = Settings {
            home_dept_only: params.bool_at(HOME_DEPT_ONLY),
            earning_type_ids: ids_for_key(EARNING_TYPES, &params),
            holiday_type_ids: ids_for_key(HOLIDAY_TYPES_PROP, &params),
            weekly_contract_hours: params.double_at(WEEKLY_CONTRACT_HOURS),
            overtime_bucket: time_card.ot_hours_distribution_type_id(),
            double_time_bucket: time_card.dt_hours_distribution_type_id(),
        };

        let mut shift_indices = time_card.shift_indices_with_distributions_for_period(work_week);
        shift_indices.sort_by_key(|&index| time_card.shifts()[index].start_time_for_ordering());

        let holidays_by_property = self.holidays_for_shifts(time_card, &shift_indices);
        let mut accumulators =
            self.seed_accumulators(time_card, work_week, &shift_indices, &settings);

        for shift_index in shift_indices {
            // `Collections.sort(shift.getHoursDistributions(), comparing(getDate))`
            // on the live list, ascending.
            time_card.shifts_mut()[shift_index]
                .hours_distributions_mut()
                .sort_by_key(|distribution| distribution.date());

            let eligible_for_ot = self.is_eligible_for_ot(
                time_card,
                &time_card.shifts()[shift_index],
                settings.home_dept_only,
            );

            let mut rows_to_add = Vec::new();

            for index in 0..time_card.shifts()[shift_index].hours_distributions().len() {
                let distribution = &time_card.shifts()[shift_index].hours_distributions()[index];
                let date = distribution.date();
                let hours = distribution.hours();

                if !(time_card.is_open_for_editing_on(date) && work_week.contains_date(date)) {
                    continue;
                }

                let period_start = time_card.pay_period_containing(date).start_date();
                let accumulator = accumulators.entry(period_start).or_default();

                let is_holiday = hours > 0.0
                    && is_eligible_for_holiday(
                        &holidays_by_property,
                        time_card.shifts()[shift_index].property_id(),
                        date,
                        &settings.holiday_type_ids,
                    );

                if is_holiday {
                    if let Some(row) = create_new_holiday_dt_distribution(
                        time_card,
                        shift_index,
                        index,
                        &settings,
                        rule_item.id(),
                        accumulator,
                        eligible_for_ot,
                    ) {
                        rows_to_add.push(row);
                    }
                } else if let Some(row) = eligible_for_ot
                    .then(|| {
                        create_new_contract_ot_distribution(
                            time_card,
                            shift_index,
                            index,
                            &settings,
                            rule_item.id(),
                            accumulator,
                        )
                    })
                    .flatten()
                {
                    rows_to_add.push(row);
                }
            }

            let shift = &mut time_card.shifts_mut()[shift_index];
            for row in rows_to_add {
                shift.add_hours_distribution(row);
            }
        }

        save_stats(time_card, &accumulators);
    }
}

impl<S, E, H, A> ContractOTHrsRule<S, E, H, A>
where
    S: EmployeeShiftPort,
    E: EmployeeEarningPort,
    H: HolidayPort,
    A: AssignmentPort,
{
    /// `holidayMap`, which Java caches on a `@Scope("request")` bean across the
    /// work weeks of one calculation.
    ///
    /// Built per `execute` here for the properties this week's shifts name —
    /// divergence 33's case again, and the same reasoning: the same DAO
    /// answers, and `execute` stays `&self`.
    fn holidays_for_shifts(
        &self,
        time_card: &dyn TimeCard,
        shift_indices: &[usize],
    ) -> HashMap<i32, Vec<crate::entity::holiday::Holiday>> {
        let mut by_property = HashMap::new();
        for &index in shift_indices {
            let property_id = time_card.shifts()[index].property_id();
            by_property
                .entry(property_id)
                .or_insert_with(|| self.holidays.find_all_for_property(property_id));
        }
        by_property
    }

    /// Seed one accumulator per pay period the work week's open distributions
    /// touch. `createOrGetPayPeriodAccumulator`, hoisted.
    ///
    /// **Divergence:** Java creates each accumulator lazily, the first time a
    /// distribution in that period is reached, and so interleaves seeding with
    /// writing. Seeding reads only earnings — which this rule never writes —
    /// and distributions dated strictly **before** the work week, while every
    /// write is guarded on `workWeek.containsDate`. The two windows are
    /// disjoint, so hoisting is behaviour-identical and lets `execute` hold
    /// `&mut` for the write loop without re-reading the card through it.
    ///
    /// `setContractHours` runs on every Java call rather than only on creation;
    /// it is the same value each time for a given period, so once is the same.
    fn seed_accumulators(
        &self,
        time_card: &dyn TimeCard,
        work_week: &DateRange,
        shift_indices: &[usize],
        settings: &Settings,
    ) -> HashMap<LocalDate, PayPeriodAccumulator> {
        let mut accumulators: HashMap<LocalDate, PayPeriodAccumulator> = HashMap::new();

        for &shift_index in shift_indices {
            for distribution in time_card.shifts()[shift_index].hours_distributions() {
                let date = distribution.date();
                if !(time_card.is_open_for_editing_on(date) && work_week.contains_date(date)) {
                    continue;
                }

                let pay_period = time_card.pay_period_containing(date);
                let period_start = pay_period.start_date();
                if accumulators.contains_key(&period_start) {
                    continue;
                }

                // `[payPeriodStartDate, workWeek.getStartDate() - 1]`, which is
                // inverted for any week starting on or before the period.
                let shift_period =
                    DateRange::new(period_start, work_week.start_date().minus_days(1));

                let mut accumulator = PayPeriodAccumulator::default();
                if time_card.dataset_start_date() <= period_start {
                    self.accumulate_using_dataset(
                        time_card,
                        &mut accumulator,
                        &pay_period,
                        &shift_period,
                        settings,
                    );
                } else {
                    self.accumulate_using_ports(
                        time_card,
                        &mut accumulator,
                        &pay_period,
                        &shift_period,
                        settings,
                    );
                }

                accumulator.contract_hours = ContractCalculator::for_property(
                    time_card.pay_period_type(),
                    time_card.schedule_mode(),
                )
                .contract_hours_for_pay_period(settings.weekly_contract_hours, &pay_period);

                accumulators.insert(period_start, accumulator);
            }
        }

        accumulators
    }

    /// `accumulateUsingDataset`.
    fn accumulate_using_dataset(
        &self,
        time_card: &dyn TimeCard,
        accumulator: &mut PayPeriodAccumulator,
        pay_period: &DateRange,
        shift_period: &DateRange,
        settings: &Settings,
    ) {
        for earning in time_card.earnings() {
            if settings
                .earning_type_ids
                .contains(&earning.earning_type_id())
                && pay_period.contains_date(earning.earning_date())
            {
                accumulator.pay_period_hours =
                    round_hours(accumulator.pay_period_hours + earning.hours());
                accumulator.earning_hours =
                    round_hours(accumulator.earning_hours + earning.hours());
            }
        }

        for shift in time_card.shifts() {
            if !self.is_eligible_for_ot(time_card, shift, settings.home_dept_only) {
                continue;
            }

            let in_period = || {
                shift
                    .hours_distributions()
                    .iter()
                    .filter(|d| shift_period.contains_date(d.date()))
            };

            let sum_of = |bucket: Option<i32>| -> f64 {
                bucket.map_or(0.0, |id| {
                    in_period()
                        .filter(|d| d.is_of_type(id))
                        .map(|d| d.hours())
                        .sum()
                })
            };

            let shift_ot_hours = sum_of(settings.overtime_bucket);
            let shift_dt_hours = sum_of(settings.double_time_bucket);
            // Every distribution, premium rows included — so hours already paid
            // as overtime are counted toward the contract a second time.
            let shift_net_hours: f64 = in_period().map(|d| d.hours()).sum();

            accumulator.pay_period_ot =
                round_hours(accumulator.pay_period_ot + shift_ot_hours + shift_dt_hours);
            accumulator.pay_period_hours =
                round_hours(accumulator.pay_period_hours + shift_net_hours);
        }
    }

    /// `accumulateUsingDAOs`.
    fn accumulate_using_ports(
        &self,
        time_card: &dyn TimeCard,
        accumulator: &mut PayPeriodAccumulator,
        pay_period: &DateRange,
        shift_period: &DateRange,
        settings: &Settings,
    ) {
        let calculation_mode = time_card.calculation_mode();
        let employee_id = time_card.employee_id();

        // Earnings are folded in only in TA mode, where the card path folds
        // them in regardless.
        if calculation_mode == EmployeeCalculationMode::Ta
            && let Some(earnings) = self.earnings.earning_hours_for_period_and_types(
                employee_id,
                pay_period,
                &settings.earning_type_ids,
            )
        {
            accumulator.pay_period_hours = round_hours(accumulator.pay_period_hours + earnings);
            accumulator.earning_hours = round_hours(accumulator.earning_hours + earnings);
        }

        let shift_type = if calculation_mode == EmployeeCalculationMode::Ta {
            ShiftType::Actual
        } else {
            ShiftType::Schedule
        };

        if let Some(totals) = self.shifts.net_and_ot_hours_for_period(
            employee_id,
            shift_period,
            shift_type,
            settings.home_dept_only,
        ) {
            accumulator.pay_period_hours =
                round_hours(accumulator.pay_period_hours + totals.net_hours);
            // Two separate `+=` in Java, so two separate roundings.
            accumulator.pay_period_ot = round_hours(accumulator.pay_period_ot + totals.ot_hours);
            accumulator.pay_period_ot = round_hours(accumulator.pay_period_ot + totals.dt_hours);
        }
    }

    /// `isEligibleForOT`.
    ///
    /// **Divergence:** Java resolves the employee's home job **before** testing
    /// `homeDeptOnly`, so an employee with no home job status on the shift's
    /// date throws even when the flag is off and the home job is irrelevant.
    /// The lookup happens only when the flag is set here, which is divergence
    /// 39's reading: an absent record establishes nothing, so with the flag off
    /// the shift is calculated normally and with it on the department match
    /// cannot be shown and the shift is excluded.
    fn is_eligible_for_ot(
        &self,
        time_card: &dyn TimeCard,
        shift: &EmployeeShift,
        home_dept_only: bool,
    ) -> bool {
        if !time_card.shift_is_not_salaried_exempt(shift) {
            return false;
        }
        if !home_dept_only {
            return true;
        }

        self.same_department(time_card, shift)
    }

    /// `shiftJob.getParentAssignment() == homeJob.getParentAssignment()`.
    ///
    /// A **reference** comparison in Java, over entities the shift and the home
    /// job status reach directly. The shift carries only a job id here, so the
    /// parents come back through [`AssignmentPort`] and are compared by id.
    ///
    /// Two jobs that both have no parent compare equal in Java (`null == null`)
    /// and equal here (`None == None`). A job the port cannot find also reads
    /// as `None`, which conflates "no parent" with "no such job" — Java would
    /// have thrown on the latter.
    fn same_department(&self, time_card: &dyn TimeCard, shift: &EmployeeShift) -> bool {
        let parent_of = |job_id: i32| {
            self.assignments
                .find_by_id(job_id)
                .and_then(|assignment| assignment.parent_assignment_id())
        };

        let Some(home_status) = time_card
            .employee()
            .and_then(|employee| employee.home_employee_job_status(shift.shift_date()))
        else {
            return false;
        };

        parent_of(shift.job_id()) == parent_of(home_status.job_id())
    }
}

/// `isEligibleForHoliday`.
///
/// The calendar consulted is the **shift's** property, through
/// `shift.getJob().getProperty()` — where `HolidayDTHrs` consults the
/// *distribution's* property. Two rules in one family, two answers for a
/// multi-property employee.
fn is_eligible_for_holiday(
    holidays_by_property: &HashMap<i32, Vec<crate::entity::holiday::Holiday>>,
    property_id: i32,
    date: LocalDate,
    holiday_type_ids: &[i32],
) -> bool {
    holidays_by_property
        .get(&property_id)
        .is_some_and(|holidays| {
            holidays
                .iter()
                .filter(|holiday| holiday_type_ids.contains(&holiday.holiday_type_id()))
                .any(|holiday| holiday.holiday_date() == date)
        })
}

/// `createNewHolidayDTDistribution` — the whole row moves to double time.
fn create_new_holiday_dt_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    index: usize,
    settings: &Settings,
    rule_item_id: i32,
    accumulator: &mut PayPeriodAccumulator,
    eligible_for_ot: bool,
) -> Option<crate::entity::hours_distribution::HoursDistribution> {
    let shift = &mut time_card.shifts_mut()[shift_index];
    let distribution = &shift.hours_distributions()[index];

    if eligible_for_ot {
        accumulator.pay_period_hours =
            round_hours(accumulator.pay_period_hours + distribution.original_hours());
        accumulator.pay_period_ot = round_hours(accumulator.pay_period_ot + distribution.hours());
    }

    // Divergence 37: with no double-time bucket there is nowhere to put the
    // hours, so the row is left alone rather than zeroed into nothing.
    let bucket = settings.double_time_bucket?;

    let row = create_premium_distribution(
        distribution,
        bucket,
        distribution.hours(),
        Some(rule_item_id),
    );
    shift.hours_distributions_mut()[index].set_hours(0.0);

    Some(row)
}

/// `createNewContractOTDistribution` — everything past the contract.
fn create_new_contract_ot_distribution(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    index: usize,
    settings: &Settings,
    rule_item_id: i32,
    accumulator: &mut PayPeriodAccumulator,
) -> Option<crate::entity::hours_distribution::HoursDistribution> {
    let shift = &mut time_card.shifts_mut()[shift_index];
    let distribution = &shift.hours_distributions()[index];

    accumulator.pay_period_hours =
        round_hours(accumulator.pay_period_hours + distribution.original_hours());

    let mut distribution_ot_hours = 0.0;
    if accumulator.pay_period_hours > accumulator.contract_hours {
        // Rounded twice: the excess, then the excess net of what is already paid.
        distribution_ot_hours = round_hours(
            round_hours(accumulator.pay_period_hours - accumulator.contract_hours)
                - accumulator.pay_period_ot,
        );
    }
    distribution_ot_hours = distribution_ot_hours.max(0.0);
    accumulator.pay_period_ot = round_hours(accumulator.pay_period_ot + distribution_ot_hours);

    if distribution_ot_hours <= 0.0 {
        return None;
    }

    let bucket = settings.overtime_bucket?;

    let row = create_premium_distribution(
        distribution,
        bucket,
        distribution_ot_hours,
        Some(rule_item_id),
    );
    let reduced = round_hours(distribution.hours() - distribution_ot_hours);
    shift.hours_distributions_mut()[index].set_hours(reduced);

    Some(row)
}

/// `saveStats` — three statistics per pay period, keyed by its start date.
fn save_stats(
    time_card: &mut dyn TimeCard,
    accumulators: &HashMap<LocalDate, PayPeriodAccumulator>,
) {
    for (&period_start, accumulator) in accumulators {
        let stats = time_card.stat_map_mut();

        stats.insert(
            (
                CalcDataSetStat::GapToContractByPeriodStartDate,
                period_start,
            ),
            round_hours(accumulator.contract_hours - accumulator.pay_period_hours),
        );
        stats.insert(
            (
                CalcDataSetStat::PeriodTotalHoursByPeriodStartDate,
                period_start,
            ),
            round_hours(accumulator.pay_period_hours),
        );
        stats.insert(
            (
                CalcDataSetStat::PeriodTotalEarningHoursByPeriodStartDate,
                period_start,
            ),
            round_hours(accumulator.earning_hours),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::pay_period_type::PayPeriodType;
    use crate::common::enums::schedule_mode::ScheduleMode;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::holiday::Holiday;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::ports::NetAndOtHours;
    use crate::rules::rule_class::RuleClass;

    pub(super) const PROPERTY: i32 = 3;
    pub(super) const JOB: i32 = 2;
    pub(super) const DEPT: i32 = 11;
    pub(super) const SECONDARY_JOB: i32 = 7;
    pub(super) const SECONDARY_DEPT: i32 = 45;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// `jan(1)` is 2016-01-01, the work week's first day.
    pub(super) fn jan(day: i64) -> LocalDate {
        LocalDate::of(2016, 1, 1).plus_days(day - 1)
    }

    pub(super) fn week() -> DateRange {
        DateRange::new(jan(1), jan(7))
    }

    /// `Mock(EmployeeShiftDAO)` — the spec never reaches it, because the
    /// dataset always starts before the pay period.
    #[derive(Debug, Default, Clone, Copy)]
    pub(super) struct Shifts(pub Option<NetAndOtHours>);

    impl EmployeeShiftPort for Shifts {
        fn net_actual_hours_in_period(&self, _employee_id: i32, _period: &DateRange) -> f64 {
            0.0
        }
        fn is_scheduled(
            &self,
            _employee_id: i32,
            _date_time: joda_rs::LocalDateTime,
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

    /// `Mock(EmployeeEarningDAO)`.
    #[derive(Debug, Default, Clone, Copy)]
    pub(super) struct Earnings(pub Option<f64>);

    impl EmployeeEarningPort for Earnings {
        fn save(&mut self, _earning: EmployeeEarning) {}
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            self.0
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

    /// `Mock(HolidayDAO) { findAllForProperty(property) >> [...] }`.
    #[derive(Debug, Default, Clone)]
    pub(super) struct Holidays(pub Vec<Holiday>);

    impl HolidayPort for Holidays {
        fn find_all_for_property(&self, _property_id: i32) -> Vec<Holiday> {
            self.0.clone()
        }
    }

    /// The job graph Java reaches through the entity references.
    #[derive(Debug, Default, Clone, Copy)]
    pub(super) struct Assignments;

    impl AssignmentPort for Assignments {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            let parent = match id {
                JOB => Some(DEPT),
                SECONDARY_JOB => Some(SECONDARY_DEPT),
                _ => return None,
            };
            Some(Assignment::new(id, PROPERTY, "", "", parent))
        }
    }

    pub(super) type Rule = ContractOTHrsRule<Shifts, Earnings, Holidays, Assignments>;

    pub(super) fn rule_with(holidays: Vec<Holiday>) -> Rule {
        ContractOTHrsRule::new(
            Shifts(None),
            Earnings(None),
            Holidays(holidays),
            Assignments,
        )
    }

    pub(super) fn rule() -> Rule {
        rule_with(Vec::new())
    }

    pub(super) fn job_status(job_id: i32, home: bool) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            1,
            job_id,
            jan(1),
            jan(7),
            EmployeePayType::Hourly,
            0.0,
            home,
        )
    }

    pub(super) fn employee() -> Employee {
        Employee::new(1, PROPERTY, "", vec![job_status(JOB, true)])
    }

    pub(super) fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(PROPERTY, date, Some(REGULAR), hours, 0.0)
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
        .with_property_id(PROPERTY)
        .with_hours_distributions(distributions)
    }

    /// The `setup()` card: a weekly pay period running the work week, a weekly
    /// schedule mode, and the system-generated buckets.
    ///
    /// The mocked `PayGroup` is divergence 24's derivation, so the calculation
    /// start date is set directly; it is the current period's start, 2016-01-01,
    /// which leaves every day of the week open.
    pub(super) fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_current_pay_period(week())
            .with_contract_periods(PayPeriodType::Weekly, ScheduleMode::Weekly)
            .with_calculation_start_date(jan(1))
    }

    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(5, 1, "", RuleClass::ContractOtHdr, rule_params)
    }

    /// Every distribution on a shift as `(date, type, hours, original_hours)`.
    pub(super) fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, i32, f64, f64)> {
        let mut rows: Vec<(LocalDate, i32, f64, f64)> = card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| {
                (
                    d.date(),
                    d.hours_distribution_type_id().unwrap_or(0),
                    d.hours(),
                    d.original_hours(),
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        rows
    }

    pub(super) fn stat(card: &TimeCardData, stat: CalcDataSetStat, date: LocalDate) -> Option<f64> {
        card.stat_map().get(&(stat, date)).copied()
    }

    #[test]
    fn a_week_inside_the_contract_pays_nothing() {
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 8.0)])]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(WEEKLY_CONTRACT_HOURS, "39.0")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(1), REGULAR, 8.0, 8.0)]);
    }

    #[test]
    fn the_stats_are_written_once_per_pay_period() {
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 8.0)])]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_eq!(
            stat(
                &card,
                CalcDataSetStat::GapToContractByPeriodStartDate,
                jan(1)
            ),
            Some(2.0),
            "ten contracted less eight worked"
        );
        assert_eq!(
            stat(
                &card,
                CalcDataSetStat::PeriodTotalHoursByPeriodStartDate,
                jan(1)
            ),
            Some(8.0)
        );
        assert_eq!(
            stat(
                &card,
                CalcDataSetStat::PeriodTotalEarningHoursByPeriodStartDate,
                jan(1)
            ),
            Some(0.0)
        );
    }

    #[test]
    fn auto_schedule_returns_before_anything_is_written() {
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 20.0)])])
            .with_calculation_mode(EmployeeCalculationMode::AutoSchedule);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), REGULAR, 20.0, 20.0)]);
        assert!(card.stat_map().is_empty(), "not even the stats");
    }

    #[test]
    fn a_distribution_outside_the_work_week_is_skipped() {
        // The guard is `isOpenForEditingOn && workWeek.containsDate`, so a
        // spanning shift's hours on the next week are left for that week's run.
        let mut card = card(vec![shift(
            1,
            JOB,
            jan(7),
            vec![regular(jan(7), 8.0), regular(jan(8), 8.0)],
        )]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "4")]));

        assert_eq!(
            rows(&card, 0),
            vec![
                (jan(7), REGULAR, 4.0, 8.0),
                (jan(7), OVERTIME, 4.0, 0.0),
                (jan(8), REGULAR, 8.0, 8.0),
            ]
        );
    }

    #[test]
    fn a_salaried_exempt_shift_is_not_eligible() {
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 20.0)])])
            .with_employee(Employee::new(
                1,
                PROPERTY,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    1,
                    JOB,
                    jan(1),
                    jan(7),
                    EmployeePayType::SalariedExempt,
                    0.0,
                    true,
                )],
            ));

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), REGULAR, 20.0, 20.0)]);
    }

    #[test]
    fn the_distribution_list_is_left_sorted_ascending_by_date() {
        // Sorted in place, whether or not anything is paid — and the opposite
        // direction from TwentyFourHourOT's.
        let mut card = card(vec![shift(
            1,
            JOB,
            jan(3),
            vec![regular(jan(4), 4.0), regular(jan(3), 4.0)],
        )]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "39")]));

        let dates: Vec<LocalDate> = card.shifts()[0]
            .hours_distributions()
            .iter()
            .map(|d| d.date())
            .collect();
        assert_eq!(dates, vec![jan(3), jan(4)]);
    }

    #[test]
    fn a_holiday_row_already_at_zero_falls_through_to_the_overtime_branch() {
        // The holiday branch is guarded on `hours > 0`, so a second pass over a
        // calculated holiday takes the other branch.
        let holidays = vec![Holiday::new(1, PROPERTY, jan(2), "", 3)];
        let mut card = card(vec![shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)])]);
        let params = item(&[(HOLIDAY_TYPES_PROP, "[3]"), (WEEKLY_CONTRACT_HOURS, "4")]);

        rule_with(holidays.clone()).execute(&mut card, &week(), &params);
        assert_eq!(
            rows(&card, 0),
            vec![(jan(2), REGULAR, 0.0, 8.0), (jan(2), DOUBLE_TIME, 8.0, 0.0)]
        );

        // Second pass: the regular row is at zero, so it is measured against
        // the contract instead — and the accumulator starts fresh.
        rule_with(holidays).execute(&mut card, &week(), &params);
        assert_eq!(
            stat(
                &card,
                CalcDataSetStat::PeriodTotalHoursByPeriodStartDate,
                jan(1)
            ),
            Some(8.0),
            "the zeroed regular row still carries its original hours"
        );
    }

    #[test]
    fn a_property_with_no_double_time_bucket_leaves_the_holiday_alone() {
        let holidays = vec![Holiday::new(1, PROPERTY, jan(2), "", 3)];
        let mut card = card(vec![shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)])])
            .with_hours_distribution_types(vec![
                HoursDistributionType::new(REGULAR, "Regular", false),
                HoursDistributionType::new(OVERTIME, "Overtime", true),
            ]);

        rule_with(holidays).execute(
            &mut card,
            &week(),
            &item(&[(HOLIDAY_TYPES_PROP, "[3]"), (WEEKLY_CONTRACT_HOURS, "4")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(2), REGULAR, 8.0, 8.0)]);
    }

    #[test]
    fn a_holiday_of_an_unconfigured_type_is_ordinary_work() {
        let holidays = vec![Holiday::new(1, PROPERTY, jan(2), "", 99)];
        let mut card = card(vec![shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)])]);

        rule_with(holidays).execute(
            &mut card,
            &week(),
            &item(&[(HOLIDAY_TYPES_PROP, "[3]"), (WEEKLY_CONTRACT_HOURS, "4")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![(jan(2), REGULAR, 4.0, 8.0), (jan(2), OVERTIME, 4.0, 0.0)]
        );
    }

    #[test]
    fn the_ports_seed_the_period_when_it_starts_before_the_dataset() {
        // `getDatasetStartDate()` after the period start sends seeding to the
        // DAOs. In production the gap is the ten days divergence 24 describes;
        // here it is set directly, so the day stays open for editing.
        let mut card = card(vec![shift(1, JOB, jan(5), vec![regular(jan(5), 4.0)])])
            .with_dataset_start_date(jan(3));

        let rule = ContractOTHrsRule::new(
            Shifts(Some(NetAndOtHours {
                net_hours: 20.0,
                ot_hours: 1.0,
                dt_hours: 2.0,
            })),
            Earnings(Some(6.0)),
            Holidays(Vec::new()),
            Assignments,
        );
        rule.execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "25")]));

        // 6 earning + 20 net seeded, then 4 worked: 30 against a 25 contract is
        // 5 over, less the 3 of overtime the DAO already reported.
        assert_eq!(
            rows(&card, 0),
            vec![(jan(5), REGULAR, 2.0, 4.0), (jan(5), OVERTIME, 2.0, 0.0)]
        );
        assert_eq!(
            stat(
                &card,
                CalcDataSetStat::PeriodTotalEarningHoursByPeriodStartDate,
                jan(1)
            ),
            Some(6.0)
        );
    }

    #[test]
    fn the_dao_path_counts_double_time_as_overtime_already_paid() {
        // RollingXWeeksOTHrs reads the same query and ignores the third column.
        let seeded = |dt_hours: f64| {
            let mut card = card(vec![shift(1, JOB, jan(5), vec![regular(jan(5), 4.0)])])
                .with_dataset_start_date(jan(3));
            ContractOTHrsRule::new(
                Shifts(Some(NetAndOtHours {
                    net_hours: 24.0,
                    ot_hours: 0.0,
                    dt_hours,
                })),
                Earnings(None),
                Holidays(Vec::new()),
                Assignments,
            )
            .execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "25")]));
            card
        };

        assert_eq!(rows(&seeded(0.0), 0)[1].2, 3.0, "28 hours, 25 contracted");
        assert_eq!(
            rows(&seeded(2.0), 0)[1].2,
            1.0,
            "two of those three are already paid as double time"
        );
    }
}

/// `ContractOTHrsRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/ContractOTHrsRuleImplTest.groovy`
/// — all six cases, each asserting every distribution on every shift plus the
/// three statistics for the pay period.
///
/// The mocked `PayGroup` is divergence 24's derivation, so the calculation
/// start date and the current pay period are set on the card directly; the
/// property's `payPeriodType` and `scheduleMode` come across as card accessors
/// for the same reason. Neither DAO is reached: the fixture's dataset always
/// begins before the pay period, so every case takes the card seeding path.
///
/// # Three of the six cases assert almost nothing
///
/// `rule gives ot when hours are worked in excess of the contract hours`,
/// `…but does not count non home job dept hours…` and `…including earnings…`
/// write their `any {}` predicates as four bare comparisons on consecutive
/// lines with no `&&`, so only the **last** is the closure's return value. Each
/// reduces to "some distribution is of this type", with the hours, date and
/// original hours evaluated and discarded. That is the sixth spec-weakness of
/// this kind in the family, after `RollingXWeeksOTHrs`'s, the one-sided
/// tolerance in `CaliforniaExtendedOTHrsRuleImplTest`, the never-reached branch
/// in `PayPeriodOTHrsRuleImplTest`, and the two weak `CaliforniaOTHrs` cases.
///
/// **Two of those discarded lines are wrong.** Both cases assert
/// `hd.originalHours == 10` on a row they also assert is of the *overtime*
/// type, and a factory-built premium row carries `originalHours` of **zero** —
/// the finding this file already records against
/// `HoursDistributionFactory.createPremiumDistribution`. The predicates would
/// fail if they were joined with `&&`. The transcriptions assert the whole row,
/// with the original hours the factory actually writes.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        DOUBLE_TIME, JOB, OVERTIME, PROPERTY, REGULAR, SECONDARY_JOB, card, item, jan, job_status,
        regular, rows, rule, rule_with, shift, stat, week,
    };
    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::holiday::Holiday;
    use crate::entity::time_card::TimeCardData;

    /// The three statistics for the week's one pay period.
    fn assert_stats(card: &TimeCardData, gap: f64, period_hours: f64, earning_hours: f64) {
        assert_eq!(
            (
                stat(
                    card,
                    CalcDataSetStat::GapToContractByPeriodStartDate,
                    jan(1)
                ),
                stat(
                    card,
                    CalcDataSetStat::PeriodTotalHoursByPeriodStartDate,
                    jan(1)
                ),
                stat(
                    card,
                    CalcDataSetStat::PeriodTotalEarningHoursByPeriodStartDate,
                    jan(1)
                ),
            ),
            (Some(gap), Some(period_hours), Some(earning_hours))
        );
    }

    #[test]
    fn rule_gives_ot_when_hours_are_worked_in_excess_of_the_contract_hours() {
        let mut card = card(vec![
            shift(1, JOB, jan(1), vec![regular(jan(1), 10.0)]),
            shift(2, JOB, jan(3), vec![regular(jan(3), 5.0)]),
        ]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(1), REGULAR, 10.0, 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (jan(3), REGULAR, 0.0, 5.0),
                // The Groovy claims `originalHours == 5` here, in a line its
                // predicate discards. A premium row carries zero.
                (jan(3), OVERTIME, 5.0, 0.0),
            ]
        );

        assert_stats(&card, -5.0, 15.0, 0.0);
    }

    #[test]
    fn ot_starts_backfilling_on_the_second_day_for_spanning_shifts() {
        let mut card = card(vec![
            shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)]),
            shift(
                2,
                JOB,
                jan(3),
                vec![regular(jan(3), 4.0), regular(jan(4), 4.0)],
            ),
        ]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_eq!(rows(&card, 0), vec![(jan(2), REGULAR, 8.0, 8.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (jan(3), REGULAR, 2.0, 4.0),
                (jan(3), OVERTIME, 2.0, 0.0),
                (jan(4), REGULAR, 0.0, 4.0),
                (jan(4), OVERTIME, 4.0, 0.0),
            ]
        );

        assert_stats(&card, -6.0, 16.0, 0.0);
    }

    #[test]
    fn worked_hours_on_holidays_are_paid_as_all_dt_and_do_not_count_toward_the_contract() {
        let mut card = card(vec![
            shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)]),
            shift(2, JOB, jan(3), vec![regular(jan(3), 5.0)]),
            shift(3, JOB, jan(4), vec![regular(jan(4), 4.0)]),
        ]);

        rule_with(vec![Holiday::new(1, PROPERTY, jan(3), "", 3)]).execute(
            &mut card,
            &week(),
            &item(&[(HOLIDAY_TYPES_PROP, "[3]"), (WEEKLY_CONTRACT_HOURS, "11")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(2), REGULAR, 8.0, 8.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), REGULAR, 0.0, 5.0), (jan(3), DOUBLE_TIME, 5.0, 0.0),]
        );
        assert_eq!(
            rows(&card, 2),
            vec![(jan(4), REGULAR, 3.0, 4.0), (jan(4), OVERTIME, 1.0, 0.0)]
        );

        // The holiday's hours did count toward the period total — the title
        // says otherwise, but the branch adds them before paying them.
        assert_stats(&card, -6.0, 17.0, 0.0);
    }

    #[test]
    fn a_holiday_shift_that_spans_the_day_cut_pays_dt_only_for_the_holiday_half() {
        let mut card = card(vec![
            shift(1, JOB, jan(2), vec![regular(jan(2), 8.0)]),
            shift(
                2,
                JOB,
                jan(3),
                vec![regular(jan(3), 4.0), regular(jan(4), 4.0)],
            ),
        ]);

        // No WEEKLY_CONTRACT_HOURS, so the 39-hour default applies and nothing
        // reaches the contract.
        rule_with(vec![Holiday::new(1, PROPERTY, jan(3), "", 3)]).execute(
            &mut card,
            &week(),
            &item(&[(HOLIDAY_TYPES_PROP, "[3]")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(2), REGULAR, 8.0, 8.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![
                (jan(3), REGULAR, 0.0, 4.0),
                (jan(3), DOUBLE_TIME, 4.0, 0.0),
                (jan(4), REGULAR, 4.0, 4.0),
            ]
        );

        assert_stats(&card, 23.0, 16.0, 0.0);
    }

    #[test]
    fn home_dept_only_does_not_count_hours_worked_in_another_department() {
        let mut card = card(vec![
            shift(1, JOB, jan(2), vec![regular(jan(2), 10.0)]),
            shift(2, SECONDARY_JOB, jan(3), vec![regular(jan(3), 5.0)]),
        ])
        .with_employee(Employee::new(
            1,
            PROPERTY,
            "",
            vec![job_status(JOB, true), job_status(SECONDARY_JOB, false)],
        ));

        rule().execute(
            &mut card,
            &week(),
            &item(&[(WEEKLY_CONTRACT_HOURS, "10"), (HOME_DEPT_ONLY, "true")]),
        );

        assert_eq!(rows(&card, 0), vec![(jan(2), REGULAR, 10.0, 10.0)]);
        assert_eq!(
            rows(&card, 1),
            vec![(jan(3), REGULAR, 5.0, 5.0)],
            "a different parent assignment, so not eligible at all"
        );

        assert_stats(&card, 0.0, 10.0, 0.0);
    }

    #[test]
    fn earnings_of_a_configured_type_count_toward_the_contract() {
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 10.0)])])
            .with_earnings(vec![EmployeeEarning::new(
                1,
                1,
                JOB,
                3,
                jan(1),
                10.0,
                0.0,
                crate::common::enums::earning_source::EarningSource::Auto,
            )]);

        rule().execute(
            &mut card,
            &week(),
            &item(&[(WEEKLY_CONTRACT_HOURS, "10"), (EARNING_TYPES, "[3]")]),
        );

        assert_eq!(
            rows(&card, 0),
            vec![
                (jan(1), REGULAR, 0.0, 10.0),
                // The Groovy claims `originalHours == 10` here too.
                (jan(1), OVERTIME, 10.0, 0.0),
            ]
        );

        assert_stats(&card, -10.0, 20.0, 10.0);
    }

    #[test]
    fn the_seeding_window_is_inverted_for_a_week_starting_the_period() {
        // Not a Java case: the one above passes only because `shiftPeriod` runs
        // from the period start back to the day before the week, which is
        // backwards here, so its shift loop contributes nothing and the earning
        // is the whole seed. A shift dated inside that window is ignored.
        let mut card = card(vec![shift(1, JOB, jan(1), vec![regular(jan(1), 10.0)])]);

        rule().execute(&mut card, &week(), &item(&[(WEEKLY_CONTRACT_HOURS, "10")]));

        assert_stats(&card, 0.0, 10.0, 0.0);
    }
}
