//! `RuleType::ShiftDifferential` — 23 rules behind `ShiftEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/shiftearning/`,
//! runner at `.../server/labor/calcshift/rules/ShiftDifferentialRunner.java`.
//!
//! The last of the seven families prioritized for scheduling, scoped in
//! `PARITY_AUDIT.md` ("Scoping — `shiftearning`") and ported in five slices.
//! **Slices 1-3** are here: the shared base, nine simple rules, the four
//! break and window rules, and the three time-of-day rules.
//!
//! # Per shift
//!
//! `execute(EmployeeShift, TimeCard, RuleItem)` runs once for each shift. The
//! rule reads the shift and writes earnings onto the card, so it takes the shift
//! by shared reference and the card mutably; the runner (slice 5) hands it a
//! clone of the card's shift, as `ShiftCorrectionRunner` does.
//!
//! # The base class's pricing helpers
//!
//! `ShiftEarningRuleImpl` has `getEarningRate` (a job's rate on a date, floored
//! at minimum wage if the earning type says so), `getEarningRateMinimumWage`
//! and `getEarningRateHomeJob`. They are
//! [`earning_rate_for_job`](crate::rules::algorithm::dailyearning::earning_rate_for_job),
//! [`MinWagePort::min_wage`](crate::rules::ports::MinWagePort::min_wage) and
//! [`earning_rate_home_job`](crate::rules::algorithm::dailyearning::earning_rate_home_job),
//! shared with `dailyearning`.

pub mod anniversary_worked_shift_differential;
pub mod break_period_penalty;
pub mod break_period_penalty_hours;
pub mod config;
pub mod contract_guarantee;
pub mod contract_to_full_time;
pub mod daily_spread_shift_differential;
pub mod dow_flat;
pub mod dow_holiday;
pub mod dow_rate_factor;
pub mod holiday_rate_factor;
pub mod holiday_time_of_day;
pub mod hours_threshold_fixed;
pub mod hours_threshold_fixed_with_max_cap;
pub mod minimum_hours_worked_shift;
pub mod no_earning;
pub mod paid_break_fixed_hrs;
pub mod shift_start_time_flat;
pub mod split_shift_differential_rate;
pub mod start_end_between_premium;
pub mod time_between_shifts;
pub mod time_of_day_differential_rate;
pub mod time_of_day_pct_differential_rate;
pub mod utility;
pub mod weekly_job_hrs_fixed;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A shift-differential rule. `ShiftEarningRuleImpl`.
pub trait ShiftEarningRule {
    /// `execute(EmployeeShift, TimeCard, RuleItem)`.
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem);
}

/// The gate most of the family opens with:
/// `shift.getErrors().isEmpty() && dataset.isOpenForEditingFor(shift)`.
pub fn shift_is_eligible(shift: &EmployeeShift, time_card: &dyn TimeCard) -> bool {
    !shift.has_errors() && time_card.is_open_for_editing_for_shift(shift)
}

/// The earning every rule here starts from: a RULE earning on the shift's
/// employee and job, dated and paid on the shift's date, tagged with the rule
/// item (id **and** class) and, when `attach_shift`, with the shift itself.
pub fn new_shift_earning(
    shift: &EmployeeShift,
    earning_type_id: i32,
    rule_item: &RuleItem,
    attach_shift: bool,
) -> EmployeeEarning {
    let date = shift.shift_date();
    let mut earning = EmployeeEarning::new(
        0,
        shift.employee_id(),
        shift.job_id(),
        earning_type_id,
        date,
        0.0,
        0.0,
        crate::common::enums::earning_source::EarningSource::Rule,
    )
    .from_rule_item(rule_item, attach_shift.then(|| shift.id()));
    earning.set_pay_date(Some(date));
    earning
}

use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{
    AssignmentPort, EarningTypePort, EmployeeEarningPort, HolidayPort, JobMinWagePort, MinWagePort,
    PropertyPort,
};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use anniversary_worked_shift_differential::AnniversaryWorkedShiftDifferentialRule;
use break_period_penalty::BreakPeriodPenaltyRule;
use break_period_penalty_hours::BreakPeriodPenaltyHoursRule;
use contract_guarantee::ContractGuaranteeRule;
use contract_to_full_time::ContractToFullTimeRule;
use daily_spread_shift_differential::DailySpreadShiftDifferentialRule;
use dow_flat::DOWFlatRule;
use dow_holiday::DOWHolidayRule;
use dow_rate_factor::DOWRateFactorRule;
use holiday_rate_factor::HolidayRateFactorRule;
use holiday_time_of_day::HolidayTimeOfDayRule;
use hours_threshold_fixed::HoursThresholdFixedRule;
use hours_threshold_fixed_with_max_cap::HoursThresholdFixedWithMaxCapRule;
use minimum_hours_worked_shift::MinimumHoursWorkedShiftDifferentialRule;
use no_earning::NoEarningRule;
use paid_break_fixed_hrs::PaidBreakFixedHrsRule;
use shift_start_time_flat::ShiftStartTimeFlatRule;
use split_shift_differential_rate::SplitShiftDifferentialRateRule;
use start_end_between_premium::StartEndBetweenPremiumRule;
use time_between_shifts::TimeBetweenShiftsRule;
use time_of_day_differential_rate::TimeOfDayDifferentialRateRule;
use time_of_day_pct_differential_rate::TimeOfDayPctDifferentialRateRule;
use weekly_job_hrs_fixed::WeeklyJobHrsFixedRule;

