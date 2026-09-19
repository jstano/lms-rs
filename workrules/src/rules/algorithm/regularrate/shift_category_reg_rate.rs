//! Port of `ShiftCategoryRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/ShiftCategoryRegRateRuleImpl.java`.
//!
//! Rewrites the rate only for shifts (or earnings, unconditionally) matching a
//! configured set of shift-category ids: fixed rate, or job rate ± addition/
//! multiplier, then optionally floored at minimum wage. Distributions outside
//! the category either keep the plain job-status rate or are skipped
//! entirely, gated by `isOpenForEditingOn`/`isOpenForEditingFor` and by
//! `onlyAdjustShiftsWithCategory`.
//!
//! Not the same rule as `ShiftCategoryMinWageRegRateRuleImpl` despite the
//! name overlap — see the family finding in `PARITY_AUDIT.md`.
//!
//! # The earning path ignores the shift-category gate entirely
//!
//! `execute(EmployeeEarning, ...)` never reads `SHIFT_CATEGORIES`,
//! `USE_FIXED_RATE_PROP` or any of the rate-shaping parameters — it always
//! prices at the plain job-status rate, as if no shift category ever matched.
//! Whatever this rule is configured to do, an earning is unaffected.
//!
//! Ported cases: `ShiftCategoryRegRateRuleImplTest.groovy` (three cases).

use crate::common::json_ids::ids_for_key;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::config::{
    ENFORCE_MINIMUM_WAGE_PROP, FIXED_RATE, JOB_RATE_ADDITION_PROP, JOB_RATE_MULTIPLIER_PROP,
    ONLY_ADJUST_SHIFTS_WITH_CATEGORY_PROP, SHIFT_CATEGORIES, ShiftCategoryRegRateRuleConfig,
    USE_FIXED_RATE_PROP,
};
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use crate::rules::params::RuleParams;
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;

/// `ShiftCategoryRegRateRuleImpl`.
pub struct ShiftCategoryRegRateRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> ShiftCategoryRegRateRule<M> {
    /// Build the rule over the port its minimum-wage floor comes from.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    /// `getRate(EmployeeShift, TimeCard, Map, LocalDate)`.
    fn matching_category_rate(
        &self,
        shift: &EmployeeShift,
        dataset: &dyn TimeCard,
        params: &RuleParams,
    ) -> f64 {
        let job_status_rate = job_status_rate(shift, dataset);

        let mut rate = if params.bool_at(USE_FIXED_RATE_PROP) {
            params.double_at(FIXED_RATE)
        } else {
            let addition = params.double_at(JOB_RATE_ADDITION_PROP);
            let multiplier = params.double_at(JOB_RATE_MULTIPLIER_PROP);
            let base = if multiplier > 0.0 {
                job_status_rate * multiplier
            } else {
                job_status_rate
            };
            (base + addition).max(0.0)
        };

        if params.bool_at(ENFORCE_MINIMUM_WAGE_PROP) {
            let property_id = dataset
                .employee()
                .map_or(0, |employee| employee.property_id());
            let min_wage =
                self.min_wage
                    .min_wage(property_id, Some(shift.job_id()), shift.shift_date());
            rate = rate.max(min_wage);
        }

        rate
    }
}

/// `shift.getEmployeeJobStatus().getHourlyRate()` — unguarded in Java, a
/// panic here on a shift with no covering job status.
fn job_status_rate(shift: &EmployeeShift, dataset: &dyn TimeCard) -> f64 {
    dataset
        .employee_job_status_for_shift(shift)
        .unwrap_or_else(|| panic!("no job status for shift {}", shift.id()))
        .hourly_rate()
}

