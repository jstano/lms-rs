//! Port of `HomeJobRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/HomeJobRateRuleImpl.java`.
//!
//! Home job-status rate by UOM (hourly/piece), `0.0` if the employee has no
//! home job that date, times a configured factor, floored at minimum wage if
//! the earning type says so.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::config::{HomeJobRateRuleConfig, RATE_FACTOR};
use crate::rules::algorithm::earningrate::{EarningRateRule, set_earning_rate};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;

/// `HomeJobRateRuleImpl`.
pub struct HomeJobRateRule<E: EarningTypePort, M: MinWagePort> {
    earning_types: E,
    min_wage: M,
}

impl<E: EarningTypePort, M: MinWagePort> HomeJobRateRule<E, M> {
    /// Build the rule over the ports its earning type and minimum-wage floor
    /// come from.
    pub fn new(earning_types: E, min_wage: M) -> Self {
        Self {
            earning_types,
            min_wage,
        }
    }
}

impl<E: EarningTypePort, M: MinWagePort> EarningRateRule for HomeJobRateRule<E, M> {
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&HomeJobRateRuleConfig.default_values());
        let rate_factor = params.double_at(RATE_FACTOR);

        let employee = dataset.employee().expect("no employee on the dataset");
        let job_status = employee.home_employee_job_status(earning.earning_date());

        let earning_type = self
            .earning_types
            .find_by_id(earning.earning_type_id())
            .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));

        let hourly_rate = job_status.map_or(0.0, |status| match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars => status.hourly_rate(),
            UOM::Units => status.piece_rate(),
            _ => 0.0,
        });
        let mut hourly_rate = round_currency(hourly_rate * rate_factor);

        let min_wage = self.min_wage.min_wage(
            employee.property_id(),
            Some(earning.job_id()),
            earning.earning_date(),
        );
        if earning_type.pay_at_least_min_wage() && hourly_rate < min_wage {
            hourly_rate = min_wage;
        }

        set_earning_rate(earning, hourly_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    struct EarningTypes(HashMap<i32, EarningType>);
    impl EarningTypePort for EarningTypes {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }

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

    fn job_status(job_id: i32, hourly_rate: f64, home: bool) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            job_id,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            home,
        )
    }

    #[test]
    fn no_home_job_prices_the_earning_at_zero() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, false)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = HomeJobRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        assert_eq!(earning.rate(), 0.0);
    }

    #[test]
    fn the_home_job_rate_is_scaled_by_the_factor() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = HomeJobRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "2.0" },
        );

        assert_eq!(earning.rate(), 20.0);
    }
}
