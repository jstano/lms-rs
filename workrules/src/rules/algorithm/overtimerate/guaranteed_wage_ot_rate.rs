//! Port of `GuaranteedWageOTRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/GuaranteedWageOTRateRuleImpl.java`.
//!
//! Prices overtime off a weekly guaranteed wage: `homeJobRate ×
//! configuredWeeklyHours`, divided by the week's actual worked hours (shifts
//! plus regular/premium earnings) to get an effective hourly rate, then ×
//! factor, then rounded with `roundHours` — not `roundCurrency`, unlike every
//! sibling in the family.
//!
//! # The earning path does not round the addition
//!
//! Every other rule in `regularrate`/`doubletimerate`/`overtimerate` that
//! adds a computed rate onto an earning's existing one wraps the sum in
//! `roundCurrency` (see [`add_overtime_rate`](crate::rules::algorithm::overtimerate::add_overtime_rate)).
//! This one does not: `earning.setRate(getOtRate(...) + earning.getRate())`,
//! no rounding around the `+`. `getOtRate` has already rounded its own result
//! with `roundHours`, so nothing is lost here in practice, but the shape is
//! different enough that this rule does not use the shared helper.
//!
//! Ported cases: `GuaranteedWageOTRateRuleImplTest.groovy` (two `where:`
//! tables — one over shifts of varying weekly hours, one over earnings).

