//! Port of `EarningOverrideJobRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/EarningOverrideJobRateRuleImpl.java`.
//!
//! If the employee's plain job rate already meets minimum wage, **delegates
//! entirely** to a fresh [`EarningFactorRateRule`](super::earning_factor_rate::EarningFactorRateRule)
//! constructed with that rule's *default* config values — not this rule's own
//! params. Otherwise applies a configured override rate (itself possibly
//! replaced by minimum wage) times a factor.
//!
//! # An unusual cross-rule call
//!
//! Java autowires a `EarningFactorRateRuleImpl` field and calls its `execute`
//! directly with `new EarningFactorRateRuleConfig().getDefaultValues()` —
//! not a shared helper, and not this rule's own parameters. The port mirrors
//! that exactly: the struct owns an `EarningFactorRateRule` and calls its
//! `execute` with [`EarningFactorRateRuleConfig::default_values`], discarding
//! whatever params this rule itself was configured with.
//!
//! `employeeJobRate` reads `earning.getEmployee().getEmployeeJobStatus(...)`
//! with no null guard — an NPE in Java if the earning's job has no status
//! covering its date; reproduced as a panic.
//!
//! Ported cases: `EarningOverrideJobRateRuleImplTest.groovy` (six cases).

use crate::common::numbers::round_display_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::EarningRateRule;
use crate::rules::algorithm::earningrate::config::{
    EarningFactorRateRuleConfig, EarningOverrideJobRateRuleConfig, OVERRIDE_RATE, RATE_FACTOR,
    USE_MIN_WAGE,
};
use crate::rules::algorithm::earningrate::earning_factor_rate::EarningFactorRateRule;
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;

/// `EarningOverrideJobRateRuleImpl`.
pub struct EarningOverrideJobRateRule<E: EarningTypePort + Clone, M: MinWagePort + Clone> {
    default_rate_rule: EarningFactorRateRule<E, M>,
    earning_types: E,
    min_wage: M,
}

impl<E: EarningTypePort + Clone, M: MinWagePort + Clone> EarningOverrideJobRateRule<E, M> {
    /// Build the rule over the ports its own lookups and the delegated
    /// `EarningFactorRateRule` need.
    pub fn new(earning_types: E, min_wage: M) -> Self {
        Self {
            default_rate_rule: EarningFactorRateRule::new(earning_types.clone(), min_wage.clone()),
            earning_types,
            min_wage,
        }
    }

    fn employee_job_rate(&self, dataset: &dyn TimeCard, earning: &EmployeeEarning) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        employee
            .employee_job_status(earning.job_id(), earning.earning_date())
            .unwrap_or_else(|| panic!("no job status for earning {}", earning.id()))
            .hourly_rate()
    }

    fn job_min_wage(&self, dataset: &dyn TimeCard, earning: &EmployeeEarning) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        self.min_wage.min_wage(
            employee.property_id(),
            Some(earning.job_id()),
            earning.earning_date(),
        )
    }
}

