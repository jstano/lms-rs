//! Port of `DailyDifferentialRunner`, `DifferentialRunner` and
//! `ClearTimeCardEarnings`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/DailyDifferentialRunner.java`,
//! `.../DifferentialRunner.java` and `.../ClearTimeCardEarnings.java`.
//!
//! Drives the `dailyearning` family over a time card, **one calendar day at a
//! time**:
//!
//! 1. **Clear** the earnings earlier runs of this family left: every earning
//!    that is open for editing, was written by a rule (`source == RULE`) whose
//!    rule item belongs to a `DAILY_DIFFERENTIAL` rule class, and whose shift
//!    (if it has one) is also open. They are removed from the card and
//!    deleted through the earning port.
//! 2. **Skip** the rest if the card has neither shifts nor earnings — unless
//!    the property's `showDailyDifferentialEarningsOnEmptyTimeCards` flag is
//!    on (default off).
//! 3. For each date from the calculation start date to
//!    `max(maxActualDate, currentPayPeriodEnd)`: take the employee's **home**
//!    job on that date (no home job, no run), resolve its `DAILY_DIFFERENTIAL`
//!    rule set, sort the items by priority, and run each.
//!
//! # Resolution is per day, on the home job
//!
//! Unlike the shift runners, the rule set is looked up for the home job on
//! each date, so a mid-period change of home job or rule set takes effect on
//! the right day. `rule_set_for(job_id, date)` is that call.
//!
//! # Clearing needs a rule-type lookup (divergence 92)
//!
//! Java walks `earning.getRuleItem().getRuleClass().getRuleType()`. An
//! `EmployeeEarning` here carries only its rule item's id, so the caller
//! supplies `rule_type_of_item`. An earning whose id is 0 has never been
//! persisted and is removed from the card without a delete.

use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::DailyEarningRule;
use crate::rules::algorithm::dailyearning::config::{
    AnniversaryEarningRuleConfig, DailySalariedAllowanceRuleConfig,
    DaysWorkedDailyDifferentialRuleConfig, HolidayDifferentialRuleConfig,
    MinDailyHoursEarningRuleConfig, MinDailyWorkedHoursEarningRuleConfig,
    MinimumHoursWorkedDifferentialRuleConfig, NoDailyEarningRuleConfig,
    NoDailyHoursWithLabelEarningRuleConfig,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{EmployeeEarningPort, PropertyDataKey, PropertyDataPort};
use crate::rules::priority::sort_by_priority;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use crate::rules::rule_type::RuleType;
use crate::rules::runner::clear_time_card_earnings::clear_time_card_earnings;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// Runs the daily-differential family over a time card.
pub struct DailyDifferentialRunner<'r, E: EmployeeEarningPort, P: PropertyDataPort> {
    registry: &'r dyn RuleRegistry<dyn DailyEarningRule>,
    earnings: RefCell<E>,
    property_data: P,
}

impl<'r, E: EmployeeEarningPort, P: PropertyDataPort> DailyDifferentialRunner<'r, E, P> {
    /// Build the runner over the family's dispatch and the ports it needs.
    pub fn new(
        registry: &'r dyn RuleRegistry<dyn DailyEarningRule>,
        earnings: E,
        property_data: P,
    ) -> Self {
        Self {
            registry,
            earnings: RefCell::new(earnings),
            property_data,
        }
    }

    /// `ClearTimeCardEarnings.clearEarnings(timeCard, [DAILY_DIFFERENTIAL])`.
    pub fn clear_earnings(
        &self,
        time_card: &mut dyn TimeCard,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        clear_time_card_earnings(
            time_card,
            &mut *self.earnings.borrow_mut(),
            &[RuleType::DailyDifferential],
            rule_type_of_item,
        );
    }

    /// `runRules(ActualsTimeCard)`.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        self.clear_earnings(time_card, rule_type_of_item);

        let Some(employee) = time_card.employee() else {
            return;
        };
        let property_id = employee.property_id();

        let show_on_empty = self
            .property_data
            .value_as_boolean(
                property_id,
                PropertyDataKey::ShowDailyDifferentialEarningsOnEmptyTimeCards,
            )
            .unwrap_or(false);
        if !(time_card.has_shifts_or_earnings() || show_on_empty) {
            return;
        }

        let pay_period_end = time_card.current_pay_period().end_date();
        let max_actual = time_card.max_actual_date();
        let last = if pay_period_end > max_actual {
            pay_period_end
        } else {
            max_actual
        };

        let mut date = time_card.calculation_start_date();
        while date <= last {
            self.run_rule_set_for_date(time_card, date, &rule_set_for);
            date = date.plus_days(1);
        }
    }

    fn run_rule_set_for_date<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        date: LocalDate,
        rule_set_for: &impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
    ) {
        let Some(home_job_id) = time_card
            .employee()
            .and_then(|e| e.home_employee_job_status(date))
            .map(|home| home.job_id())
        else {
            return;
        };
        let Some(rule_set) = rule_set_for(home_job_id, date) else {
            return;
        };

        let mut items: Vec<&RuleItem> = rule_set.rule_items().iter().collect();
        sort_by_priority(&mut items, |item| config_for(item.rule_class()));
        for item in items {
            if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                rule.execute(time_card, date, item);
            }
        }
    }
}