use crate::common::enums::earn_type::EarnType;
use crate::common::numbers::{round_currency, round_hours, safe_divide_default};
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::overtimerate::OvertimeRateRule;
use crate::rules::algorithm::overtimerate::config::{
    GuaranteedWageOTRateRuleConfig, OVERTIME_FACTOR_PROP, WEEKLY_HOURS, earning_type_ids,
};
use crate::rules::ports::{EarningTypePort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `GuaranteedWageOTRateRuleImpl`.
pub struct GuaranteedWageOTRateRule<P: PropertyPort, E: EarningTypePort> {
    property: P,
    earning_types: E,
}

impl<P: PropertyPort, E: EarningTypePort> GuaranteedWageOTRateRule<P, E> {
    /// Build the rule over the ports its work week and earning-type category
    /// come from.
    pub fn new(property: P, earning_types: E) -> Self {
        Self {
            property,
            earning_types,
        }
    }

    fn work_week(&self, employee_property_id: i32, date: LocalDate) -> DateRange {
        let period_end_date = self.property.period_end_date(employee_property_id);
        WeeklyDateRange::with_end_date(period_end_date).range_containing_date(date)
    }

    fn total_hours_for_week(&self, dataset: &dyn TimeCard, work_week: &DateRange) -> f64 {
        let shift_hours: f64 = dataset
            .shifts_for_period(work_week)
            .iter()
            .map(|shift| shift.net_hours())
            .sum();

        let earning_hours: f64 = dataset
            .earnings_for_period(work_week)
            .iter()
            .filter(|earning| {
                self.earning_types
                    .find_by_id(earning.earning_type_id())
                    .is_some_and(|earning_type| {
                        matches!(
                            earning_type.earn_type(),
                            EarnType::Premium | EarnType::Regular
                        )
                    })
            })
            .map(|earning| earning.hours())
            .sum();

        shift_hours + earning_hours
    }

    /// `getOtRate(LocalDate, TimeCard, Map)`.
    fn ot_rate(
        &self,
        date: LocalDate,
        dataset: &dyn TimeCard,
        ot_factor: f64,
        weekly_hours: f64,
    ) -> f64 {
        let employee = dataset.employee().expect("no employee on the dataset");
        let work_week = self.work_week(employee.property_id(), date);

        let home_job_rate = employee
            .last_home_employee_job_status_for_period(&work_week)
            .map_or(0.0, |status| status.hourly_rate());

        let weekly_guaranteed_wages = round_currency(home_job_rate * weekly_hours);
        let total_hours = self.total_hours_for_week(dataset, &work_week);
        let weekly_guaranteed_rate =
            round_currency(safe_divide_default(weekly_guaranteed_wages, total_hours));

        round_hours(weekly_guaranteed_rate * ot_factor)
    }
}

impl<P: PropertyPort, E: EarningTypePort> OvertimeRateRule for GuaranteedWageOTRateRule<P, E> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&GuaranteedWageOTRateRuleConfig.default_values());

        let rate = self.ot_rate(
            distribution.date(),
            dataset,
            params.double_at(OVERTIME_FACTOR_PROP),
            params.double_at(WEEKLY_HOURS),
        );

        distribution.set_premium_rate(rate);
        distribution.set_rate_rule_item_id(Some(rule_item.id()));
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
            .fixed(&GuaranteedWageOTRateRuleConfig.default_values());

        if !earning_type_ids(&params).contains(&earning.earning_type_id()) {
            return;
        }

        let rate = self.ot_rate(
            earning.earning_date(),
            dataset,
            params.double_at(OVERTIME_FACTOR_PROP),
            params.double_at(WEEKLY_HOURS),
        );

        earning.set_rate(rate + earning.rate());
        earning.set_dollars(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
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

    struct EarningTypes(HashMap<i32, EarnType>);
    impl EarningTypePort for EarningTypes {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).map(|earn_type| {
                EarningType::new(
                    id,
                    1,
                    "Type",
                    *earn_type,
                    crate::common::enums::uom::UOM::Hours,
                )
            })
        }
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(
            id,
            1,
            "Guaranteed wage OT rate",
            RuleClass::GuaranteedWageOrr,
            params,
        )
    }

    fn home_job_status(hourly_rate: f64, start: LocalDate, end: LocalDate) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            1,
            start,
            end,
            EmployeePayType::Hourly,
            hourly_rate,
            true,
        )
    }

    fn shift(date: LocalDate, net_hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 100, 1, date, ShiftType::Actual, Vec::new()).with_net_hours(net_hours)
    }

    mod java_parity_tests {
        use super::*;

        /// `GuaranteedWageOTRateRuleImplTest`: "the correct otRate is set for
        /// shifts regardless if there is an employee home job status".
        #[test]
        fn the_correct_ot_rate_is_set_for_shifts() {
            let shift_date = LocalDate::of(1999, 12, 31);
            let employee = Employee::new(
                100,
                1,
                "",
                vec![home_job_status(17.50, shift_date, shift_date)],
            );

            for (shifts, expected_rate) in [
                (vec![], 0.0),
                (
                    vec![
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(3), 3.0),
                    ],
                    18.42,
                ),
                (
                    vec![
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(2), 8.0),
                        shift(shift_date.minus_days(3), 6.0),
                    ],
                    11.67,
                ),
                (
                    vec![
                        shift(shift_date.plus_days(1), 15.0),
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(2), 8.0),
                        shift(shift_date.minus_days(3), 8.0),
                        shift(shift_date.minus_days(4), 8.0),
                    ],
                    8.75,
                ),
                (
                    vec![
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(2), 8.0),
                        shift(shift_date.minus_days(3), 8.0),
                        shift(shift_date.minus_days(4), 8.0),
                        shift(shift_date.minus_days(5), 10.0),
                    ],
                    7.0,
                ),
                (
                    vec![
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(2), 8.0),
                        shift(shift_date.minus_days(3), 8.0),
                        shift(shift_date.minus_days(4), 8.0),
                        shift(shift_date.minus_days(5), 10.0),
                        shift(shift_date.minus_days(6), 10.0),
                        shift(shift_date.minus_days(7), 15.0),
                    ],
                    5.83,
                ),
                (
                    vec![
                        shift(shift_date, 8.0),
                        shift(shift_date.minus_days(1), 8.0),
                        shift(shift_date.minus_days(2), 8.0),
                        shift(shift_date.minus_days(3), 8.0),
                        shift(shift_date.minus_days(4), 8.0),
                        shift(shift_date.minus_days(5), 10.0),
                        shift(shift_date.minus_days(6), 20.0),
                    ],
                    5.0,
                ),
            ] {
                let dataset = TimeCardData::new()
                    .with_employee(employee.clone())
                    .with_shifts(shifts.clone());
                let this_shift =
                    EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new())
                        .with_reg_rate(17.50);
                let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
                let item = rule_item(1, RuleParams::new());
                let rule = GuaranteedWageOTRateRule::new(
                    FixedProperty(shift_date),
                    EarningTypes(HashMap::new()),
                );

                rule.execute_for_shift(&this_shift, &mut distribution, &dataset, &item);

                assert_eq!(
                    distribution.premium_rate(),
                    expected_rate,
                    "shifts={shifts:?}"
                );
            }
        }

        /// `GuaranteedWageOTRateRuleImplTest`: "the correct otRate is set for
        /// earnings regardless if there is an employee home job status and
        /// only if the earningType is in the configures earning types".
        #[test]
        fn the_correct_ot_rate_is_set_for_earnings() {
            let earning_date = LocalDate::of(2000, 12, 31);
            let employee = Employee::new(
                100,
                1,
                "",
                vec![home_job_status(17.50, earning_date, earning_date)],
            );
            let earnings = vec![
                EmployeeEarning::new(1, 100, 1, 2, earning_date, 8.0, 0.0, EarningSource::Rule),
                EmployeeEarning::new(
                    2,
                    100,
                    1,
                    2,
                    earning_date.minus_days(3),
                    3.0,
                    0.0,
                    EarningSource::Rule,
                ),
            ];
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_earnings(earnings);
            let mut earning =
                EmployeeEarning::new(3, 100, 1, 2, earning_date, 0.0, 17.50, EarningSource::Rule);
            let item = rule_item(
                1,
                rule_params! { crate::rules::algorithm::overtimerate::config::PREMIUM_TYPES => "[2]" },
            );
            let rule = GuaranteedWageOTRateRule::new(
                FixedProperty(earning_date),
                EarningTypes(HashMap::from([(2, EarnType::Premium)])),
            );

            rule.execute_for_earning(&mut earning, &dataset, &item);

            assert_eq!(earning.rate(), 49.32);
        }
    }
}
