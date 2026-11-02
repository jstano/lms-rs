//! `RuleType::ShiftDiffOt` — `ShiftDifferenceOTRuleImpl`, the family's one rule.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/ShiftDifferenceOTRuleImpl.java`
//! (Java keeps it in `hoursdistribution`; it belongs to its own rule type, whose
//! catalogue entry `SHIFT_DIFFERENCE_OT_SDO` uses this same config — so the
//! audit's open question "is it the same class as the deferred hoursdistribution
//! rule?" is answered: it is).
//!
//! Pays — or converts to overtime — hours worked after a **short rest**: when the
//! gap between one actual shift's end and the next one's start is shorter than
//! `hoursBetweenShifts` (default 10), the shortfall in hours is premium time on the
//! second shift. Two modes:
//!
//! - **Earning** (default): one earning on the second shift for
//!   `min(shortfall, the shift's hours)` — or all its hours with
//!   `convertFullSecondShift` — at `round_currency(hourly rate × otFactorRate)`,
//!   where the rate is floored at minimum wage. Needs hours across the card's
//!   distribution types to be positive.
//! - **`convertHoursToOT`**: moves hours out of each *regular* distribution of the
//!   second shift into an overtime distribution (`min(distribution hours,
//!   shortfall)`, or all with `convertFullSecondShift`), priced by the property's
//!   `OVERTIME_RATE` rule set — or the supplied default set — and merged into any
//!   overtime distribution already on the shift. `reduceSecondShiftOTByFirstShiftOT`
//!   first takes off the overtime the *previous* shift already carries.
//!
//! # Which shifts and in what order
//!
//! Open-for-editing, with both times, in a job whose pay type is not
//! salaried-exempt; sorted by start time. Only `ACTUAL` shifts calculate, but the
//! walk keeps a `prevShift`: the first shift on the calculation start date looks
//! the previous *calendar day's* first shift up from the whole card; after that it
//! is the last shift visited. `excludeShiftsOnSeparateDays` /
//! `excludeShiftsOnSameDay` skip a pair whose start dates differ / match (the
//! second still becomes the new previous shift).
//!
//! # Dead write, and how overtime rates are asked for
//!
//! Java stores the new premium distribution in a transient
//! `shiftDiffOTHoursDistribution` on the shift that nothing reads; not ported.
//! The overtime rate is asked of the eight rate rules through their
//! `getRate(…)` methods. The Rust rules expose only `execute_for_shift`, so the
//! rate is read off a scratch distribution (this shift's date, the overtime hours
//! and the regular distribution's base rate) that the selected rule prices
//! (divergence 125). The default rule set is used when the property has none; both
//! come from the caller.

pub mod config;

use crate::common::dates::duration_in_minutes;
use crate::common::enums::shift_type::ShiftType;
use crate::common::enums::uom::UOM;
use crate::common::numbers::{round_currency, round_hours, round_to};
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::overtimerate::OvertimeRateRule;
use crate::rules::algorithm::shiftdifferenceot::config::{
    CONVERT_FULL_SECOND_SHIFT_PROP, CONVERT_HOURS_TO_OT_PROP, EARNING,
    EXCLUDE_SHIFTS_ON_SAME_DAY_PROP, EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP,
    HOURS_BETWEEN_SHIFTS_PROP, OT_FACTOR_PROP, REDUCE_SECOND_SHIFT_OT_PROP,
    ShiftDifferenceOTRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution_at_rate;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use crate::rules::rule_type::RuleType;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// How the rule prices overtime: the eight overtime-rate rules, the property's
/// `OVERTIME_RATE` rule set for a job and date, and the default set used when
/// that is absent or empty. `getOTRateByOTRateRuleSet`'s collaborators.
pub struct OvertimeRateLookup<'a> {
    /// Dispatch for the overtime-rate family.
    pub registry: &'a dyn RuleRegistry<dyn OvertimeRateRule>,
    /// `ruleUtils.getRuleSet(employee, job, date, OVERTIME_RATE)`.
    pub rule_set_for: &'a dyn Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    /// The default overtime rule set the runner is given.
    pub default_rule_set: &'a RuleSet,
}