/// Dispatch for the twenty-three shift-differential rules. `RuleImplFactory`,
/// narrowed to this family.
///
/// Every rule owns its ports, so those several rules share must be `Clone` —
/// in practice a cheap handle onto the same store.
#[allow(clippy::type_complexity)]
pub struct ShiftEarningRegistry<T, E, M, P, H, A, W>
where
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    P: PropertyPort,
    H: HolidayPort,
    A: AssignmentPort,
    W: JobMinWagePort,
{
    time_of_day: TimeOfDayDifferentialRateRule<T, E>,
    time_of_day_pct: TimeOfDayPctDifferentialRateRule<T, E, M>,
    hours_threshold_fixed: HoursThresholdFixedRule<E>,
    split_shift: SplitShiftDifferentialRateRule<T, E, W>,
    break_period_penalty: BreakPeriodPenaltyRule<T, E, M>,
    break_period_penalty_hours: BreakPeriodPenaltyHoursRule<T, E, M>,
    no_earning: NoEarningRule,
    dow_flat: DOWFlatRule<E>,
    dow_rate_factor: DOWRateFactorRule<T, E, M>,
    dow_holiday: DOWHolidayRule<T, E, M, H, A>,
    shift_start_flat: ShiftStartTimeFlatRule<E>,
    weekly_job_hrs_fixed: WeeklyJobHrsFixedRule<E, P>,
    paid_break_fixed_hrs: PaidBreakFixedHrsRule<T, E, M>,
    holiday_rate_factor: HolidayRateFactorRule<T, E, M, H, A>,
    holiday_time_of_day: HolidayTimeOfDayRule<T, E, M, H, A>,
    contract_guarantee: ContractGuaranteeRule<T, E, A>,
    contract_to_full_time: ContractToFullTimeRule<T, E, P>,
    start_end_between: StartEndBetweenPremiumRule<E>,
    minimum_hours_worked: MinimumHoursWorkedShiftDifferentialRule<T, E, M>,
    daily_spread: DailySpreadShiftDifferentialRule<T, E, M>,
    hours_threshold_fixed_max_cap: HoursThresholdFixedWithMaxCapRule<E, P>,
    anniversary_worked: AnniversaryWorkedShiftDifferentialRule<T, M>,
    time_between_shifts: TimeBetweenShiftsRule<T, E>,
}

impl<T, E, M, P, H, A, W> ShiftEarningRegistry<T, E, M, P, H, A, W>
where
    T: EarningTypePort + Clone,
    E: EmployeeEarningPort + Clone,
    M: MinWagePort + Clone,
    P: PropertyPort + Clone,
    H: HolidayPort + Clone,
    A: AssignmentPort + Clone,
    W: JobMinWagePort,
{
    /// Build every rule over the shared ports.
    pub fn new(
        earning_types: T,
        earnings: E,
        min_wage: M,
        property: P,
        holidays: H,
        assignments: A,
        job_min_wage: W,
    ) -> Self {
        Self {
            time_of_day: TimeOfDayDifferentialRateRule::new(
                earning_types.clone(),
                earnings.clone(),
            ),
            time_of_day_pct: TimeOfDayPctDifferentialRateRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            hours_threshold_fixed: HoursThresholdFixedRule::new(earnings.clone()),
            split_shift: SplitShiftDifferentialRateRule::new(
                earning_types.clone(),
                earnings.clone(),
                job_min_wage,
            ),
            break_period_penalty: BreakPeriodPenaltyRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            break_period_penalty_hours: BreakPeriodPenaltyHoursRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            no_earning: NoEarningRule,
            dow_flat: DOWFlatRule::new(earnings.clone()),
            dow_rate_factor: DOWRateFactorRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            dow_holiday: DOWHolidayRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
                holidays.clone(),
                assignments.clone(),
            ),
            shift_start_flat: ShiftStartTimeFlatRule::new(earnings.clone()),
            weekly_job_hrs_fixed: WeeklyJobHrsFixedRule::new(earnings.clone(), property.clone()),
            paid_break_fixed_hrs: PaidBreakFixedHrsRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            holiday_rate_factor: HolidayRateFactorRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
                holidays.clone(),
                assignments.clone(),
            ),
            holiday_time_of_day: HolidayTimeOfDayRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
                holidays,
                assignments.clone(),
            ),
            contract_guarantee: ContractGuaranteeRule::new(
                earning_types.clone(),
                earnings.clone(),
                assignments,
            ),
            contract_to_full_time: ContractToFullTimeRule::new(
                earning_types.clone(),
                earnings.clone(),
                property.clone(),
            ),
            start_end_between: StartEndBetweenPremiumRule::new(earnings.clone()),
            minimum_hours_worked: MinimumHoursWorkedShiftDifferentialRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            daily_spread: DailySpreadShiftDifferentialRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            hours_threshold_fixed_max_cap: HoursThresholdFixedWithMaxCapRule::new(
                earnings.clone(),
                property,
            ),
            anniversary_worked: AnniversaryWorkedShiftDifferentialRule::new(
                earning_types.clone(),
                min_wage,
            ),
            time_between_shifts: TimeBetweenShiftsRule::new(earning_types, earnings),
        }
    }
}

