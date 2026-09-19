//! Port of `HomeDeptRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/HomeDeptRateRuleImpl.java`.
//!
//! The same department-match-then-fallback shape as
//! [`HomeDeptRegRateRuleImpl`](crate::rules::algorithm::regularrate::home_dept_reg_rate),
//! but UOM-aware: the hourly path compares departments and falls back to the
//! job's own effective rate the same way, while the piece-rate path only ever
//! reads the home job status directly — no department comparison, no fallback,
//! and (faithfully) no null guard: an earning priced in units with no home job
//! status panics, the way Java's unguarded `jobStatus.getPieceRate()` NPEs.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::config::{HomeDeptRateRuleConfig, RATE_FACTOR};
use crate::rules::algorithm::earningrate::{EarningRateRule, set_earning_rate};
use crate::rules::params::RuleParams;
use crate::rules::ports::{AssignmentPort, EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `HomeDeptRateRuleImpl`.
pub struct HomeDeptRateRule<A: AssignmentPort, E: EarningTypePort, M: MinWagePort> {
    assignments: A,
    earning_types: E,
    min_wage: M,
}

impl<A: AssignmentPort, E: EarningTypePort, M: MinWagePort> HomeDeptRateRule<A, E, M> {
    /// Build the rule over the ports its job, earning type and
    /// minimum-wage-floor lookups come from.
    pub fn new(assignments: A, earning_types: E, min_wage: M) -> Self {
        Self {
            assignments,
            earning_types,
            min_wage,
        }
    }

    /// `getRate(LocalDate, Assignment, Employee)`.
    fn rate(
        &self,
        effective_date: LocalDate,
        job_id: i32,
        employee: &crate::entity::employee::Employee,
    ) -> f64 {
        let Some(job) = self.assignments.find_by_id(job_id) else {
            return 0.0;
        };
        let home_status = employee.home_employee_job_status(effective_date);
        let home_shares_department = home_status.is_some_and(|status| {
            self.assignments
                .find_by_id(status.job_id())
                .is_some_and(|home_job| {
                    home_job.parent_assignment_id() == job.parent_assignment_id()
                })
        });

        if let Some(home_status) = home_status.filter(|_| home_shares_department) {
            home_status.hourly_rate()
        } else {
            job.effective_hourly_pay_rate(effective_date, &self.assignments)
        }
    }
}

impl<A: AssignmentPort, E: EarningTypePort, M: MinWagePort> EarningRateRule
    for HomeDeptRateRule<A, E, M>
{
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&HomeDeptRateRuleConfig.default_values());
        let rate_factor = params.double_at(RATE_FACTOR);

        let employee = dataset.employee().expect("no employee on the dataset");
        let earning_type = self
            .earning_types
            .find_by_id(earning.earning_type_id())
            .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));

        let hourly_rate = match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars => {
                self.rate(earning.earning_date(), earning.job_id(), employee)
            }
            UOM::Units => employee
                .home_employee_job_status(earning.earning_date())
                .unwrap_or_else(|| panic!("no home job status for earning {}", earning.id()))
                .piece_rate(),
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

        set_earning_rate(earning, hourly_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_pay_rate::AssignmentPayRate;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use std::collections::HashMap;

    struct Assignments(HashMap<i32, Assignment>);
    impl AssignmentPort for Assignments {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            self.0.get(&id).cloned()
        }
    }

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
    fn the_home_job_rate_wins_when_it_shares_the_department() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let parent = Assignment::new(2, 1, "Department", "DEPT", None);
        let job = Assignment::new(1, 1, "Front Desk", "FD", Some(2))
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let assignments = Assignments(HashMap::from([(1, job), (2, parent)]));
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]));

        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 7.5, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let mut rule = HomeDeptRateRule::new(assignments, earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        assert_eq!(earning.rate(), 7.5);
    }

    #[test]
    fn piece_rate_reads_only_the_home_job_status() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let assignments = Assignments(HashMap::new());
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(7, 1, "Piece", EarnType::Regular, UOM::Units),
        )]));

        let employee = Employee::new(
            100,
            1,
            "Alex Kim",
            vec![job_status(1, 7.5, true).with_salaried_fields(0.0, 3.25, None, None)],
        );
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 4.0, 0.0, EarningSource::Rule);
        let mut rule = HomeDeptRateRule::new(assignments, earning_types, FixedMinWage(0.0));

        rule.execute(
            &dataset,
            &mut earning,
            &rule_params! { RATE_FACTOR => "1.0" },
        );

        assert_eq!(earning.rate(), 3.25);
    }
}
