//! Port of `WeightedOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/WeightedOTRateRuleImpl.java`.
//!
//! The FLSA weighted-average-rate calculation, but backs out only the
//! **incremental** rate to add on top of the distribution's own `baseRate`:
//! computes total OT dollars due at the true weighted rate × factor,
//! subtracts what straight-time-at-`baseRate` would already cover, and
//! divides the remainder back over the hours — so what this writes as
//! `premiumRate` is a top-up, not the whole rate. The earning overload does
//! the same subtraction against `earning.getRate() * earning.getHours()`.
//!
//! Reading `setOvertimeRates(shift, distribution, rate, ...)` in isolation
//! elsewhere in the family suggests `rate` is the whole overtime rate; here
//! it is an increment. Flagged so a future reader of this family doesn't
//! assume the shared shape.
//!
//! No Groovy spec exists for this rule; the behaviour tests below are written
//! from the Java.

use crate::common::numbers::{round_currency, round_to};
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::overtimerate::config::{
    MIN_WAGE_BEFORE_CALC, OVERTIME_FACTOR_PROP, WeightedOTRateRuleConfig, earning_type_ids,
};
use crate::rules::algorithm::overtimerate::{OvertimeRateRule, set_overtime_rates};
use crate::rules::params::RuleParams;
use crate::rules::ports::{MinWagePort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `WeightedOTRateRuleImpl`.
pub struct WeightedOTRateRule<P: PropertyPort, M: MinWagePort> {
    property: P,
    min_wage: M,
}

impl<P: PropertyPort, M: MinWagePort> WeightedOTRateRule<P, M> {
    /// Build the rule over the ports its FLSA week and minimum-wage floor
    /// come from.
    pub fn new(property: P, min_wage: M) -> Self {
        Self { property, min_wage }
    }

    /// The weighted (FLSA) rate for the week, floored at minimum wage.
    fn weighted_rate(
        &self,
        date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        min_wage_before_calc: bool,
    ) -> (f64, f64) {
        let employee = dataset.employee().expect("no employee on the dataset");
        let period_end_date = self.property.period_end_date(employee.property_id());
        let work_week = WeeklyDateRange::with_end_date(period_end_date).range_containing_date(date);

        let flsa_data = dataset
            .flsa_data_map()
            .get(&work_week.end_date())
            .unwrap_or_else(|| panic!("no FLSA data for week ending {}", work_week.end_date()));
        let flsa_rate = flsa_data.effective_regular_rate(min_wage_before_calc);

        let min_wage = self
            .min_wage
            .min_wage(employee.property_id(), Some(job_id), date);
        let ot_rate = flsa_rate.max(min_wage);

        (flsa_rate, ot_rate)
    }

    /// `getRate(LocalDate, Assignment, TimeCard, Employee, Map, double, double)`.
    fn rate(
        &self,
        date: LocalDate,
        job_id: i32,
        dataset: &dyn TimeCard,
        params: &RuleParams,
        ot_distribution_hours: f64,
        base_rate: f64,
    ) -> f64 {
        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let min_wage_before_calc = params.bool_at(MIN_WAGE_BEFORE_CALC);
        let (flsa_rate, ot_rate) = self.weighted_rate(date, job_id, dataset, min_wage_before_calc);

        let ot_dollars_due = round_currency(flsa_rate * ot_distribution_hours)
            + round_currency(round_currency(ot_distribution_hours * ot_rate) * (ot_factor - 1.0));
        let straight_time_ot_dollars = round_to(base_rate * ot_distribution_hours, 2);

        if ot_distribution_hours > 0.0 {
            round_currency((ot_dollars_due - straight_time_ot_dollars) / ot_distribution_hours)
        } else {
            0.0
        }
    }
}

impl<P: PropertyPort, M: MinWagePort> OvertimeRateRule for WeightedOTRateRule<P, M> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&WeightedOTRateRuleConfig.default_values());

        let rate = self.rate(
            distribution.date(),
            shift.job_id(),
            dataset,
            &params,
            distribution.hours(),
            distribution.base_rate(),
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
            .fixed(&WeightedOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        if earning.hours() <= 0.0 {
            return;
        }

        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let min_wage_before_calc = params.bool_at(MIN_WAGE_BEFORE_CALC);
        let (flsa_rate, ot_rate) = self.weighted_rate(
            earning.earning_date(),
            earning.job_id(),
            dataset,
            min_wage_before_calc,
        );

        let ot_dollars_due = round_currency(flsa_rate * earning.hours())
            + round_currency(round_currency(earning.hours() * ot_rate) * (ot_factor - 1.0));
        let straight_time_ot_dollars = round_to(earning.rate() * earning.hours(), 2);

        earning.set_rate(round_currency(
            (ot_dollars_due - straight_time_ot_dollars) / earning.hours() + earning.rate(),
        ));
        earning.set_dollars(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::flsa_data::FlsaData;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
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
        RuleItem::new(id, 1, "Weighted OT rate", RuleClass::WeightedOrr, params)
    }

    fn dataset_with_flsa(week_end_date: LocalDate, regular_rate: f64) -> TimeCardData {
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let mut flsa_data_map = HashMap::new();
        flsa_data_map.insert(
            week_end_date,
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
                regular_rate,
                regular_rate,
            ),
        );
        TimeCardData::new()
            .with_employee(employee)
            .with_flsa_data(flsa_data_map)
    }

    #[test]
    fn the_top_up_covers_the_gap_between_weighted_ot_dollars_and_straight_time() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 10.0);
        let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
        // 10 OT hours at baseRate 8.0
        let mut distribution = HoursDistribution::new(1, today, None, 10.0, 8.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "1.5" });
        let rule = WeightedOTRateRule::new(
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
        );

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        // otDollarsDue = 10*10 + (10*10)*(0.5) = 100 + 50 = 150
        // straightTimeOTDollars = 8*10 = 80
        // topUp = (150-80)/10 = 7.0
        assert_eq!(distribution.premium_rate(), 7.0);
    }

    #[test]
    fn zero_hours_prices_to_zero() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 10.0);
        let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, today, None, 0.0, 8.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "1.5" });
        let rule = WeightedOTRateRule::new(
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
        );

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 0.0);
    }

    #[test]
    fn a_zero_hours_earning_is_left_alone() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 10.0);
        let mut earning = EmployeeEarning::new(1, 100, 1, 22, today, 0.0, 8.0, EarningSource::Rule);
        let item = rule_item(
            1,
            rule_params! { crate::rules::algorithm::overtimerate::config::PREMIUM_TYPES => "[22]" },
        );
        let rule = WeightedOTRateRule::new(
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
            FixedEnv {
                period_end_date: today,
                min_wage: 0.0,
            },
        );

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 8.0);
    }
}