impl OvertimeRateLookup<'_> {
    /// The premium rate for `ot_hours` of the shift, given the regular
    /// distribution's base rate.
    fn rate(
        &self,
        shift: &EmployeeShift,
        time_card: &dyn TimeCard,
        ot_hours: f64,
        base_rate: f64,
    ) -> f64 {
        let set = (self.rule_set_for)(shift.job_id(), shift.shift_date())
            .filter(|set| !set.rule_items().is_empty())
            .unwrap_or(self.default_rule_set);
        let Some(item) = set.rule_items().first() else {
            return 0.0;
        };
        let Some(rule) = self.registry.rule_for(item.rule_class()) else {
            return 0.0;
        };
        let mut scratch = HoursDistribution::new(
            shift.property_id(),
            shift.shift_date(),
            None,
            ot_hours,
            base_rate,
        );
        rule.execute_for_shift(shift, &mut scratch, time_card, item);
        scratch.premium_rate()
    }
}

/// `ShiftDifferenceOTRule`: what the runner calls.
pub trait ShiftDifferenceOT {
    /// `execute(TimeCard, RuleItem, RuleImplFactory, RuleSet defaultOTRuleSet)`.
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rates: &OvertimeRateLookup<'_>,
    );
}

/// `ShiftDifferenceOTRuleImpl`.
pub struct ShiftDifferenceOTRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

