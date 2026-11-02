//! `RuleType::ScheduleChangeValidation` — three rules that price a manager's
//! edit to a published schedule.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulechangevalidation/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/schedulechangevalidation/`,
//! runner `…/calcshift/rules/ScheduleChangeValidationRunner.java`.
//!
//! When a manager adds, removes or moves a shift close to its start (a "fair work
//! week" rule), the employee may be owed a penalty. These rules decide that; they
//! **return an [`EarningRequest`]** rather than writing an earning, so the
//! scheduling UI can show it and let a user with permission overwrite it.
//! Either shift may be absent: `original == None` is an added shift,
//! `updated == None` a removed one.
//!
//! # The shared checks
//!
//! - **Length:** the shift's stored net hours — the original's, or the updated's
//!   when there is no original — must lie within `[minShiftLength, maxShiftLength]`.
//! - **Notice:** the days from "now at the current property" to the shift's start
//!   must be **below** `firstThreshold` and **above** `secondThreshold`.
//! - **The planning-period option is dead.** Java reads `usePlanningPeriodStartDate`
//!   with `Boolean.getBoolean(String)`, which looks up a *system property* of that
//!   name and so is false for any normal configuration. The shift's own start
//!   time is therefore always used, and the port ignores the parameter
//!   (divergence 122).
//!
//! # The request
//!
//! A `MANUAL` earning dated and paid on the shift date, for the original shift's
//! employee and job (or the updated's), `forceDuplicate = true`. The amount is
//! `earningAmount` as hours or dollars according to the earning type's unit
//! (hours for `HOURS`, cost for the cost-based units, an unknown type counting as
//! `HOURS`). The note is the supplied description, else the earning type's name.
//! The add/remove rules can instead pay `earningAmount` **percent** of the hours
//! changed.
//!
//! `execute` takes the parameters separately from the rule item, as Java does;
//! the specs pass a blank rule item with their own parameter map. Java also takes
//! the `ScheduleCalcDataSet`, which no rule here reads, so it is dropped.

pub mod add_hours_penalty;
pub mod config;
pub mod remove_hours_penalty;
pub mod shift_modification_penalty;

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::earning_source::EarningSource;
use crate::entity::earning_request::EarningRequest;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::schedulechangevalidation::config::{
    EARNING_AMOUNT, EARNING_TYPE, FIRST_THRESHOLD, MAX_SHIFT_LENGTH, MIN_SHIFT_LENGTH,
    PERCENTAGE_OF_HOURS, SECOND_THRESHOLD,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::params::RuleParams;
use crate::rules::ports::{CurrentDateTimePort, EarningTypePort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use add_hours_penalty::AddHoursPenaltyRule;
use remove_hours_penalty::RemoveHoursPenaltyRule;
use shift_modification_penalty::ShiftModificationPenaltyRule;

/// `ScheduleChangeValidationRuleImpl`.
pub trait ScheduleChangeValidationRule {
    /// `execute(originalShift, updatedShift, dataset, ruleItem, params, penaltyDescription)`.
    fn execute(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        rule_item: &RuleItem,
        params: &RuleParams,
        penalty_description: Option<&str>,
    ) -> Option<EarningRequest>;
}

/// The checks and request-building the three rules share
/// (`ScheduleChangeValidationRuleImpl` and `AddRemoveHoursPenaltyRuleImpl`).
pub struct PenaltySupport<T: EarningTypePort, C: CurrentDateTimePort> {
    earning_types: T,
    clock: C,
}

impl<T: EarningTypePort, C: CurrentDateTimePort> PenaltySupport<T, C> {
    /// Build over the earning-type lookup and the property clock.
    pub fn new(earning_types: T, clock: C) -> Self {
        Self {
            earning_types,
            clock,
        }
    }

    /// `getShiftNetHours`: zero for an absent shift.
    pub fn net_hours(shift: Option<&EmployeeShift>) -> f64 {
        shift.map_or(0.0, |s| s.net_hours())
    }

    /// `getShiftHoursChange`: updated less original.
    pub fn hours_change(original: Option<&EmployeeShift>, updated: Option<&EmployeeShift>) -> f64 {
        Self::net_hours(updated) - Self::net_hours(original)
    }

    /// `shiftLengthFallsInThreshold`.
    pub fn length_falls_in_threshold(
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        params: &RuleParams,
    ) -> bool {
        let hours = Self::net_hours(original.or(updated));
        hours <= params.double_at(MAX_SHIFT_LENGTH) && hours >= params.double_at(MIN_SHIFT_LENGTH)
    }

    /// `shiftIsWithinThreshold`.
    pub fn within_notice_threshold(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        params: &RuleParams,
    ) -> bool {
        let Some(start) = original.or(updated).and_then(|s| s.start_date_time()) else {
            return false;
        };
        let now = self.clock.current_date_time_for_current_property();
        if now > start {
            return false;
        }
        let days = duration_in_fractional_hours(now, start) / 24.0;
        days < params.double_at(FIRST_THRESHOLD) && days > params.double_at(SECOND_THRESHOLD)
    }

    /// `getEarningTypeHoursAndCost`: `(hours, cost)` for an amount of the type.
    fn hours_and_cost(&self, earning_type_id: i32, amount: f64) -> (f64, f64, Option<String>) {
        let kind = self.earning_types.find_by_id(earning_type_id);
        let uom = kind
            .as_ref()
            .map_or(crate::common::enums::uom::UOM::Hours, |k| k.uom());
        (
            if uom.is_hours() { amount } else { 0.0 },
            if uom.is_cost_based() { amount } else { 0.0 },
            kind.map(|k| k.name().to_string()),
        )
    }

    /// `createEarningRequest` of the base rule.
    pub fn create_request(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        params: &RuleParams,
        penalty_description: Option<&str>,
    ) -> Option<EarningRequest> {
        let shift = original.or(updated)?;
        let earning_type_id = params.int_at(EARNING_TYPE);
        let (hours, cost, type_name) =
            self.hours_and_cost(earning_type_id, params.double_at(EARNING_AMOUNT));
        let date = shift.shift_date();
        Some(EarningRequest {
            id: 0,
            employee_id: shift.employee_id(),
            earning_type_id,
            job_id: shift.job_id(),
            earning_date: date,
            pay_date: date,
            source: EarningSource::Manual,
            cost,
            hours,
            rate: 0.0,
            note: penalty_description
                .map(str::to_string)
                .or(type_name)
                .unwrap_or_default(),
            force_duplicate: true,
        })
    }

    /// `createEarningRequest` of `AddRemoveHoursPenaltyRuleImpl`: with
    /// `percentageOfHours`, the amount becomes that percent of the hours changed.
    pub fn create_add_remove_request(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        params: &RuleParams,
        hours_change: f64,
        penalty_description: Option<&str>,
    ) -> Option<EarningRequest> {
        let mut request = self.create_request(original, updated, params, penalty_description)?;
        if params.bool_at(PERCENTAGE_OF_HOURS) {
            let amount = hours_change.abs() * (params.double_at(EARNING_AMOUNT) / 100.0);
            let (hours, cost, _) = self.hours_and_cost(request.earning_type_id, amount);
            request.hours = hours;
            request.cost = cost;
        }
        Some(request)
    }
}

/// Dispatch for the three schedule-change-validation rules.
pub struct ScheduleChangeValidationRegistry<T: EarningTypePort, C: CurrentDateTimePort> {
    add_hours: AddHoursPenaltyRule<T, C>,
    remove_hours: RemoveHoursPenaltyRule<T, C>,
    shift_modification: ShiftModificationPenaltyRule<T, C>,
}

impl<T: EarningTypePort + Clone, C: CurrentDateTimePort + Clone>
    ScheduleChangeValidationRegistry<T, C>
{
    /// Build every rule over the shared ports.
    pub fn new(earning_types: T, clock: C) -> Self {
        Self {
            add_hours: AddHoursPenaltyRule::new(earning_types.clone(), clock.clone()),
            remove_hours: RemoveHoursPenaltyRule::new(earning_types.clone(), clock.clone()),
            shift_modification: ShiftModificationPenaltyRule::new(earning_types, clock),
        }
    }
}

impl<T, C> RuleRegistry<dyn ScheduleChangeValidationRule> for ScheduleChangeValidationRegistry<T, C>
where
    T: EarningTypePort + 'static,
    C: CurrentDateTimePort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::ScheduleChangeValidation
    }

    fn rule_for(
        &self,
        rule_class: RuleClass,
    ) -> Option<&(dyn ScheduleChangeValidationRule + 'static)> {
        match rule_class {
            RuleClass::AddHoursSchedValidation => Some(&self.add_hours),
            RuleClass::RemoveHoursSchedValidation => Some(&self.remove_hours),
            RuleClass::ShiftModificationSchedValidation => Some(&self.shift_modification),
            _ => None,
        }
    }
}

