//! Port of `FixedMealEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/mealpunch/FixedMealEarningRuleImpl.java`.
//!
//! **Permission.** With `scheduledOnly` (default on) an employee not scheduled
//! within `gracePreSchedule` minutes before / `gracePostSchedule` after the moment
//! is disallowed outright. Otherwise the configured `punchPermission` applies: if
//! it is `ALLOW`, the answer is `ALLOW`; if it is anything else, it applies only
//! while the employee **is on the clock** — off the clock they are allowed.
//!
//! **Earning.** A meal punch earns `dollars` (no hours, no rate) dated and paid
//! the punch's date, on the job the employee holds for the punch's job and date
//! (none, no earning), unattached to any shift. The note is
//! `"<rule set> - <rule item> <punch date and time>"`.

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::meal_punch_permission::MealPunchPermission;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::temp_punch::TempPunch;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::EARNING;
use crate::rules::algorithm::mealpunch::MealPunchRule;
use crate::rules::algorithm::mealpunch::config::{
    DOLLARS, FixedMealEarningRuleConfig, GRACE_POST_SCHED, GRACE_PRE_SCHED, PUNCH_PERMISSION,
    SCHEDULED_ONLY,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, OnClockPort, SchedulePresencePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;

/// `FixedMealEarningRuleImpl`.
pub struct FixedMealEarningRule<T: EarningTypePort, S: SchedulePresencePort, C: OnClockPort> {
    earning_types: T,
    schedule: S,
    on_clock: C,
}

impl<T: EarningTypePort, S: SchedulePresencePort, C: OnClockPort> FixedMealEarningRule<T, S, C> {
    /// Build the rule over its ports.
    pub fn new(earning_types: T, schedule: S, on_clock: C) -> Self {
        Self {
            earning_types,
            schedule,
            on_clock,
        }
    }
}

impl<T: EarningTypePort, S: SchedulePresencePort, C: OnClockPort> MealPunchRule
    for FixedMealEarningRule<T, S, C>
{
    fn execute(
        &self,
        employee: &Employee,
        client_date_time: LocalDateTime,
        params: &RuleParams,
    ) -> MealPunchPermission {
        let params = params.fixed(&FixedMealEarningRuleConfig.default_values());

        if params.bool_at(SCHEDULED_ONLY)
            && !self.schedule.is_scheduled(
                employee,
                client_date_time,
                params.int_at(GRACE_PRE_SCHED),
                params.int_at(GRACE_POST_SCHED),
            )
        {
            return MealPunchPermission::Disallow;
        }

        // `fromCode` throws on an unknown code; the port treats one as DISALLOW.
        let permission = params
            .get(PUNCH_PERMISSION)
            .and_then(MealPunchPermission::from_code)
            .unwrap_or(MealPunchPermission::Disallow);
        if permission != MealPunchPermission::Allow {
            return if self
                .on_clock
                .is_employee_on_clock(employee, client_date_time)
            {
                permission
            } else {
                MealPunchPermission::Allow
            };
        }
        MealPunchPermission::Allow
    }

    fn process_meal_punch(
        &self,
        temp_punch: &TempPunch,
        _time_card: &dyn TimeCard,
        rule_item: &RuleItem,
        rule_set_name: &str,
    ) -> Option<EmployeeEarning> {
        let params = rule_item
            .params()
            .fixed(&FixedMealEarningRuleConfig.default_values());
        let date = temp_punch.punch_date_time().to_local_date();
        let status = temp_punch
            .employee()
            .employee_job_status(temp_punch.job_id(), date)?;
        let earning_type = self.earning_types.find_by_id(params.int_at(EARNING))?;

        let mut earning = EmployeeEarning::new(
            0,
            temp_punch.employee().id(),
            status.job_id(),
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_dollars(params.double_at(DOLLARS));
        earning.set_rate(0.0);
        earning.calc_and_set_total_dollars();
        earning.set_note(format!(
            "{} - {} {}",
            rule_set_name,
            rule_item.name(),
            temp_punch.punch_date_time()
        ));
        Some(earning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{FixedEarningType, earning_type};
    use crate::rules::algorithm::mealpunch::NoMealPunchRule;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;
    use std::cell::{Cell, RefCell};

    struct Scheduled(bool, RefCell<Vec<(i32, i32)>>);
    impl SchedulePresencePort for Scheduled {
        fn is_scheduled(&self, _e: &Employee, _t: LocalDateTime, pre: i32, post: i32) -> bool {
            self.1.borrow_mut().push((pre, post));
            self.0
        }
    }
    struct OnClock(bool, Cell<u32>);
    impl OnClockPort for OnClock {
        fn is_employee_on_clock(&self, _e: &Employee, _t: LocalDateTime) -> bool {
            self.1.set(self.1.get() + 1);
            self.0
        }
    }

    fn at() -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, 12, 0, 0)
    }
    fn employee() -> Employee {
        let d = LocalDate::of(2014, 5, 6);
        Employee::new(
            7,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                7,
                10,
                d.minus_years(1),
                d.plus_years(1),
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }
    fn rule(
        scheduled: bool,
        on_clock: bool,
    ) -> FixedMealEarningRule<FixedEarningType, Scheduled, OnClock> {
        FixedMealEarningRule::new(
            FixedEarningType(earning_type(UOM::Dollars)),
            Scheduled(scheduled, RefCell::default()),
            OnClock(on_clock, Cell::new(0)),
        )
    }
    fn permission(p: MealPunchPermission) -> RuleParams {
        rule_params! { SCHEDULED_ONLY => "false", PUNCH_PERMISSION => p.code() }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn scheduled_only_disallows_an_unscheduled_employee_and_passes_the_grace_minutes() {
        let r = rule(false, false);
        let got = r.execute(
            &employee(),
            at(),
            &rule_params! { GRACE_PRE_SCHED => "15", GRACE_POST_SCHED => "45" },
        );
        assert_eq!(got, MealPunchPermission::Disallow);
        assert_eq!(*r.schedule.1.borrow(), vec![(15, 45)]);
    }

    #[test]
    fn a_scheduled_employee_gets_the_configured_permission_only_while_on_the_clock() {
        let on = rule(true, true);
        let p = rule_params! { PUNCH_PERMISSION => MealPunchPermission::AllowWithOffClock.code() };
        assert_eq!(
            on.execute(&employee(), at(), &p),
            MealPunchPermission::AllowWithOffClock
        );
        let off = rule(true, false);
        assert_eq!(
            off.execute(&employee(), at(), &p),
            MealPunchPermission::Allow
        );
    }

    #[test]
    fn the_default_permission_disallows_while_on_the_clock_and_allows_off_it() {
        assert_eq!(
            rule(true, true).execute(&employee(), at(), &RuleParams::new()),
            MealPunchPermission::Disallow
        );
        assert_eq!(
            rule(true, false).execute(&employee(), at(), &RuleParams::new()),
            MealPunchPermission::Allow
        );
    }

    #[test]
    fn allow_never_asks_the_clock_and_a_non_scheduled_only_rule_never_asks_the_schedule() {
        let r = rule(false, true);
        assert_eq!(
            r.execute(&employee(), at(), &permission(MealPunchPermission::Allow)),
            MealPunchPermission::Allow
        );
        assert_eq!(r.on_clock.1.get(), 0);
        assert!(r.schedule.1.borrow().is_empty());
    }

    #[test]
    fn a_meal_punch_earns_the_fixed_dollars_unattached_to_a_shift() {
        let punch = TempPunch::new(employee(), 10, at());
        let item = RuleItem::new(
            5,
            1,
            "Meal",
            RuleClass::FixedEarningMpr,
            rule_params! { EARNING => "3", DOLLARS => "7.50" },
        );
        let earning = rule(true, true)
            .process_meal_punch(&punch, &TimeCardData::new(), &item, "Cafe set")
            .unwrap();
        assert_eq!(
            (earning.dollars(), earning.rate(), earning.hours()),
            (7.5, 0.0, 0.0)
        );
        assert_eq!(earning.total_dollars(), 7.5);
        assert_eq!(earning.earning_date(), LocalDate::of(2014, 5, 6));
        assert_eq!(earning.pay_date(), Some(LocalDate::of(2014, 5, 6)));
        assert_eq!((earning.employee_id(), earning.job_id()), (7, 10));
        assert_eq!(earning.source(), EarningSource::Rule);
        assert_eq!(
            (earning.rule_item_id(), earning.shift_id()),
            (Some(5), None)
        );
        assert!(earning.note().starts_with("Cafe set - Meal "));
    }

    #[test]
    fn no_job_status_for_the_punch_means_no_earning() {
        let punch = TempPunch::new(employee(), 99, at());
        let item = RuleItem::new(5, 1, "Meal", RuleClass::FixedEarningMpr, Default::default());
        assert!(
            rule(true, true)
                .process_meal_punch(&punch, &TimeCardData::new(), &item, "s")
                .is_none()
        );
    }

    #[test]
    fn the_no_meal_punch_rule_disallows_and_earns_nothing() {
        assert_eq!(
            NoMealPunchRule.execute(&employee(), at(), &RuleParams::new()),
            MealPunchPermission::Disallow
        );
        let punch = TempPunch::new(employee(), 10, at());
        let item = RuleItem::new(5, 1, "Meal", RuleClass::NoMpr, Default::default());
        assert!(
            NoMealPunchRule
                .process_meal_punch(&punch, &TimeCardData::new(), &item, "s")
                .is_none()
        );
    }

    #[test]
    fn the_registry_dispatches_both_meal_punch_rule_classes() {
        use crate::rules::dispatch::RuleRegistry;
        let registry = crate::rules::algorithm::mealpunch::MealPunchRegistry::new(
            FixedEarningType(earning_type(UOM::Dollars)),
            Scheduled(true, RefCell::default()),
            OnClock(true, Cell::new(0)),
        );
        assert_eq!(registry.dispatchable().len(), 2);
        assert!(registry.missing().is_empty());
    }
}
