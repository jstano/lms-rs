//! Port of `JobDTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/doubletimerate/JobDTRateRuleImpl.java`.
//!
//! Job-status rate, floored at minimum wage, times the configured double-time
//! factor.
//!
//! # No guard against a missing job status
//!
//! Unlike `JobOTRateRuleImpl` (its `overtimerate` counterpart, which throws a
//! named `RuntimeException`), this rule does not guard the lookup at all —
//! `jobStatus.getHourlyRate()` on a null status is an unguarded NPE in Java on
//! *both* overloads, reproduced here as a panic on `.unwrap()`. See the
//! family-wide finding in `PARITY_AUDIT.md` about three different
//! missing-job-status behaviours across `regularrate`/`doubletimerate`.
//!
//! No Groovy spec exists for this rule; the behaviour tests below are written
//! from the Java.

use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::doubletimerate::config::{
    DOUBLETIME_FACTOR_PROP, JobDTRateRuleConfig, earning_type_ids,
};
use crate::rules::algorithm::doubletimerate::{
    DoubleTimeRateRule, add_double_time_rate, set_double_time_rates,
};
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `JobDTRateRuleImpl`.
pub struct JobDTRateRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> JobDTRateRule<M> {
    /// Build the rule over the port its minimum-wage floor comes from.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    /// `getRate(LocalDate, Assignment, TimeCard, Map)`.
    fn rate(
        &self,
        effective_date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        dt_factor: f64,
    ) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        let hourly_rate = employee
            .employee_job_status(job_id, effective_date)
            .unwrap_or_else(|| panic!("no job status for job {job_id} on {effective_date}"))
            .hourly_rate();

        let min_wage = self
            .min_wage
            .min_wage(employee.property_id(), Some(job_id), effective_date);
        let hourly_rate = hourly_rate.max(min_wage);

        round_currency(hourly_rate * dt_factor)
    }
}

impl<M: MinWagePort> DoubleTimeRateRule for JobDTRateRule<M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&JobDTRateRuleConfig.default_values());
        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);

        let rate = self.rate(distribution.date(), shift.job_id(), dataset, dt_factor);
        set_double_time_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&JobDTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);
        let rate = self.rate(earning.earning_date(), earning.job_id(), dataset, dt_factor);
        add_double_time_rate(earning, rate);
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
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

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

    fn job_status(hourly_rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            false,
        )
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "Job DT rate", RuleClass::JobDrr, params)
    }

    #[test]
    fn the_job_rate_is_multiplied_by_the_dt_factor() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(10.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { DOUBLETIME_FACTOR_PROP => "2.0" });
        let rule = JobDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 20.0);
        assert_eq!(distribution.rate_rule_item_id(), Some(1));
    }

    #[test]
    fn the_rate_is_floored_at_minimum_wage() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(5.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { DOUBLETIME_FACTOR_PROP => "2.0" });
        let rule = JobDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 14.5);
    }

    #[test]
    #[should_panic]
    fn a_missing_job_status_panics() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let rule = JobDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(
            &shift,
            &mut distribution,
            &dataset,
            &rule_item(1, RuleParams::new()),
        );
    }

    #[test]
    fn a_non_premium_earning_type_is_left_alone() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(10.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 5, earning_date, 4.0, 3.0, EarningSource::Rule);
        let item = rule_item(
            1,
            rule_params! { crate::rules::algorithm::doubletimerate::config::PREMIUM_TYPES => "[99]" },
        );
        let rule = JobDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 3.0);
    }

    #[test]
    fn a_premium_earning_type_gets_the_dt_rate_added_on_top() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(10.0)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 5, earning_date, 4.0, 3.0, EarningSource::Rule);
        let item = rule_item(
            1,
            rule_params! {
                DOUBLETIME_FACTOR_PROP => "1.0",
                crate::rules::algorithm::doubletimerate::config::PREMIUM_TYPES => "[5]"
            },
        );
        let rule = JobDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 13.0);
        assert_eq!(earning.dollars(), 0.0);
    }
}
