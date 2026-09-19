//! Port of `AvgDayXWeeksRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/AvgDayXWeeksRateRuleImpl.java`.
//!
//! Prices an average-day benefit payout: home job rate times a configured
//! factor (floored at minimum wage if the earning type says so), times `(net
//! hours / distinct worked days)` averaged over a DAO-supplied window
//! starting `lastXWeeks` weeks before the current scheduling week — `0.0` if
//! the employee was hired after that window's end, or if the DAO finds no
//! start date for the window.
//!
//! # The window's end date, not its start, gates the hire-date check
//!
//! `calcPeriodEndDate` is the day before the week containing the earning
//! date starts — Java's `week.getStartDate().minusDays(1)` — and it is that
//! date, not `startDate`, the hire-date guard compares against. The window's
//! start only matters once the guard has already passed.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::EarningRateRule;
use crate::rules::algorithm::earningrate::config::{
    AvgDayXWeeksRateRuleConfig, LAST_X_WEEKS, RATE_FACTOR,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, HolidayDataPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;

/// `AvgDayXWeeksRateRuleImpl`.
pub struct AvgDayXWeeksRateRule<H: HolidayDataPort, E: EarningTypePort, M: MinWagePort> {
    holiday_data: H,
    earning_types: E,
    min_wage: M,
}

impl<H: HolidayDataPort, E: EarningTypePort, M: MinWagePort> AvgDayXWeeksRateRule<H, E, M> {
    /// Build the rule over the ports its holiday-eligibility window, earning
    /// type and minimum-wage floor come from.
    pub fn new(holiday_data: H, earning_types: E, min_wage: M) -> Self {
        Self {
            holiday_data,
            earning_types,
            min_wage,
        }
    }
}

impl<H: HolidayDataPort, E: EarningTypePort, M: MinWagePort> EarningRateRule
    for AvgDayXWeeksRateRule<H, E, M>
{
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&AvgDayXWeeksRateRuleConfig.default_values());
        let rate_factor = params.double_at(RATE_FACTOR);
        let x_weeks = params.int_at(LAST_X_WEEKS);

        let employee = dataset.employee().expect("no employee on the dataset");
        let earning_date = earning.earning_date();

        let mut hourly_rate = employee
            .home_employee_job_status(earning_date)
            .map_or(0.0, |status| status.hourly_rate());
        hourly_rate = round_currency(hourly_rate * rate_factor);

        let earning_type = self
            .earning_types
            .find_by_id(earning.earning_type_id())
            .unwrap_or_else(|| panic!("no earning type {}", earning.earning_type_id()));
        let min_wage =
            self.min_wage
                .min_wage(employee.property_id(), Some(earning.job_id()), earning_date);
        if earning_type.pay_at_least_min_wage() && hourly_rate < min_wage {
            hourly_rate = min_wage;
        }

        let mut daily_rate = 0.0;

        let week = WeeklyDateRange::with_end_date(dataset.current_pay_period().end_date())
            .range_containing_date(earning_date);
        let calc_period_end_date = week.start_date().minus_days(1);

        let hired_before_window_ends = employee
            .hire_date()
            .is_some_and(|hire_date| hire_date.is_on_or_before(calc_period_end_date));

        if hired_before_window_ends {
            let start_date = self.holiday_data.week_start_of_nth_past_worked_week(
                employee.id(),
                x_weeks,
                calc_period_end_date,
            );

            if let Some(start_date) = start_date {
                let period = DateRange::new(start_date, calc_period_end_date);
                let eligible_hours = self.holiday_data.eligible_hours(employee.id(), &period);

                if let Some(date_count) = eligible_hours
                    .and_then(|hours| hours.date_count)
                    .filter(|count| *count != 0)
                {
                    let net_hours = eligible_hours
                        .and_then(|hours| hours.net_hours)
                        .unwrap_or(0.0);
                    let avg_day = net_hours / f64::from(date_count);
                    daily_rate = round_currency(avg_day * hourly_rate);
                }
            }
        }

        earning.set_rate(daily_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::ports::EligibleHours;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    struct EarningTypes(HashMap<i32, EarningType>);
    impl EarningTypePort for EarningTypes {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
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

    struct FixedHolidayData {
        start_date: Option<LocalDate>,
        eligible_hours: Option<EligibleHours>,
    }
    impl HolidayDataPort for FixedHolidayData {
        fn week_start_of_nth_past_worked_week(
            &self,
            _employee_id: i32,
            _x_weeks: i32,
            _calc_period_end_date: LocalDate,
        ) -> Option<LocalDate> {
            self.start_date
        }
        fn eligible_hours(&self, _employee_id: i32, _period: &DateRange) -> Option<EligibleHours> {
            self.eligible_hours
        }
    }

    fn home_status(hourly_rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            1,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            true,
        )
    }

    fn dataset(employee: Employee, pay_period_end: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee)
            .with_current_pay_period(DateRange::new(pay_period_end.minus_days(6), pay_period_end))
    }

    #[test]
    fn the_average_day_rate_divides_net_hours_by_date_count() {
        let earning_date = LocalDate::of(2020, 6, 10);
        let mut employee = Employee::new(100, 1, "Alex Kim", vec![home_status(20.0)]);
        employee = employee.with_dates(Some(LocalDate::of(2000, 1, 1)), None, None);
        let dataset = dataset(employee, earning_date);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 8.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(
                7,
                1,
                "Regular",
                crate::common::enums::earn_type::EarnType::Regular,
                crate::common::enums::uom::UOM::Hours,
            ),
        )]));
        let holiday_data = FixedHolidayData {
            start_date: Some(LocalDate::of(2020, 5, 1)),
            eligible_hours: Some(EligibleHours {
                date_count: Some(4),
                net_hours: Some(32.0),
                reg_hours: Some(32.0),
            }),
        };
        let mut rule = AvgDayXWeeksRateRule::new(holiday_data, earning_types, FixedMinWage(0.0));

        rule.execute(&dataset, &mut earning, &RuleParams::new());

        // 32/4 = 8 avg day hours * 20.0 hourly = 160.0
        assert_eq!(earning.rate(), 160.0);
    }

    #[test]
    fn hired_after_the_window_prices_the_earning_at_zero() {
        let earning_date = LocalDate::of(2020, 6, 10);
        let mut employee = Employee::new(100, 1, "Alex Kim", vec![home_status(20.0)]);
        employee = employee.with_dates(Some(LocalDate::of(2020, 6, 9)), None, None);
        let dataset = dataset(employee, earning_date);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 7, earning_date, 8.0, 0.0, EarningSource::Rule);
        let earning_types = EarningTypes(HashMap::from([(
            7,
            EarningType::new(
                7,
                1,
                "Regular",
                EarnType::Regular,
                crate::common::enums::uom::UOM::Hours,
            ),
        )]));
        let holiday_data = FixedHolidayData {
            start_date: Some(LocalDate::of(2020, 5, 1)),
            eligible_hours: Some(EligibleHours {
                date_count: Some(4),
                net_hours: Some(32.0),
                reg_hours: Some(32.0),
            }),
        };
        let mut rule = AvgDayXWeeksRateRule::new(holiday_data, earning_types, FixedMinWage(0.0));

        rule.execute(&dataset, &mut earning, &RuleParams::new());

        assert_eq!(earning.rate(), 0.0);
    }
}
