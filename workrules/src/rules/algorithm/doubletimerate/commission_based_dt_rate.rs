//! Port of `CommissionBasedDTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/doubletimerate/CommissionBasedDTRateRuleImpl.java`.
//!
//! Same-date, same-job earnings-of-configured-types divided by same-date
//! same-job worked hours, floored at a configured minimum commission rate
//! *and* at minimum wage, times the double-time factor. Structurally
//! identical arithmetic to `CommissionBasedOTRateRuleImpl` (its `overtimerate`
//! sibling, not yet ported), own config keys (`DOUBLETIME_FACTOR_PROP`
//! instead of `OVERTIME_FACTOR_PROP`).
//!
//! # `EARNING_TYPES_PROP` is declared but never read
//!
//! See the module doc on `config.rs`: both the earning-total computation and
//! the earning-path gate read the inherited `premiumTypes` parameter, not the
//! rule's own `earningTypes`. One list, `earning_type_ids`, does both jobs
//! here, matching the Java exactly.
//!
//! No Groovy spec exists for this rule; the behaviour tests below are written
//! from the Java.

use crate::common::numbers::{round_currency, round_to};
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::doubletimerate::config::{
    CommissionBasedDTRateRuleConfig, DOUBLETIME_FACTOR_PROP, MIN_COMM_RATE, earning_type_ids,
};
use crate::rules::algorithm::doubletimerate::{
    DoubleTimeRateRule, add_double_time_rate, set_double_time_rates,
};
use crate::rules::ports::MinWagePort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// `CommissionBasedDTRateRuleImpl`.
pub struct CommissionBasedDTRateRule<M: MinWagePort> {
    min_wage: M,
}

impl<M: MinWagePort> CommissionBasedDTRateRule<M> {
    /// Build the rule over the port its minimum-wage floor comes from.
    pub fn new(min_wage: M) -> Self {
        Self { min_wage }
    }

    /// `getTotalHours(LocalDate, Assignment, TimeCard)`.
    fn total_hours(&self, date: LocalDate, job_id: i32, dataset: &dyn TimeCard) -> f64 {
        dataset
            .shifts()
            .iter()
            .filter(|shift| shift.shift_date() == date && shift.job_id() == job_id)
            .fold(0.0, |total, shift| round_to(total + shift.net_hours(), 2))
    }

    /// `getTotalEarnings(LocalDate, Assignment, TimeCard, List)`.
    fn total_earnings(
        &self,
        date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        earning_type_ids: &[i32],
    ) -> f64 {
        dataset
            .earnings()
            .iter()
            .filter(|earning| {
                earning.earning_date() == date
                    && earning.job_id() == job_id
                    && earning_type_ids.contains(&earning.earning_type_id())
            })
            .fold(0.0, |total, earning| {
                round_to(total + earning.total_dollars(), 2)
            })
    }

    /// The commission rate common to both overloads' `execute`: earnings over
    /// hours, floored at the configured minimum commission and then at
    /// minimum wage.
    fn commission_rate(
        &self,
        date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        earning_type_ids: &[i32],
        min_comm: f64,
    ) -> f64 {
        let total_hours = self.total_hours(date, job_id, dataset);
        let total_earnings = self.total_earnings(date, job_id, dataset, earning_type_ids);

        let commission_rate = if total_hours != 0.0 {
            total_earnings / total_hours
        } else {
            0.0
        };
        let commission_rate = commission_rate.max(min_comm);

        let property_id = dataset
            .employee()
            .map_or(0, |employee| employee.property_id());
        let min_wage = self.min_wage.min_wage(property_id, Some(job_id), date);
        commission_rate.max(min_wage)
    }
}