/// One execution's parameters.
struct Settings {
    ot_factor: f64,
    earning_id: i32,
    required_gap_minutes: f64,
    convert_hours_to_ot: bool,
    convert_full_second_shift: bool,
    reduce_second_shift_ot: bool,
    exclude_separate_days: bool,
    exclude_same_day: bool,
    rule_item_id: i32,
    ot_type_id: Option<i32>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftDifferenceOTRule<T, E, M> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }

    fn run(
        &self,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rates: &OvertimeRateLookup<'_>,
    ) {
        let params = rule_item
            .params()
            .fixed(&ShiftDifferenceOTRuleConfig.default_values());
        let settings = Settings {
            ot_factor: params.double_at(OT_FACTOR_PROP),
            earning_id: params.int_at(EARNING),
            required_gap_minutes: params.double_at(HOURS_BETWEEN_SHIFTS_PROP) * 60.0,
            convert_hours_to_ot: params.bool_at(CONVERT_HOURS_TO_OT_PROP),
            convert_full_second_shift: params.bool_at(CONVERT_FULL_SECOND_SHIFT_PROP),
            reduce_second_shift_ot: params.bool_at(REDUCE_SECOND_SHIFT_OT_PROP),
            exclude_separate_days: params.bool_at(EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP),
            exclude_same_day: params.bool_at(EXCLUDE_SHIFTS_ON_SAME_DAY_PROP),
            rule_item_id: rule_item.id(),
            ot_type_id: time_card.ot_hours_distribution_type_id(),
        };

        let mut order: Vec<usize> = (0..time_card.shifts().len())
            .filter(|&i| {
                let shift = &time_card.shifts()[i];
                time_card.is_open_for_editing_for_shift(shift)
                    && self.can_calculate(time_card, shift)
            })
            .collect();
        order.sort_by_key(|&i| time_card.shifts()[i].start_time_for_ordering());

        let mut previous: Option<usize> = None;
        for index in order {
            previous =
                self.calculate_shift(time_card, index, previous, &settings, rule_item, rates);
        }
    }

    /// `canCalculateShiftOT`.
    fn can_calculate(&self, time_card: &dyn TimeCard, shift: &EmployeeShift) -> bool {
        time_card
            .employee()
            .and_then(|e| e.employee_job_status(shift.job_id(), shift.shift_date()))
            .is_some_and(|status| status.pay_type().is_not_salaried_exempt())
            && shift.start_date_time().is_some()
            && shift.end_date_time().is_some()
    }

    /// `calculateShiftOT`; returns the new `prevShift`.
    fn calculate_shift(
        &self,
        time_card: &mut dyn TimeCard,
        index: usize,
        mut previous: Option<usize>,
        settings: &Settings,
        rule_item: &RuleItem,
        rates: &OvertimeRateLookup<'_>,
    ) -> Option<usize> {
        if time_card.shifts()[index].shift_type() != ShiftType::Actual {
            return previous;
        }
        if previous.is_none()
            && time_card.shifts()[index].shift_date() == time_card.calculation_start_date()
        {
            let day_before = time_card.shifts()[index].shift_date().minus_days(1);
            previous = time_card.shifts().iter().position(|s| {
                s.shift_date() == day_before
                    && s.start_date_time().is_some()
                    && s.end_date_time().is_some()
            });
        }
        if let Some(prev) = previous {
            let start_date = |i: usize| {
                time_card.shifts()[i]
                    .start_date_time()
                    .map(|t| t.to_local_date())
            };
            if settings.exclude_separate_days && start_date(index) != start_date(prev) {
                return Some(index);
            }
            if settings.exclude_same_day && start_date(index) == start_date(prev) {
                return Some(index);
            }
            self.compute_for_distribution(time_card, index, prev, settings, rule_item, rates);
        }
        Some(index)
    }

    /// `computeOTForDistribution`.
    fn compute_for_distribution(
        &self,
        time_card: &mut dyn TimeCard,
        index: usize,
        prev: usize,
        settings: &Settings,
        rule_item: &RuleItem,
        rates: &OvertimeRateLookup<'_>,
    ) {
        let (Some(prev_end), Some(start)) = (
            time_card.shifts()[prev].end_date_time(),
            time_card.shifts()[index].start_date_time(),
        ) else {
            return;
        };
        let gap_minutes = f64::from(duration_in_minutes(prev_end, start));
        let shortfall_hours = (settings.required_gap_minutes - gap_minutes) / 60.0;
        if gap_minutes >= settings.required_gap_minutes {
            return;
        }
        if !settings.convert_hours_to_ot {
            self.compute_earning(time_card, index, rule_item, shortfall_hours, settings);
            return;
        }

        let regular_ids = time_card.regular_hours_distribution_type_ids();
        let mut regulars: Vec<usize> = time_card.shifts()[index]
            .hours_distributions()
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.hours_distribution_type_id()
                    .is_some_and(|id| regular_ids.contains(&id))
            })
            .map(|(i, _)| i)
            .collect();
        regulars.sort_by_key(|&i| time_card.shifts()[index].hours_distributions()[i].date());
        for distribution_index in regulars {
            self.compute_shift_ot(
                time_card,
                index,
                prev,
                distribution_index,
                shortfall_hours,
                settings,
                rates,
            );
        }
    }

    /// `computeEarning`.
    fn compute_earning(
        &self,
        time_card: &mut dyn TimeCard,
        index: usize,
        rule_item: &RuleItem,
        shortfall_hours: f64,
        settings: &Settings,
    ) {
        let type_ids = time_card.hours_distribution_type_ids();
        let shift = time_card.shifts()[index].clone();
        let Some(rate) = self.rate(time_card, &shift, settings.ot_factor) else {
            return;
        };
        let total_hours: f64 = shift
            .hours_distributions()
            .iter()
            .filter(|d| {
                d.hours_distribution_type_id()
                    .is_some_and(|id| type_ids.contains(&id))
            })
            .map(|d| d.hours())
            .sum();
        if total_hours > 0.0 {
            let hours = if settings.convert_full_second_shift {
                total_hours
            } else {
                total_hours.min(shortfall_hours)
            };
            self.create_earning(
                time_card,
                &shift,
                rule_item,
                rate,
                hours,
                settings.earning_id,
            );
        }
    }

    /// `getRate`: hourly rate floored at minimum wage, times the factor.
    fn rate(&self, time_card: &dyn TimeCard, shift: &EmployeeShift, factor: f64) -> Option<f64> {
        let employee = time_card.employee()?;
        let status = employee.employee_job_status(shift.job_id(), shift.shift_date())?;
        let wage = self.min_wage.min_wage(
            employee.property_id(),
            Some(shift.job_id()),
            shift.shift_date(),
        );
        let hourly = if status.hourly_rate() < wage {
            wage
        } else {
            status.hourly_rate()
        };
        Some(round_currency(hourly * factor))
    }

    /// `createEarning`.
    fn create_earning(
        &self,
        time_card: &mut dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
        rate: f64,
        hours: f64,
        earning_id: i32,
    ) {
        let Some(earning_type) = self.earning_types.find_by_id(earning_id) else {
            return;
        };
        let mut earning = crate::rules::algorithm::shiftearning::new_shift_earning(
            shift,
            earning_type.id(),
            rule_item,
            true,
        );
        earning.set_note(rule_item.name());
        match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars | UOM::Units | UOM::Days => {
                earning.set_dollars(0.0);
                earning.set_hours(hours);
                earning.set_rate(rate);
            }
            UOM::Dollars => {
                earning.set_dollars(round_to(rate * hours, 2));
                earning.set_hours(0.0);
                earning.set_rate(0.0);
            }
            _ => {}
        }
        earning.calc_and_set_total_dollars();
        if earning.dollars() != 0.0 || earning.hours() != 0.0 {
            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }

    /// `computeShiftOT`.
    #[allow(clippy::too_many_arguments)]
    fn compute_shift_ot(
        &self,
        time_card: &mut dyn TimeCard,
        index: usize,
        prev: usize,
        distribution_index: usize,
        shortfall_hours: f64,
        settings: &Settings,
        rates: &OvertimeRateLookup<'_>,
    ) {
        let regular = time_card.shifts()[index].hours_distributions()[distribution_index].clone();
        let total_regular = regular.hours();
        let mut ot_hours = if settings.convert_full_second_shift || total_regular < shortfall_hours
        {
            total_regular
        } else {
            shortfall_hours
        };
        if settings.reduce_second_shift_ot
            && let Some(prior) = ot_distribution(&time_card.shifts()[prev], settings.ot_type_id)
            && prior.hours() > 0.0
        {
            ot_hours -= prior.hours();
            if ot_hours <= 0.0 {
                return;
            }
        }

        let shift = time_card.shifts()[index].clone();
        let premium_rate = rates.rate(&shift, time_card, ot_hours, regular.base_rate());
        self.update_ot_distribution(time_card, index, settings, &regular, premium_rate, ot_hours);

        let remaining = if settings.convert_full_second_shift || total_regular == ot_hours {
            round_hours(0.0)
        } else {
            round_hours(total_regular - ot_hours)
        };
        time_card.shifts_mut()[index].hours_distributions_mut()[distribution_index]
            .set_hours(remaining);
    }

    /// `updateOTHoursDistribution`.
    fn update_ot_distribution(
        &self,
        time_card: &mut dyn TimeCard,
        index: usize,
        settings: &Settings,
        regular: &HoursDistribution,
        premium_rate: f64,
        hours: f64,
    ) {
        let Some(ot_type) = settings.ot_type_id else {
            return;
        };
        let new_distribution = create_premium_distribution_at_rate(
            regular,
            hours,
            premium_rate,
            ot_type,
            Some(settings.rule_item_id),
            Some(settings.rule_item_id),
        );
        let shift = &mut time_card.shifts_mut()[index];
        match shift
            .hours_distributions_mut()
            .iter_mut()
            .find(|d| d.hours_distribution_type_id() == Some(ot_type))
        {
            Some(existing) => {
                let total_costs = round_currency(hours * (regular.base_rate() + premium_rate));
                let total_hours = round_hours(existing.hours() + hours);
                let updated_rate =
                    ((existing.total_costs() + total_costs) / total_hours) - regular.base_rate();
                existing.set_hours(total_hours);
                existing.set_premium_rate(updated_rate);
                existing.set_rate_rule_item_id(Some(settings.rule_item_id));
                existing.set_hours_rule_item_id(Some(settings.rule_item_id));
            }
            None => shift.add_hours_distribution(new_distribution),
        }
    }
}