impl<T, E, M, P, H, A, W> RuleRegistry<dyn ShiftEarningRule>
    for ShiftEarningRegistry<T, E, M, P, H, A, W>
where
    T: EarningTypePort + 'static,
    E: EmployeeEarningPort + 'static,
    M: MinWagePort + 'static,
    P: PropertyPort + 'static,
    H: HolidayPort + 'static,
    A: AssignmentPort + 'static,
    W: JobMinWagePort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ShiftDifferential
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn ShiftEarningRule + 'static)> {
        match rule_class {
            RuleClass::TimeOfDayDifRateDif => Some(&self.time_of_day),
            RuleClass::TodPctDifRateDif => Some(&self.time_of_day_pct),
            RuleClass::HrsThreshFixedDif => Some(&self.hours_threshold_fixed),
            RuleClass::SplitShiftDifRateDif => Some(&self.split_shift),
            RuleClass::BreakPeriodPenaltyDif => Some(&self.break_period_penalty),
            RuleClass::BreakPerPenaltyHrsDif => Some(&self.break_period_penalty_hours),
            RuleClass::NoDif => Some(&self.no_earning),
            RuleClass::DowFlatDif => Some(&self.dow_flat),
            RuleClass::DowFactorDif => Some(&self.dow_rate_factor),
            RuleClass::DowHolidayDif => Some(&self.dow_holiday),
            RuleClass::ShiftStartFlatDif => Some(&self.shift_start_flat),
            RuleClass::WklyJobHrsFixedDif => Some(&self.weekly_job_hrs_fixed),
            RuleClass::PdBreakFixedHrsDif => Some(&self.paid_break_fixed_hrs),
            RuleClass::HolidayFactorDif => Some(&self.holiday_rate_factor),
            RuleClass::HolidayTodDif => Some(&self.holiday_time_of_day),
            RuleClass::ContractGuaranteeDif => Some(&self.contract_guarantee),
            RuleClass::ContractToFullDif => Some(&self.contract_to_full_time),
            RuleClass::StartEndBetweenDif => Some(&self.start_end_between),
            RuleClass::MinimumHoursWorkedShiftDif => Some(&self.minimum_hours_worked),
            RuleClass::DailySpreadShiftDif => Some(&self.daily_spread),
            RuleClass::HrsThreshFixedWithMaxCapDif => Some(&self.hours_threshold_fixed_max_cap),
            RuleClass::AnnWrkdDif => Some(&self.anniversary_worked),
            RuleClass::TimeBetweenShiftsDif => Some(&self.time_between_shifts),
            _ => None,
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::common::enums::uom::UOM;
    use crate::entity::assignment::Assignment;
    use crate::entity::holiday::Holiday;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use joda_rs::LocalDate;

    #[derive(Clone)]
    struct Types;
    impl EarningTypePort for Types {
        fn find_by_id(&self, _id: i32) -> Option<crate::entity::earning_type::EarningType> {
            Some(earning_type(UOM::Hours))
        }
    }
    #[derive(Clone)]
    struct Property;
    impl PropertyPort for Property {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            LocalDate::of(2014, 5, 11)
        }
    }
    #[derive(Clone)]
    struct Calendar;
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            Vec::new()
        }
    }
    #[derive(Clone)]
    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }
    struct Wages;
    impl JobMinWagePort for Wages {
        fn effective_min_wage(&self, _j: i32, _d: LocalDate) -> f64 {
            0.0
        }
        fn current_property_min_wage(&self) -> f64 {
            0.0
        }
    }

    fn registry()
    -> ShiftEarningRegistry<Types, SavedEarnings, FixedMinWage, Property, Calendar, Jobs, Wages>
    {
        ShiftEarningRegistry::new(
            Types,
            SavedEarnings::default(),
            FixedMinWage(1.0),
            Property,
            Calendar,
            Jobs,
            Wages,
        )
    }

    /// All twenty-three catalogue entries dispatch, none is missing.
    #[test]
    fn every_shift_differential_rule_class_dispatches() {
        let registry = registry();
        assert_eq!(registry.dispatchable().len(), 23);
        assert!(registry.missing().is_empty());
    }
}
