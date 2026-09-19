//! Port of `FLSADTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/doubletimerate/FLSADTRateRuleImpl.java`.
//!
//! `flsaData.getEffectiveRegularRate(applyMinWagePerShift) * dtFactor`, keyed
//! by `new Weeks(property.getPeriodEndDate()).getWeekForDate(date)` — the
//! `doubletimerate` sibling of `FLSAWeightedOTRateRuleImpl`, not of
//! `FLSAOTRateRuleImpl`: no minimum-wage shortfall make-up term, and no
//! `Property.getCurrentWeek()`/`getPayPeriod()`/`PayPeriodType` branch. See
//! the family finding in `PARITY_AUDIT.md` about the two different
//! FLSA-week-resolution mechanisms living side by side in these families.
//!
//! Ported cases: `FLSADTRateRuleImplTest.groovy` (two `where:` tables, four
//! rows each).

use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::doubletimerate::config::{
    APPLY_MIN_WAGE_PER_SHIFT, DOUBLETIME_FACTOR_PROP, FLSADTRateRuleConfig, earning_type_ids,
};
use crate::rules::algorithm::doubletimerate::{
    DoubleTimeRateRule, add_double_time_rate, set_double_time_rates,
};
use crate::rules::ports::PropertyPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `FLSADTRateRuleImpl`.
pub struct FLSADTRateRule<P: PropertyPort> {
    property: P,
}

impl<P: PropertyPort> FLSADTRateRule<P> {
    /// Build the rule over the port its FLSA week comes from.
    pub fn new(property: P) -> Self {
        Self { property }
    }

    /// `getRate(LocalDate, TimeCard, Map)`.
    fn rate(
        &self,
        date: LocalDate,
        dataset: &dyn TimeCard,
        dt_factor: f64,
        apply_min_wage_per_shift: bool,
    ) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        let period_end_date = self.property.period_end_date(employee.property_id());
        let work_week = WeeklyDateRange::with_end_date(period_end_date).range_containing_date(date);

        let flsa_data = dataset
            .flsa_data_map()
            .get(&work_week.end_date())
            .unwrap_or_else(|| panic!("no FLSA data for week ending {}", work_week.end_date()));

        round_currency(flsa_data.effective_regular_rate(apply_min_wage_per_shift) * dt_factor)
    }
}