/// `getHoursDistributionFromShift`: the first distribution of this type.
fn ot_distribution(shift: &EmployeeShift, type_id: Option<i32>) -> Option<&HoursDistribution> {
    shift
        .hours_distributions()
        .iter()
        .find(|d| d.hours_distribution_type_id() == type_id)
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftDifferenceOT
    for ShiftDifferenceOTRule<T, E, M>
{
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        rates: &OvertimeRateLookup<'_>,
    ) {
        self.run(time_card, rule_item, rates);
    }
}

/// Dispatch for the family's single rule class.
pub struct ShiftDifferenceOTRegistry<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    rule: ShiftDifferenceOTRule<T, E, M>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    ShiftDifferenceOTRegistry<T, E, M>
{
    /// Build over the ports the rule needs.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            rule: ShiftDifferenceOTRule::new(earning_types, earnings, min_wage),
        }
    }
}

impl<T, E, M> RuleRegistry<dyn ShiftDifferenceOT> for ShiftDifferenceOTRegistry<T, E, M>
where
    T: EarningTypePort + 'static,
    E: EmployeeEarningPort + 'static,
    M: MinWagePort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ShiftDiffOt
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ShiftDifferenceOT + 'static)> {
        (rule_class == RuleClass::ShiftDifferenceOtSdo)
            .then_some(&self.rule as &dyn ShiftDifferenceOT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings, earning_type,
    };
    use crate::rules::algorithm::overtimerate::job_ot_rate::JobOTRateRule;
    use crate::rules::params::RuleParams;
    use joda_rs::LocalDateTime;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }
    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                1,
                today().minus_years(1),
                today().plus_years(1),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        )
    }
    fn item(extra: RuleParams) -> RuleItem {
        let mut p = rule_params! { OT_FACTOR_PROP => "1.5", EARNING => "1" };
        for (k, v) in extra.iter() {
            p.set(k, v);
        }
        RuleItem::new(
            1,
            1,
            "Shift Difference OT",
            RuleClass::ShiftDifferenceOtSdo,
            p,
        )
    }
    fn dist(day: i64, type_id: i32, hours: f64) -> HoursDistribution {
        HoursDistribution::new(1, today().plus_days(day), Some(type_id), hours, 0.0)
    }
    fn shift(
        id: i32,
        day: i64,
        from: i32,
        to: i32,
        dists: Vec<HoursDistribution>,
    ) -> EmployeeShift {
        let d = today().plus_days(day);
        let at = |h| d.at_time(joda_rs::LocalTime::of(h, 0, 0));
        EmployeeShift::new(id, 1, 1, d, ShiftType::Actual, Vec::new())
            .with_property_id(1)
            .with_times(Some(at(from)), Some(at(to)))
            .with_hours_distributions(dists)
    }
    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
            .with_calculation_start_date(today())
    }
    fn rule(
        saved: SavedEarnings,
    ) -> ShiftDifferenceOTRule<FixedEarningType, SavedEarnings, FixedMinWage> {
        ShiftDifferenceOTRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved,
            FixedMinWage(5.0),
        )
    }

    struct OtRates(JobOTRateRule<FixedMinWage>);
    impl RuleRegistry<dyn OvertimeRateRule> for OtRates {
        fn rule_type(&self) -> RuleType {
            RuleType::OvertimeRate
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn OvertimeRateRule + 'static)> {
            Some(&self.0)
        }
    }
    fn job_ot_set() -> RuleSet {
        RuleSet::new(
            1,
            1,
            "ot",
            RuleType::OvertimeRate,
            0,
            vec![RuleItem::new(
                9,
                1,
                "job ot",
                RuleClass::JobOrr,
                Default::default(),
            )],
        )
    }
    fn run(c: &mut TimeCardData, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        let registry = OtRates(JobOTRateRule::new(FixedMinWage(5.0)));
        let default_set = job_ot_set();
        let none = |_: i32, _: LocalDate| -> Option<&RuleSet> { None };
        let lookup = OvertimeRateLookup {
            registry: &registry,
            rule_set_for: &none,
            default_rule_set: &default_set,
        };
        rule(saved.clone()).execute(c, &item(params), &lookup);
        saved
    }

    fn single_day() -> Vec<EmployeeShift> {
        vec![
            shift(1, 0, 12, 22, vec![dist(0, 1, 10.0)]),
            shift(2, 1, 0, 10, vec![dist(1, 1, 10.0)]),
        ]
    }

    // ---- java_parity_tests: ShiftDifferenceOTRuleImplTest.groovy ----

    #[test]
    fn rule_should_create_a_new_earning_if_shift_difference_is_less_than_10_hours() {
        let mut c = card(single_day());
        run(&mut c, RuleParams::new());
        assert_eq!(c.shifts()[0].hours_distributions().len(), 1);
        assert_eq!(c.shifts()[0].hours_distributions()[0].hours(), 10.0);
        assert_eq!(c.earnings().len(), 1);
        let e = &c.earnings()[0];
        assert_eq!(
            (e.shift_id(), e.rule_item_id(), e.hours()),
            (Some(2), Some(1), 8.0)
        );
        // The distributions are untouched.
        let second = c.shifts()[1].hours_distributions();
        assert!(second.iter().all(|d| d.date() == today().plus_days(1)));
        assert!(
            second
                .iter()
                .any(|d| d.hours_distribution_type_id() == Some(1) && d.hours() == 10.0)
        );
    }

    #[test]
    fn should_include_hours_distributions_towards_ot_if_the_gap_is_not_short() {
        let split = vec![
            shift(1, -1, 19, 5, vec![dist(-1, 1, 5.0), dist(0, 1, 5.0)]).with_times(
                Some(LocalDateTime::of(2014, 5, 5, 19, 0, 0)),
                Some(LocalDateTime::of(2014, 5, 6, 5, 0, 0)),
            ),
            shift(2, 1, 0, 10, vec![dist(1, 1, 10.0)]),
        ];
        let mut c = card(split);
        run(&mut c, RuleParams::new());
        assert_eq!(c.shifts()[0].hours_distributions().len(), 2);
        assert_eq!(c.shifts()[1].hours_distributions().len(), 1);
        assert_eq!(c.shifts()[1].hours_distributions()[0].hours(), 10.0);
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn should_still_award_earning_if_hours_are_already_paid_at_premium() {
        let shifts = vec![
            shift(1, 0, 12, 22, vec![dist(0, 1, 10.0)]),
            shift(2, 1, 0, 10, vec![dist(1, 2, 10.0)]),
        ];
        let mut c = card(shifts);
        run(&mut c, RuleParams::new());
        assert_eq!(
            c.shifts()[1].hours_distributions()[0].hours_distribution_type_id(),
            Some(2)
        );
        assert_eq!(c.shifts()[1].hours_distributions()[0].hours(), 10.0);
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn if_shift_is_already_paid_at_ot_the_earning_covers_only_the_hours_across_all_types() {
        let mut ot = dist(1, 2, 5.0);
        ot.set_base_rate(10.0);
        ot.set_premium_rate(5.0);
        let mut regular = dist(1, 1, 2.0);
        regular.set_base_rate(10.0);
        let shifts = vec![
            shift(1, 0, 12, 22, vec![dist(0, 1, 10.0)]),
            shift(2, 1, 5, 15, vec![regular, ot]),
        ];
        let mut c = card(shifts);
        run(&mut c, RuleParams::new());
        let second = c.shifts()[1].hours_distributions();
        assert!(
            second
                .iter()
                .any(|d| d.hours_distribution_type_id() == Some(1) && d.hours() == 2.0)
        );
        assert!(
            second
                .iter()
                .any(|d| d.hours_distribution_type_id() == Some(2)
                    && d.hours() == 5.0
                    && d.total_costs() == 75.0
                    && d.premium_rate() == 5.0)
        );
        assert_eq!(c.earnings().len(), 1);
        let e = &c.earnings()[0];
        assert_eq!(
            (e.shift_id(), e.rule_item_id(), e.hours()),
            (Some(2), Some(1), 3.0)
        );
    }

    #[test]
    fn excluding_separate_days_skips_shifts_on_different_days() {
        let mut c = card(single_day());
        run(
            &mut c,
            rule_params! { EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP => "true" },
        );
        assert!(c.earnings().is_empty());
    }

    fn same_day() -> Vec<EmployeeShift> {
        vec![
            shift(1, 0, 6, 10, vec![dist(0, 1, 4.0)]),
            shift(2, 0, 12, 22, vec![dist(0, 1, 10.0)]),
        ]
    }

    #[test]
    fn excluding_same_day_skips_shifts_starting_the_same_day() {
        let mut c = card(same_day());
        run(
            &mut c,
            rule_params! { EXCLUDE_SHIFTS_ON_SAME_DAY_PROP => "true" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn excluding_separate_days_still_pays_same_day_shifts() {
        let mut c = card(same_day());
        run(
            &mut c,
            rule_params! { EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP => "true" },
        );
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn excluding_same_day_still_pays_shifts_on_different_days() {
        let mut c = card(single_day());
        run(
            &mut c,
            rule_params! { EXCLUDE_SHIFTS_ON_SAME_DAY_PROP => "true" },
        );
        assert_eq!(c.earnings().len(), 1);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_earning_is_priced_at_the_hourly_rate_times_the_factor() {
        let mut c = card(single_day());
        run(&mut c, RuleParams::new());
        assert_eq!(c.earnings()[0].rate(), 15.0);
    }

    #[test]
    fn convert_full_second_shift_pays_every_hour_of_the_second_shift() {
        let mut c = card(single_day());
        run(
            &mut c,
            rule_params! { CONVERT_FULL_SECOND_SHIFT_PROP => "true" },
        );
        assert_eq!(c.earnings()[0].hours(), 10.0);
    }

    #[test]
    fn convert_hours_to_ot_moves_the_shortfall_into_an_overtime_distribution() {
        let mut c = card(single_day());
        let saved = run(&mut c, rule_params! { CONVERT_HOURS_TO_OT_PROP => "true" });
        assert!(saved.0.borrow().is_empty());
        let second = c.shifts()[1].hours_distributions();
        let regular = second
            .iter()
            .find(|d| d.hours_distribution_type_id() == Some(1))
            .unwrap();
        let ot = second
            .iter()
            .find(|d| d.hours_distribution_type_id() == Some(2))
            .unwrap();
        // 8 of the 10 hours move; the premium rate is whatever the default rule set prices.
        assert_eq!((regular.hours(), ot.hours()), (2.0, 8.0));
        assert_eq!(ot.rate_rule_item_id(), Some(1));
        assert_eq!(ot.hours_rule_item_id(), Some(1));
        assert!(ot.premium_rate() > 0.0);
    }

    #[test]
    fn a_second_conversion_merges_into_the_existing_overtime_distribution() {
        let shifts = vec![
            shift(1, 0, 12, 22, vec![dist(0, 1, 10.0)]),
            shift(2, 1, 0, 10, vec![dist(1, 1, 10.0), dist(1, 2, 1.0)]),
        ];
        let mut c = card(shifts);
        run(&mut c, rule_params! { CONVERT_HOURS_TO_OT_PROP => "true" });
        let second = c.shifts()[1].hours_distributions();
        assert_eq!(
            second
                .iter()
                .filter(|d| d.hours_distribution_type_id() == Some(2))
                .count(),
            1
        );
        let ot = second
            .iter()
            .find(|d| d.hours_distribution_type_id() == Some(2))
            .unwrap();
        assert_eq!(ot.hours(), 9.0);
    }

    #[test]
    fn reduce_second_shift_ot_takes_off_the_previous_shifts_overtime() {
        let shifts = vec![
            shift(1, 0, 12, 22, vec![dist(0, 1, 5.0), dist(0, 2, 3.0)]),
            shift(2, 1, 0, 10, vec![dist(1, 1, 10.0)]),
        ];
        let mut c = card(shifts);
        run(
            &mut c,
            rule_params! { CONVERT_HOURS_TO_OT_PROP => "true", REDUCE_SECOND_SHIFT_OT_PROP => "true" },
        );
        let second = c.shifts()[1].hours_distributions();
        let ot = second
            .iter()
            .find(|d| d.hours_distribution_type_id() == Some(2))
            .unwrap();
        // 8 hours short, less the 3 the first shift already carries.
        assert_eq!(ot.hours(), 5.0);
    }

    #[test]
    fn non_actual_shifts_and_salaried_exempt_jobs_are_ignored() {
        let mut shifts = single_day();
        shifts[1] = EmployeeShift::new(
            2,
            1,
            1,
            today().plus_days(1),
            ShiftType::Schedule,
            Vec::new(),
        )
        .with_times(
            Some(LocalDateTime::of(2014, 5, 7, 0, 0, 0)),
            Some(LocalDateTime::of(2014, 5, 7, 10, 0, 0)),
        )
        .with_hours_distributions(vec![dist(1, 1, 10.0)]);
        let mut c = card(shifts);
        run(&mut c, RuleParams::new());
        assert!(c.earnings().is_empty());

        let exempt = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                1,
                today().minus_years(1),
                today().plus_years(1),
                EmployeePayType::SalariedExempt,
                10.0,
                true,
            )],
        );
        let mut c = card(single_day()).with_employee(exempt);
        run(&mut c, RuleParams::new());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_first_shift_on_the_calculation_start_date_looks_back_to_the_day_before() {
        // Only the second shift is open (the first is in a closed period), yet it
        // is still compared with yesterday's shift on the card.
        let shifts = vec![
            shift(1, -1, 20, 23, vec![dist(-1, 1, 3.0)]),
            shift(2, 0, 3, 10, vec![dist(0, 1, 7.0)]),
        ];
        let mut c = card(shifts);
        run(&mut c, RuleParams::new());
        // Gap 23:00 -> 03:00 is 4h: 6h short, but only 7 hours exist -> min(7, 6) = 6.
        assert_eq!(c.earnings().len(), 1);
        assert_eq!(c.earnings()[0].hours(), 6.0);
    }

    #[test]
    fn config_validation_matches_the_groovy_table() {
        let v = |earning: &str,
                 factor: &str,
                 hours: &str,
                 convert: &str,
                 full: &str,
                 reduce: &str,
                 sep: &str,
                 same: &str| {
            ShiftDifferenceOTRuleConfig.validate(&rule_params! {
                EARNING => earning, OT_FACTOR_PROP => factor, HOURS_BETWEEN_SHIFTS_PROP => hours,
                CONVERT_HOURS_TO_OT_PROP => convert, CONVERT_FULL_SECOND_SHIFT_PROP => full,
                REDUCE_SECOND_SHIFT_OT_PROP => reduce, EXCLUDE_SHIFTS_ON_SEPARATE_DAYS_PROP => sep,
                EXCLUDE_SHIFTS_ON_SAME_DAY_PROP => same
            })
        };
        let none = |r: Vec<String>| r.is_empty();
        assert!(none(v(
            "0", "1", "10.0", "false", "false", "false", "false", "false"
        )));
        assert!(!none(v(
            "0", "0", "10.0", "true", "false", "false", "false", "false"
        )));
        assert!(!none(v(
            "0", "1", "0", "false", "false", "false", "false", "false"
        )));
        assert!(none(v(
            "0", "1", "10.0", "true", "true", "false", "false", "false"
        )));
        assert!(none(v(
            "0", "1", "10.0", "false", "true", "false", "false", "false"
        )));
        assert!(!none(v(
            "0", "1", "10.0", "false", "true", "true", "false", "false"
        )));
        assert!(!none(v(
            "0", "1", "10.0", "true", "true", "true", "false", "false"
        )));
        assert!(!none(v(
            "0", "1", "10.0", "false", "false", "false", "true", "true"
        )));
        assert!(none(v(
            "0", "1", "10.0", "false", "false", "false", "true", "false"
        )));
        assert!(none(v(
            "0", "1", "10.0", "false", "false", "false", "false", "true"
        )));
    }

    #[test]
    fn the_defaults_match_the_groovy_config_test() {
        let d = ShiftDifferenceOTRuleConfig.default_values();
        assert_eq!(d.get(OT_FACTOR_PROP), Some("1.5"));
        assert_eq!(d.get(HOURS_BETWEEN_SHIFTS_PROP), Some("10.0"));
        assert_eq!(d.get(EARNING), Some("0"));
        assert_eq!(d.get(CONVERT_HOURS_TO_OT_PROP), Some("false"));
        assert_eq!(d.get(REDUCE_SECOND_SHIFT_OT_PROP), Some("false"));
    }
}
