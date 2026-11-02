//! `RuleType::MealPunch` — two rules deciding whether an employee may take a meal
//! punch, and what it earns.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/mealpunch/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/mealpunch/`.
//!
//! A meal rule answers two questions, so [`MealPunchRule`] has two methods:
//!
//! - [`execute`](MealPunchRule::execute) — at a clock, may this employee punch for
//!   a meal right now? (`MealPunchPermission`: allow, disallow, or allow with the
//!   time off the clock.)
//! - [`process_meal_punch`](MealPunchRule::process_meal_punch) — a meal punch has
//!   arrived; does it earn something (a fixed dollar amount)?
//!
//! There is **no runner**: the punch loader's `PunchMatcher.handleMealPunches`
//! drives the second method, and the clock's meal-punch service the first. That
//! orchestration (matching temp punches to shifts, de-duplicating earnings) is the
//! punch loader's, outside the rules tree. Only the rules and configs come across.

pub mod config;
pub mod fixed_meal_earning;

use crate::common::enums::meal_punch_permission::MealPunchPermission;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::temp_punch::TempPunch;
use crate::entity::time_card::TimeCard;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, OnClockPort, SchedulePresencePort};
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_type::RuleType;
use fixed_meal_earning::FixedMealEarningRule;
use joda_rs::LocalDateTime;

/// `MealPunchRule`.
pub trait MealPunchRule {
    /// `execute(employee, clientDateTime, params)`: may the employee meal-punch now?
    fn execute(
        &self,
        employee: &Employee,
        client_date_time: LocalDateTime,
        params: &RuleParams,
    ) -> MealPunchPermission;

    /// `processMealPunch(tempPunch, dataSet, ruleItem)`. `rule_set_name` is
    /// `ruleItem.getRuleSet().getName()`, which a [`RuleItem`] here (holding only
    /// the rule set's id) cannot supply.
    fn process_meal_punch(
        &self,
        temp_punch: &TempPunch,
        time_card: &dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    ) -> Option<EmployeeEarning>;
}

/// `NoMealPunchRuleImpl`: nobody may meal-punch and nothing is earned.
pub struct NoMealPunchRule;

impl MealPunchRule for NoMealPunchRule {
    fn execute(&self, _e: &Employee, _t: LocalDateTime, _p: &RuleParams) -> MealPunchPermission {
        MealPunchPermission::Disallow
    }

    fn process_meal_punch(
        &self,
        _temp_punch: &TempPunch,
        _time_card: &dyn TimeCard,
        _rule_item: &RuleItem,
        _rule_set_name: &str,
    ) -> Option<EmployeeEarning> {
        None
    }
}

/// Dispatch for the two meal-punch rules. `RuleImplFactory`, narrowed to this family.
pub struct MealPunchRegistry<T: EarningTypePort, S: SchedulePresencePort, C: OnClockPort> {
    no_meal_punch: NoMealPunchRule,
    fixed_meal_earning: FixedMealEarningRule<T, S, C>,
}

impl<T: EarningTypePort, S: SchedulePresencePort, C: OnClockPort> MealPunchRegistry<T, S, C> {
    /// Build both rules over the ports.
    pub fn new(earning_types: T, schedule: S, on_clock: C) -> Self {
        Self {
            no_meal_punch: NoMealPunchRule,
            fixed_meal_earning: FixedMealEarningRule::new(earning_types, schedule, on_clock),
        }
    }
}

impl<T, S, C> RuleRegistry<dyn MealPunchRule> for MealPunchRegistry<T, S, C>
where
    T: EarningTypePort + 'static,
    S: SchedulePresencePort + 'static,
    C: OnClockPort + 'static,
{
    fn rule_type(&self) -> RuleType {
        RuleType::MealPunch
    }

    fn rule_for(&self, rule_class: RuleClass) -> Option<&(dyn MealPunchRule + 'static)> {
        match rule_class {
            RuleClass::NoMpr => Some(&self.no_meal_punch),
            RuleClass::FixedEarningMpr => Some(&self.fixed_meal_earning),
            _ => None,
        }
    }
}
