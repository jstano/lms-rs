//! `RuleType::DailyDifferential` — nine rules behind `DailyEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/dailyearning/`.
//!
//! Fifth of the seven families prioritized for scheduling; scoped in
//! `PARITY_AUDIT.md` ("Scoping — `dailyearning`"). **Slices 1 and 2** are here:
//! the shared surface and eight of the nine rules — everything except
//! `HolidayDifferential` and the runner.
//!
//! # One call per calendar day
//!
//! Unlike every earlier family, `execute` is not driven per shift: the runner
//! calls it once for each date, and the rule writes that day's earnings onto
//! the time card. Hence `&mut dyn TimeCard` and a `LocalDate`.
//!
//! # What the label-gated rules share
//!
//! All three skip a day when the employee is not active on it
//! ([`Employee::is_active_on_date`](crate::entity::employee::Employee::is_active_on_date))
//! or the day is in a closed period; they measure the day's hours as the sum of
//! `originalHours` over the **hours distributions dated that day** (not shift
//! net hours); and they gate on the day's schedule label.

pub mod anniversary_earning;
pub mod config;
pub mod daily_salaried_allowance;
pub mod days_worked;
pub mod holiday_differential;
pub mod min_daily_hours;
pub mod min_daily_worked_hours;
pub mod minimum_hours_worked;
pub mod no_daily_earning;
pub mod no_daily_hours_with_label;

use crate::entity::earning_type::EarningType;
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::ports::MinWagePort;
use joda_rs::LocalDate;

/// A daily-differential rule. `DailyEarningRuleImpl`.
pub trait DailyEarningRule {
    /// `execute(TimeCard, LocalDate, RuleItem)`.
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem);
}

/// The gate every slice-1 rule opens with:
/// `!employee.isActiveOnDate(date) || !timeCard.isOpenForEditingOn(date)`.
///
/// A card with no employee counts as inactive (Java would throw).
pub fn should_not_run_for_date(time_card: &dyn TimeCard, date: LocalDate) -> bool {
    !time_card
        .employee()
        .is_some_and(|employee| employee.is_active_on_date(date))
        || !time_card.is_open_for_editing_on(date)
}

/// The day's hours as the label-gated rules measure them: the sum of
/// `getOriginalHours()` over every shift's hours distributions dated `date`.
pub fn original_hours_on_day(time_card: &dyn TimeCard, date: LocalDate) -> f64 {
    time_card
        .shifts()
        .iter()
        .flat_map(|shift| shift.hours_distributions())
        .filter(|distribution| distribution.date() == date)
        .map(|distribution| distribution.original_hours())
        .sum()
}

/// `scheduleLabelIsIncluded`: when validation is on (or always, for
/// `NoDailyHoursWithLabel`), the day's first label must equal `label_to_match`,
/// ignoring case and surrounding whitespace on both sides. A day with no label
/// compares as the empty string.
pub fn schedule_label_matches(
    time_card: &dyn TimeCard,
    date: LocalDate,
    label_to_match: &str,
) -> bool {
    let label = time_card
        .schedule_label_for_date(date)
        .map_or("", |l| l.label());
    label.trim().to_lowercase() == label_to_match.trim().to_lowercase()
}

/// `DailyEarningRuleImpl.getEarningRateHomeJob`: the home job's hourly rate on
/// `date`, raised to the minimum wage when the earning type pays at least
/// minimum wage. `None` when the employee has no home job on that date, where
/// Java would throw a `NullPointerException`.
pub fn earning_rate_home_job(
    employee: &Employee,
    date: LocalDate,
    earning_type: &EarningType,
    min_wage: &dyn MinWagePort,
) -> Option<f64> {
    let home = employee.home_employee_job_status(date)?;
    let rate = home.hourly_rate();
    let wage = min_wage.min_wage(employee.property_id(), Some(home.job_id()), date);
    Some(if earning_type.pay_at_least_min_wage() && rate < wage {
        wage
    } else {
        rate
    })
}

