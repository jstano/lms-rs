//! Port of `ContractDailyRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/ContractDailyRateRuleImpl.java`.
//!
//! Backs out a **daily** rate from a contract: home job's hourly-or-piece
//! rate (by UOM — `Hours`, `HoursDollars` and `Days` all read the hourly
//! rate; `Units` reads the piece rate) times a configured factor, floored at
//! minimum wage, times `contractHours / contractDays`. `0.0` outright if
//! there is no home job status that date, or if `contractDays == 0.0`.
//!
//! # An unguarded division, faithfully reproduced
//!
//! Java's `homeEmployeeJobStatus.getContractHours()` is a boxed `Double`; if
//! it is null while `contractDays != 0.0`, unboxing it for the division NPEs.
//! `EmployeeJobStatus::contract_hours` is `Option<f64>` for the same reason —
//! this port panics in the same case rather than silently treating a missing
//! `contractHours` as `0.0`.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::config::{ContractDailyRateRuleConfig, RATE_FACTOR};
use crate::rules::algorithm::earningrate::{EarningRateRule, set_earning_rate};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;

/// `ContractDailyRateRuleImpl`.
pub struct ContractDailyRateRule<E: EarningTypePort, M: MinWagePort> {
    earning_types: E,
    min_wage: M,
}

impl<E: EarningTypePort, M: MinWagePort> ContractDailyRateRule<E, M> {
    /// Build the rule over the ports its earning type and minimum-wage floor
    /// come from.
    pub fn new(earning_types: E, min_wage: M) -> Self {
        Self {
            earning_types,
            min_wage,
        }
    }
}

impl<E: EarningTypePort, M: MinWagePort> EarningRateRule for ContractDailyRateRule<E, M> {
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&ContractDailyRateRuleConfig.default_values());
        let rate_factor = params.double_at(RATE_FACTOR);

        let employee = dataset.employee().expect("no employee on the dataset");
        let mut daily_rate = 0.0;

        if let Some(home_status) = employee.home_employee_job_status(earning.earning_date()) {
            let earning_type = self
                .earning_types
                .find_by_id(earning.earning_type_id())
                .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));

            let hourly_rate = match earning_type.uom() {
                UOM::Hours | UOM::HoursDollars | UOM::Days => home_status.hourly_rate(),
                UOM::Units => home_status.piece_rate(),
                _ => 0.0,
            };
            let mut hourly_rate = round_currency(hourly_rate * rate_factor);

            let min_wage = self.min_wage.min_wage(
                employee.property_id(),
                Some(earning.job_id()),
                earning.earning_date(),
            );
            if earning_type.pay_at_least_min_wage() && hourly_rate < min_wage {
                hourly_rate = min_wage;
            }

            if home_status.contract_days() != 0.0 {
                let contract_hours = home_status.contract_hours().unwrap_or_else(|| {
                    panic!(
                        "no contract hours for job status {} with nonzero contract days",
                        home_status.id()
                    )
                });
                let daily_hours = contract_hours / home_status.contract_days();
                daily_rate = round_currency(daily_hours * hourly_rate);
            }
        }

        set_earning_rate(earning, daily_rate);
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

    fn home_status(hourly_rate: f64, contract_hours: f64, contract_days: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            true,
        )
        .with_salaried_fields(0.0, 0.0, Some(contract_hours), None)
        .with_contract_days(contract_days)
    }

    #[test]
    fn the_daily_rate_divides_contract_hours_by_contract_days() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![home_status(10.0, 40.0, 5.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = ContractDailyRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        // 40 / 5 = 8 daily hours * 10.0 hourly = 80.0
        assert_eq!(earning.rate(), 80.0);
    }

    #[test]
    fn zero_contract_days_prices_the_earning_at_zero() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![home_status(10.0, 40.0, 0.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = ContractDailyRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        assert_eq!(earning.rate(), 0.0);
    }

    #[test]
    fn no_home_job_status_prices_the_earning_at_zero() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::new());
        let mut rule = ContractDailyRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        assert_eq!(earning.rate(), 0.0);
    }
}
