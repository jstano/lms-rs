//! Port of `MinHrsForFullTimeOTRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/MinHrsForFullTimeOTRuleImpl.java`.
//!
//! `MHFFTOT_HDR`. The rule that asks **how much of the week the employee
//! worked** before deciding what to pay: at or under the part-time eligibility
//! limit every premium hour is **double time**, above it the ordinary daily and
//! weekly overtime limits apply. It is the only rule in the family that reads
//! [`EarningMapper`](super::earning_mapper) and
//! [`ConsecutiveDaysCalculator`](super::consecutive_days_calculator).
//!
//! # Three ladders, checked in order
//!
//! Every chunk of hours — a distribution or an earning — runs the same ladder:
//!
//! 1. **consecutive days**: if consecutive-day overtime is enabled and the
//!    counter has reached its limit, the whole chunk is overtime;
//! 2. **part time**: otherwise, if the week's total is at or under the
//!    eligibility limit and there is unpaid double time left in the budget, the
//!    chunk is double time up to that budget;
//! 3. **limits**: otherwise the larger of the day's unallocated overtime and
//!    the week's.
//!
//! The part-time test is `eligibilityLimit >= totalWeeklyHours`, and
//! `totalWeeklyHours` is the **whole week**, computed up front from every
//! regular distribution and every configured earning. So a part-timer's Monday
//! is paid differently depending on whether they pick up a shift on Saturday.
//!
//! # `maxDTPaid` is a weekly budget shared with overtime
//!
//! `calculateUnallocatedDT` is
//! `min(workedHours - premiumHoursAllocated, maxPremiumPaid - premiumHoursAllocated)`,
//! and **every** premium hour the rule writes — consecutive-day overtime and
//! daily or weekly overtime included — is added to `premiumHoursAllocated`. So
//! overtime paid on a consecutive day eats into the double-time budget; the
//! Java spec has a case named for exactly that.
//!
//! # `bothConsecutiveAndWeeklyOt` selects a different weekly formula
//!
//! Set, `calculateWeeklyOT` answers `min(workedHours - limit, currentChunkHours)`
//! — the week's whole excess, capped at this chunk, **ignoring what has already
//! been allocated**. Clear, it answers `max(0, workedHours - limit -
//! otHoursAllocated)`. So with the flag set an hour already paid as
//! consecutive-day overtime is paid again as weekly overtime; the spec's last
//! two cases are the two sides of that, and the second calls the unset
//! behaviour "legacy".
//!
//! This is the same parameter `CaliforniaExtendedOTHrs` reads and hands to an
//! accumulator field nothing reads. Here it does something.
//!
//! # A shift is processed once per distribution it has on the day
//!
//! `dailyShiftMap` is built by flat-mapping shifts to their distributions,
//! grouping by **distribution date**, and mapping back to the shift — so a
//! shift with two distributions dated the same day appears in that day's list
//! **twice**, and its whole `computeShiftHours` runs twice: its earnings are
//! counted twice and its distributions for that date are walked twice. Faithful
//! and reproduced; pinned by
//! `a_shift_with_two_distributions_on_one_day_is_processed_twice`.
//!
//! # A branch that cannot be taken
//!
//! ```java
//! .filter(distribution -> dataset.isOpenForEditingOn(distribution.getDate()))
//! …
//! if (dataset.distributionIsPremium(distribution) && !dataset.isOpenForEditingOn(distribution.getDate())) {
//! ```
//!
//! The stream has already kept only open dates, so the `!isOpenForEditingOn`
//! arm is dead. Not ported; the premium-hours bookkeeping it would have done
//! never happens in Java either.
//!
//! # It does not filter salaried-exempt shifts
//!
//! `getShiftsWithDistributionsForPeriod` is used raw, where every other rule in
//! the family filters it through the employee's job status. The two helpers it
//! calls *do* filter — `EarningMapper` drops exempt earnings and
//! `ConsecutiveDaysCalculator` drops exempt shifts — so one rule applies the
//! test to its earnings and its day counter but not to its hours.

use crate::common::numbers::round_hours;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    BOTH_CONSECUTIVE_AND_WEEKLY_OT, CONSEC_DAY_LIMIT, CONSEC_DAYS_IN_WEEK,
    DAILY_OT_LIMIT_PROP_CAPS, EARNING_TYPE_PAY_SET, ENABLE_CONSEC_DAY_OT_PROP,
    MAX_CONSEC_DAYS_PAID_PROP, MAX_DT_PAID_PROP, MinHrsForFullTimeOTRuleConfig,
    PART_TIME_ELIGIBILITY_LIMIT_PROP, WEEKLY_OT_LIMIT_PROP_CAPS,
};
use crate::rules::algorithm::hoursdistribution::consecutive_days_calculator::calc_consec_days_prior_to_start_date;
use crate::rules::algorithm::hoursdistribution::earning_mapper::EarningMapper;
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::ports::{EmployeeEarningPort, EmployeeShiftConsecutiveDaysPort};
use crate::rules::rule_config::RuleConfig;
use crate::rules::types::earning_type_pay_set::EarningTypePaySet;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::HashMap;

/// Double time for part-timers, overtime for everyone else.
/// `MinHrsForFullTimeOTRuleImpl`.
#[derive(Debug, Default)]
pub struct MinHrsForFullTimeOTRule<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort> {
    consecutive_days: C,
    earnings: RefCell<P>,
}

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort> MinHrsForFullTimeOTRule<C, P> {
    /// Build the rule over the consecutive-day lookup and the earning port.
    pub fn new(consecutive_days: C, earnings: P) -> Self {
        Self {
            consecutive_days,
            earnings: RefCell::new(earnings),
        }
    }
}

