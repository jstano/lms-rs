//! Port of `FLSAOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/FLSAOTRateRuleImpl.java`.
//!
//! `flsaData.getEffectiveRegularRate(applyMinWagePerShift) * otFactor`, plus a
//! minimum-wage shortfall make-up added on top if `shift.getRegRate() <
//! minWage`. The only overtime rule that reads `shift.getRegRate()` — the
//! only one with a real dependency on `RegularRate` having run first, and the
//! only one keyed by `Property.getCurrentWeek()`/`getPayPeriod()` gated on
//! `PayPeriodType` rather than a fixed `Weeks(periodEndDate)`.
//!
//! # Two week-resolution branches
//!
//! `calculateOverWeeks = !usePayPeriod || payPeriodType == WEEKLY`: unless
//! the caller explicitly asked to calculate over the pay period *and* the
//! property is not weekly, this reaches the weekly FLSA map via
//! `Property.getCurrentWeek()` — `WeeklyDateRange::with_end_date(property
//! .period_end_date(id)).range_containing_date(date)`, the same mechanism
//! `CombinationJobsRegRateRuleImpl` and `FLSADTRateRuleImpl` use. Otherwise it
//! reaches the pay-period FLSA map via `TimeCard::pay_period_containing`,
//! which already stands in for `Property.getPayPeriod().getDateRangeContainingDate`
//! (divergence 24).
//!
//! Ported cases: `FLSAOTRateRuleImplTest.groovy` (two `where:` tables).

