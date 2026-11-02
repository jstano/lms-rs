//! Port of `ShiftDifferentialRunner`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/ShiftDifferentialRunner.java`
//! (and `DifferentialRunner` / `ClearTimeCardEarnings`, shared with the other
//! differential runners).
//!
//! Drives the `shiftearning` family over a time card, **one shift at a time**:
//!
//! 1. **Clear** the earnings earlier runs left. This runner clears three rule
//!    types, not one: `SHIFT_DIFFERENTIAL`, `HOURS_DISTRIBUTION` and
//!    `POST_CALC` — a shift-differential run discards the hours-distribution and
//!    post-calc earnings too (they are recreated by their own runners later in
//!    the calculation). Same eligibility as the other runners: open for
//!    editing, rule-written, and attached to an open shift if any.
//! 2. For each shift **with no persisted errors**, resolve its
//!    `SHIFT_DIFFERENTIAL` rule set from the shift's employee, **job** and date,
//!    sort the items by priority (`RuleItemPriorityComparator`: only
//!    `HolidayTimeOfDay`'s config has a priority; everything else ranks at the
//!    default and keeps rule-set order) and run each.
//!
//! The rule runs against a copy of the shift the rules receive by shared
//! reference; Java hands them the card's own object, but no rule in this family
//! changes it.
//!
//! `rule_set_for(shift)` is `ruleUtils.getRuleSet(...)`; `rule_type_of_item` is
//! divergence 92 again.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::ShiftEarningRule;
use crate::rules::algorithm::shiftearning::config::{
    HolidayTimeOfDayRuleConfig, NoEarningRuleConfig,
};
use crate::rules::dispatch::RuleRegistry;
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::priority::sort_by_priority;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use crate::rules::rule_type::RuleType;
use crate::rules::runner::clear_time_card_earnings::clear_time_card_earnings;
use std::cell::RefCell;

/// The rule types this runner clears. `ShiftDifferentialRunner.validRuleTypes`.
pub const CLEARED_RULE_TYPES: [RuleType; 3] = [
    RuleType::ShiftDifferential,
    RuleType::HoursDistribution,
    RuleType::PostCalc,
];

/// Runs the shift-differential family over a time card.
pub struct ShiftDifferentialRunner<'r, E: EmployeeEarningPort> {
    registry: &'r dyn RuleRegistry<dyn ShiftEarningRule>,
    earnings: RefCell<E>,
}

impl<'r, E: EmployeeEarningPort> ShiftDifferentialRunner<'r, E> {
    /// Build the runner over the family's dispatch and the earning port.
    pub fn new(registry: &'r dyn RuleRegistry<dyn ShiftEarningRule>, earnings: E) -> Self {
        Self {
            registry,
            earnings: RefCell::new(earnings),
        }
    }

    /// `clearEarnings(timeCard)`.
    pub fn clear_earnings(
        &self,
        time_card: &mut dyn TimeCard,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        clear_time_card_earnings(
            time_card,
            &mut *self.earnings.borrow_mut(),
            &CLEARED_RULE_TYPES,
            rule_type_of_item,
        );
    }

    /// `runRules(ActualsTimeCard)`.
    pub fn run_rules<'a>(
        &self,
        time_card: &mut dyn TimeCard,
        rule_set_for: impl Fn(&EmployeeShift) -> Option<&'a RuleSet>,
        rule_type_of_item: &dyn Fn(i32) -> Option<RuleType>,
    ) {
        self.clear_earnings(time_card, rule_type_of_item);

        // Java iterates a stream over the card's shifts; rules do not add shifts.
        let count = time_card.shifts().len();
        for index in 0..count {
            let shift = time_card.shifts()[index].clone();
            if shift.has_errors() {
                continue;
            }
            let Some(rule_set) = rule_set_for(&shift) else {
                continue;
            };

            let mut items: Vec<&RuleItem> = rule_set.rule_items().iter().collect();
            sort_by_priority(&mut items, |item| config_for(item.rule_class()));
            for item in items {
                if let Some(rule) = self.registry.rule_for(item.rule_class()) {
                    rule.execute(&shift, time_card, item);
                }
            }
        }
    }
}

