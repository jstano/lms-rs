//! Port of `MissingSalaryDistRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/reconcileemployee/MissingSalaryDistRuleImpl.java`.
//!
//! Gives every salaried-exempt job status that lacks a salary distribution one,
//! and re-derives its rates.
//!
//! - Statuses are first **sorted in place** by job id, then start date.
//! - For each such status, its distribution is the **prior status of the same job**
//!   (the one in effect the day before it starts) when that is salaried-exempt and
//!   has one — so a raise or reassignment keeps its pattern — otherwise the
//!   configured `salaryDistID`.
//! - When a **home** status was just given the configured default (not an inherited
//!   one), every *other* salaried-exempt status active on that status's start date
//!   is given the `secondarySalaryDistID` pattern (the default again when that one
//!   cannot be found), overwriting whatever it had.
//!
//! "Given the configured default" is an identity test in Java; the port compares
//! distribution ids, which are the same thing for a loaded entity.

use crate::entity::employee::Employee;
use crate::entity::salary_dist::SalaryDist;
use crate::rules::algorithm::reconcileemployee::ReconcileEmployeeRule;
use crate::rules::algorithm::reconcileemployee::config::{
    MissingSalaryDistRuleConfig, SALARY_DIST_ID, SECONDARY_SALARY_DIST_ID,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::SalaryDistPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `MissingSalaryDistRuleImpl`.
pub struct MissingSalaryDistRule<S: SalaryDistPort> {
    salary_dists: S,
}

impl<S: SalaryDistPort> MissingSalaryDistRule<S> {
    /// Build the rule over the salary-distribution lookup.
    pub fn new(salary_dists: S) -> Self {
        Self { salary_dists }
    }
}

impl<S: SalaryDistPort> ReconcileEmployeeRule for MissingSalaryDistRule<S> {
    fn execute(&self, employee: &mut Employee, params: &RuleParams, _today: LocalDate) {
        let params = params.fixed(&MissingSalaryDistRuleConfig.default_values());
        let default = self.salary_dists.find_by_id(params.int_at(SALARY_DIST_ID));
        let secondary = self
            .salary_dists
            .find_by_id(params.int_at(SECONDARY_SALARY_DIST_ID))
            .or_else(|| default.clone());

        employee
            .employee_job_statuses_mut()
            .sort_by_key(|s| (s.job_id(), s.start_date()));

        for index in 0..employee.employee_job_statuses().len() {
            let status = &employee.employee_job_statuses()[index];
            if !(status.pay_type().is_salaried_exempt() && status.salary_dist().is_none()) {
                continue;
            }
            let (job_id, start) = (status.job_id(), status.start_date());
            let prior_dist: Option<SalaryDist> = employee
                .effective_job_status(start.minus_days(1), job_id)
                .filter(|p| p.pay_type().is_salaried_exempt())
                .and_then(|p| p.salary_dist().cloned());
            let effective = prior_dist.or_else(|| default.clone());

            let status = &mut employee.employee_job_statuses_mut()[index];
            status.set_salary_dist(effective);
            status.calculate_and_set_rates();

            let status = &employee.employee_job_statuses()[index];
            let given_default = status.home()
                && status.salary_dist().map(SalaryDist::id) == default.as_ref().map(SalaryDist::id);
            if given_default {
                let others: Vec<usize> = (0..employee.employee_job_statuses().len())
                    .filter(|&j| {
                        let other = &employee.employee_job_statuses()[j];
                        j != index
                            && other.contains_date(start)
                            && other.pay_type().is_salaried_exempt()
                    })
                    .collect();
                for j in others {
                    let other = &mut employee.employee_job_statuses_mut()[j];
                    other.set_salary_dist(secondary.clone());
                    other.calculate_and_set_rates();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::rule_params;

    struct Dists;
    impl SalaryDistPort for Dists {
        fn find_by_id(&self, id: i32) -> Option<SalaryDist> {
            (id > 0).then(|| SalaryDist::new(id, [0.0, 8.0, 8.0, 8.0, 8.0, 8.0, 0.0]))
        }
    }

    fn d(day: i64) -> LocalDate {
        LocalDate::of(2024, 6, 1).plus_days(day)
    }
    fn status(
        id: i32,
        job: i32,
        start: i64,
        end: i64,
        home: bool,
        dist: Option<i32>,
    ) -> EmployeeJobStatus {
        let mut s = EmployeeJobStatus::new(
            id,
            1,
            job,
            d(start),
            d(end),
            EmployeePayType::SalariedExempt,
            0.0,
            home,
        )
        .with_salaried_fields(104000.0, 0.0, None, None);
        s.set_salary_dist(dist.and_then(|i| Dists.find_by_id(i)));
        s
    }
    fn params(default: &str, secondary: &str) -> RuleParams {
        rule_params! { SALARY_DIST_ID => default, SECONDARY_SALARY_DIST_ID => secondary }
    }
    fn run(statuses: Vec<EmployeeJobStatus>, p: RuleParams) -> Vec<(i32, Option<i32>)> {
        let mut e = Employee::new(1, 1, "Pat", statuses);
        MissingSalaryDistRule::new(Dists).execute(&mut e, &p, d(0));
        e.employee_job_statuses()
            .iter()
            .map(|s| (s.id(), s.salary_dist_id()))
            .collect()
    }

    // ---- behaviour tests from the Java (there is no Groovy spec) ----

    #[test]
    fn a_status_missing_a_distribution_gets_the_default() {
        let out = run(vec![status(1, 10, 0, 100, true, None)], params("3", "4"));
        assert_eq!(out, vec![(1, Some(3))]);
    }

    #[test]
    fn a_status_inherits_the_prior_statuss_distribution_for_the_same_job() {
        let out = run(
            vec![
                status(1, 10, 0, 49, true, Some(9)),
                status(2, 10, 50, 100, true, None),
            ],
            params("3", "4"),
        );
        assert_eq!(out, vec![(1, Some(9)), (2, Some(9))]);
    }

    #[test]
    fn a_home_status_given_the_default_overwrites_the_secondaries_with_the_secondary_default() {
        let out = run(
            vec![
                status(1, 10, 0, 100, true, None),
                status(2, 20, 0, 100, false, Some(9)),
            ],
            params("3", "4"),
        );
        // Sorted by job id: the home status (job 10) first.
        assert_eq!(out, vec![(1, Some(3)), (2, Some(4))]);
    }

    #[test]
    fn an_inherited_distribution_does_not_trigger_the_secondary_overwrite() {
        let out = run(
            vec![
                status(1, 10, 0, 49, true, Some(9)),
                status(2, 10, 50, 100, true, None),
                status(3, 20, 50, 100, false, Some(8)),
            ],
            params("3", "4"),
        );
        assert_eq!(out, vec![(1, Some(9)), (2, Some(9)), (3, Some(8))]);
    }

    #[test]
    fn a_missing_secondary_falls_back_to_the_default() {
        let out = run(
            vec![
                status(1, 10, 0, 100, true, None),
                status(2, 20, 0, 100, false, Some(9)),
            ],
            params("3", "0"),
        );
        assert_eq!(out, vec![(1, Some(3)), (2, Some(3))]);
    }

    #[test]
    fn statuses_are_sorted_by_job_then_start_date_and_non_salaried_ones_are_ignored() {
        let mut hourly =
            EmployeeJobStatus::new(9, 1, 5, d(0), d(100), EmployeePayType::Hourly, 10.0, false);
        hourly.set_salary_dist(None);
        let out = run(
            vec![
                status(3, 20, 0, 100, false, None),
                hourly,
                status(1, 10, 0, 100, false, None),
            ],
            params("3", "4"),
        );
        assert_eq!(out, vec![(9, None), (1, Some(3)), (3, Some(3))]);
    }

    #[test]
    fn the_rates_are_rederived_from_the_new_distribution() {
        let mut e = Employee::new(1, 1, "Pat", vec![status(1, 10, 0, 100, false, None)]);
        // Annual-based with a 40 hour week: 104,000 / 52 / 40.
        e.employee_job_statuses_mut()[0].set_annual_based(true);
        MissingSalaryDistRule::new(Dists).execute(&mut e, &params("3", "4"), d(0));
        assert_eq!(e.employee_job_statuses()[0].hourly_rate(), 50.0);
    }
}