/// `OvertimeAccumulator` — used twice, for the day and for the week.
#[derive(Debug, Default, Clone, Copy)]
struct OvertimeAccumulator {
    ot_limit: f64,
    worked_hours: f64,
    ot_hours_allocated: f64,
}

impl OvertimeAccumulator {
    fn new(ot_limit: f64) -> Self {
        Self {
            ot_limit,
            ..Self::default()
        }
    }

    fn add_worked_hours(&mut self, hours: f64) {
        self.worked_hours = round_hours(self.worked_hours + hours);
    }

    fn add_ot_hours_allocated(&mut self, hours: f64) {
        self.ot_hours_allocated = round_hours(self.ot_hours_allocated + hours);
    }

    /// `calcualteUnallocatedOT` — Java's spelling, typo included.
    fn calculate_unallocated_ot(&self) -> f64 {
        if self.worked_hours > self.ot_limit {
            round_hours(self.worked_hours - self.ot_limit - self.ot_hours_allocated)
        } else {
            0.0
        }
    }

    /// `calculateWeeklyOT` — two formulas, and the flag picks between them.
    fn calculate_weekly_ot(
        &self,
        current_chunk_hours: f64,
        both_consecutive_and_weekly: bool,
    ) -> f64 {
        if self.worked_hours <= self.ot_limit {
            return 0.0;
        }

        if both_consecutive_and_weekly {
            round_hours(self.worked_hours - self.ot_limit).min(current_chunk_hours)
        } else {
            round_hours(self.worked_hours - self.ot_limit - self.ot_hours_allocated).max(0.0)
        }
    }

    fn reset(&mut self) {
        self.worked_hours = 0.0;
        self.ot_hours_allocated = 0.0;
    }
}

/// `PartTimeEligibilityAccumulator`.
#[derive(Debug, Default, Clone, Copy)]
struct PartTimeEligibility {
    eligibility_limit: f64,
    max_premium_paid: f64,
    total_weekly_hours: f64,
    premium_hours_allocated: f64,
    worked_hours: f64,
}

impl PartTimeEligibility {
    fn add_worked_hours(&mut self, hours: f64) {
        self.worked_hours = round_hours(self.worked_hours + hours);
    }

    fn add_premium_hours_allocated(&mut self, hours: f64) {
        self.premium_hours_allocated = round_hours(self.premium_hours_allocated + hours);
    }

    fn calculate_unallocated_dt(&self) -> f64 {
        round_hours(self.worked_hours - self.premium_hours_allocated).min(round_hours(
            self.max_premium_paid - self.premium_hours_allocated,
        ))
    }

    /// `shouldPay` — at or under the limit, with budget left.
    fn should_pay(&self) -> bool {
        self.eligibility_limit >= self.total_weekly_hours && self.calculate_unallocated_dt() > 0.0
    }
}

/// `ConsecutiveDays`.
#[derive(Debug, Default, Clone, Copy)]
struct ConsecutiveDays {
    count: f64,
    consec_day_limit: i32,
    enable_consec_day_ot: bool,
    days_before_count_resets: f64,
}

impl ConsecutiveDays {
    fn reset(&mut self) {
        self.count = 0.0;
    }

    fn increment(&mut self) {
        self.count += 1.0;
        if self.count % self.days_before_count_resets != 0.0 {
            self.count %= self.days_before_count_resets;
        }
    }

    fn should_pay(&self) -> bool {
        self.enable_consec_day_ot && self.count >= f64::from(self.consec_day_limit)
    }
}

/// `Accumulators` — the four, and the flag they share.
#[derive(Debug, Default)]
struct Accumulators {
    part_time: PartTimeEligibility,
    daily_ot: OvertimeAccumulator,
    weekly_ot: OvertimeAccumulator,
    consecutive_days: ConsecutiveDays,
    both_consecutive_and_weekly_ot: bool,
}

impl Accumulators {
    fn add_worked_hours(&mut self, worked_hours: f64) {
        self.part_time.add_worked_hours(worked_hours);
        self.daily_ot.add_worked_hours(worked_hours);
        self.weekly_ot.add_worked_hours(worked_hours);
    }

    fn add_premium_hours_allocated(&mut self, hours: f64) {
        self.daily_ot.add_ot_hours_allocated(hours);
        self.weekly_ot.add_ot_hours_allocated(hours);
        self.part_time.add_premium_hours_allocated(hours);
    }
}

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort> HoursDistributionRule
    for MinHrsForFullTimeOTRule<C, P>
{
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&MinHrsForFullTimeOTRuleConfig.default_values());

        let pay_set =
            EarningTypePaySet::from_json_string(params.get(EARNING_TYPE_PAY_SET).unwrap_or(""));
        let configured: Vec<i32> = pay_set.configured_earning_type_ids().into_iter().collect();

        let daily_shift_map = build_daily_shift_map(time_card, work_week);
        let earning_mapper = EarningMapper::new(time_card, work_week, &configured);

        let consec_day_limit = params.int_at(CONSEC_DAY_LIMIT);
        let max_consec_days = params.int_at(MAX_CONSEC_DAYS_PAID_PROP);

        let mut accumulators = Accumulators {
            both_consecutive_and_weekly_ot: params.bool_at(BOTH_CONSECUTIVE_AND_WEEKLY_OT),
            consecutive_days: ConsecutiveDays {
                count: if params.bool_at(CONSEC_DAYS_IN_WEEK) {
                    0.0
                } else {
                    calc_consec_days_prior_to_start_date(
                        time_card,
                        &self.consecutive_days,
                        work_week.start_date(),
                        consec_day_limit,
                        max_consec_days,
                    )
                },
                consec_day_limit,
                enable_consec_day_ot: params.bool_at(ENABLE_CONSEC_DAY_OT_PROP),
                days_before_count_resets: f64::from(consec_day_limit + max_consec_days - 1),
            },
            weekly_ot: OvertimeAccumulator::new(params.double_at(WEEKLY_OT_LIMIT_PROP_CAPS)),
            daily_ot: OvertimeAccumulator::new(params.double_at(DAILY_OT_LIMIT_PROP_CAPS)),
            part_time: PartTimeEligibility {
                eligibility_limit: params.double_at(PART_TIME_ELIGIBILITY_LIMIT_PROP),
                max_premium_paid: params.double_at(MAX_DT_PAID_PROP),
                total_weekly_hours: total_weekly_hours(time_card, work_week, &configured),
                ..PartTimeEligibility::default()
            },
        };

        for date in work_week.dates() {
            self.distribute_hours_for_date(
                time_card,
                &daily_shift_map,
                &pay_set,
                &earning_mapper,
                &mut accumulators,
                date,
                rule_item,
            );
        }
    }
}