/// `DailyEarningRuleImpl.getEarningRate(shift, ...)`: the hourly rate of the
/// job status the **shift's own job** held on the shift's date, raised to the
/// minimum wage when the earning type says so. `None` where Java would throw
/// (no status for that job and date).
pub fn earning_rate_for_shift(
    employee: &Employee,
    shift: &EmployeeShift,
    earning_type: &EarningType,
    min_wage: &dyn MinWagePort,
) -> Option<f64> {
    let status = employee.employee_job_status(shift.job_id(), shift.shift_date())?;
    let rate = status.hourly_rate();
    let wage = min_wage.min_wage(
        employee.property_id(),
        Some(shift.job_id()),
        shift.shift_date(),
    );
    Some(if earning_type.pay_at_least_min_wage() && rate < wage {
        wage
    } else {
        rate
    })
}

use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{
    AssignmentPort, EarningTypePort as EarningTypes, EmployeeEarningPort as EmployeeEarnings,
    HolidayPort, PropertyPort,
};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use anniversary_earning::AnniversaryEarningRule;
use daily_salaried_allowance::DailySalariedAllowanceRule;
use days_worked::DaysWorkedDailyDifferentialRule;
use holiday_differential::HolidayDifferentialRule;
use min_daily_hours::MinDailyHoursEarningRule;
use min_daily_worked_hours::MinDailyWorkedHoursEarningRule;
use minimum_hours_worked::MinimumHoursWorkedDifferentialRule;
use no_daily_earning::NoDailyEarningRule;
use no_daily_hours_with_label::NoDailyHoursWithLabelEarningRule;

/// Dispatch for the nine daily-differential rules. `RuleImplFactory`, narrowed
/// to this family.
///
/// Every rule owns its ports, so the three that several rules share (earning
/// types, the earning writer and minimum wage) must be `Clone` — in practice a
/// cheap handle onto the same store.
pub struct DailyEarningRegistry<T, E, M, H, A, P>
where
    T: EarningTypes,
    E: EmployeeEarnings,
    M: MinWagePort,
    H: HolidayPort,
    A: AssignmentPort,
    P: PropertyPort,
{
    no_daily: NoDailyEarningRule,
    holiday: HolidayDifferentialRule<T, E, M, H, A>,
    min_hours_worked: MinimumHoursWorkedDifferentialRule<T, E, M>,
    days_worked: DaysWorkedDailyDifferentialRule<T, E, P>,
    min_daily_hours: MinDailyHoursEarningRule<T, E, M>,
    no_daily_hours_with_label: NoDailyHoursWithLabelEarningRule<T, E, M>,
    min_daily_worked_hours: MinDailyWorkedHoursEarningRule<T, E, M>,
    salaried_allowance: DailySalariedAllowanceRule<T, E>,
    anniversary: AnniversaryEarningRule<T>,
}

impl<T, E, M, H, A, P> DailyEarningRegistry<T, E, M, H, A, P>
where
    T: EarningTypes + Clone,
    E: EmployeeEarnings + Clone,
    M: MinWagePort + Clone,
    H: HolidayPort,
    A: AssignmentPort,
    P: PropertyPort,
{
    /// Build every rule over the shared ports.
    pub fn new(
        earning_types: T,
        earnings: E,
        min_wage: M,
        holidays: H,
        assignments: A,
        property: P,
    ) -> Self {
        Self {
            no_daily: NoDailyEarningRule,
            holiday: HolidayDifferentialRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
                holidays,
                assignments,
            ),
            min_hours_worked: MinimumHoursWorkedDifferentialRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            days_worked: DaysWorkedDailyDifferentialRule::new(
                earning_types.clone(),
                earnings.clone(),
                property,
            ),
            min_daily_hours: MinDailyHoursEarningRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            no_daily_hours_with_label: NoDailyHoursWithLabelEarningRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage.clone(),
            ),
            min_daily_worked_hours: MinDailyWorkedHoursEarningRule::new(
                earning_types.clone(),
                earnings.clone(),
                min_wage,
            ),
            salaried_allowance: DailySalariedAllowanceRule::new(earning_types.clone(), earnings),
            anniversary: AnniversaryEarningRule::new(earning_types),
        }
    }
}

