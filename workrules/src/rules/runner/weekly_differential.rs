//! Port of `WeeklyDifferentialRuleRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/WeeklyDifferentialRuleRunner.java`.
//!
//! Drives the `weeklyearning` family over a time card, **one work week at a
//! time**:
//!
//! 1. **Clear** what earlier runs wrote ([`clear_time_card_earnings`]), then stop
//!    if the card has no shifts or earnings (unlike `DailyDifferentialRunner`
//!    there is no property flag to force a run).
//! 2. The weeks are the 7-day ranges, ending on the property's period-end
//!    weekday, that contain any date from the calculation start to
//!    `max(maxActualDate, currentPayPeriodEnd)`.
//! 3. For each week: the **first home job** the employee holds in it; the rule
//!    set is resolved for that job on the later of the job's start and the
//!    week's start; and **every** rule item runs, in rule-set order — this
//!    runner does not sort by priority.
//!
//! # The description
//!
//! Each rule is handed `"Rule: <rule set name> - <rule name>"`. Java takes the
//! second half from the localized `RuleConfig.getName()`; the port has no such
//! name and uses the rule item's own name (divergence 98).

use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::WeeklyEarningRule;
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::{EmployeeEarningPort, PropertyPort};
use crate::rules::rule_type::RuleType;
use crate::rules::runner::clear_time_card_earnings::clear_time_card_earnings;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// Runs the weekly-differential family over a time card.
pub struct WeeklyDifferentialRunner<'r, E: EmployeeEarningPort, P: PropertyPort> {
    registry: &'r dyn RuleRegistry<dyn WeeklyEarningRule>,
    earnings: RefCell<E>,
    property: P,
}

impl<'r, E: EmployeeEarningPort, P: PropertyPort> WeeklyDifferentialRunner<'r, E, P> {
    /// Build the runner over the family's dispatch and the ports it needs.
    pub fn new(
        registry: &'r dyn RuleRegistry<dyn WeeklyEarningRule>,
        earnings: E,
        property: P,
    ) -> Self {
        Self {
            registry,
            earnings: RefCell::new(earnings),
            property,
        }
    }