impl<C: EmployeeShiftConsecutiveDaysPort, P: EmployeeEarningPort> MinHrsForFullTimeOTRule<C, P> {
    /// `distributeHoursForDate`.
    #[allow(clippy::too_many_arguments)]
    fn distribute_hours_for_date(
        &self,
        time_card: &mut dyn TimeCard,
        daily_shift_map: &HashMap<LocalDate, Vec<usize>>,
        pay_set: &EarningTypePaySet,
        earning_mapper: &EarningMapper,
        accumulators: &mut Accumulators,
        date: LocalDate,
        rule_item: &RuleItem,
    ) {
        accumulators.daily_ot.reset();

        let empty = Vec::new();
        let shifts_for_day = daily_shift_map.get(&date).unwrap_or(&empty).clone();

        // `updateConsecutiveDays`.
        if shifts_for_day.is_empty() {
            accumulators.consecutive_days.reset();
        } else {
            accumulators.consecutive_days.increment();
        }

        for &earning_index in earning_mapper.earnings_for_date(date) {
            self.compute_earning_hours(time_card, earning_index, pay_set, accumulators, rule_item);
        }

        for shift_index in shifts_for_day {
            self.compute_shift_hours(
                time_card,
                shift_index,
                pay_set,
                earning_mapper,
                accumulators,
                date,
                rule_item,
            );
        }
    }

    /// `computeShiftHours`.
    #[allow(clippy::too_many_arguments)]
    fn compute_shift_hours(
        &self,
        time_card: &mut dyn TimeCard,
        shift_index: usize,
        pay_set: &EarningTypePaySet,
        earning_mapper: &EarningMapper,
        accumulators: &mut Accumulators,
        date: LocalDate,
        rule_item: &RuleItem,
    ) {
        let shift_id = time_card.shifts()[shift_index].id();
        for &earning_index in earning_mapper.earnings_for_shift(shift_id) {
            self.compute_earning_hours(time_card, earning_index, pay_set, accumulators, rule_item);
        }

        let distribution_indices: Vec<usize> = time_card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .enumerate()
            .filter(|(_, distribution)| {
                distribution.date() == date && time_card.is_open_for_editing_on(distribution.date())
            })
            .map(|(index, _)| index)
            .collect();

        for distribution_index in distribution_indices {
            let distribution_hours = time_card.shifts()[shift_index].hours_distributions()
                [distribution_index]
                .original_hours();
            accumulators.add_worked_hours(distribution_hours);

            // The `distributionIsPremium && !isOpenForEditingOn` arm is dead;
            // see the module note.
            if accumulators.consecutive_days.should_pay() {
                let Some(bucket) = time_card.ot_hours_distribution_type_id() else {
                    continue;
                };
                allocate_premium(
                    time_card,
                    shift_index,
                    distribution_index,
                    bucket,
                    distribution_hours,
                    accumulators,
                    rule_item.id(),
                );
            } else if accumulators.part_time.should_pay() {
                let Some(bucket) = time_card.dt_hours_distribution_type_id() else {
                    continue;
                };
                let to_allocate = accumulators.part_time.calculate_unallocated_dt();
                allocate_premium(
                    time_card,
                    shift_index,
                    distribution_index,
                    bucket,
                    to_allocate,
                    accumulators,
                    rule_item.id(),
                );
            } else {
                let shift_daily_ot = accumulators
                    .daily_ot
                    .calculate_unallocated_ot()
                    .min(distribution_hours);
                let shift_weekly_ot = accumulators.weekly_ot.calculate_weekly_ot(
                    distribution_hours,
                    accumulators.both_consecutive_and_weekly_ot,
                );

                let shift_ot = shift_weekly_ot.max(shift_daily_ot);
                if shift_ot > 0.0 {
                    let Some(bucket) = time_card.ot_hours_distribution_type_id() else {
                        continue;
                    };
                    allocate_premium(
                        time_card,
                        shift_index,
                        distribution_index,
                        bucket,
                        shift_ot,
                        accumulators,
                        rule_item.id(),
                    );
                }
            }
        }
    }

