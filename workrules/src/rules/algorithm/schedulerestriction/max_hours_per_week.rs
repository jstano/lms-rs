//! Port of `MaxHoursPerWeekRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/MaxHoursPerWeekRuleImpl.java`.
//!
//! The summed net hours of every shift in the property's work week
//! containing the shift under test must not exceed `maxHours`.
//!
//! `Property.getWorkWeekForDate(date)` reaches the same value
//! `FLSAOTRateRuleImpl`/`CombinationJobsRegRateRuleImpl` already do —
//! `WeeklyDateRange::with_end_date(property.period_end_date(id)).range_containing_date(date)`
//! through [`PropertyPort`] — no new surface needed.
//!
//! Ported cases: `MaxHoursPerWeekRuleImplTest.groovy` (three cases).

use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::config::{
    MAX_HOURS_PER_WEEK_PROP, MaxHoursPerWeekRuleConfig,
};
use crate::rules::algorithm::schedulerestriction::{
    ScheduleRestrictionResult, ScheduleRestrictionRule,
};
use crate::rules::ports::PropertyPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;

/// `MaxHoursPerWeekRuleImpl`.
pub struct MaxHoursPerWeekRule<P: PropertyPort> {
    property: P,
}

impl<P: PropertyPort> MaxHoursPerWeekRule<P> {
    /// Build the rule over the port its work week comes from.
    pub fn new(property: P) -> Self {
        Self { property }
    }
}

impl<P: PropertyPort> ScheduleRestrictionRule for MaxHoursPerWeekRule<P> {
    fn can_employee_work_shift(
        &self,
        dataset: &dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult {
        let params = rule_item
            .params()
            .fixed(&MaxHoursPerWeekRuleConfig.default_values());
        let employee = dataset.employee().expect("no employee on the dataset");
        let period_end_date = self.property.period_end_date(employee.property_id());
        let work_week = WeeklyDateRange::with_end_date(period_end_date)
            .range_containing_date(shift.shift_date());

        let max_hours = params.double_at(MAX_HOURS_PER_WEEK_PROP);
        let mut hours_for_week = 0.0;
        for shift_in_week in dataset.shifts_for_period(&work_week) {
            hours_for_week += shift_in_week.net_hours();
            if hours_for_week > max_hours {
                return ScheduleRestrictionResult::error(ShiftErrorType::MaxHoursPerWeek);
            }
        }

        ScheduleRestrictionResult::ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    struct FixedPeriodEnd(LocalDate);
    impl PropertyPort for FixedPeriodEnd {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.0
        }
    }

    fn shift(net_hours: f64, shift_date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new())
            .with_net_hours(net_hours)
    }

    fn rule_item() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "Max hours per week",
            RuleClass::MaxHoursPerWeekSsr,
            RuleParams::new(),
        )
    }

    fn dataset(shifts: Vec<EmployeeShift>) -> TimeCardData {
        let employee = Employee::new(1, 1, "", Vec::new());
        TimeCardData::new()
            .with_employee(employee)
            .with_shifts(shifts)
    }

    mod java_parity_tests {
        use super::*;

        /// `MaxHoursPerWeekRuleImplTest`: "if a shift hours are equal to max
        /// hours an ok result should be returned".
        #[test]
        fn hours_equal_to_max_hours_is_ok() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = dataset(vec![
                target.clone(),
                shift(11.0, LocalDate::of(2018, 2, 20)),
                shift(11.0, LocalDate::of(2018, 2, 19)),
                shift(10.0, LocalDate::of(2018, 2, 24)),
            ]);
            let rule = MaxHoursPerWeekRule::new(FixedPeriodEnd(LocalDate::of(2018, 2, 24)));

            let result = rule.can_employee_work_shift(&dataset, &target, &rule_item());

            assert!(result.is_ok());
        }

        /// `MaxHoursPerWeekRuleImplTest`: "if shift hours in a week are
        /// pushed over max hours by the new shift it should return an
        /// error".
        #[test]
        fn pushed_over_max_hours_by_the_new_shift_is_an_error() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = dataset(vec![
                shift(18.0, LocalDate::of(2018, 2, 20)),
                shift(18.0, LocalDate::of(2018, 2, 22)),
                target.clone(),
            ]);
            let rule = MaxHoursPerWeekRule::new(FixedPeriodEnd(LocalDate::of(2018, 2, 24)));

            let result = rule.can_employee_work_shift(&dataset, &target, &rule_item());

            assert_eq!(
                result.shift_error_type(),
                Some(ShiftErrorType::MaxHoursPerWeek)
            );
        }

        /// `MaxHoursPerWeekRuleImplTest`: "hours from other weeks should not
        /// be included in max hours calculation".
        #[test]
        fn hours_from_other_weeks_are_not_included() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = dataset(vec![
                target.clone(),
                shift(40.0, LocalDate::of(2018, 2, 17)),
                shift(40.0, LocalDate::of(2018, 2, 25)),
            ]);
            let rule = MaxHoursPerWeekRule::new(FixedPeriodEnd(LocalDate::of(2018, 2, 24)));

            let result = rule.can_employee_work_shift(&dataset, &target, &rule_item());

            assert!(result.is_ok());
        }
    }
}