use crate::common::enums::pay_period_type::PayPeriodType;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::overtimerate::config::{
    ADJUST_RATE_FOR_SHORTFALL, APPLY_MIN_WAGE_PER_SHIFT, FLSAOTRateRuleConfig,
    OVERTIME_FACTOR_PROP, USE_PAY_PERIOD, earning_type_ids,
};
use crate::rules::algorithm::overtimerate::{
    OvertimeRateRule, add_overtime_rate, set_overtime_rates,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{MinWagePort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `FLSAOTRateRuleImpl`.
pub struct FLSAOTRateRule<P: PropertyPort, M: MinWagePort> {
    property: P,
    min_wage: M,
}

impl<P: PropertyPort, M: MinWagePort> FLSAOTRateRule<P, M> {
    /// Build the rule over the ports its FLSA week and minimum-wage floor
    /// come from.
    pub fn new(property: P, min_wage: M) -> Self {
        Self { property, min_wage }
    }

    /// `getRate(double regRate, LocalDate, Assignment, TimeCard, Map)`.
    fn rate(
        &self,
        reg_rate: f64,
        date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        params: &RuleParams,
    ) -> f64 {
        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let apply_min_wage_per_shift = params.bool_at(APPLY_MIN_WAGE_PER_SHIFT);
        let adjust_rate_for_shortfall = params.bool_at(ADJUST_RATE_FOR_SHORTFALL);
        let use_pay_period = params.bool_at(USE_PAY_PERIOD);

        let employee = dataset.employee().expect("no employee on the dataset");
        let calculate_over_weeks =
            !use_pay_period || dataset.pay_period_type() == PayPeriodType::Weekly;

        let flsa_rate = if calculate_over_weeks {
            let period_end_date = self.property.period_end_date(employee.property_id());
            let period_end_date = WeeklyDateRange::with_end_date(period_end_date)
                .range_containing_date(date)
                .end_date();
            let flsa_data = dataset
                .flsa_data_map()
                .get(&period_end_date)
                .unwrap_or_else(|| panic!("no FLSA data for week ending {period_end_date}"));
            flsa_data.effective_regular_rate(apply_min_wage_per_shift)
        } else {
            let period_end_date = dataset.pay_period_containing(date).end_date();
            let flsa_data = dataset
                .flsa_pay_period_data_map()
                .get(&period_end_date)
                .unwrap_or_else(|| panic!("no FLSA pay period data ending {period_end_date}"));
            flsa_data.effective_regular_rate(apply_min_wage_per_shift)
        };

        let min_wage = self
            .min_wage
            .min_wage(employee.property_id(), Some(job_id), date);
        let make_up = if reg_rate < min_wage && adjust_rate_for_shortfall {
            min_wage - reg_rate
        } else {
            0.0
        };

        round_currency(flsa_rate * ot_factor + make_up)
    }
}

impl<P: PropertyPort, M: MinWagePort> OvertimeRateRule for FLSAOTRateRule<P, M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FLSAOTRateRuleConfig.default_values());

        let rate = self.rate(
            shift.reg_rate(),
            distribution.date(),
            shift.job_id(),
            dataset,
            &params,
        );
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
            .fixed(&FLSAOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let rate = self.rate(
            earning.rate(),
            earning.earning_date(),
            earning.job_id(),
            dataset,
            &params,
        );
        add_overtime_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::schedule_mode::ScheduleMode;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use date_range_rs::DateRange;
    use std::collections::HashMap;

    struct FixedEnv {
        period_end_date: LocalDate,
        min_wage: f64,
    }
    impl PropertyPort for FixedEnv {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.period_end_date
        }
    }
    impl MinWagePort for FixedEnv {
        fn min_wage(
            &self,
            _property_id: i32,
            _job_id: Option<i32>,
            _effective_date: LocalDate,
        ) -> f64 {
            self.min_wage
        }
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "FLSA OT rate", RuleClass::FlsaOrr, params)
    }

    fn flsa_data(end_date: LocalDate, regular_rate: f64) -> FlsaData {
        FlsaData::new(
            end_date,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            regular_rate,
            regular_rate,
        )
    }

    fn dataset(
        today: LocalDate,
        weekly_rate: f64,
        pay_period_rate: f64,
        pay_period: DateRange,
        pay_period_type: PayPeriodType,
    ) -> TimeCardData {
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());

        let mut weekly = HashMap::new();
        weekly.insert(week_end, flsa_data(week_end, weekly_rate));

        let mut pay_period_map = HashMap::new();
        pay_period_map.insert(
            pay_period.end_date(),
            flsa_data(pay_period.end_date(), pay_period_rate),
        );

        TimeCardData::new()
            .with_employee(employee)
            .with_flsa_data(weekly)
            .with_flsa_pay_period_data(pay_period_map)
            .with_current_pay_period(pay_period)
            .with_contract_periods(pay_period_type, ScheduleMode::Weekly)
    }

    mod java_parity_tests {
        use super::*;

        /// `FLSAOTRateRuleImplTest`: "shift OT Rate should be get the correct
        /// rate when rule is run".
        #[test]
        fn shift_ot_rate_should_be_get_the_correct_rate_when_rule_is_run() {
            let today = LocalDate::of(2016, 6, 1);
            let pay_period = DateRange::new(LocalDate::of(2016, 6, 1), LocalDate::of(2016, 6, 30));

            for (calc_over_pay_period, handle_shortfall, expected_rate) in [
                (false, false, 10.0 * 0.5),
                (false, true, 10.0 * 0.5 + 1.0),
                (true, false, 15.0 * 0.5),
            ] {
                let pay_period_type = if calc_over_pay_period {
                    PayPeriodType::Monthly
                } else {
                    PayPeriodType::Weekly
                };
                let data = dataset(today, 10.0, 15.0, pay_period, pay_period_type);
                let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new())
                    .with_reg_rate(8.0);
                let mut distribution = HoursDistribution::new(1, today, None, 0.0, 0.0);
                let item = rule_item(
                    1,
                    rule_params! {
                        ADJUST_RATE_FOR_SHORTFALL => handle_shortfall.to_string(),
                        USE_PAY_PERIOD => calc_over_pay_period.to_string()
                    },
                );
                let rule = FLSAOTRateRule::new(
                    FixedEnv {
                        period_end_date: today,
                        min_wage: 9.0,
                    },
                    FixedEnv {
                        period_end_date: today,
                        min_wage: 9.0,
                    },
                );

                rule.execute_for_shift(&shift, &mut distribution, &data, &item);

                assert_eq!(
                    distribution.premium_rate(),
                    expected_rate,
                    "calcOverPayPeriod={calc_over_pay_period} handleShortFall={handle_shortfall}"
                );
            }
        }

        /// `FLSAOTRateRuleImplTest`: "the earning Rate should be get the
        /// correct rate when rule is run".
        #[test]
        fn the_earning_rate_should_be_get_the_correct_rate_when_rule_is_run() {
            let today = LocalDate::of(2016, 6, 1);
            let pay_period = DateRange::new(LocalDate::of(2016, 6, 1), LocalDate::of(2016, 6, 30));
            let data = dataset(today, 10.0, 15.0, pay_period, PayPeriodType::Weekly);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 22, today, 5.0, 5.0, EarningSource::Rule);
            let item = rule_item(
                1,
                rule_params! { crate::rules::algorithm::overtimerate::config::PREMIUM_TYPES => "[22]" },
            );
            let rule = FLSAOTRateRule::new(
                FixedEnv {
                    period_end_date: today,
                    min_wage: 9.0,
                },
                FixedEnv {
                    period_end_date: today,
                    min_wage: 9.0,
                },
            );

            rule.execute_for_earning(&mut earning, &data, &item);

            assert_eq!(earning.rate(), 10.0);
            assert_eq!(earning.dollars(), 0.0);
            assert_eq!(earning.total_dollars(), 50.0);
        }
    }
}