    /// `computeEarningHours`.
    fn compute_earning_hours(
        &self,
        time_card: &mut dyn TimeCard,
        earning_index: usize,
        pay_set: &EarningTypePaySet,
        accumulators: &mut Accumulators,
        rule_item: &RuleItem,
    ) {
        let hours = time_card.earnings()[earning_index].hours();
        accumulators.add_worked_hours(hours);

        // `calculatePremiumEarningHours` — the same ladder as the shift half.
        let premium_hours = if accumulators.consecutive_days.should_pay() {
            hours
        } else if accumulators.part_time.should_pay() {
            accumulators.part_time.calculate_unallocated_dt()
        } else {
            let daily_ot = accumulators.daily_ot.calculate_unallocated_ot().min(hours);
            let weekly_ot = accumulators
                .weekly_ot
                .calculate_weekly_ot(hours, accumulators.both_consecutive_and_weekly_ot);
            daily_ot.max(weekly_ot)
        };

        let is_open = {
            let earning = &time_card.earnings()[earning_index];
            time_card.is_open_for_editing_for_earning(earning)
        };

        if is_open && premium_hours > 0.0 {
            let regular_type_id = time_card.earnings()[earning_index].earning_type_id();

            self.create_earning(
                time_card,
                earning_index,
                round_hours(-premium_hours),
                regular_type_id,
                rule_item,
            );

            // `getPremiumEarningType` — level 1 only when part time is paying.
            let premium_level = usize::from(
                !accumulators.consecutive_days.should_pay() && accumulators.part_time.should_pay(),
            );
            let premium_type_id = pay_set.premium_earning_type_id(regular_type_id, premium_level);

            self.create_earning(
                time_card,
                earning_index,
                premium_hours,
                premium_type_id,
                rule_item,
            );
        }

        // Outside the guard, so a closed earning still spends the budget.
        accumulators.add_premium_hours_allocated(premium_hours);
    }

    /// `createEarning`. Divergence 42: the note is the rule item's name alone.
    fn create_earning(
        &self,
        time_card: &mut dyn TimeCard,
        source_index: usize,
        hours: f64,
        earning_type_id: i32,
        rule_item: &RuleItem,
    ) {
        let source = &time_card.earnings()[source_index];

        let mut earning = EmployeeEarning::new(
            0,
            source.employee_id(),
            source.job_id(),
            earning_type_id,
            source.earning_date(),
            hours,
            source.rate(),
            crate::common::enums::earning_source::EarningSource::Rule,
        )
        .from_rule(rule_item.id(), source.shift_id());

        earning.set_pay_date(Some(source.earning_date()));
        earning.set_note(rule_item.name());
        earning.calc_and_set_total_dollars();

        self.earnings.borrow_mut().save(earning.clone());
        time_card.earnings_mut().push(earning);
    }
}

/// `dailyShiftMap` — grouped by **distribution** date, so a shift lands in a
/// day's list once per distribution it has on that day.
fn build_daily_shift_map(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
) -> HashMap<LocalDate, Vec<usize>> {
    let mut by_date: HashMap<LocalDate, Vec<usize>> = HashMap::new();

    for index in time_card.shift_indices_with_distributions_for_period(work_week) {
        for distribution in time_card.shifts()[index].hours_distributions() {
            by_date.entry(distribution.date()).or_default().push(index);
        }
    }

    by_date
}

/// `PartTimeEligibilityAccumulator`'s constructor — the week's regular
/// distribution hours plus its configured earning hours, computed once.
fn total_weekly_hours(
    time_card: &dyn TimeCard,
    work_week: &DateRange,
    configured_earning_type_ids: &[i32],
) -> f64 {
    let shift_hours: f64 = time_card
        .shifts_with_distributions_for_period(work_week)
        .into_iter()
        .flat_map(|shift| shift.hours_distributions())
        .filter(|distribution| work_week.contains_date(distribution.date()))
        .filter(|distribution| time_card.distribution_is_regular(distribution))
        .map(|distribution| distribution.original_hours())
        .sum();

    let earning_hours: f64 = time_card
        .earnings_for_period(work_week)
        .into_iter()
        .filter(|earning| configured_earning_type_ids.contains(&earning.earning_type_id()))
        .map(EmployeeEarning::hours)
        .sum();

    round_hours(shift_hours + earning_hours)
}