/// Each daily rule's config, for the priority sort. None of the nine extends
/// `PriorityRuleConfig`, so they all rank at the default and keep rule-set
/// order — the sort is here because Java does it, and a later config gaining a
/// priority is then handled.
fn config_for(rule_class: RuleClass) -> &'static dyn RuleConfig {
    match rule_class {
        RuleClass::HolidayDifDdf => &HolidayDifferentialRuleConfig,
        RuleClass::MinHoursWorkedDdf => &MinimumHoursWorkedDifferentialRuleConfig,
        RuleClass::DaysWorkedDdf => &DaysWorkedDailyDifferentialRuleConfig,
        RuleClass::MinDailyHrsDdf => &MinDailyHoursEarningRuleConfig,
        RuleClass::NoDailyHrsDdf => &NoDailyHoursWithLabelEarningRuleConfig,
        RuleClass::MinDailyWrkdHrsDdf => &MinDailyWorkedHoursEarningRuleConfig,
        RuleClass::DlySalAllowDdf => &DailySalariedAllowanceRuleConfig,
        RuleClass::AnniDdf => &AnniversaryEarningRuleConfig,
        _ => &NoDailyEarningRuleConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use date_range_rs::DateRange;
    use std::rc::Rc;

    const JOB: i32 = 10;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }

    struct Flag(Option<bool>);
    impl PropertyDataPort for Flag {
        fn value(&self, _p: i32, _k: PropertyDataKey) -> Option<String> {
            None
        }
        fn value_as_double(&self, _p: i32, _k: PropertyDataKey) -> Option<f64> {
            None
        }
        fn value_as_boolean(&self, _p: i32, _k: PropertyDataKey) -> Option<bool> {
            self.0
        }
    }

    /// Records `(date, rule item id)` for every call.
    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(LocalDate, i32)>>,
    }
    impl DailyEarningRule for Recorder {
        fn execute(&self, _tc: &mut dyn TimeCard, date: LocalDate, item: &RuleItem) {
            self.calls.borrow_mut().push((date, item.id()));
        }
    }
    struct Registry(Recorder);
    impl RuleRegistry<dyn DailyEarningRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::DailyDifferential
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn DailyEarningRule + 'static)> {
            Some(&self.0)
        }
    }

    fn employee() -> Employee {
        let tomorrow = today().plus_days(1);
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                today().minus_years(1),
                tomorrow,
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            today().minus_years(1),
            tomorrow,
            EmployeeStatusType::Active,
        )])
    }

    fn rule_set() -> RuleSet {
        RuleSet::new(
            1,
            1,
            "dd",
            RuleType::DailyDifferential,
            0,
            vec![
                RuleItem::new(
                    1,
                    1,
                    "holiday",
                    RuleClass::HolidayDifDdf,
                    Default::default(),
                ),
                RuleItem::new(2, 1, "none", RuleClass::NoDdf, Default::default()),
            ],
        )
    }

    fn base_card(has_shifts: bool, has_earnings: bool) -> TimeCardData {
        let tomorrow = today().plus_days(1);
        let shift = |id| {
            EmployeeShift::new(id, 1, JOB, tomorrow, ShiftType::Actual, Vec::new())
                .with_hours_distributions(vec![HoursDistribution::new(1, tomorrow, None, 1.0, 0.0)])
        };
        TimeCardData::new()
            .with_employee(employee())
            .with_calculation_start_date(today().minus_days(1))
            .with_current_pay_period(DateRange::new(today().minus_days(6), tomorrow))
            .with_shifts(if has_shifts {
                vec![shift(2), shift(3)]
            } else {
                Vec::new()
            })
            .with_earnings(if has_earnings {
                vec![EmployeeEarning::new(
                    0,
                    1,
                    JOB,
                    1,
                    today().minus_days(1),
                    0.0,
                    0.0,
                    EarningSource::Auto,
                )]
            } else {
                Vec::new()
            })
    }

    fn no_rule_type(_: i32) -> Option<RuleType> {
        None
    }

    // ---- java_parity_tests: DailyDifferentialRunnerTest.groovy ----

    #[test]
    fn run_daily_differential_rules() {
        let tomorrow = today().plus_days(1);
        let set = rule_set();
        for (has_shifts, has_earnings, runs) in [
            (true, true, 1),
            (true, false, 1),
            (false, true, 1),
            (false, false, 0),
        ] {
            let registry = Registry(Recorder::default());
            let runner = DailyDifferentialRunner::new(
                &registry,
                SavedEarnings::default(),
                Flag(Some(false)),
            );
            let mut card = base_card(has_shifts, has_earnings);
            // Like the Groovy mock, only `tomorrow` resolves to a rule set.
            runner.run_rules(
                &mut card,
                |job, date| (job == JOB && date == tomorrow).then_some(&set),
                &no_rule_type,
            );
            assert_eq!(
                *registry.0.calls.borrow(),
                if runs == 1 {
                    vec![(tomorrow, 1), (tomorrow, 2)]
                } else {
                    Vec::new()
                },
                "shifts={has_shifts} earnings={has_earnings}"
            );
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_empty_time_card_flag_forces_a_run() {
        let tomorrow = today().plus_days(1);
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner =
            DailyDifferentialRunner::new(&registry, SavedEarnings::default(), Flag(Some(true)));
        let mut card = base_card(false, false);
        runner.run_rules(
            &mut card,
            |_, d| (d == tomorrow).then_some(&set),
            &no_rule_type,
        );
        assert_eq!(registry.0.calls.borrow().len(), 2);
    }

    #[test]
    fn it_runs_every_day_from_the_calculation_start_to_the_period_end() {
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner = DailyDifferentialRunner::new(&registry, SavedEarnings::default(), Flag(None));
        let mut card = base_card(true, false);
        runner.run_rules(&mut card, |_, _| Some(&set), &no_rule_type);
        let dates: Vec<LocalDate> = registry
            .0
            .calls
            .borrow()
            .iter()
            .step_by(2)
            .map(|c| c.0)
            .collect();
        // calculation start (yesterday) through tomorrow.
        assert_eq!(
            dates,
            vec![today().minus_days(1), today(), today().plus_days(1)]
        );
    }

    #[test]
    fn a_day_with_no_home_job_is_skipped() {
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner = DailyDifferentialRunner::new(&registry, SavedEarnings::default(), Flag(None));
        let mut card = base_card(true, false).with_calculation_start_date(today().plus_days(2));
        // The home job ends tomorrow; the day after has none.
        runner.run_rules(&mut card, |_, _| Some(&set), &no_rule_type);
        assert!(registry.0.calls.borrow().is_empty());
    }

    fn rule_earning(id: i32, source: EarningSource, rule_item: i32) -> EmployeeEarning {
        let earning = EmployeeEarning::new(id, 1, JOB, 1, today(), 1.0, 1.0, source);
        // `from_rule` stamps the source RULE, so a non-rule earning has no item.
        if source == EarningSource::Rule {
            earning.from_rule(rule_item, None)
        } else {
            earning
        }
    }

    #[test]
    fn clearing_removes_only_open_daily_differential_rule_earnings() {
        let removed = Rc::new(RefCell::new(Vec::new()));
        struct Deleting(Rc<RefCell<Vec<i32>>>);
        impl EmployeeEarningPort for Deleting {
            fn save(&mut self, _e: EmployeeEarning) {}
            fn remove(&mut self, id: i32) {
                self.0.borrow_mut().push(id);
            }
            fn earning_hours_for_period_and_types(
                &self,
                _e: i32,
                _p: &DateRange,
                _t: &[i32],
            ) -> Option<f64> {
                None
            }
            fn banked_rate_for_rule(&self, _e: i32, _h: i32, _c: i32) -> Option<f64> {
                None
            }
        }
        let registry = Registry(Recorder::default());
        let runner = DailyDifferentialRunner::new(&registry, Deleting(removed.clone()), Flag(None));
        let mut card = base_card(false, false).with_earnings(vec![
            rule_earning(11, EarningSource::Rule, 100), // daily differential: goes
            rule_earning(12, EarningSource::Rule, 200), // another rule type: stays
            rule_earning(13, EarningSource::Auto, 100), // not rule-sourced: stays
            rule_earning(0, EarningSource::Rule, 100),  // never persisted: goes, no delete
        ]);
        let type_of = |item: i32| match item {
            100 => Some(RuleType::DailyDifferential),
            200 => Some(RuleType::HoursDistribution),
            _ => None,
        };
        runner.clear_earnings(&mut card, &type_of);
        let kept: Vec<i32> = card.earnings().iter().map(|e| e.id()).collect();
        assert_eq!(kept, vec![12, 13]);
        assert_eq!(*removed.borrow(), vec![11]);
    }

    #[test]
    fn clearing_leaves_earnings_in_a_closed_period() {
        let registry = Registry(Recorder::default());
        let runner = DailyDifferentialRunner::new(&registry, SavedEarnings::default(), Flag(None));
        let mut card = base_card(false, false)
            .with_calculation_start_date(today().plus_days(1))
            .with_earnings(vec![rule_earning(11, EarningSource::Rule, 100)]);
        runner.clear_earnings(&mut card, &|_| Some(RuleType::DailyDifferential));
        assert_eq!(card.earnings().len(), 1);
    }
}
