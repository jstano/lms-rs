//! Port of `com.unifocus.watson.common.labor.contract`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/contract/`.
//!
//! Turning a property's **weekly** contracted hours into the hours contracted
//! for one pay period. Java models this as a `ContractCalculator` interface
//! with three singleton implementations and a factory that picks one from the
//! property's pay period type and schedule mode; the three are collapsed here
//! into one enum and a `match`, the way divergence 3 collapsed the four
//! rounding strategies.
//!
//! `ContractStringFormatter` is display code and does not come across.
//!
//! # A weekly schedule pins the contract to one week
//!
//! The factory's first test is `scheduleMode == WEEKLY && payPeriodType !=
//! WEEKLY`, which selects the calculator that ignores the period entirely and
//! answers the weekly figure unchanged. So a property that schedules weekly but
//! pays monthly contracts for **one week's** hours a period, not a month's.
//! That is either the point or a long-standing bug; it is reproduced either
//! way.
//!
//! # The two real calculators round differently, and deliberately
//!
//! `WeekBasedContractCalculator` rounds to two places; `DaysInPeriodContractCalculator`
//! rounds to **zero** — and per the `TDouble` finding those are different
//! rounding rules, not the same one at different precisions. Zero places is
//! plain half-to-even; two places carries the epsilon nudge. Both go through
//! [`round_to`], which spells the difference out.
//!
//! [`round_to`]: crate::common::numbers::round_to

use crate::common::enums::pay_period_type::PayPeriodType;
use crate::common::enums::schedule_mode::ScheduleMode;
use crate::common::numbers::round_to;
use date_range_rs::DateRange;

/// Days in a week. `WeekBasedContractCalculator.DAYS_IN_WEEK`.
const DAYS_IN_WEEK: i64 = 7;

/// `DaysInPeriodContractCalculator.WEEKS_IN_YEAR`.
const WEEKS_IN_YEAR: f64 = 52.0;

/// `DaysInPeriodContractCalculator.DAYS_IN_YEAR`.
const DAYS_IN_YEAR: f64 = 365.0;

/// How a property's weekly contract scales to one pay period.
/// `ContractCalculator` and its three implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractCalculator {
    /// `DefaultContractCalculator` — the weekly figure, whatever the period.
    Default,
    /// `WeekBasedContractCalculator` — whole weeks plus a pro-rated remainder.
    WeekBased,
    /// `DaysInPeriodContractCalculator` — an annual figure spread over 365 days.
    DaysInPeriod,
}

impl ContractCalculator {
    /// `ContractCalculatorFactory.createCalculator`.
    ///
    /// Java throws `IllegalArgumentException` when either argument is null, and
    /// again for a pay period type outside the four it switches on. Both
    /// arguments are enums here, and the match is exhaustive, so neither
    /// condition is expressible.
    pub fn for_property(pay_period_type: PayPeriodType, schedule_mode: ScheduleMode) -> Self {
        if schedule_mode == ScheduleMode::Weekly && pay_period_type != PayPeriodType::Weekly {
            return Self::Default;
        }

        match pay_period_type {
            PayPeriodType::Weekly | PayPeriodType::BiWeekly => Self::WeekBased,
            PayPeriodType::SemiMonthly | PayPeriodType::Monthly => Self::DaysInPeriod,
        }
    }

    /// The hours contracted for `pay_period`. `contractHoursForPayPeriod`.
    pub fn contract_hours_for_pay_period(
        self,
        weekly_contract_hours: f64,
        pay_period: &DateRange,
    ) -> f64 {
        let days_in_period = days_in_range(pay_period);

        match self {
            Self::Default => weekly_contract_hours,

            Self::WeekBased => {
                let whole_weeks = days_in_period / DAYS_IN_WEEK;
                let whole_weeks_hours = round_to(weekly_contract_hours * whole_weeks as f64, 2);

                let days_in_partial_week = days_in_period % DAYS_IN_WEEK;
                let avg_hours_per_day = weekly_contract_hours / DAYS_IN_WEEK as f64;
                let partial_weeks_hours =
                    round_to(avg_hours_per_day * days_in_partial_week as f64, 2);

                round_to(partial_weeks_hours + whole_weeks_hours, 2)
            }

            Self::DaysInPeriod => {
                let annual_contract = weekly_contract_hours * WEEKS_IN_YEAR;
                let daily_contract_hours = annual_contract / DAYS_IN_YEAR;
                // Zero places is plain half-to-even, not the nudged rule.
                round_to(daily_contract_hours * days_in_period as f64, 0)
            }
        }
    }
}

/// `DateRange.getNumberOfDaysInRange()` — inclusive of both ends.
fn days_in_range(range: &DateRange) -> i64 {
    range.dates().len() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalDate;
    use rstest::rstest;

    fn period(days: i64) -> DateRange {
        let start = LocalDate::of(2016, 1, 1);
        DateRange::new(start, start.plus_days(days - 1))
    }

    #[rstest]
    // A weekly schedule short-circuits to Default for every non-weekly period.
    #[case(
        PayPeriodType::Monthly,
        ScheduleMode::Weekly,
        ContractCalculator::Default
    )]
    #[case(
        PayPeriodType::BiWeekly,
        ScheduleMode::Weekly,
        ContractCalculator::Default
    )]
    #[case(
        PayPeriodType::SemiMonthly,
        ScheduleMode::Weekly,
        ContractCalculator::Default
    )]
    // …but a weekly period with a weekly schedule falls through to WeekBased.
    #[case(
        PayPeriodType::Weekly,
        ScheduleMode::Weekly,
        ContractCalculator::WeekBased
    )]
    #[case(
        PayPeriodType::Weekly,
        ScheduleMode::Monthly,
        ContractCalculator::WeekBased
    )]
    #[case(
        PayPeriodType::BiWeekly,
        ScheduleMode::Monthly,
        ContractCalculator::WeekBased
    )]
    #[case(
        PayPeriodType::SemiMonthly,
        ScheduleMode::Monthly,
        ContractCalculator::DaysInPeriod
    )]
    #[case(
        PayPeriodType::Monthly,
        ScheduleMode::Monthly,
        ContractCalculator::DaysInPeriod
    )]
    fn the_factory_picks_a_calculator_from_the_period_and_the_schedule(
        #[case] pay_period_type: PayPeriodType,
        #[case] schedule_mode: ScheduleMode,
        #[case] expected: ContractCalculator,
    ) {
        assert_eq!(
            ContractCalculator::for_property(pay_period_type, schedule_mode),
            expected
        );
    }

    #[test]
    fn the_default_calculator_ignores_the_period() {
        assert_eq!(
            ContractCalculator::Default.contract_hours_for_pay_period(39.0, &period(30)),
            39.0
        );
    }

    #[test]
    fn the_week_based_calculator_pro_rates_the_partial_week() {
        let week_based = ContractCalculator::WeekBased;

        assert_eq!(
            week_based.contract_hours_for_pay_period(39.0, &period(7)),
            39.0
        );
        assert_eq!(
            week_based.contract_hours_for_pay_period(39.0, &period(14)),
            78.0
        );
        // Ten days: one whole week plus three days at 39/7 = 5.571… an hour.
        assert_eq!(
            week_based.contract_hours_for_pay_period(39.0, &period(10)),
            55.71
        );
    }

    #[test]
    fn the_days_in_period_calculator_rounds_to_whole_hours() {
        // 39 * 52 / 365 = 5.556… a day, over 31 days, rounded to no places.
        assert_eq!(
            ContractCalculator::DaysInPeriod.contract_hours_for_pay_period(39.0, &period(31)),
            172.0
        );
    }
}