/// Each rule's config for the priority sort. Only `HolidayTimeOfDay` extends
/// `PriorityRuleConfig`; any other class ranks at the default, which
/// `NoEarningRuleConfig` also answers.
fn config_for(rule_class: RuleClass) -> &'static dyn RuleConfig {
    match rule_class {
        RuleClass::HolidayTodDif => &HolidayTimeOfDayRuleConfig,
        _ => &NoEarningRuleConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::rule_config::PRIORITY;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    /// Records `(shift id, rule item id)` for every execution.
    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<(i32, i32)>>,
    }
    impl ShiftEarningRule for Recorder {
        fn execute(&self, shift: &EmployeeShift, _tc: &mut dyn TimeCard, item: &RuleItem) {
            self.calls.borrow_mut().push((shift.id(), item.id()));
        }
    }
    struct Registry(Recorder);
    impl RuleRegistry<dyn ShiftEarningRule> for Registry {
        fn rule_type(&self) -> RuleType {
            RuleType::ShiftDifferential
        }
        fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ShiftEarningRule + 'static)> {
            Some(&self.0)
        }
    }
    fn runner(registry: &Registry) -> ShiftDifferentialRunner<'_, SavedEarnings> {
        ShiftDifferentialRunner::new(registry, SavedEarnings::default())
    }

    fn start() -> LocalDate {
        LocalDate::of(2012, 12, 12)
    }
    fn base_card() -> TimeCardData {
        TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_calculation_start_date(start())
            .with_current_pay_period(DateRange::new(start(), start().plus_days(13)))
    }
    fn rule_earning(date: LocalDate, item: i32, shift: Option<i32>) -> EmployeeEarning {
        EmployeeEarning::new(0, 1, 10, 1, date, 0.0, 0.0, EarningSource::Rule)
            .from_rule(item, shift)
    }
    fn type_of(item: i32) -> Option<RuleType> {
        match item {
            1 | 4 | 5 => Some(RuleType::ShiftDifferential),
            2 => Some(RuleType::HoursDistribution),
            3 => Some(RuleType::PostCalc),
            _ => None,
        }
    }

    // ---- java_parity_tests: ShiftDifferentialRunnerTest.groovy ----

    #[test]
    fn clean_up_earnings() {
        let registry = Registry(Recorder::default());
        let jan = LocalDate::of(2013, 1, 1);
        let kept = rule_earning(LocalDate::of(2011, 1, 1), 4, None);
        let mut card = base_card().with_earnings(vec![
            rule_earning(jan, 1, None),
            rule_earning(jan, 2, None),
            rule_earning(jan, 3, None),
            kept,
        ]);
        runner(&registry).run_rules(&mut card, |_| None, &type_of);
        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.earnings()[0].rule_item_id(), Some(4));
    }

    #[test]
    fn clean_up_earnings_should_not_remove_earnings_attached_to_shifts_in_closed_periods() {
        let registry = Registry(Recorder::default());
        let open_shift = EmployeeShift::new(1, 1, 10, start(), ShiftType::Actual, Vec::new());
        let closed_shift = EmployeeShift::new(
            2,
            1,
            10,
            start().minus_days(1),
            ShiftType::Actual,
            Vec::new(),
        );
        let mut card = base_card()
            .with_shifts(vec![open_shift, closed_shift])
            .with_earnings(vec![
                rule_earning(start(), 1, Some(1)),
                rule_earning(start(), 5, Some(2)),
            ]);
        runner(&registry).run_rules(&mut card, |_| None, &type_of);
        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.earnings()[0].shift_id(), Some(2));
    }

    #[test]
    fn run_shift_differential_rules() {
        let registry = Registry(Recorder::default());
        let shift = |id| EmployeeShift::new(id, 1, 10, start(), ShiftType::Actual, Vec::new());
        let with_error = shift(1).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut card = base_card().with_shifts(vec![with_error, shift(2), shift(3)]);
        let item1 = RuleItem::new(
            11,
            1,
            "a",
            RuleClass::HolidayTodDif,
            rule_params! { PRIORITY => "7" },
        );
        let item2 = RuleItem::new(
            12,
            1,
            "b",
            RuleClass::HolidayTodDif,
            rule_params! { PRIORITY => "2" },
        );
        let item3 = RuleItem::new(
            13,
            1,
            "c",
            RuleClass::BreakPerPenaltyHrsDif,
            Default::default(),
        );
        let set = RuleSet::new(
            1,
            1,
            "set",
            RuleType::ShiftDifferential,
            0,
            vec![item1, item2, item3],
        );

        // Shift 1 has errors (never asked); shift 2 has no rule set; shift 3 runs
        // priority 2, then the default (5), then priority 7.
        let asked = RefCell::new(Vec::new());
        runner(&registry).run_rules(
            &mut card,
            |s| {
                asked.borrow_mut().push(s.id());
                (s.id() == 3).then_some(&set)
            },
            &type_of,
        );
        assert_eq!(*asked.borrow(), vec![2, 3]);
        assert_eq!(*registry.0.calls.borrow(), vec![(3, 12), (3, 13), (3, 11)]);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_runner_clears_exactly_the_three_types() {
        assert_eq!(
            CLEARED_RULE_TYPES,
            [
                RuleType::ShiftDifferential,
                RuleType::HoursDistribution,
                RuleType::PostCalc
            ]
        );
    }

    #[test]
    fn a_class_without_a_registered_rule_is_skipped() {
        struct Empty;
        impl RuleRegistry<dyn ShiftEarningRule> for Empty {
            fn rule_type(&self) -> RuleType {
                RuleType::ShiftDifferential
            }
            fn rule_for(&self, _c: RuleClass) -> Option<&(dyn ShiftEarningRule + 'static)> {
                None
            }
        }
        let shift = EmployeeShift::new(1, 1, 10, start(), ShiftType::Actual, Vec::new());
        let mut card = base_card().with_shifts(vec![shift]);
        let set = RuleSet::new(
            1,
            1,
            "set",
            RuleType::ShiftDifferential,
            0,
            vec![RuleItem::new(
                1,
                1,
                "x",
                RuleClass::NoDif,
                Default::default(),
            )],
        );
        ShiftDifferentialRunner::new(&Empty, SavedEarnings::default()).run_rules(
            &mut card,
            |_| Some(&set),
            &type_of,
        );
    }
}
