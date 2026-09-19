//! Port of `EarningFactorRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/EarningFactorRateRuleImpl.java`.
//!
//! Job-status rate (home job's if configured, else the earning's own job) by
//! UOM (hourly or piece), scaled by a configured factor **only if** the
//! earning's type is in a configured allow-list, then floored at minimum wage
//! if the earning type says so.
//!
//! # The only rule in the family that reads `selectedEarnings`
//!
//! See the family finding in `config.rs`: every config declares and
//! validates `selectedEarnings`, but this is the one rule whose algorithm
//! actually reads it — as the gate deciding whether `rateFactor` applies at
//! all, not as a job/earning filter the way its name might suggest.
//!
//! Also called directly by [`EarningOverrideJobRateRuleImpl`](super::earning_override_job_rate)
//! with a fresh, differently-configured instance — see that module's doc.
//!
//! No Groovy spec of its own; behaviour tests are written from the Java.

use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::config::{
    EarningFactorRateRuleConfig, HOME_AS_BASE_PROP, RATE_FACTOR, selected_earning_ids,
};
use crate::rules::algorithm::earningrate::{EarningRateRule, set_earning_rate};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;

/// `EarningFactorRateRuleImpl`.
pub struct EarningFactorRateRule<E: EarningTypePort, M: MinWagePort> {
    earning_types: E,
    min_wage: M,
}

impl<E: EarningTypePort, M: MinWagePort> EarningFactorRateRule<E, M> {
    /// Build the rule over the ports its earning type and minimum-wage floor
    /// come from.
    pub fn new(earning_types: E, min_wage: M) -> Self {
        Self {
            earning_types,
            min_wage,
        }
    }
}

impl<E: EarningTypePort, M: MinWagePort> EarningRateRule for EarningFactorRateRule<E, M> {
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&EarningFactorRateRuleConfig.default_values());
        let earning_type_ids = selected_earning_ids(&params);
        let rate_factor = params.double_at(RATE_FACTOR);
        let home_job_rate = params.bool_at(HOME_AS_BASE_PROP);

        let employee = dataset.employee().expect("no employee on the dataset");
        let job_status = || {
            if home_job_rate {
                employee.home_employee_job_status(earning.earning_date())
            } else {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            }
            .unwrap_or_else(|| panic!("no job status for earning {}", earning.id()))
        };

        let earning_type = self
            .earning_types
            .find_by_id(earning.earning_type_id())
            .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));

        // Java only dereferences `jobStatus` inside the HOURS/HOURS_DOLLARS/
        // UNITS branches, so an unmatched UOM never NPEs on a missing status.
        let mut hourly_rate = match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars => job_status().hourly_rate(),
            UOM::Units => job_status().piece_rate(),
            _ => 0.0,
        };

        if earning_type_ids.contains(&earning.earning_type_id()) {
            hourly_rate = round_currency(hourly_rate * rate_factor);
        }

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
    fn the_rate_factor_only_applies_to_selected_earning_types() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, false)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = EarningFactorRateRule::new(earning_types, FixedMinWage(0.0));
        let params = rule_params! { "selectedEarnings" => "[]", RATE_FACTOR => "2.0" };

        rule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 10.0, "not selected, no factor applied");

        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = EarningFactorRateRule::new(earning_types, FixedMinWage(0.0));
        let params = rule_params! { "selectedEarnings" => "[7]", RATE_FACTOR => "2.0" };

        rule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 20.0, "selected, factor applied");
    }

    #[test]
    fn the_home_job_status_is_used_when_configured() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(
            100,
            1,
            "Alex Kim",
            vec![job_status(1, 10.0, false), job_status(2, 20.0, true)],
        );
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = EarningFactorRateRule::new(earning_types, FixedMinWage(0.0));
        let params = rule_params! { HOME_AS_BASE_PROP => "true" };

        rule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 20.0);
    }

    #[test]
    fn a_rate_below_minimum_wage_is_floored_when_the_earning_type_requires_it() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 5.0, false)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                .with_flags(true, false, false, false),
        )]));
        let mut rule = EarningFactorRateRule::new(earning_types, FixedMinWage(7.25));

        rule.execute(&dataset, &mut earning, &RuleParams::new());

        assert_eq!(earning.rate(), 7.25);
    }
}