impl<M: MinWagePort> DoubleTimeRateRule for CommissionBasedDTRateRule<M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&CommissionBasedDTRateRuleConfig.default_values());
        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);
        let min_comm = params.double_at(MIN_COMM_RATE);
        let earning_type_ids = earning_type_ids(&params);

        let commission_rate = self.commission_rate(
            shift.shift_date(),
            shift.job_id(),
            dataset,
            &earning_type_ids,
            min_comm,
        );

        let rate = round_currency(commission_rate * dt_factor);
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
            .fixed(&CommissionBasedDTRateRuleConfig.default_values());
        let earning_type_ids = earning_type_ids(&params);

        if !earning_type_ids.contains(&earning.earning_type_id()) {
            return;
        }

        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);
        let min_comm = params.double_at(MIN_COMM_RATE);

        let commission_rate = self.commission_rate(
            earning.earning_date(),
            earning.job_id(),
            dataset,
            &earning_type_ids,
            min_comm,
        );

        add_double_time_rate(earning, commission_rate * dt_factor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::doubletimerate::config::PREMIUM_TYPES;
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

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(
            id,
            1,
            "Commission based DT rate",
            RuleClass::ComBasedDrr,
            params,
        )
    }

    #[test]
    fn the_commission_rate_is_earnings_over_hours_times_the_factor() {
        let date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let shift =
            EmployeeShift::new(1, 100, 1, date, ShiftType::Actual, Vec::new()).with_net_hours(8.0);
        let mut commission =
            EmployeeEarning::new(2, 100, 1, 22, date, 0.0, 0.0, EarningSource::Rule);
        commission.set_total_dollars(80.0);
        let dataset = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift.clone()])
            .with_earnings(vec![commission]);
        let mut distribution = HoursDistribution::new(1, date, None, 0.0, 0.0);
        let item = rule_item(
            1,
            rule_params! { DOUBLETIME_FACTOR_PROP => "2.0", PREMIUM_TYPES => "[22]" },
        );
        let rule = CommissionBasedDTRateRule::new(FixedMinWage(0.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        // 80.0 / 8.0 = 10.0, * 2.0 factor = 20.0
        assert_eq!(distribution.premium_rate(), 20.0);
    }

    #[test]
    fn zero_hours_prices_from_zero_before_the_floors_apply() {
        let date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let shift = EmployeeShift::new(1, 100, 1, date, ShiftType::Actual, Vec::new());
        let dataset = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift.clone()]);
        let mut distribution = HoursDistribution::new(1, date, None, 0.0, 0.0);
        let item = rule_item(
            1,
            rule_params! { DOUBLETIME_FACTOR_PROP => "1.0", MIN_COMM_RATE => "5.0" },
        );
        let rule = CommissionBasedDTRateRule::new(FixedMinWage(7.25));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        // no hours -> 0.0 commission, floored at minComm 5.0, then at min wage 7.25
        assert_eq!(distribution.premium_rate(), 7.25);
    }

    #[test]
    fn the_minimum_commission_rate_floors_a_low_commission() {
        let date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let shift =
            EmployeeShift::new(1, 100, 1, date, ShiftType::Actual, Vec::new()).with_net_hours(8.0);
        let mut commission =
            EmployeeEarning::new(2, 100, 1, 22, date, 0.0, 0.0, EarningSource::Rule);
        commission.set_total_dollars(8.0);
        let dataset = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift.clone()])
            .with_earnings(vec![commission]);
        let mut distribution = HoursDistribution::new(1, date, None, 0.0, 0.0);
        let item = rule_item(
            1,
            rule_params! {
                DOUBLETIME_FACTOR_PROP => "1.0",
                MIN_COMM_RATE => "5.0",
                PREMIUM_TYPES => "[22]"
            },
        );
        let rule = CommissionBasedDTRateRule::new(FixedMinWage(0.0));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        // 8.0/8.0 = 1.0, floored at minComm 5.0
        assert_eq!(distribution.premium_rate(), 5.0);
    }

    #[test]
    fn a_non_premium_earning_type_is_left_alone() {
        let date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning = EmployeeEarning::new(1, 100, 1, 5, date, 4.0, 3.0, EarningSource::Rule);
        let item = rule_item(1, rule_params! { PREMIUM_TYPES => "[99]" });
        let rule = CommissionBasedDTRateRule::new(FixedMinWage(0.0));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 3.0);
    }
}