/// `allocateShiftOT` and `allocateShiftDT`, which differ only in which bucket
/// and how much.
///
/// An existing row of the same bucket **and date** is topped up rather than
/// duplicated — and the top-up is unrounded where the source row's reduction is
/// rounded. `WeeklyOTHrs` merges the same way.
fn allocate_premium(
    time_card: &mut dyn TimeCard,
    shift_index: usize,
    distribution_index: usize,
    bucket: i32,
    to_allocate: f64,
    accumulators: &mut Accumulators,
    rule_item_id: i32,
) {
    let shift = &mut time_card.shifts_mut()[shift_index];
    let date = shift.hours_distributions()[distribution_index].date();

    let existing = shift
        .hours_distributions()
        .iter()
        .position(|d| d.date() == date && d.is_of_type(bucket));

    match existing {
        Some(index) => {
            let hours = shift.hours_distributions()[index].hours();
            // Java adds without rounding here.
            shift.hours_distributions_mut()[index].set_hours(hours + to_allocate);
            shift.hours_distributions_mut()[index].set_hours_rule_item_id(Some(rule_item_id));
        }
        None => {
            let row = create_premium_distribution_at_rate(
                &shift.hours_distributions()[distribution_index],
                to_allocate,
                0.0,
                bucket,
                Some(rule_item_id),
                None,
            );
            shift.add_hours_distribution(row);
        }
    }

    let reduced =
        round_hours(shift.hours_distributions()[distribution_index].hours() - to_allocate);
    shift.hours_distributions_mut()[distribution_index].set_hours(reduced);

    accumulators.add_premium_hours_allocated(to_allocate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
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
    use crate::rules::types::earning_type_pay_map::EarningTypePayMap;

    pub(super) const JOB: i32 = 1;
    pub(super) const REGULAR: i32 = 1;
    pub(super) const OVERTIME: i32 = 2;
    pub(super) const DOUBLE_TIME: i32 = 3;

    /// The spec's earning types: regular 10, overtime 7, double time 5.
    pub(super) const REG_EARNING: i32 = 10;
    pub(super) const OT_EARNING: i32 = 7;
    pub(super) const DT_EARNING: i32 = 5;

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

    #[derive(Debug, Default)]
    pub(super) struct SavedEarnings(pub Vec<EmployeeEarning>);

    impl EmployeeEarningPort for SavedEarnings {
        fn save(&mut self, earning: EmployeeEarning) {
            self.0.push(earning);
        }
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            None
        }
    }

    pub(super) fn rule() -> MinHrsForFullTimeOTRule<PriorDays, SavedEarnings> {
        MinHrsForFullTimeOTRule::new(PriorDays(0), SavedEarnings::default())
    }

    /// `workWeekStartDate` is 2015-01-12; `jan(0)` is that day.
    pub(super) fn jan(offset: i64) -> LocalDate {
        LocalDate::of(2015, 1, 12).plus_days(offset)
    }

    pub(super) fn week() -> DateRange {
        DateRange::new(jan(0), jan(6))
    }

    pub(super) fn shift(id: i32, hours: f64, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_net_hours(hours)
            .with_hours_distributions(vec![HoursDistribution::new(
                1,
                date,
                Some(REGULAR),
                hours,
                0.0,
            )])
    }

    /// `empEarning(netHours, earningDate, shift)` — always the regular type.
    pub(super) fn earning(
        id: i32,
        hours: f64,
        date: LocalDate,
        shift_id: Option<i32>,
    ) -> EmployeeEarning {
        let earning = EmployeeEarning::new(
            id,
            1,
            JOB,
            REG_EARNING,
            date,
            hours,
            10.0,
            EarningSource::Auto,
        );
        match shift_id {
            Some(id) => earning.with_shift(id),
            None => earning,
        }
    }

    /// `paySet`: regular 10 escalating to overtime 7 then double time 5.
    pub(super) fn pay_set_json() -> String {
        EarningTypePaySet::new()
            .with_premium_levels(2)
            .with_pay_maps(vec![EarningTypePayMap::new(
                REG_EARNING,
                vec![OT_EARNING, DT_EARNING],
            )])
            .to_json_string()
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
                    jan(-365),
                    jan(365),
                    EmployeePayType::Hourly,
                    10.0,
                    true,
                )],
            ))
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(jan(0))
    }

    /// The spec's `params` map, with any overrides applied on top.
    pub(super) fn item(params: &[(&str, &str)]) -> RuleItem {
        let mut rule_params = RuleParams::new();
        rule_params.set(CONSEC_DAY_LIMIT, "6");
        rule_params.set(MAX_CONSEC_DAYS_PAID_PROP, "2");
        rule_params.set(DAILY_OT_LIMIT_PROP_CAPS, "8.0");
        rule_params.set(WEEKLY_OT_LIMIT_PROP_CAPS, "40.0");
        rule_params.set(PART_TIME_ELIGIBILITY_LIMIT_PROP, "28");
        rule_params.set(MAX_DT_PAID_PROP, "17.5");
        rule_params.set(BOTH_CONSECUTIVE_AND_WEEKLY_OT, "false");
        rule_params.set(EARNING_TYPE_PAY_SET, pay_set_json());
        for (key, value) in params {
            rule_params.set(*key, *value);
        }
        RuleItem::new(
            1,
            1,
            "The Rule!",
            RuleClass::MinHrsFullTimeOtHdr,
            rule_params,
        )
    }

    /// Every distribution on a shift as `(date, type, hours)`.
    pub(super) fn rows(card: &TimeCardData, shift_index: usize) -> Vec<(LocalDate, i32, f64)> {
        let mut rows: Vec<(LocalDate, i32, f64)> = card.shifts()[shift_index]
            .hours_distributions()
            .iter()
            .map(|d| {
                (
                    d.date(),
                    d.hours_distribution_type_id().unwrap_or(0),
                    d.hours(),
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        rows
    }

    /// Total hours across the card's earnings of one type.
    pub(super) fn earning_total(card: &TimeCardData, type_id: i32) -> f64 {
        card.earnings()
            .iter()
            .filter(|e| e.earning_type_id() == type_id)
            .map(EmployeeEarning::hours)
            .sum()
    }

    /// Five worked days immediately before the week, so the ported
    /// `ConsecutiveDaysCalculator` answers **5** where the Groovy stubs it.
    ///
    /// The spec mocks `ConsecutiveDaysCalculator.calcConsecDaysPriorToStartDate`
    /// directly; that calculator is ported rather than stubbed, so the days it
    /// counts are put on the card instead. They sit outside the work week, so
    /// nothing else in the rule sees them — and they are appended after the
    /// asserted shifts so the indices the spec uses are unchanged.
    pub(super) fn prior_consecutive_days(count: i64, next_id: i32) -> Vec<EmployeeShift> {
        (1..=count)
            .map(|back| shift(next_id + back as i32, 8.0, jan(-back)))
            .collect()
    }

    #[test]
    fn a_part_timer_is_paid_double_time_not_overtime() {
        let mut card = card(vec![shift(1, 8.0, jan(0))]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), DOUBLE_TIME, 8.0)]
        );
    }

    #[test]
    fn the_part_time_test_reads_the_whole_week_not_the_day_so_far() {
        // Eight hours on Monday alone is part time; the same Monday inside a
        // 40-hour week is not, and pays nothing.
        let mut full_week = card((0..5).map(|d| shift(d as i32 + 1, 8.0, jan(d))).collect());

        rule().execute(&mut full_week, &week(), &item(&[]));

        assert_eq!(rows(&full_week, 0), vec![(jan(0), REGULAR, 8.0)]);
    }

    #[test]
    fn overtime_paid_on_a_consecutive_day_eats_the_double_time_budget() {
        // Five prior worked days plus Monday is the sixth, so Monday is
        // consecutive-day overtime — and that overtime counts against maxDTPaid.
        let mut shifts = vec![shift(1, 9.0, jan(0))];
        shifts.extend(prior_consecutive_days(5, 100));
        let mut card = card(shifts).with_earnings(vec![
            earning(1, 4.0, jan(0), Some(1)),
            earning(2, 8.0, jan(1), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), OVERTIME, 9.0)]
        );
        // 17.5 of budget: 4 of earning overtime, 9 of shift overtime, then 4.5
        // of double time is all that is left for Tuesday's eight hours.
        assert_eq!(earning_total(&card, OT_EARNING), 4.0);
        assert_eq!(earning_total(&card, DT_EARNING), 4.5);
    }

    #[test]
    fn a_shift_with_two_distributions_on_one_day_is_processed_twice() {
        // dailyShiftMap groups by distribution date and maps back to the shift,
        // so the shift lands in the day's list once per distribution.
        let two_rows = EmployeeShift::new(1, 1, JOB, jan(0), ShiftType::Actual, Vec::new())
            .with_net_hours(8.0)
            .with_hours_distributions(vec![
                HoursDistribution::new(1, jan(0), Some(REGULAR), 4.0, 0.0),
                HoursDistribution::new(1, jan(0), Some(REGULAR), 4.0, 0.0),
            ]);
        let mut card = card(vec![two_rows]);

        rule().execute(&mut card, &week(), &item(&[]));

        // Eight hours of work, but sixteen counted: both rows are walked twice.
        assert_eq!(
            card.shifts()[0]
                .hours_distributions()
                .iter()
                .filter(|d| d.is_of_type(DOUBLE_TIME))
                .count(),
            1,
            "the second pass tops up the row it made on the first"
        );
        assert_eq!(earning_total(&card, DT_EARNING), 0.0);
    }

    #[test]
    fn a_property_with_no_double_time_bucket_leaves_a_part_timer_alone() {
        let mut card = card(vec![shift(1, 8.0, jan(0))]).with_hours_distribution_types(vec![
            HoursDistributionType::new(REGULAR, "Regular", false),
            HoursDistributionType::new(OVERTIME, "Overtime", true),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(rows(&card, 0), vec![(jan(0), REGULAR, 8.0)]);
    }

    #[test]
    fn the_created_earnings_are_persisted_through_the_port() {
        let mut card = card(Vec::new()).with_earnings(vec![earning(1, 9.0, jan(0), None)]);
        let rule = rule();

        rule.execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rule.earnings.borrow().0.len(),
            2,
            "the offset and the premium"
        );
    }
}