impl<T, E, M, H, A, P> RuleRegistry<dyn DailyEarningRule> for DailyEarningRegistry<T, E, M, H, A, P>
where
    T: EarningTypes + 'static,
    E: EmployeeEarnings + 'static,
    M: MinWagePort + 'static,
    H: HolidayPort + 'static,
    A: AssignmentPort + 'static,
    P: PropertyPort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::DailyDifferential
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn DailyEarningRule + 'static)> {
        match rule_class {
            RuleClass::NoDdf => Some(&self.no_daily),
            RuleClass::HolidayDifDdf => Some(&self.holiday),
            RuleClass::MinHoursWorkedDdf => Some(&self.min_hours_worked),
            RuleClass::DaysWorkedDdf => Some(&self.days_worked),
            RuleClass::MinDailyHrsDdf => Some(&self.min_daily_hours),
            RuleClass::NoDailyHrsDdf => Some(&self.no_daily_hours_with_label),
            RuleClass::MinDailyWrkdHrsDdf => Some(&self.min_daily_worked_hours),
            RuleClass::DlySalAllowDdf => Some(&self.salaried_allowance),
            RuleClass::AnniDdf => Some(&self.anniversary),
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
    impl EarningTypes for Types {
        fn find_by_id(&self, _id: i32) -> Option<EarningType> {
            Some(earning_type(UOM::Hours))
        }
    }
    struct Calendar;
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            Vec::new()
        }
    }
    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, _id: i32) -> Option<Assignment> {
            None
        }
    }
    struct Week;
    impl PropertyPort for Week {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            LocalDate::of(2014, 5, 6)
        }
    }

    /// All nine catalogue entries dispatch, none is missing.
    #[test]
    fn every_daily_differential_rule_class_dispatches() {
        let registry = DailyEarningRegistry::new(
            Types,
            SavedEarnings::default(),
            FixedMinWage(10.0),
            Calendar,
            Jobs,
            Week,
        );
        assert_eq!(registry.dispatchable().len(), 9);
        assert!(registry.missing().is_empty());
    }
}

/// Fixtures shared by the slice's rule tests.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::ports::{EarningTypePort, EmployeeEarningPort};
    use std::cell::RefCell;
    use std::rc::Rc;

    pub const TODAY: fn() -> LocalDate = || LocalDate::of(2014, 5, 6);
    pub const JOB: i32 = 10;

    pub fn today() -> LocalDate {
        TODAY()
    }

    /// Always answers one earning type, whatever id is asked for — the specs
    /// stub `findByID(_)` / `findByID(0)` the same way.
    pub struct FixedEarningType(pub EarningType);

    impl EarningTypePort for FixedEarningType {
        fn find_by_id(&self, _id: i32) -> Option<EarningType> {
            Some(self.0.clone())
        }
    }

    /// An earning type of the given unit, flags cleared.
    pub fn earning_type(uom: UOM) -> EarningType {
        EarningType::new(0, 1, "Lieu", EarnType::Regular, uom)
    }

    #[derive(Clone)]
    pub struct FixedMinWage(pub f64);

    impl MinWagePort for FixedMinWage {
        fn min_wage(&self, _p: i32, _j: Option<i32>, _d: LocalDate) -> f64 {
            self.0
        }
    }

    /// Records every save through a handle the test keeps.
    #[derive(Clone, Default)]
    pub struct SavedEarnings(pub Rc<RefCell<Vec<EmployeeEarning>>>);

    impl EmployeeEarningPort for SavedEarnings {
        fn save(&mut self, earning: EmployeeEarning) {
            self.0.borrow_mut().push(earning);
        }
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _e: i32,
            _p: &date_range_rs::DateRange,
            _t: &[i32],
        ) -> Option<f64> {
            None
        }
        fn banked_rate_for_rule(&self, _e: i32, _h: i32, _c: i32) -> Option<f64> {
            None
        }
    }

    /// An hourly employee with a home job at `rate`, active across `today`.
    pub fn employee(rate: f64, status: EmployeeStatusType) -> Employee {
        let day = today();
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                LocalDate::of(2013, 5, 6),
                LocalDate::of(2015, 5, 6),
                EmployeePayType::Hourly,
                rate,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(day, day, status)])
    }

    /// A shift carrying one distribution of `original_hours` dated `date`.
    pub fn shift_with_hours(date: LocalDate, original_hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new()).with_hours_distributions(
            vec![HoursDistribution::new(1, date, None, original_hours, 0.0)],
        )
    }

    pub fn card(employee: Employee, shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee)
            .with_shifts(shifts)
    }
}