impl<E: EarningTypePort + Clone, M: MinWagePort + Clone> EarningRateRule
    for EarningOverrideJobRateRule<E, M>
{
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&EarningOverrideJobRateRuleConfig.default_values());

        if self.employee_job_rate(dataset, earning) >= self.job_min_wage(dataset, earning) {
            self.default_rate_rule.execute(
                dataset,
                earning,
                &EarningFactorRateRuleConfig.default_values(),
            );
            return;
        }

        let override_rate = params.double_at(OVERRIDE_RATE);
        let job_min_wage = self.job_min_wage(dataset, earning);
        let earning_type = self
            .earning_types
            .find_by_id(earning.earning_type_id())
            .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));
        let pay_at_least_min_wage = earning_type.pay_at_least_min_wage();

        let use_min_wage =
            params.bool_at(USE_MIN_WAGE) || (override_rate < job_min_wage && pay_at_least_min_wage);
        let earning_rate = if use_min_wage {
            job_min_wage
        } else {
            override_rate
        };

        let factor = params.double_at(RATE_FACTOR);
        earning.set_rate(round_display_currency(earning_rate * factor));
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

    #[derive(Clone)]
    struct EarningTypes(HashMap<i32, EarningType>);
    impl EarningTypePort for EarningTypes {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }

    #[derive(Clone)]
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
    fn delegates_to_the_default_factor_rule_when_the_job_meets_minimum_wage() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = EarningOverrideJobRateRule::new(earning_types, FixedMinWage(0.0));

        rule.execute(&dataset, &mut earning, &RuleParams::new());

        // EarningFactorRateRuleConfig's own defaults: rateFactor 1.0, no
        // selected earnings, so the plain job rate passes through unscaled.
        assert_eq!(earning.rate(), 10.0);
    }

    #[test]
    fn uses_the_override_rate_when_the_job_underpays_minimum_wage() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 5.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));
        let mut rule = EarningOverrideJobRateRule::new(earning_types, FixedMinWage(7.25));
        let params = rule_params! {
            OVERRIDE_RATE => "9.99",
            USE_MIN_WAGE => "false",
            RATE_FACTOR => "1.0"
        };

        rule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 9.99);
    }

    use crate::common::enums::uom::UOM;

    mod java_parity_tests {
        use super::*;

        fn setup() -> (Employee, EmployeeEarning) {
            let today = LocalDate::of(2020, 6, 1);
            let employee = Employee::new(
                100,
                1,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    100,
                    1,
                    LocalDate::of(2019, 6, 1),
                    LocalDate::of(2021, 6, 1),
                    EmployeePayType::Hourly,
                    10.0,
                    false,
                )],
            );
            let earning = EmployeeEarning::new(1, 100, 1, 7, today, 4.0, 0.0, EarningSource::Rule);
            (employee, earning)
        }

        /// `EarningOverrideJobRateRuleImplTest`: "No change to earning rate
        /// if no earning types are selected".
        #[test]
        fn no_change_to_earning_rate_if_no_earning_types_are_selected() {
            let (employee, mut earning) = setup();
            let dataset = TimeCardData::new().with_employee(employee);
            let earning_types = EarningTypes(HashMap::from([(
                7,
                EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
            )]));
            let mut rule = EarningOverrideJobRateRule::new(earning_types, FixedMinWage(0.0));

            rule.execute(&dataset, &mut earning, &RuleParams::new());

            assert_eq!(earning.rate(), 10.0);
        }

        /// `EarningOverrideJobRateRuleImplTest`: "No change to the earning
        /// rate if the employee makes minimum wage".
        #[test]
        fn no_change_to_the_earning_rate_if_the_employee_makes_minimum_wage() {
            let today = LocalDate::of(2020, 6, 1);
            let minimum_wage = 10.0; // Groovy spec declares `static int minimumWage = 10.55`, truncated to 10 by its int type
            let employee = Employee::new(
                100,
                1,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    100,
                    1,
                    LocalDate::of(2019, 6, 1),
                    LocalDate::of(2021, 6, 1),
                    EmployeePayType::Hourly,
                    minimum_wage,
                    false,
                )],
            );
            let dataset = TimeCardData::new().with_employee(employee);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 7, today, 4.0, 0.0, EarningSource::Rule);
            let earning_types = EarningTypes(HashMap::from([(
                7,
                EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                    .with_flags(false, false, false, false),
            )]));
            let mut rule =
                EarningOverrideJobRateRule::new(earning_types, FixedMinWage(minimum_wage));
            let params = rule_params! {
                OVERRIDE_RATE => "9.99",
                USE_MIN_WAGE => "true",
                RATE_FACTOR => "2"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), minimum_wage);
        }

        /// `EarningOverrideJobRateRuleImplTest`: "will pay the correct rate
        /// if useMinWage parameter is false".
        #[test]
        fn will_pay_the_correct_rate_if_use_min_wage_parameter_is_false() {
            let today = LocalDate::of(2020, 6, 1);
            let minimum_wage = 10.0; // Groovy spec declares `static int minimumWage = 10.55`, truncated to 10 by its int type

            for (pay_min_wage, expected_rate) in [(true, minimum_wage), (false, 9.99)] {
                let employee = Employee::new(
                    100,
                    1,
                    "",
                    vec![EmployeeJobStatus::new(
                        1,
                        100,
                        1,
                        LocalDate::of(2019, 6, 1),
                        LocalDate::of(2021, 6, 1),
                        EmployeePayType::Hourly,
                        9.0,
                        false,
                    )],
                );
                let dataset = TimeCardData::new().with_employee(employee);
                let mut earning =
                    EmployeeEarning::new(1, 100, 1, 7, today, 4.0, 0.0, EarningSource::Rule);
                let earning_types =
                    EarningTypes(HashMap::from([(
                        7,
                        EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                            .with_flags(pay_min_wage, false, false, false),
                    )]));
                let mut rule =
                    EarningOverrideJobRateRule::new(earning_types, FixedMinWage(minimum_wage));
                let params = rule_params! {
                    OVERRIDE_RATE => "9.99",
                    USE_MIN_WAGE => "false"
                };

                rule.execute(&dataset, &mut earning, &params);

                assert_eq!(earning.rate(), expected_rate, "payMinWage={pay_min_wage}");
            }
        }

        /// `EarningOverrideJobRateRuleImplTest`: "will pay the correct rate
        /// if useMinWage parameter is true".
        #[test]
        fn will_pay_the_correct_rate_if_use_min_wage_parameter_is_true() {
            let minimum_wage = 10.0; // Groovy spec declares `static int minimumWage = 10.55`, truncated to 10 by its int type
            let (employee, mut earning) = setup();
            let dataset = TimeCardData::new().with_employee(employee);
            let earning_types = EarningTypes(HashMap::from([(
                7,
                EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                    .with_flags(false, false, false, false),
            )]));
            let mut rule =
                EarningOverrideJobRateRule::new(earning_types, FixedMinWage(minimum_wage));
            let params = rule_params! {
                OVERRIDE_RATE => "9.99",
                USE_MIN_WAGE => "true"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), minimum_wage);
        }

        /// `EarningOverrideJobRateRuleImplTest`: "earnings with employees who
        /// have a rate greater than the minimum wage are not affected".
        #[test]
        fn earnings_with_employees_who_have_a_rate_greater_than_the_minimum_wage_are_not_affected()
        {
            let today = LocalDate::of(2020, 6, 1);
            let employee = Employee::new(
                100,
                1,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    100,
                    1,
                    LocalDate::of(2019, 6, 1),
                    LocalDate::of(2021, 6, 1),
                    EmployeePayType::Hourly,
                    30.0,
                    false,
                )],
            );
            let dataset = TimeCardData::new().with_employee(employee);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 7, today, 4.0, 0.0, EarningSource::Rule);
            let earning_types = EarningTypes(HashMap::from([(
                7,
                EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                    .with_flags(true, false, false, false),
            )]));
            let mut rule = EarningOverrideJobRateRule::new(earning_types, FixedMinWage(0.0));
            let params = rule_params! {
                OVERRIDE_RATE => "9.99",
                USE_MIN_WAGE => "false"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), 30.0);
        }

        /// `EarningOverrideJobRateRuleImplTest`: "rates are multiplied by the
        /// factor when there are selected earnings".
        #[test]
        fn rates_are_multiplied_by_the_factor_when_there_are_selected_earnings() {
            let today = LocalDate::of(2020, 6, 1);
            let minimum_wage = 10.0; // Groovy spec declares `static int minimumWage = 10.55`, truncated to 10 by its int type
            let employee = Employee::new(
                100,
                1,
                "",
                vec![EmployeeJobStatus::new(
                    1,
                    100,
                    1,
                    LocalDate::of(2019, 6, 1),
                    LocalDate::of(2021, 6, 1),
                    EmployeePayType::Hourly,
                    9.0,
                    false,
                )],
            );
            let dataset = TimeCardData::new().with_employee(employee);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 7, today, 4.0, 0.0, EarningSource::Rule);
            let earning_types = EarningTypes(HashMap::from([(
                7,
                EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours)
                    .with_flags(true, false, false, false),
            )]));
            let mut rule =
                EarningOverrideJobRateRule::new(earning_types, FixedMinWage(minimum_wage));
            let params = rule_params! {
                OVERRIDE_RATE => "9.99",
                USE_MIN_WAGE => "false",
                RATE_FACTOR => "3"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), 30.0);
        }
    }
}