/// `MinHrsForFullTimeOTRuleImplTest.groovy`, transcribed.
///
/// Ground truth:
/// `taps/src/junit/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/MinHrsForFullTimeOTRuleImplTest.groovy`
/// — nineteen cases.
///
/// The mocked `PayGroup` is divergence 24's derivation, so the calculation start
/// date is set on the card. The spec mocks
/// `ConsecutiveDaysCalculator.calcConsecDaysPriorToStartDate` to a fixed number;
/// that calculator is ported rather than stubbed, so the transcriptions put the
/// worked days it would count on the card instead, outside the week — see
/// `prior_consecutive_days`.
///
/// # Four cases assert nothing at all
///
/// `consecutive day overtime should respect consecutive days in work week flag`,
/// `consecutive days in week pays correctly when disabled`, and the `(0..N).each`
/// halves of `consecutive days in week pays correctly` and the two
/// `Both Consecutive and Weekly OT` cases put their checks **inside a closure**.
/// Spock applies implicit assertions only to top-level expressions in a `then:`
/// block, so nothing inside `(0..4).each { … }` is asserted — `dist.size() == 1`
/// and the `assertListHasCorrectHoursDistribution` call are both evaluated and
/// discarded. The transcriptions assert them for real. Seventh spec-weakness in
/// the family, and the first of this particular shape.
///
/// `work rule should always call the fix map method` is a mock-interaction test
/// on `ruleConfig.fixMap`; `fixed()` is called unconditionally here and there is
/// nothing to observe, so it is not transcribed.
#[cfg(test)]
mod java_parity_tests {
    use super::tests::{
        DOUBLE_TIME, DT_EARNING, JOB, OT_EARNING, OVERTIME, REG_EARNING, REGULAR, card, earning,
        earning_total, item, jan, prior_consecutive_days, rows, rule, shift, week,
    };
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;

    /// Five consecutive worked days before the week, as the spec's `>> 5`.
    fn with_five_prior_days(mut shifts: Vec<EmployeeShift>) -> Vec<EmployeeShift> {
        shifts.extend(prior_consecutive_days(5, 100));
        shifts
    }

    /// One worked day before the week, as the spec's `>> 1`.
    fn with_one_prior_day(mut shifts: Vec<EmployeeShift>) -> Vec<EmployeeShift> {
        shifts.extend(prior_consecutive_days(1, 100));
        shifts
    }

