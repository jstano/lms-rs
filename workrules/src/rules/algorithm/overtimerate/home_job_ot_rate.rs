//! Port of `HomeJobOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/HomeJobOTRateRuleImpl.java`.
//!
//! The employee's home job-status rate on the effective date — `0.0` if they
//! have none that date — floored at minimum wage, times the configured
//! overtime factor. Same shape as `HomeJobDTRateRuleImpl` one family over.
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
    HomeJobOTRateRuleConfig, OVERTIME_FACTOR_PROP, earning_type_ids,
};
use crate::rules::algorithm::overtimerate::{
    OvertimeRateRule, add_overtime_rate, set_overtime_rates,
};
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `HomeJobOTRateRuleImpl`.
pub struct HomeJobOTRateRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> HomeJobOTRateRule<M> {
    /// Build the rule over the port its minimum-wage floor comes from.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    fn rate(
        &self,
        job_id: i32,
        effective_date: LocalDate,
        dataset: &dyn TimeCard,
        ot_factor: f64,
    ) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        let hourly_rate = employee
            .home_employee_job_status(effective_date)
            .map_or(0.0, |status| status.hourly_rate());

        let min_wage = self
            .min_wage
            .min_wage(employee.property_id(), Some(job_id), effective_date);
        let hourly_rate = hourly_rate.max(min_wage);

        round_currency(hourly_rate * ot_factor)
    }
}

impl<M: MinWagePort> OvertimeRateRule for HomeJobOTRateRule<M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&HomeJobOTRateRuleConfig.default_values());
        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);

        let rate = self.rate(shift.job_id(), distribution.date(), dataset, ot_factor);
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
            .fixed(&HomeJobOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let rate = self.rate(earning.job_id(), earning.earning_date(), dataset, ot_factor);
        add_overtime_rate(earning, rate);
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
        RuleItem::new(id, 1, "Home job OT rate", RuleClass::HomeJobOrr, params)
    }

    #[test]
    fn the_home_job_rate_is_multiplied_by_the_ot_factor() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 10.0, true)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "0.5" });
        let rule = HomeJobOTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 5.0);
    }

    #[test]
    fn no_home_job_status_prices_from_zero_floored_at_minimum_wage() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "1.0" });
        let rule = HomeJobOTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 7.25);
    }

    #[test]
    fn a_premium_earning_type_gets_the_ot_rate_added_on_top() {
        let earning_date = LocalDate::of(2016, 6, 1);
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
        let rule = HomeJobOTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 8.0);
    }
}
