//! Port of `ManipulateAnnualRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/reconcileemployee/ManipulateAnnualRateRuleImpl.java`.
//!
//! Splits the **home job's annual salary** across all the employee's current
//! salaried-exempt jobs in proportion to the weekly hours of their salary
//! distributions: each such job gets `its hours / total hours × home annual
//! salary`, is marked annual-based, and has its hourly rate re-derived.
//!
//! - "Current" means the status contains today; a status that has ended or has not
//!   started is left alone, as is one that is not salaried-exempt or has no
//!   distribution.
//! - The home annual rate is read **once, before any change**, from the first
//!   current home status — the home job itself is rewritten like the others, so a
//!   lone home job keeps its salary and with a secondary it keeps only its share.
//! - No current home status: nothing happens (Java: `NoSuchElementException`).
//! - Distributions with zero total hours divide `0 / 0`; the result is `NaN`, as in
//!   Java.

use crate::entity::employee::Employee;
use crate::rules::algorithm::reconcileemployee::ReconcileEmployeeRule;
use crate::rules::algorithm::reconcileemployee::config::ManipulateAnnualRateRuleConfig;
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `ManipulateAnnualRateRuleImpl`.
pub struct ManipulateAnnualRateRule;

impl ReconcileEmployeeRule for ManipulateAnnualRateRule {
    fn execute(&self, employee: &mut Employee, params: &RuleParams, today: LocalDate) {
        let _ = params.fixed(&ManipulateAnnualRateRuleConfig.default_values());

        let statuses = employee.employee_job_statuses_mut();
        let current: Vec<usize> = (0..statuses.len())
            .filter(|&i| statuses[i].contains_date(today))
            .collect();
        let salaried =
            |i: usize, statuses: &Vec<crate::entity::employee_job_status::EmployeeJobStatus>| {
                statuses[i].pay_type().is_salaried_exempt() && statuses[i].salary_dist().is_some()
            };

        let total_weekly_hours: f64 = current
            .iter()
            .filter(|&&i| salaried(i, statuses))
            .map(|&i| {
                statuses[i]
                    .salary_dist()
                    .map_or(0.0, |d| d.hours_per_week())
            })
            .sum();
        let Some(home_annual_rate) = current
            .iter()
            .find(|&&i| statuses[i].home())
            .map(|&i| statuses[i].annual_rate())
        else {
            return;
        };

        for &i in &current {
            if salaried(i, statuses) {
                let hours = statuses[i]
                    .salary_dist()
                    .map_or(0.0, |d| d.hours_per_week());
                let share = hours / total_weekly_hours;
                statuses[i].set_annual_rate(share * home_annual_rate);
                statuses[i].set_annual_based(true);
                statuses[i].calculate_and_set_rates();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::salary_dist::SalaryDist;

    fn today() -> LocalDate {
        LocalDate::of(2024, 6, 15)
    }
    fn dist(id: i32, per_day: f64) -> SalaryDist {
        SalaryDist::new(id, [0.0, per_day, per_day, per_day, per_day, per_day, 0.0])
    }
    fn status(
        id: i32,
        start: i64,
        end: i64,
        home: bool,
        annual: f64,
        dist: Option<SalaryDist>,
        pay_type: EmployeePayType,
    ) -> EmployeeJobStatus {
        let mut s = EmployeeJobStatus::new(
            id,
            1,
            id,
            today().plus_days(start),
            today().plus_days(end),
            pay_type,
            0.0,
            home,
        )
        .with_salaried_fields(annual, 0.0, None, None);
        s.set_salary_dist(dist);
        s
    }
    fn run(statuses: Vec<EmployeeJobStatus>) -> Vec<f64> {
        let mut employee = Employee::new(1, 1, "Pat", statuses);
        ManipulateAnnualRateRule.execute(&mut employee, &RuleParams::new(), today());
        employee
            .employee_job_statuses()
            .iter()
            .map(|s| s.annual_rate())
            .collect()
    }
    use EmployeePayType::{Hourly, SalariedExempt as Exempt};

    // ---- java_parity_tests: ManipulateAnnualRateRuleImplTest.groovy ----

    #[test]
    fn annual_rate_of_home_job_is_distributed_across_all_secondary_jobs_by_salary_distribution() {
        let rates = run(vec![
            status(1, -10, 2080, true, 100000.0, Some(dist(1, 4.0)), Exempt),
            status(2, -20, -1, false, 0.0, Some(dist(2, 3.0)), Exempt),
            status(3, -10, 2080, false, 0.0, Some(dist(2, 3.0)), Exempt),
            status(4, -10, 2080, false, 0.0, Some(dist(3, 1.0)), Exempt),
            status(5, -10, 2080, false, 0.0, None, Exempt),
        ]);
        assert_eq!(rates, vec![50000.0, 0.0, 37500.0, 12500.0, 0.0]);
    }

    #[test]
    fn annual_rate_is_not_distributed_if_the_secondary_jobs_are_invalid() {
        let rates = run(vec![
            status(1, -10, 2080, true, 100000.0, Some(dist(1, 4.0)), Exempt),
            status(2, -20, -1, false, 0.0, Some(dist(2, 3.0)), Exempt),
            status(3, -10, 2080, false, 0.0, None, Exempt),
            status(4, -10, 2080, false, 0.0, Some(dist(3, 1.0)), Hourly),
            status(5, -10, 2080, false, 0.0, None, Exempt),
        ]);
        assert_eq!(rates, vec![100000.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn annual_rate_of_home_job_does_not_change_with_no_secondary_jobs() {
        let rates = run(vec![status(
            1,
            -10,
            2080,
            true,
            100000.0,
            Some(dist(1, 8.0)),
            Exempt,
        )]);
        assert_eq!(rates, vec![100000.0]);
    }

    #[test]
    fn only_current_job_statuses_are_distributed_not_future_ones() {
        let rates = run(vec![
            status(1, -10, 20, true, 100000.0, Some(dist(1, 4.0)), Exempt),
            status(2, -10, 20, false, 0.0, Some(dist(2, 3.0)), Exempt),
            status(3, -10, 20, false, 0.0, Some(dist(3, 1.0)), Exempt),
            status(4, 21, 2080, true, 150000.0, Some(dist(1, 4.0)), Exempt),
            status(5, 21, 2080, false, 0.0, Some(dist(2, 3.0)), Exempt),
            status(6, 21, 2080, false, 0.0, Some(dist(3, 1.0)), Exempt),
        ]);
        assert_eq!(rates, vec![50000.0, 37500.0, 12500.0, 150000.0, 0.0, 0.0]);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn rewritten_statuses_become_annual_based_with_a_derived_hourly_rate() {
        let mut employee = Employee::new(
            1,
            1,
            "Pat",
            vec![status(
                1,
                -10,
                100,
                true,
                104000.0,
                Some(dist(1, 8.0)),
                Exempt,
            )],
        );
        ManipulateAnnualRateRule.execute(&mut employee, &RuleParams::new(), today());
        let s = &employee.employee_job_statuses()[0];
        assert!(s.annual_based());
        // 104,000 / 52 weeks / 40 hours.
        assert_eq!(s.hourly_rate(), 50.0);
    }

    #[test]
    fn with_no_current_home_status_nothing_changes() {
        let rates = run(vec![status(
            1,
            -10,
            100,
            false,
            0.0,
            Some(dist(1, 8.0)),
            Exempt,
        )]);
        assert_eq!(rates, vec![0.0]);
    }
}