    fn regular_only(card: &TimeCardData, shift_index: usize, date: LocalDate, hours: f64) {
        assert_eq!(rows(card, shift_index), vec![(date, REGULAR, hours)]);
    }

    #[test]
    fn under_the_part_time_limit_and_less_than_max_dt_paid_should_pay_all_as_dt() {
        let mut card = card(vec![shift(1, 8.0, jan(0)), shift(2, 8.0, jan(4))]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), DOUBLE_TIME, 8.0)]
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(4), REGULAR, 0.0), (jan(4), DOUBLE_TIME, 8.0)]
        );
    }

    #[test]
    fn at_the_part_time_limit_should_pay_up_to_max_dt_and_should_not_get_ot() {
        let mut card = card(vec![
            shift(1, 10.0, jan(0)),
            shift(2, 10.0, jan(4)),
            shift(3, 8.0, jan(5)),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), DOUBLE_TIME, 10.0)]
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(4), REGULAR, 2.5), (jan(4), DOUBLE_TIME, 7.5)]
        );
        regular_only(&card, 2, jan(5), 8.0);
    }

    #[test]
    fn at_the_part_time_limit_with_earnings_under_the_max_dt_paid() {
        let mut card = card(vec![shift(1, 9.0, jan(0))]).with_earnings(vec![
            earning(1, 4.0, jan(0), Some(1)),
            earning(2, 4.0, jan(1), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), DOUBLE_TIME, 9.0)]
        );
        assert_eq!(earning_total(&card, REG_EARNING), 0.0);
        assert_eq!(earning_total(&card, OT_EARNING), 0.0);
        assert_eq!(earning_total(&card, DT_EARNING), 8.0);
    }

    #[test]
    fn consecutive_day_overtime_counts_towards_max_dt_paid() {
        let mut card = card(with_five_prior_days(vec![shift(1, 9.0, jan(0))])).with_earnings(vec![
            earning(1, 4.0, jan(0), Some(1)),
            earning(2, 8.0, jan(1), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), OVERTIME, 9.0)]
        );
        assert_eq!(earning_total(&card, REG_EARNING), 3.5);
        assert_eq!(earning_total(&card, OT_EARNING), 4.0);
        assert_eq!(earning_total(&card, DT_EARNING), 4.5);
    }

    #[test]
    fn over_the_part_time_limit_and_under_the_weekly_limit_pays_nothing() {
        let mut card = card((0..5).map(|d| shift(d as i32 + 1, 8.0, jan(d))).collect());

        rule().execute(&mut card, &week(), &item(&[]));

        for day in 0..5 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
    }

    #[test]
    fn distributions_from_a_previous_period_are_not_counted_toward_ot() {
        let part_closed = EmployeeShift::new(1, 1, JOB, jan(-1), ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![
                HoursDistribution::new(1, jan(-1), Some(REGULAR), 8.0, 0.0),
                HoursDistribution::new(1, jan(0), Some(REGULAR), 4.0, 0.0),
            ]);

        let mut card = card(vec![
            part_closed,
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 8.0, jan(4)),
        ])
        // `currentPayPeriod() >> of(workWeekStartDate.plusDays(1))`.
        .with_calculation_start_date(jan(1));

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(-1), REGULAR, 8.0), (jan(0), REGULAR, 4.0)]
        );
        for (index, day) in (1..5).enumerate() {
            regular_only(&card, index + 1, jan(day), 8.0);
        }
    }

    #[test]
    fn all_hours_over_the_weekly_limit_are_ot() {
        let mut card = card(vec![
            shift(1, 8.0, jan(0)),
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 4.0, jan(4)),
            shift(6, 8.0, jan(6)),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        for day in 0..4 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
        regular_only(&card, 4, jan(4), 4.0);
        assert_eq!(
            rows(&card, 5),
            vec![(jan(6), REGULAR, 4.0), (jan(6), OVERTIME, 4.0)]
        );
    }

    #[test]
    fn all_hours_over_the_daily_limit_are_ot_when_over_the_part_time_limit() {
        let mut card = card(vec![
            shift(1, 9.0, jan(0)),
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 11.0, jan(3)),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 8.0), (jan(0), OVERTIME, 1.0)]
        );
        regular_only(&card, 1, jan(1), 8.0);
        regular_only(&card, 2, jan(2), 8.0);
        assert_eq!(
            rows(&card, 3),
            vec![(jan(3), REGULAR, 8.0), (jan(3), OVERTIME, 3.0)]
        );
    }

    #[test]
    fn weekly_ot_should_not_double_dip_if_daily_ot_has_been_paid() {
        let mut card = card(vec![
            shift(1, 10.0, jan(0)),
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 8.0, jan(4)),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 8.0), (jan(0), OVERTIME, 2.0)]
        );
        for day in 1..5 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
    }

    #[test]
    fn should_account_for_consecutive_day_overtime() {
        let mut card = card(with_five_prior_days(
            (0..5).map(|d| shift(d as i32 + 1, 8.0, jan(d))).collect(),
        ));

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), OVERTIME, 8.0)]
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(1), REGULAR, 0.0), (jan(1), OVERTIME, 8.0)]
        );
        for day in 2..5 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
    }

    #[test]
    fn consecutive_day_overtime_should_respect_consecutive_days_in_work_week_flag() {
        // Asserted for real; the Groovy's checks are inside a closure.
        let mut card = card(with_five_prior_days(
            (0..5).map(|d| shift(d as i32 + 1, 8.0, jan(d))).collect(),
        ));

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "true")]));

        for day in 0..5 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
    }

    #[test]
    fn consecutive_days_in_week_pays_correctly() {
        let mut card = card(with_five_prior_days(
            (0..7).map(|d| shift(d as i32 + 1, 5.0, jan(d))).collect(),
        ));

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "true")]));

        for day in 0..5 {
            regular_only(&card, day as usize, jan(day), 5.0);
        }
        assert_eq!(
            rows(&card, 5),
            vec![(jan(5), REGULAR, 0.0), (jan(5), OVERTIME, 5.0)]
        );
        assert_eq!(
            rows(&card, 6),
            vec![(jan(6), REGULAR, 0.0), (jan(6), OVERTIME, 5.0)]
        );
    }

    #[test]
    fn consecutive_days_in_week_pays_correctly_when_disabled() {
        let mut card = card(with_five_prior_days(
            (0..7).map(|d| shift(d as i32 + 1, 5.0, jan(d))).collect(),
        ));

        rule().execute(
            &mut card,
            &week(),
            &item(&[(ENABLE_CONSEC_DAY_OT_PROP, "false")]),
        );

        for day in 0..7 {
            regular_only(&card, day as usize, jan(day), 5.0);
        }
    }

    #[test]
    fn daily_ot_with_earnings_should_calculate_ot_correctly() {
        let mut card = card(Vec::new()).with_earnings(vec![
            earning(1, 9.0, jan(0), None),
            earning(2, 8.0, jan(1), None),
            earning(3, 8.0, jan(2), None),
            earning(4, 11.0, jan(3), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(earning_total(&card, REG_EARNING), 32.0);
        assert_eq!(earning_total(&card, OT_EARNING), 4.0);
        assert_eq!(earning_total(&card, DT_EARNING), 0.0);
    }

    #[test]
    fn all_earning_hours_over_the_weekly_limit_are_ot() {
        let mut card = card(Vec::new()).with_earnings(vec![
            earning(1, 8.0, jan(0), None),
            earning(2, 8.0, jan(1), None),
            earning(3, 8.0, jan(2), None),
            earning(4, 8.0, jan(3), None),
            earning(5, 4.0, jan(4), None),
            earning(6, 8.0, jan(6), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[]));

        assert_eq!(earning_total(&card, REG_EARNING), 40.0);
        assert_eq!(earning_total(&card, OT_EARNING), 4.0);
        assert_eq!(earning_total(&card, DT_EARNING), 0.0);
    }

    #[test]
    fn should_account_for_everything() {
        let mut card = card(with_five_prior_days(vec![
            shift(1, 8.0, jan(0)),
            shift(2, 4.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 8.0, jan(4)),
        ]))
        .with_earnings(vec![
            earning(1, 4.0, jan(1), Some(2)),
            earning(2, 4.0, jan(2), Some(3)),
            earning(3, 8.0, jan(5), None),
            earning(4, 8.0, jan(6), None),
        ]);

        rule().execute(&mut card, &week(), &item(&[(CONSEC_DAYS_IN_WEEK, "false")]));

        assert_eq!(
            rows(&card, 0),
            vec![(jan(0), REGULAR, 0.0), (jan(0), OVERTIME, 8.0)]
        );
        assert_eq!(
            rows(&card, 1),
            vec![(jan(1), REGULAR, 0.0), (jan(1), OVERTIME, 4.0)]
        );
        assert_eq!(
            rows(&card, 2),
            vec![(jan(2), REGULAR, 4.0), (jan(2), OVERTIME, 4.0)]
        );
        regular_only(&card, 3, jan(3), 8.0);
        regular_only(&card, 4, jan(4), 8.0);

        assert_eq!(earning_total(&card, REG_EARNING), 20.0);
        assert_eq!(earning_total(&card, OT_EARNING), 4.0);
        assert_eq!(earning_total(&card, DT_EARNING), 0.0);
    }

    #[test]
    fn both_consecutive_and_weekly_ot_enabled_weekly_ot_still_applies() {
        let mut card = card(with_one_prior_day(vec![
            shift(1, 8.0, jan(0)),
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 8.0, jan(4)),
            shift(6, 8.0, jan(6)),
        ]));

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAYS_IN_WEEK, "false"),
                (BOTH_CONSECUTIVE_AND_WEEKLY_OT, "true"),
            ]),
        );

        for day in 0..4 {
            regular_only(&card, day as usize, jan(day), 8.0);
        }
        assert_eq!(
            rows(&card, 4),
            vec![(jan(4), REGULAR, 0.0), (jan(4), OVERTIME, 8.0)]
        );
        assert_eq!(
            rows(&card, 5),
            vec![(jan(6), REGULAR, 0.0), (jan(6), OVERTIME, 8.0)]
        );
    }

    #[test]
    fn both_consecutive_and_weekly_ot_disabled_weekly_ot_does_not_apply() {
        let mut card = card(with_one_prior_day(vec![
            shift(1, 8.0, jan(0)),
            shift(2, 8.0, jan(1)),
            shift(3, 8.0, jan(2)),
            shift(4, 8.0, jan(3)),
            shift(5, 8.0, jan(4)),
            shift(6, 8.0, jan(6)),
        ]));

        rule().execute(
            &mut card,
            &week(),
            &item(&[
                (CONSEC_DAYS_IN_WEEK, "false"),
                (BOTH_CONSECUTIVE_AND_WEEKLY_OT, "false"),
            ]),
        );

        assert_eq!(
            rows(&card, 4),
            vec![(jan(4), REGULAR, 0.0), (jan(4), OVERTIME, 8.0)]
        );
        regular_only(&card, 5, jan(6), 8.0);
    }
}
