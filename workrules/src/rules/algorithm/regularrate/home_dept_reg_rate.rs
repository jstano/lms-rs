//! Port of `HomeDeptRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/HomeDeptRegRateRuleImpl.java`.
//!
//! Home job's rate **if** the shift's job shares the home job's parent
//! department; otherwise the job's own effective rate (from
//! [`Assignment::effective_hourly_pay_rate`](crate::entity::assignment::Assignment::effective_hourly_pay_rate),
//! not the job status), optionally raised to the employee's own job-status
//! rate if `payGreater` is set.
//!
//! # Reaching the parent assignment
//!
//! Java compares `Assignment` references directly:
//! `currentDepartment == homeEmployeeJobStatus.getJob().getParentAssignment()`.
//! This crate's entities carry only ids one-way, so the comparison is on
//! parent-assignment **id** instead — the same outcome, since Java's reference
//! equality here is really "the same department row".
//!
//! Ported cases: `HomeDeptRegRateRuleImplTest.groovy` (two cases).

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::config::{HomeDeptRegRateRuleConfig, PAY_GREATER_RATE};
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use crate::rules::ports::AssignmentPort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `HomeDeptRegRateRuleImpl`.
pub struct HomeDeptRegRateRule<A: AssignmentPort> {
    assignments: A,
}

impl<A: AssignmentPort> HomeDeptRegRateRule<A> {
    /// Build the rule over the port its job lookups come from.
    pub fn new(assignments: A) -> Self {
        Self { assignments }
    }

    /// `getRate(LocalDate, Assignment, TimeCard, Map)`.
    fn rate(
        &self,
        effective_date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        pay_greater: bool,
    ) -> f64 {
        let Some(employee) = dataset.employee() else {
            return 0.0;
        };
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
            return home_status.hourly_rate();
        }

        let default_rate = job.effective_hourly_pay_rate(effective_date, &self.assignments);
        if pay_greater {
            let job_status_rate = employee
                .employee_job_status(job_id, effective_date)
                .map_or(0.0, |status| status.hourly_rate());
            default_rate.max(job_status_rate)
        } else {
            default_rate
        }
    }
}

impl<A: AssignmentPort> RegularRateRule for HomeDeptRegRateRule<A> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&HomeDeptRegRateRuleConfig.default_values());
        let pay_greater = params.bool_at(PAY_GREATER_RATE);

        let rate = self.rate(distribution.date(), shift.job_id(), dataset, pay_greater);
        set_regular_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&HomeDeptRegRateRuleConfig.default_values());
        let pay_greater = params.bool_at(PAY_GREATER_RATE);

        let rate = self.rate(
            earning.earning_date(),
            earning.job_id(),
            dataset,
            pay_greater,
        );
        set_earning_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_pay_rate::AssignmentPayRate;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use std::collections::HashMap;

    struct Assignments(HashMap<i32, Assignment>);

    impl AssignmentPort for Assignments {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            self.0.get(&id).cloned()
        }
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "Home dept rate", RuleClass::HomeDeptRrr, params)
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
        let shift_date = LocalDate::of(2016, 6, 1);
        let parent = Assignment::new(2, 1, "Department", "DEPT", None);
        let job = Assignment::new(1, 1, "Front Desk", "FD", Some(2))
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let assignments = Assignments(HashMap::from([(1, job), (2, parent)]));

        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 7.5, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
            .with_net_hours(8.0);
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let rule = HomeDeptRegRateRule::new(assignments);

        rule.execute_for_shift(
            &shift,
            &mut distribution,
            &dataset,
            &rule_item(1, RuleParams::new()),
        );

        assert_eq!(distribution.base_rate(), 7.5);
        assert_eq!(distribution.rate_rule_item_id(), Some(1));
    }

    mod java_parity_tests {
        use super::*;

        /// `HomeDeptRegRateRuleImplTest`: "rates should be set to home job
        /// rate".
        #[test]
        fn rates_should_be_set_to_home_job_rate() {
            let shift_date = LocalDate::of(2016, 6, 1);
            let parent = Assignment::new(2, 1, "Department", "DEPT", None);
            let job = Assignment::new(1, 1, "Front Desk", "FD", Some(2))
                .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
            let assignments = Assignments(HashMap::from([(1, job), (2, parent)]));

            let employee = Employee::new(100, 1, "", vec![job_status(1, 7.5, true)]);
            let dataset = TimeCardData::new().with_employee(employee);
            let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                .with_net_hours(8.0);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
            let rule = HomeDeptRegRateRule::new(assignments);

            rule.execute_for_shift(
                &shift,
                &mut distribution,
                &dataset,
                &rule_item(1, RuleParams::new()),
            );

            assert_eq!(distribution.base_rate(), 7.5);
            assert_eq!(distribution.rate_rule_item_id(), Some(1));
        }

        /// `HomeDeptRegRateRuleImplTest`: "rates should default to the
        /// assignment rate if the pay greater rate flag isnt checked".
        #[test]
        fn rates_should_default_to_the_assignment_rate_if_the_pay_greater_rate_flag_isnt_checked() {
            let shift_date = LocalDate::of(2016, 6, 1);

            for (pay_greater_rate, expected_rate) in [("true", 7.5), ("false", 6.5)] {
                let job = Assignment::new(1, 1, "Front Desk", "FD", None)
                    .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
                let assignments = Assignments(HashMap::from([(1, job)]));

                let employee = Employee::new(100, 1, "", vec![job_status(1, 7.5, false)]);
                let dataset = TimeCardData::new().with_employee(employee);
                let shift =
                    EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                        .with_net_hours(8.0);
                let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
                let rule = HomeDeptRegRateRule::new(assignments);
                let item = rule_item(1, rule_params! { PAY_GREATER_RATE => pay_greater_rate });

                rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

                assert_eq!(
                    distribution.base_rate(),
                    expected_rate,
                    "payGreaterRate={pay_greater_rate}"
                );
            }
        }
    }
}