impl<P: PropertyPort> DoubleTimeRateRule for FLSADTRateRule<P> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FLSADTRateRuleConfig.default_values());
        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);
        let apply_min_wage_per_shift = params.bool_at(APPLY_MIN_WAGE_PER_SHIFT);

        let rate = self.rate(
            distribution.date(),
            dataset,
            dt_factor,
            apply_min_wage_per_shift,
        );
        set_double_time_rates(distribution, rate, rule_item);
        let _ = shift;
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FLSADTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let dt_factor = params.double_at(DOUBLETIME_FACTOR_PROP);
        let apply_min_wage_per_shift = params.bool_at(APPLY_MIN_WAGE_PER_SHIFT);
        let rate = self.rate(
            earning.earning_date(),
            dataset,
            dt_factor,
            apply_min_wage_per_shift,
        );
        add_double_time_rate(earning, rate);
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

    struct FixedProperty(LocalDate);
    impl PropertyPort for FixedProperty {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.0
        }
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(id, 1, "FLSA DT rate", RuleClass::FlsaDrr, params)
    }

    fn flsa_data(
        week_end_date: LocalDate,
        regular_rate: f64,
        regular_rate_min_wage: f64,
    ) -> FlsaData {
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
            regular_rate_min_wage,
        )
    }

    fn dataset_with_flsa(
        week_end_date: LocalDate,
        regular_rate: f64,
        regular_rate_min_wage: f64,
    ) -> TimeCardData {
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let mut flsa_data_map = HashMap::new();
        flsa_data_map.insert(
            week_end_date,
            flsa_data(week_end_date, regular_rate, regular_rate_min_wage),
        );
        TimeCardData::new()
            .with_employee(employee)
            .with_flsa_data(flsa_data_map)
    }

    #[test]
    fn the_flsa_rate_is_multiplied_by_the_dt_factor() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 10.0, 15.0);
        let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, today, None, 0.0, 0.0);
        let item = rule_item(
            1,
            rule_params! { DOUBLETIME_FACTOR_PROP => "2.0", APPLY_MIN_WAGE_PER_SHIFT => "false" },
        );
        let rule = FLSADTRateRule::new(FixedProperty(today));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 20.0);
    }

    #[test]
    fn applying_minimum_wage_per_shift_uses_the_other_flsa_field() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 10.0, 15.0);
        let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, today, None, 0.0, 0.0);
        let item = rule_item(
            1,
            rule_params! { DOUBLETIME_FACTOR_PROP => "1.0", APPLY_MIN_WAGE_PER_SHIFT => "true" },
        );
        let rule = FLSADTRateRule::new(FixedProperty(today));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 15.0);
    }

    mod java_parity_tests {
        use super::*;

        /// `FLSADTRateRuleImplTest`: "shift DT Rate should be get the correct
        /// rate when rule is run".
        #[test]
        fn shift_dt_rate_should_be_get_the_correct_rate_when_rule_is_run() {
            let today = LocalDate::of(2016, 6, 1);
            let week_end = WeeklyDateRange::with_end_date(today)
                .range_containing_date(today)
                .end_date();

            for (factor, use_min_wage, expected_rate) in [
                (1.0, false, 10.0),
                (0.75, false, 7.50),
                (1.0, true, 15.0),
                (0.75, true, 11.25),
            ] {
                let dataset = dataset_with_flsa(week_end, 10.0, 15.0);
                let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
                let mut distribution = HoursDistribution::new(1, today, None, 0.0, 0.0);
                let item = rule_item(
                    1,
                    rule_params! {
                        DOUBLETIME_FACTOR_PROP => factor.to_string(),
                        APPLY_MIN_WAGE_PER_SHIFT => use_min_wage.to_string()
                    },
                );
                let rule = FLSADTRateRule::new(FixedProperty(today));

                rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

                assert_eq!(
                    distribution.premium_rate(),
                    expected_rate,
                    "factor={factor} useMinWage={use_min_wage}"
                );
            }
        }

        /// `FLSADTRateRuleImplTest`: "the earning Rate should be get the
        /// correct rate when rule is run".
        #[test]
        fn the_earning_rate_should_be_get_the_correct_rate_when_rule_is_run() {
            let today = LocalDate::of(2016, 6, 1);
            let week_end = WeeklyDateRange::with_end_date(today)
                .range_containing_date(today)
                .end_date();

            for (factor, use_min_wage, expected_rate, expected_dollars) in [
                (1.0, false, 15.0, 75.0),
                (0.75, false, 12.5, 62.5),
                (1.0, true, 20.0, 100.0),
                (0.75, true, 16.25, 81.25),
            ] {
                let dataset = dataset_with_flsa(week_end, 10.0, 15.0);
                let mut earning =
                    EmployeeEarning::new(1, 100, 1, 22, today, 5.0, 5.0, EarningSource::Rule);
                let item = rule_item(
                    1,
                    rule_params! {
                        crate::rules::algorithm::doubletimerate::config::PREMIUM_TYPES => "[22]",
                        DOUBLETIME_FACTOR_PROP => factor.to_string(),
                        APPLY_MIN_WAGE_PER_SHIFT => use_min_wage.to_string()
                    },
                );
                let rule = FLSADTRateRule::new(FixedProperty(today));

                rule.execute_for_earning(&mut earning, &dataset, &item);

                assert_eq!(
                    earning.rate(),
                    expected_rate,
                    "factor={factor} useMinWage={use_min_wage}"
                );
                assert_eq!(earning.dollars(), 0.0);
                assert_eq!(earning.total_dollars(), expected_dollars);
            }
        }
    }
}