/// Shared test scaffolding for the family.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::earning_type::EarningType;
    use joda_rs::{LocalDate, LocalDateTime};

    #[derive(Clone)]
    pub struct Types(pub UOM);
    impl EarningTypePort for Types {
        fn find_by_id(&self, _id: i32) -> Option<EarningType> {
            Some(EarningType::new(
                21,
                1,
                "Penalty",
                EarnType::Earning,
                self.0,
            ))
        }
    }

    #[derive(Clone)]
    pub struct Now(pub LocalDateTime);
    impl CurrentDateTimePort for Now {
        fn current_date_time_for_current_property(&self) -> LocalDateTime {
            self.0
        }
    }

    pub fn shift_date() -> LocalDate {
        LocalDate::of(2021, 1, 1)
    }

    /// A shift on 2021-01-01 with the given stored net hours and times, job 34, employee 11.
    pub fn shift(net: f64, from: (i32, i32), to: (i32, i32)) -> EmployeeShift {
        let at = |(h, m): (i32, i32)| LocalDateTime::of(2021, 1, 1, h, m, 0);
        EmployeeShift::new(1, 11, 34, shift_date(), ShiftType::Schedule, Vec::new())
            .with_times(Some(at(from)), Some(at(to)))
            .with_net_hours(net)
    }

    pub fn expected(hours: f64) -> EarningRequest {
        EarningRequest {
            id: 0,
            employee_id: 11,
            earning_type_id: 21,
            job_id: 34,
            earning_date: shift_date(),
            pay_date: shift_date(),
            source: EarningSource::Manual,
            cost: 0.0,
            hours,
            rate: 0.0,
            note: "temp".to_string(),
            force_duplicate: true,
        }
    }

    #[test]
    fn the_registry_dispatches_every_schedule_change_validation_rule_class() {
        let registry = ScheduleChangeValidationRegistry::new(
            Types(UOM::Hours),
            Now(LocalDateTime::of(2020, 12, 30, 0, 0, 0)),
        );
        assert_eq!(registry.dispatchable().len(), 3);
        assert!(registry.missing().is_empty());
    }
}