    /// `runRules(ActualsTimeCard)`. `rule_set_for(job_id, date)` resolves the
    /// `WEEKLY_DIFFERENTIAL` rule set for the job on that date.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rule_set_for: impl Fn(i32, LocalDate) -> Option<&'a RuleSet>,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        clear_time_card_earnings(
            time_card,
            &mut *self.earnings.borrow_mut(),
            &[RuleType::WeeklyDifferential],
            rule_type_of_item,
        );

        if !time_card.has_shifts_or_earnings() {
            return;
        }
        let Some(employee) = time_card.employee() else {
            return;
        };

        let weeks =
            WeeklyDateRange::with_end_date(self.property.period_end_date(employee.property_id()));
        let pay_period_end = time_card.current_pay_period().end_date();
        let max_actual = time_card.max_actual_date();
        let last = if pay_period_end > max_actual {
            pay_period_end
        } else {
            max_actual
        };

        for work_week in weeks.ranges_containing_span(time_card.calculation_start_date(), last) {
            let Some(employee) = time_card.employee() else {
                return;
            };
            let Some(home) = employee.first_home_employee_job_status_for_period(&work_week) else {
                continue;
            };
            let adjusted = if home.start_date() > work_week.start_date() {
                home.start_date()
            } else {
                work_week.start_date()
            };
            let Some(rule_set) = rule_set_for(home.job_id(), adjusted) else {
                continue;
            };

            for item in rule_set.rule_items() {
                if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                    let description = format!("Rule: {} - {}", rule_set.name(), item.name());
                    rule.execute(time_card, &work_week, item, &description);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::rule_item::RuleItem;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_class::RuleClass;
    use date_range_rs::DateRange;
    use std::cell::RefCell;

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }

    struct WeekEndsOn(LocalDate);
    impl PropertyPort for WeekEndsOn {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            self.0
        }
    }

    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(DateRange, i32, String)>>,
    }
    impl WeeklyEarningRule for Recorder {
        fn execute(
            &self,
            _tc: &mut dyn TimeCard,
            week: &DateRange,
            item: &RuleItem,
            description: &str,
        ) {
            self.calls
                .borrow_mut()
                .push((*week, item.id(), description.to_string()));
        }
    }
    struct Registry(Recorder);
    impl RuleRegistry<dyn WeeklyEarningRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::WeeklyDifferential
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn WeeklyEarningRule + 'static)> {
            Some(&self.0)
        }
    }

    fn employee(job_start: LocalDate, job_end: LocalDate) -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                10,
                job_start,
                job_end,
                EmployeePayType::Hourly,
                0.0,
                true,
            )],
        )
    }

    fn rule_set() -> RuleSet {
        RuleSet::new(
            1,
            1,
            "weekly set",
            RuleType::WeeklyDifferential,
            0,
            vec![RuleItem::new(
                5,
                1,
                "Dow rate",
                RuleClass::DowWeeklyHrsFactorWdf,
                Default::default(),
            )],
        )
    }

    fn card(emp: Employee) -> TimeCardData {
        TimeCardData::new()
            .with_employee(emp)
            .with_calculation_start_date(today())
            .with_current_pay_period(DateRange::new(today(), today().plus_days(6)))
            .with_earnings(vec![EmployeeEarning::new(
                0,
                1,
                10,
                1,
                today().plus_days(1),
                0.0,
                0.0,
                EarningSource::Auto,
            )])
    }

    // ---- java_parity_tests: WeeklyDifferentialRuleRunnerTest.groovy ----

    #[test]
    fn weekly_earning_rules_should_be_run() {
        let statuses = [
            (today().minus_days(3), today().plus_days(3)),
            (today().plus_days(1), today().plus_days(6)),
        ];
        for (start, end) in statuses {
            let set = rule_set();
            let registry = Registry(Recorder::default());
            let runner = WeeklyDifferentialRunner::new(
                &registry,
                SavedEarnings::default(),
                WeekEndsOn(today().plus_days(6)),
            );
            let mut c = card(employee(start, end));
            runner.run_rules(&mut c, |_, _| Some(&set), &|_| None);
            assert!(
                !registry.0.calls.borrow().is_empty(),
                "job {start:?}..{end:?}"
            );
        }
    }

    #[test]
    fn earnings_are_cleared_even_if_rules_arent_run() {
        let registry = Registry(Recorder::default());
        let runner =
            WeeklyDifferentialRunner::new(&registry, SavedEarnings::default(), WeekEndsOn(today()));
        let mut c = TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_calculation_start_date(today())
            .with_earnings(vec![
                EmployeeEarning::new(
                    0,
                    1,
                    10,
                    1,
                    today().plus_days(1),
                    0.0,
                    0.0,
                    EarningSource::Rule,
                )
                .from_rule(9, None),
            ]);
        runner.run_rules(&mut c, |_, _| None, &|item| {
            (item == 9).then_some(RuleType::WeeklyDifferential)
        });
        assert!(c.earnings().is_empty());
        assert!(registry.0.calls.borrow().is_empty());
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_week_is_the_seven_days_ending_on_the_property_period_end() {
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner = WeeklyDifferentialRunner::new(
            &registry,
            SavedEarnings::default(),
            WeekEndsOn(today().plus_days(6)),
        );
        let mut c = card(employee(today().minus_days(30), today().plus_days(30)));
        runner.run_rules(&mut c, |_, _| Some(&set), &|_| None);
        let calls = registry.0.calls.borrow();
        assert_eq!(calls[0].0, DateRange::new(today(), today().plus_days(6)));
        assert_eq!(calls[0].2, "Rule: weekly set - Dow rate");
        assert_eq!(calls[0].1, 5);
    }

    #[test]
    fn the_rule_set_is_resolved_on_the_later_of_job_start_and_week_start() {
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner = WeeklyDifferentialRunner::new(
            &registry,
            SavedEarnings::default(),
            WeekEndsOn(today().plus_days(6)),
        );
        let asked = RefCell::new(Vec::new());
        let mut c = card(employee(today().plus_days(2), today().plus_days(30)));
        runner.run_rules(
            &mut c,
            |job, date| {
                asked.borrow_mut().push((job, date));
                Some(&set)
            },
            &|_| None,
        );
        assert_eq!(asked.borrow()[0], (10, today().plus_days(2)));
    }

    #[test]
    fn rule_items_run_in_rule_set_order_without_a_priority_sort() {
        let set = RuleSet::new(
            1,
            1,
            "s",
            RuleType::WeeklyDifferential,
            0,
            vec![
                RuleItem::new(2, 1, "b", RuleClass::NoWdf, Default::default()),
                RuleItem::new(1, 1, "a", RuleClass::NoWdf, Default::default()),
            ],
        );
        let registry = Registry(Recorder::default());
        let runner = WeeklyDifferentialRunner::new(
            &registry,
            SavedEarnings::default(),
            WeekEndsOn(today().plus_days(6)),
        );
        let mut c = card(employee(today().minus_days(30), today().plus_days(30)));
        runner.run_rules(&mut c, |_, _| Some(&set), &|_| None);
        let ids: Vec<i32> = registry.0.calls.borrow().iter().map(|c| c.1).collect();
        assert_eq!(&ids[..2], &[2, 1]);
    }

    #[test]
    fn a_week_with_no_home_job_is_skipped() {
        let set = rule_set();
        let registry = Registry(Recorder::default());
        let runner = WeeklyDifferentialRunner::new(
            &registry,
            SavedEarnings::default(),
            WeekEndsOn(today().plus_days(6)),
        );
        let mut c = card(employee(today().minus_days(60), today().minus_days(30)));
        runner.run_rules(&mut c, |_, _| Some(&set), &|_| None);
        assert!(registry.0.calls.borrow().is_empty());
    }
}
