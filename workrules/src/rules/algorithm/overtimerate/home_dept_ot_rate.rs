//! Port of `HomeDeptOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/HomeDeptOTRateRuleImpl.java`.
//!
//! Home job's rate if the shift's job shares the home job's parent
//! department, else the job's own effective rate — floored at minimum wage,
//! times the configured overtime factor. Same department-match shape as
//! `HomeDeptRegRateRuleImpl`/`HomeDeptDTRateRuleImpl`, with no `payGreater`
//! branch.
//!
//! No Groovy spec exists for this rule; the behaviour tests below are written
//! from the Java.

use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::overtimerate::config::{
    HomeDeptOTRateRuleConfig, OVERTIME_FACTOR_PROP, earning_type_ids,
};
use crate::rules::algorithm::overtimerate::{
    OvertimeRateRule, add_overtime_rate, set_overtime_rates,
};
use crate::rules::ports::{AssignmentPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `HomeDeptOTRateRuleImpl`.
pub struct HomeDeptOTRateRule<A: AssignmentPort, M: MinWagePort> {
    assignments: A,
    min_wage: M,
}

impl<A: AssignmentPort, M: MinWagePort> HomeDeptOTRateRule<A, M> {
    /// Build the rule over the ports its job lookup and minimum-wage floor
    /// come from.
    pub fn new(assignments: A, min_wage: M) -> Self {
        Self {
            assignments,
            min_wage,
        }
    }

    fn rate(
        &self,
        effective_date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        ot_factor: f64,
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

        let hourly_rate = if let Some(home_status) = home_status.filter(|_| home_shares_department)
        {
            home_status.hourly_rate()
        } else {
            job.effective_hourly_pay_rate(effective_date, &self.assignments)
        };

        let min_wage = self
            .min_wage
            .min_wage(employee.property_id(), Some(job_id), effective_date);
        let hourly_rate = hourly_rate.max(min_wage);

        round_currency(hourly_rate * ot_factor)
    }
}

impl<A: AssignmentPort, M: MinWagePort> OvertimeRateRule for HomeDeptOTRateRule<A, M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&HomeDeptOTRateRuleConfig.default_values());
        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);

        let rate = self.rate(distribution.date(), shift.job_id(), dataset, ot_factor);
        set_overtime_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&HomeDeptOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let rate = self.rate(earning.earning_date(), earning.job_id(), dataset, ot_factor);
        add_overtime_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
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

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "Home dept OT rate", RuleClass::HomeDeptOrr, params)
    }

    #[test]
    fn the_home_job_rate_wins_when_it_shares_the_department() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let parent = Assignment::new(2, 1, "Department", "DEPT", None);
        let job = Assignment::new(1, 1, "Front Desk", "FD", Some(2))
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let assignments = Assignments(HashMap::from([(1, job), (2, parent)]));

        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "0.5" });
        let rule = HomeDeptOTRateRule::new(assignments, FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 5.0);
    }

    #[test]
    fn a_different_department_falls_back_to_the_jobs_own_effective_rate() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let job = Assignment::new(1, 1, "Front Desk", "FD", None)
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let assignments = Assignments(HashMap::from([(1, job)]));

        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, false)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "1.0" });
        let rule = HomeDeptOTRateRule::new(assignments, FixedMinWage(5.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 6.5);
    }

    #[test]
    fn a_premium_earning_type_gets_the_ot_rate_added_on_top() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let parent = Assignment::new(2, 1, "Department", "DEPT", None);
        let job = Assignment::new(1, 1, "Front Desk", "FD", Some(2))
            .with_pay_rates(vec![AssignmentPayRate::new(LocalDate::of(1999, 1, 1), 6.5)]);
        let assignments = Assignments(HashMap::from([(1, job), (2, parent)]));

        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 5, earning_date, 4.0, 3.0, EarningSource::Rule);
        let item = rule_item(
            1,
            rule_params! {
                OVERTIME_FACTOR_PROP => "0.5",
                crate::rules::algorithm::overtimerate::config::PREMIUM_TYPES => "[5]"
            },
        );
        let rule = HomeDeptOTRateRule::new(assignments, FixedMinWage(7.25));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 8.0);
    }
}
