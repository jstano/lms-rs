//! Port of `ShiftCategoryMinWageRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/ShiftCategoryMinWageRegRateRuleImpl.java`.
//!
//! Always starts from the job-status rate; only *floors* it at minimum wage,
//! and only for shifts in the configured category list. No fixed-rate or
//! multiplier path, no open-for-editing gate — unlike its name-alike
//! `ShiftCategoryRegRateRuleImpl`, which does none of what this rule does. See
//! the family finding in `PARITY_AUDIT.md`.
//!
//! # The minimum-wage floor never applies to an earning
//!
//! Java's private `getRate` takes the shift category as a parameter; the shift
//! overload passes `shift.getShiftCategory()`, but the earning overload always
//! passes `null`. So the `shiftCategoryIDs.contains(...)` gate is permanently
//! false on the earning path — an earning is priced at the plain job-status
//! rate, minimum wage never enters into it, regardless of configuration.
//! Reproduced by threading `None` through on that path rather than by special
//! casing the earning overload.
//!
//! Ported cases: `ShiftCategoryMinWageRegRateRuleImplTest.groovy` (two cases).

use crate::common::json_ids::ids_for_key;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::config::{
    SHIFT_CATEGORIES, ShiftCategoryMinWageRegRateRuleConfig,
};
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `ShiftCategoryMinWageRegRateRuleImpl`.
pub struct ShiftCategoryMinWageRegRateRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> ShiftCategoryMinWageRegRateRule<M> {
    /// Build the rule over the port its minimum-wage floor comes from.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    /// `getRate(LocalDate, Assignment, ShiftCategory, TimeCard, Map)`.
    fn rate(
        &self,
        effective_date: LocalDate,
        job_id: i32,
        shift_category_id: Option<i32>,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) -> f64 {
        let params = rule_item
            .params()
            .fixed(&ShiftCategoryMinWageRegRateRuleConfig.default_values());

        let hourly_rate = dataset
            .employee()
            .and_then(|employee| employee.employee_job_status(job_id, effective_date))
            .map_or(0.0, |status| status.hourly_rate());

        let shift_category_ids = ids_for_key(SHIFT_CATEGORIES, &params);
        let in_category = shift_category_id.is_some_and(|id| shift_category_ids.contains(&id));

        if in_category {
            let property_id = dataset
                .employee()
                .map_or(0, |employee| employee.property_id());
            let min_wage = self
                .min_wage
                .min_wage(property_id, Some(job_id), effective_date);
            hourly_rate.max(min_wage)
        } else {
            hourly_rate
        }
    }
}

impl<M: MinWagePort> RegularRateRule for ShiftCategoryMinWageRegRateRule<M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let rate = self.rate(
            distribution.date(),
            shift.job_id(),
            shift.shift_category_id(),
            dataset,
            rule_item,
        );
        set_regular_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let rate = self.rate(
            earning.earning_date(),
            earning.job_id(),
            None,
            dataset,
            rule_item,
        );
        set_earning_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    struct FixedMinWage(f64);
    impl MinWagePort for FixedMinWage {
        fn min_wage(
            &self,
            _property_id: i32,
            _job_id: Option<i32>,
            _effective_date: LocalDate,
        ) -> f64 {
            self.0
        }
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "Shift category min wage", RuleClass::ScmwRrr, params)
    }

    fn job_status(hourly_rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            false,
        )
    }

    #[test]
    fn a_rate_below_minimum_wage_is_floored_when_the_category_matches() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(7.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_shift_category_id(Some(4));
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { SHIFT_CATEGORIES => "[4]" });
        let rule = ShiftCategoryMinWageRegRateRule::new(FixedMinWage(9.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.base_rate(), 9.0);
    }

    #[test]
    fn a_rate_above_minimum_wage_is_left_alone() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(12.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_shift_category_id(Some(4));
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { SHIFT_CATEGORIES => "[4]" });
        let rule = ShiftCategoryMinWageRegRateRule::new(FixedMinWage(9.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.base_rate(), 12.0);
    }

    #[test]
    fn the_floor_never_applies_to_an_earning() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(7.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning = EmployeeEarning::new(
            1,
            100,
            1,
            5,
            earning_date,
            4.0,
            0.0,
            crate::common::enums::earning_source::EarningSource::Rule,
        );
        let item = rule_item(1, rule_params! { SHIFT_CATEGORIES => "[4]" });
        let rule = ShiftCategoryMinWageRegRateRule::new(FixedMinWage(9.0));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(
            earning.rate(),
            7.0,
            "no shift category ever reaches the earning path"
        );
    }

    mod java_parity_tests {
        use super::*;
        use crate::entity::assignment::Assignment;
        use crate::rules::ports::AssignmentPort;

        struct NoAssignments;
        impl AssignmentPort for NoAssignments {
            fn find_by_id(&self, _id: i32) -> Option<Assignment> {
                None
            }
        }

        /// `ShiftCategoryMinWageRegRateRuleImplTest`: "execute sets the reg
        /// rate on a shift and base rate on regular hours distributions" —
        /// both rows of the `where:` table.
        #[test]
        fn execute_sets_the_reg_rate_on_a_shift_and_base_rate_on_regular_hours_distributions() {
            let _ = NoAssignments;
            let shift_date = LocalDate::of(2016, 6, 1);

            for (base_rate, min_wage, expected_rate) in [(8.5, 7.25, 8.5), (6.5, 7.25, 7.25)] {
                let employee = Employee::new(100, 1, "", vec![job_status(base_rate)]);
                let dataset = TimeCardData::new().with_employee(employee);
                let shift =
                    EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                        .with_shift_category_id(Some(1));
                let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
                let item = rule_item(1, rule_params! { SHIFT_CATEGORIES => "[1]" });
                let rule = ShiftCategoryMinWageRegRateRule::new(FixedMinWage(min_wage));

                rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

                assert_eq!(
                    distribution.base_rate(),
                    expected_rate,
                    "baseRate={base_rate} minWage={min_wage}"
                );
            }
        }
    }
}
