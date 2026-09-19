//! Port of `HomeJobRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/HomeJobRegRateRuleImpl.java`.
//!
//! The employee's home job status's rate — `0.0` if they have none that date.
//! No job or department comparison at all, unlike its `HomeDept*` sibling.
//!
//! Ported cases: `HomeJobRegRateRuleImplTest.groovy` (one case).

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use joda_rs::LocalDate;

/// `HomeJobRegRateRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HomeJobRegRateRule;

impl HomeJobRegRateRule {
    /// `getRate(LocalDate, TimeCard)`. `0.0` where Java's home status is null.
    fn rate(dataset: &dyn TimeCard, effective_date: LocalDate) -> f64 {
        dataset
            .employee()
            .and_then(|employee| employee.home_employee_job_status(effective_date))
            .map_or(0.0, |status| status.hourly_rate())
    }
}

impl RegularRateRule for HomeJobRegRateRule {
    fn execute_for_shift(
        &self,
        _shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let rate = Self::rate(dataset, distribution.date());
        set_regular_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        _rule_item: &RuleItem,
    ) {
        let rate = Self::rate(dataset, earning.earning_date());
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
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    fn rule_item(id: i32) -> RuleItem {
        RuleItem::new(
            id,
            1,
            "Home job rate",
            RuleClass::HomeJobRrr,
            RuleParams::new(),
        )
    }

    #[test]
    fn rates_are_set_to_the_home_job_rate() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let job_status = EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            7.5,
            true,
        );
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_net_hours(8.0);
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

        HomeJobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &rule_item(10));

        assert_eq!(distribution.base_rate(), 7.5);
        assert_eq!(distribution.rate_rule_item_id(), Some(10));
    }

    #[test]
    fn no_home_job_status_prices_at_zero() {
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning = EmployeeEarning::new(
            1,
            100,
            1,
            5,
            LocalDate::of(2016, 6, 1),
            4.0,
            0.0,
            EarningSource::Rule,
        );

        HomeJobRegRateRule.execute_for_earning(&mut earning, &dataset, &rule_item(10));

        assert_eq!(earning.rate(), 0.0);
    }

    mod java_parity_tests {
        use super::*;

        /// `HomeJobRegRateRuleImplTest`: "rates should be set to home job
        /// rate".
        #[test]
        fn rates_should_be_set_to_home_job_rate() {
            let shift_date = LocalDate::of(2016, 6, 1);
            let job_status = EmployeeJobStatus::new(
                1,
                100,
                1,
                LocalDate::of(1999, 10, 10),
                LocalDate::of(3000, 10, 10),
                EmployeePayType::Hourly,
                7.5,
                true,
            );
            let employee = Employee::new(100, 1, "", vec![job_status]);
            let dataset = TimeCardData::new().with_employee(employee);
            let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                .with_net_hours(8.0);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            HomeJobRegRateRule.execute_for_shift(
                &shift,
                &mut distribution,
                &dataset,
                &rule_item(10),
            );

            assert_eq!(distribution.base_rate(), 7.5);
            assert_eq!(distribution.rate_rule_item_id(), Some(10));
        }
    }
}