impl<M: MinWagePort> RegularRateRule for ShiftCategoryRegRateRule<M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        if !dataset.is_open_for_editing_on(distribution.date()) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&ShiftCategoryRegRateRuleConfig.default_values());
        let shift_category_ids = ids_for_key(SHIFT_CATEGORIES, &params);
        let in_category = shift
            .shift_category_id()
            .is_some_and(|id| shift_category_ids.contains(&id));

        let rate = if in_category {
            self.matching_category_rate(shift, dataset, &params)
        } else {
            if params.bool_at(ONLY_ADJUST_SHIFTS_WITH_CATEGORY_PROP) {
                return;
            }
            job_status_rate(shift, dataset)
        };

        set_regular_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        _rule_item: &RuleItem,
    ) {
        if !dataset.is_open_for_editing_for_earning(earning) {
            return;
        }

        let rate = dataset
            .employee()
            .and_then(|employee| {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            })
            .unwrap_or_else(|| panic!("no job status for earning {}", earning.id()))
            .hourly_rate();

        set_earning_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

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
        RuleItem::new(id, 1, "Shift category rate", RuleClass::ShiftCatRrr, params)
    }

    fn job_status(job_id: i32, hourly_rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            job_id,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            false,
        )
    }

    #[test]
    fn a_non_matching_category_leaves_the_plain_job_rate() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 8.5)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_shift_category_id(Some(9));
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { SHIFT_CATEGORIES => "[4]" });
        let rule = ShiftCategoryRegRateRule::new(FixedMinWage(0.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.base_rate(), 8.5);
    }

    mod java_parity_tests {
        use super::*;

        /// `ShiftCategoryRegRateRuleImplTest`: "rate is set to fixed rate if
        /// there is a matching shift category".
        #[test]
        fn rate_is_set_to_fixed_rate_if_there_is_a_matching_shift_category() {
            let now = LocalDate::of(2016, 6, 1);
            let employee = Employee::new(100, 1, "", vec![job_status(5, 8.5)]);
            let dataset = TimeCardData::new().with_employee(employee);
            let shift = EmployeeShift::new(1, 100, 5, now, ShiftType::Actual, Vec::new())
                .with_shift_category_id(Some(4));
            let mut distribution = HoursDistribution::new(1, now, None, 0.0, 0.0);
            let item = rule_item(
                10,
                rule_params! {
                    SHIFT_CATEGORIES => "[4]",
                    FIXED_RATE => "10.50",
                    USE_FIXED_RATE_PROP => "true",
                },
            );
            let rule = ShiftCategoryRegRateRule::new(FixedMinWage(0.0));

            rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

            assert_eq!(distribution.base_rate(), 10.50);
            assert_eq!(distribution.rate_rule_item_id(), Some(10));
        }

        /// `ShiftCategoryRegRateRuleImplTest`: "rate is set to hourly job rate
        /// if there is no shift category match".
        #[test]
        fn rate_is_set_to_hourly_job_rate_if_there_is_no_shift_category_match() {
            let now = LocalDate::of(2016, 6, 1);
            let employee = Employee::new(100, 1, "", vec![job_status(5, 8.5)]);
            let dataset = TimeCardData::new().with_employee(employee);
            let shift = EmployeeShift::new(1, 100, 5, now, ShiftType::Actual, Vec::new())
                .with_shift_category_id(Some(4));
            let mut distribution = HoursDistribution::new(1, now, None, 0.0, 0.0);
            // `"[" + shiftCategoryID + 9 + "]"` in the Groovy is `"[49]"` —
            // string concatenation, not addition — which never matches 4.
            let item = rule_item(
                10,
                rule_params! {
                    SHIFT_CATEGORIES => "[49]",
                    FIXED_RATE => "10.50",
                },
            );
            let rule = ShiftCategoryRegRateRule::new(FixedMinWage(0.0));

            rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

            assert_eq!(distribution.base_rate(), 8.5);
            assert_eq!(distribution.rate_rule_item_id(), Some(10));
        }

        /// `ShiftCategoryRegRateRuleImplTest`: "earning rate is set to job
        /// hourly rate".
        #[test]
        fn earning_rate_is_set_to_job_hourly_rate() {
            let now = LocalDate::of(2016, 6, 1);
            let employee = Employee::new(100, 1, "", vec![job_status(5, 8.5)]);
            let dataset = TimeCardData::new().with_employee(employee);
            let mut earning =
                EmployeeEarning::new(1, 100, 5, 0, now, 5.0, 0.0, EarningSource::Rule);
            let item = rule_item(
                10,
                rule_params! {
                    SHIFT_CATEGORIES => "[49]",
                    FIXED_RATE => "10.50",
                },
            );
            let rule = ShiftCategoryRegRateRule::new(FixedMinWage(0.0));

            rule.execute_for_earning(&mut earning, &dataset, &item);

            assert_eq!(earning.rate(), 8.5);
            assert_eq!(earning.total_dollars(), 5.0 * 8.5);
        }
    }
}
