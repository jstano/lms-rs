//! Port of `FactorJobRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/FactorJobRegRateRuleImpl.java`.
//!
//! The job's own rate times a configured factor — a scaling rule, not a
//! lookup rule. Both overloads unwrap the job status unguarded, matching
//! `JobRegRateRuleImpl`'s earning overload rather than its shift one: a
//! missing job status panics here on either path.
//!
//! Ported cases: `FactorJobRegRateRuleImplTest.groovy` (one case).

use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::config::{FactorJobRegRateRuleConfig, WAGE_FACTOR_PROP};
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use crate::rules::rule_config::RuleConfig;

/// `FactorJobRegRateRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FactorJobRegRateRule;

impl RegularRateRule for FactorJobRegRateRule {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FactorJobRegRateRuleConfig.default_values());
        let wage_factor = params.double_at(WAGE_FACTOR_PROP);

        let job_status = dataset
            .employee()
            .and_then(|employee| employee.employee_job_status(shift.job_id(), distribution.date()))
            .unwrap_or_else(|| {
                panic!(
                    "no job status for job {} on {}",
                    shift.job_id(),
                    distribution.date()
                )
            });

        let adjusted_rate = round_currency(job_status.hourly_rate() * wage_factor);
        set_regular_rates(distribution, adjusted_rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FactorJobRegRateRuleConfig.default_values());
        let wage_factor = params.double_at(WAGE_FACTOR_PROP);

        let job_status = dataset
            .employee()
            .and_then(|employee| {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            })
            .unwrap_or_else(|| {
                panic!(
                    "no job status for job {} on {}",
                    earning.job_id(),
                    earning.earning_date()
                )
            });

        let adjusted_rate = round_currency(job_status.hourly_rate() * wage_factor);
        set_earning_rate(earning, adjusted_rate);
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
    use joda_rs::LocalDate;

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "Factor job rate", RuleClass::FactorJobRrr, params)
    }

    #[test]
    fn the_rate_is_the_job_rate_times_the_wage_factor() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let job_status = EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            8.0,
            false,
        );
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_net_hours(8.0);
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { WAGE_FACTOR_PROP => "1.5" });

        FactorJobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.base_rate(), 12.0);
        assert_eq!(distribution.rate_rule_item_id(), Some(1));
    }

    #[test]
    #[should_panic]
    fn a_missing_job_status_panics() {
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let shift_date = LocalDate::of(2016, 6, 1);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

        FactorJobRegRateRule.execute_for_shift(
            &shift,
            &mut distribution,
            &dataset,
            &rule_item(1, RuleParams::new()),
        );
    }

    mod java_parity_tests {
        use super::*;

        /// `FactorJobRegRateRuleImplTest`: "the hours distribution and shift
        /// rate are set using the wage factor property".
        #[test]
        fn the_hours_distribution_and_shift_rate_are_set_using_the_wage_factor_property() {
            let shift_date = LocalDate::of(2016, 6, 1);
            let job_status = EmployeeJobStatus::new(
                1,
                100,
                1,
                LocalDate::of(1999, 10, 10),
                LocalDate::of(3000, 10, 10),
                EmployeePayType::Hourly,
                8.0,
                false,
            );
            let employee = Employee::new(100, 1, "", vec![job_status]);
            let dataset = TimeCardData::new().with_employee(employee);
            let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                .with_net_hours(8.0);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
            let item = rule_item(1, rule_params! { WAGE_FACTOR_PROP => "1.5" });

            FactorJobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

            assert_eq!(distribution.base_rate(), 12.0);
            assert_eq!(distribution.rate_rule_item_id(), Some(1));
        }
    }
}
