//! Port of `FLSAEarningRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/FLSAEarningRateRuleImpl.java`.
//!
//! The `earningrate` counterpart of `FLSAOTRateRuleImpl`'s core formula —
//! `flsaData.getEffectiveRegularRate(applyMinWagePerShift) * factor`, keyed
//! by `Property.getCurrentWeek()`/`getPayPeriod()` gated on `PayPeriodType`,
//! the same `calculateOverWeeks` branch `FLSAOTRateRuleImpl` and
//! `CombinationJobsRegRateRuleImpl` already use. It implements
//! `AvgWageEarningRateRule`, the family's only user of that marker
//! sub-interface — folded away here, as the module doc explains.
//!
//! Unlike every other rule in this family, it calls `earning.setDollars(0)`
//! before recomputing the total, so [`set_earning_rate`](super::set_earning_rate)
//! is not used here; the write is spelled out to match.
//!
//! Ported cases: `FLSAEarningRateRuleImplTest.groovy` (two rows of one
//! `where:` table — the third row referenced in a comment in the scoping pass
//! does not exist in the checked-out spec; two is what the file has).

use crate::common::enums::pay_period_type::PayPeriodType;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::EarningRateRule;
use crate::rules::algorithm::earningrate::config::{
    APPLY_MIN_WAGE_PER_SHIFT, FLSAEarningRateRuleConfig, RATE_FACTOR, USE_PAY_PERIOD,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::PropertyPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `FLSAEarningRateRuleImpl`.
pub struct FlsaEarningRateRule<P: PropertyPort> {
    property: P,
}

impl<P: PropertyPort> FlsaEarningRateRule<P> {
    /// Build the rule over the port its FLSA week/pay-period comes from.
    pub fn new(property: P) -> Self {
        Self { property }
    }

    /// `getRate(LocalDate, TimeCard, Map)`.
    fn rate(&self, date: LocalDate, dataset: &dyn TimeCard, params: &RuleParams) -> f64 {
        let factor = params.double_at(RATE_FACTOR);
        let apply_min_wage_per_shift = params.bool_at(APPLY_MIN_WAGE_PER_SHIFT);
        let use_pay_period = params.bool_at(USE_PAY_PERIOD);

        let employee = dataset.employee().expect("no employee on the dataset");
        let calculate_over_weeks =
            !use_pay_period || dataset.pay_period_type() == PayPeriodType::Weekly;

        let flsa_rate = if calculate_over_weeks {
            let period_end_date = self.property.period_end_date(employee.property_id());
            let period_end_date = WeeklyDateRange::with_end_date(period_end_date)
                .range_containing_date(date)
                .end_date();
            dataset
                .flsa_data_map()
                .get(&period_end_date)
                .unwrap_or_else(|| panic!("no FLSA data for week ending {period_end_date}"))
                .effective_regular_rate(apply_min_wage_per_shift)
        } else {
            let period_end_date = dataset.pay_period_containing(date).end_date();
            dataset
                .flsa_pay_period_data_map()
                .get(&period_end_date)
                .unwrap_or_else(|| panic!("no FLSA data for pay period ending {period_end_date}"))
                .effective_regular_rate(apply_min_wage_per_shift)
        };

        round_currency(flsa_rate * factor)
    }
}

impl<P: PropertyPort> EarningRateRule for FlsaEarningRateRule<P> {
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&FLSAEarningRateRuleConfig.default_values());
        let rate = self.rate(earning.earning_date(), dataset, &params);

        earning.set_rate(rate);
        earning.set_dollars(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::schedule_mode::ScheduleMode;
    use crate::entity::employee::Employee;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use std::collections::HashMap;

    struct FixedPeriodEnd(LocalDate);
    impl PropertyPort for FixedPeriodEnd {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.0
        }
    }

    /// `effective_regular_rate(true)` reads `regular_rate_min_wage`, the last
    /// field — `applyMinWagePerShift` defaults to `true`.
    fn flsa_data(week_end_date: LocalDate, regular_rate_min_wage: f64) -> FlsaData {
        FlsaData::new(
            week_end_date,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            regular_rate_min_wage,
        )
    }

    #[test]
    fn the_weekly_flsa_rate_is_scaled_by_the_factor() {
        let today = LocalDate::of(2020, 6, 5);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let flsa_data_map = HashMap::from([(week_end, flsa_data(week_end, 10.0))]);
        let dataset = TimeCardData::new()
            .with_employee(employee)
            .with_flsa_data(flsa_data_map);
        let mut earning = EmployeeEarning::new(1, 100, 1, 7, today, 5.0, 8.0, EarningSource::Rule);
        let mut rule = FlsaEarningRateRule::new(FixedPeriodEnd(today));
        let params = rule_params! { RATE_FACTOR => "1.0" };

        rule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 10.0);
        assert_eq!(earning.dollars(), 0.0);
    }

    mod java_parity_tests {
        use super::*;

        /// `FLSAEarningRateRuleImplTest`: "earning rate should be get the
        /// correct rate when rule is run" — weekly row (`calcOverPayPeriod =
        /// false`, expected `10.0 * 1.0`).
        #[test]
        fn should_not_adjust_for_short_fall_rate_should_be_flsa_rate_times_factor() {
            let today = LocalDate::of(2020, 6, 5);
            let employee = Employee::new(100, 6, "", Vec::new());
            let week_end = WeeklyDateRange::with_end_date(today)
                .range_containing_date(today)
                .end_date();
            let flsa_data_map = HashMap::from([(week_end, flsa_data(week_end, 10.0))]);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_flsa_data(flsa_data_map)
                .with_contract_periods(PayPeriodType::Weekly, ScheduleMode::Weekly);
            let mut earning =
                EmployeeEarning::new(1, 100, 5, 22, today, 5.0, 8.0, EarningSource::Rule);
            let mut rule = FlsaEarningRateRule::new(FixedPeriodEnd(today));
            let params = rule_params! {
                USE_PAY_PERIOD => "false",
                RATE_FACTOR => "1.0"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), 10.0);
        }

        /// `FLSAEarningRateRuleImplTest`: "earning rate should be get the
        /// correct rate when rule is run" — pay-period row
        /// (`calcOverPayPeriod = true`, expected `15.0 * 1.0`).
        #[test]
        fn use_the_pay_period_flsa_data_rate_should_be_flsa_pay_period_rate_times_factor() {
            let today = LocalDate::of(2020, 6, 5);
            let employee = Employee::new(100, 6, "", Vec::new());
            let pay_period = date_range_rs::DateRange::new(
                LocalDate::of(2020, 6, 1),
                LocalDate::of(2020, 6, 30),
            );
            let flsa_pay_period_data = HashMap::from([(
                pay_period.end_date(),
                flsa_data(pay_period.end_date(), 15.0),
            )]);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_flsa_pay_period_data(flsa_pay_period_data)
                .with_current_pay_period(pay_period)
                .with_contract_periods(PayPeriodType::Monthly, ScheduleMode::Monthly);
            let mut earning =
                EmployeeEarning::new(1, 100, 5, 22, today, 5.0, 8.0, EarningSource::Rule);
            let mut rule = FlsaEarningRateRule::new(FixedPeriodEnd(today));
            let params = rule_params! {
                USE_PAY_PERIOD => "true",
                RATE_FACTOR => "1.0"
            };

            rule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), 15.0);
        }
    }
}
