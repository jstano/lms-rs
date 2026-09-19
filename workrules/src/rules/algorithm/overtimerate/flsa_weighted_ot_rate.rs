//! Port of `FLSAWeightedOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/FLSAWeightedOTRateRuleImpl.java`.
//!
//! Confusingly named next to `WeightedOTRateRuleImpl` — this is the *plain*
//! FLSA-rate × factor rule (`flsaData.getEffectiveRegularRate(true) *
//! otFactor`), keyed by `Weeks(periodEndDate)` like `FLSADTRateRuleImpl`, not
//! by `Property.getCurrentWeek()`/`getPayPeriod()` like `FLSAOTRateRuleImpl`.
//! No minimum-wage shortfall make-up term, and `applyMinWagePerShift` is
//! hard-coded `true` rather than configurable.
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
    FLSAWeightedOTRateRuleConfig, OVERTIME_FACTOR_PROP, earning_type_ids,
};
use crate::rules::algorithm::overtimerate::{
    OvertimeRateRule, add_overtime_rate, set_overtime_rates,
};
use crate::rules::ports::PropertyPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `FLSAWeightedOTRateRuleImpl`.
pub struct FLSAWeightedOTRateRule<P: PropertyPort> {
    property: P,
}

impl<P: PropertyPort> FLSAWeightedOTRateRule<P> {
    /// Build the rule over the port its FLSA week comes from.
    pub fn new(property: P) -> Self {
        Self { property }
    }

    /// `getRate(LocalDate, TimeCard, Map)`.
    fn rate(&self, date: LocalDate, dataset: &dyn TimeCard, ot_factor: f64) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        let period_end_date = self.property.period_end_date(employee.property_id());
        let work_week = WeeklyDateRange::with_end_date(period_end_date).range_containing_date(date);

        let flsa_data = dataset
            .flsa_data_map()
            .get(&work_week.end_date())
            .unwrap_or_else(|| panic!("no FLSA data for week ending {}", work_week.end_date()));

        round_currency(flsa_data.effective_regular_rate(true) * ot_factor)
    }
}

impl<P: PropertyPort> OvertimeRateRule for FLSAWeightedOTRateRule<P> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&FLSAWeightedOTRateRuleConfig.default_values());
        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);

        let rate = self.rate(distribution.date(), dataset, ot_factor);
        set_overtime_rates(distribution, rate, rule_item);
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
            .fixed(&FLSAWeightedOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let ot_factor = params.double_at(OVERTIME_FACTOR_PROP);
        let rate = self.rate(earning.earning_date(), dataset, ot_factor);
        add_overtime_rate(earning, rate);
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
        RuleItem::new(
            id,
            1,
            "FLSA weighted OT rate",
            RuleClass::FlsaWeightedOrr,
            params,
        )
    }

    fn dataset_with_flsa(week_end_date: LocalDate, regular_rate_min_wage: f64) -> TimeCardData {
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
                0.0,
                regular_rate_min_wage,
            ),
        );
        TimeCardData::new()
            .with_employee(employee)
            .with_flsa_data(flsa_data_map)
    }

    #[test]
    fn the_flsa_rate_at_minimum_wage_is_multiplied_by_the_ot_factor() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 12.0);
        let shift = EmployeeShift::new(1, 100, 1, today, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, today, None, 0.0, 0.0);
        let item = rule_item(1, rule_params! { OVERTIME_FACTOR_PROP => "0.5" });
        let rule = FLSAWeightedOTRateRule::new(FixedProperty(today));

        rule.execute_for_shift(&shift, &mut distribution, &dataset, &item);

        assert_eq!(distribution.premium_rate(), 6.0);
    }

    #[test]
    fn a_premium_earning_type_gets_the_ot_rate_added_on_top() {
        let today = LocalDate::of(2016, 6, 1);
        let week_end = WeeklyDateRange::with_end_date(today)
            .range_containing_date(today)
            .end_date();
        let dataset = dataset_with_flsa(week_end, 12.0);
        let mut earning = EmployeeEarning::new(1, 100, 1, 22, today, 5.0, 3.0, EarningSource::Rule);
        let item = rule_item(
            1,
            rule_params! {
                OVERTIME_FACTOR_PROP => "0.5",
                crate::rules::algorithm::overtimerate::config::PREMIUM_TYPES => "[22]"
            },
        );
        let rule = FLSAWeightedOTRateRule::new(FixedProperty(today));

        rule.execute_for_earning(&mut earning, &dataset, &item);

        assert_eq!(earning.rate(), 9.0);
    }
}
