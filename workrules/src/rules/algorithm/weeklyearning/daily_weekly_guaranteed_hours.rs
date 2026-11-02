//! Port of `DailyWeeklyGuaranteedHoursRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/DailyWeeklyGuaranteedHoursRuleImpl.java`.
//!
//! Tops each day, and then the week, up to a guaranteed minimum — and trims it
//! back down to a maximum — by writing an hours earning, which may be
//! **negative**.
//!
//! # Daily pass
//!
//! A day's hours are the sum of its shifts' hours-distribution `hours` (every
//! shift, errors or not) plus the hours of any earnings in `additionalHours`
//! types dated that day. For each date of the week the rule can write on (open
//! for editing, employee active, **and** a home job) and which has distribution
//! hours above zero: below `minDailyHrs` it pays the shortfall, above
//! `maxDailyHrs` it pays the negative excess. A day with only additional
//! earnings and no shift hours is not considered.
//!
//! # Weekly pass
//!
//! Only if the week has hours and the **end date** can be written on. The week's
//! hours are the distribution hours plus *all* the additional hours in the
//! week (including on days with no shifts), plus the hours of every earning
//! **this rule item** has already written in the week — including the daily
//! ones from a moment ago. Below `minWeeklyHrs` it pays the shortfall, above
//! `maxWeeklyHrs` the negative excess, both dated the week's end.
//!
//! "This rule item" is Java object identity (`earning.getRuleItem() == ruleItem`);
//! the port compares rule item ids, which therefore must be distinct.
//!
//! Each earning is `hours` at `round_currency(homeRate × rateFactor)`, on the
//! home job of its date, noted `"Daily: "` or `"Weekly: "` plus the rule
//! description. The `ruleDescription` argument the runner passes is **ignored**;
//! the rule asks for the description itself (stood in for by the item's name).

use crate::common::enums::earning_source::EarningSource;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ADDITIONAL_HOURS, DailyWeeklyGuaranteedHoursRuleConfig, EARNING, MAX_DAILY_HRS, MAX_WEEKLY_HRS,
    MIN_DAILY_HRS, MIN_WEEKLY_HRS, RATE_FACTOR,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, rule_earning};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// `DailyWeeklyGuaranteedHoursRuleImpl.DAILY_PREFIX`.
pub const DAILY_PREFIX: &str = "Daily: ";
/// `DailyWeeklyGuaranteedHoursRuleImpl.WEEKLY_PREFIX`.
pub const WEEKLY_PREFIX: &str = "Weekly: ";

/// `DailyWeeklyGuaranteedHoursRuleImpl`.
pub struct DailyWeeklyGuaranteedHoursRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> DailyWeeklyGuaranteedHoursRule<E> {
    /// Build the rule over the earning writer. The earning type is only
    /// stored, so no earning-type port is needed.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }

    fn can_add_earning_on(time_card: &dyn TimeCard, date: LocalDate) -> bool {
        time_card.is_open_for_editing_on(date)
            && time_card.employee().is_some_and(|employee| {
                employee.is_active_on_date(date)
                    && employee.is_active_home_job_status_all_period(&DateRange::new(date, date))
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn create_and_save(
        &self,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        earning_type_id: i32,
        factor: f64,
        date: LocalDate,
        hours_owed: f64,
        prefix: &str,
    ) {
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        let mut earning = rule_earning(
            employee.id(),
            home.job_id(),
            earning_type_id,
            date,
            rule_item,
            None,
        );
        earning.set_hours(hours_owed);
        earning.set_dollars(0.0);
        earning.set_rate(round_currency(home.hourly_rate() * factor));
        earning.calc_and_set_total_dollars();
        earning.set_note(format!("{prefix}{}", rule_item.name()));
        debug_assert_eq!(earning.source(), EarningSource::Rule);

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

impl<E: EmployeeEarningPort> WeeklyEarningRule for DailyWeeklyGuaranteedHoursRule<E> {
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        _rule_description: &str,
    ) {
        let params = rule_item
            .params()
            .fixed(&DailyWeeklyGuaranteedHoursRuleConfig.default_values());
        let earning_type_id = params.int_at(EARNING);
        let (min_daily, max_daily) = (
            params.double_at(MIN_DAILY_HRS),
            params.double_at(MAX_DAILY_HRS),
        );
        let (min_weekly, max_weekly) = (
            params.double_at(MIN_WEEKLY_HRS),
            params.double_at(MAX_WEEKLY_HRS),
        );
        let factor = params.double_at(RATE_FACTOR);
        let additional_ids = ids_for_key(ADDITIONAL_HOURS, &params);

        let mut daily_hours: BTreeMap<LocalDate, f64> = BTreeMap::new();
        for shift in time_card.shifts() {
            for distribution in shift.hours_distributions() {
                if work_week.contains_date(distribution.date()) {
                    *daily_hours.entry(distribution.date()).or_insert(0.0) += distribution.hours();
                }
            }
        }
        let mut additional_hours: BTreeMap<LocalDate, f64> = BTreeMap::new();
        for earning in time_card.earnings() {
            if work_week.contains_date(earning.earning_date())
                && additional_ids.contains(&earning.earning_type_id())
            {
                *additional_hours
                    .entry(earning.earning_date())
                    .or_insert(0.0) += earning.hours();
            }
        }

        for date in work_week.dates() {
            let daily = daily_hours.get(&date).copied().unwrap_or(0.0);
            if !(Self::can_add_earning_on(time_card, date) && daily > 0.0) {
                continue;
            }
            let hours = round_hours(daily + additional_hours.get(&date).copied().unwrap_or(0.0));
            if hours < min_daily {
                let owed = round_hours(min_daily - hours);
                self.create_and_save(
                    time_card,
                    rule_item,
                    earning_type_id,
                    factor,
                    date,
                    owed,
                    DAILY_PREFIX,
                );
            }
            if hours > max_daily {
                let owed = round_hours(max_daily - hours);
                self.create_and_save(
                    time_card,
                    rule_item,
                    earning_type_id,
                    factor,
                    date,
                    owed,
                    DAILY_PREFIX,
                );
            }
        }

        let weekly_hours: f64 = daily_hours.values().sum();
        let end = work_week.end_date();
        if weekly_hours > 0.0 && Self::can_add_earning_on(time_card, end) {
            let weekly_additional: f64 = additional_hours.values().sum();
            let total = round_hours(weekly_hours + weekly_additional);

            // Hours this rule item has already written this week — the daily
            // earnings just added are included.
            let from_rule: f64 = time_card
                .earnings()
                .iter()
                .filter(|e| work_week.contains_date(e.earning_date()))
                .filter(|e| e.rule_item_id() == Some(rule_item.id()))
                .map(|e| e.hours())
                .sum();
            let total_weekly = total + from_rule;

            if total_weekly < min_weekly {
                let owed = round_hours(min_weekly - total_weekly);
                self.create_and_save(
                    time_card,
                    rule_item,
                    earning_type_id,
                    factor,
                    end,
                    owed,
                    WEEKLY_PREFIX,
                );
            }
            if total_weekly > max_weekly {
                let owed = round_hours(max_weekly - total_weekly);
                self.create_and_save(
                    time_card,
                    rule_item,
                    earning_type_id,
                    factor,
                    end,
                    owed,
                    WEEKLY_PREFIX,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;

    const JOB: i32 = 11;
    const DESC: &str = "ruleUtils description";

    fn max_date() -> LocalDate {
        LocalDate::of(9999, 12, 31)
    }

    fn today() -> LocalDate {
        LocalDate::of(2014, 5, 6)
    }

    fn week() -> DateRange {
        WeeklyDateRange::with_start_date(today())
    }

    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                today().minus_days(100),
                max_date(),
                EmployeePayType::Hourly,
                7.5,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            today().minus_days(100),
            max_date(),
            EmployeeStatusType::Active,
        )])
    }

    fn shift(day: i32) -> EmployeeShift {
        let date = today().plus_days(i64::from(day));
        EmployeeShift::new(day + 10, 1, JOB, date, ShiftType::Actual, Vec::new())
            .with_net_hours(8.0)
            .with_hours_distributions(vec![HoursDistribution::new(1, date, Some(1), 8.0, 0.0)])
    }

    fn five_shifts() -> Vec<EmployeeShift> {
        (1..=5).map(shift).collect()
    }

    fn all_shifts() -> Vec<EmployeeShift> {
        let mut shifts = five_shifts();
        shifts.push(shift(-1)); // before the week
        shifts.push(shift(7)); // after the week
        shifts
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_calculation_start_date(today())
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, DESC, RuleClass::DailyWeeklyGuaranteedHrsWdf, params)
    }

    fn run(c: &mut TimeCardData, p: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        DailyWeeklyGuaranteedHoursRule::new(saved.clone()).execute(
            c,
            &week(),
            &item(p),
            "legacy desc",
        );
        saved
    }

    /// `earningMatches`.
    fn matches(
        e: &EmployeeEarning,
        date: LocalDate,
        hours: f64,
        rate: f64,
        total: f64,
        prefix: &str,
    ) -> bool {
        e.employee_id() == 1
            && e.hours() == hours
            && e.rate() == rate
            && e.dollars() == 0.0
            && e.total_dollars() == total
            && e.earning_date() == date
            && e.pay_date() == Some(date)
            && e.earning_type_id() == 21
            && e.job_id() == JOB
            && e.source() == EarningSource::Rule
            && e.rule_item_id() == Some(1)
            && e.shift_id().is_none()
            && e.note() == format!("{prefix}{DESC}")
    }

    fn any(c: &TimeCardData, f: impl Fn(&EmployeeEarning) -> bool) -> bool {
        c.earnings().iter().any(f)
    }

    // ---- java_parity_tests: DailyWeeklyGuaranteedHoursRuleImplTest.groovy ----

    #[test]
    fn no_hours_should_result_in_no_earnings() {
        let mut c = card(Vec::new());
        let saved = run(&mut c, rule_params! { EARNING => "21" });
        assert!(c.earnings().is_empty());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn min_daily_hours() {
        let mut c = card(vec![shift(1)]);
        let saved = run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_DAILY_HRS => "10", MAX_DAILY_HRS => "99", MIN_WEEKLY_HRS => "10"
            },
        );
        assert!(any(&c, |e| matches(
            e,
            today().plus_days(1),
            2.0,
            7.5,
            15.0,
            DAILY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn max_daily_hours() {
        let mut c = card(vec![shift(2)]);
        let saved = run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_DAILY_HRS => "0", MAX_DAILY_HRS => "4.5", MIN_WEEKLY_HRS => "0"
            },
        );
        assert!(any(&c, |e| matches(
            e,
            today().plus_days(2),
            -3.5,
            7.5,
            -26.25,
            DAILY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn min_weekly_hours() {
        let mut c = card(all_shifts());
        let saved = run(
            &mut c,
            rule_params! { EARNING => "21", MIN_WEEKLY_HRS => "44" },
        );
        assert!(any(&c, |e| matches(
            e,
            week().end_date(),
            4.0,
            7.5,
            30.0,
            WEEKLY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn max_weekly_hours() {
        let mut c = card(all_shifts());
        let saved = run(
            &mut c,
            rule_params! { EARNING => "21", MIN_WEEKLY_HRS => "0", MAX_WEEKLY_HRS => "36" },
        );
        assert!(any(&c, |e| matches(
            e,
            week().end_date(),
            -4.0,
            7.5,
            -30.0,
            WEEKLY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn should_not_pay_weekly_if_terminated_before_end_of_week() {
        let mut c = card(all_shifts());
        let start = week().start_date();
        let e = employee().with_statuses(vec![
            EmployeeStatus::new(
                today().minus_days(100),
                start.plus_days(4),
                EmployeeStatusType::Active,
            ),
            EmployeeStatus::new(
                start.plus_days(5),
                max_date(),
                EmployeeStatusType::Terminated,
            ),
        ]);
        c = c.with_employee(e);
        let saved = run(
            &mut c,
            rule_params! { EARNING => "21", MIN_WEEKLY_HRS => "44", MAX_WEEKLY_HRS => "50" },
        );
        assert!(c.earnings().is_empty());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn should_account_for_rate_factor() {
        let mut c = card(all_shifts());
        let saved = run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_WEEKLY_HRS => "0", MAX_WEEKLY_HRS => "36", RATE_FACTOR => "2"
            },
        );
        assert!(any(&c, |e| matches(
            e,
            week().end_date(),
            -4.0,
            15.0,
            -60.0,
            WEEKLY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    fn additional_earnings() -> Vec<EmployeeEarning> {
        let earning = |type_id, day, hours| {
            EmployeeEarning::new(
                0,
                1,
                JOB,
                type_id,
                today().plus_days(day),
                hours,
                0.0,
                EarningSource::Auto,
            )
        };
        vec![
            earning(25, 1, 5.0),
            earning(30, 1, 7.0),
            earning(25, 2, 5.0),
        ]
    }

    #[test]
    fn should_account_for_additional_earnings_in_daily_if_worked_on_the_day() {
        let mut c = card(vec![shift(1)]).with_earnings(additional_earnings());
        let saved = run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_WEEKLY_HRS => "0", MAX_WEEKLY_HRS => "36", ADDITIONAL_HOURS => "[25]"
            },
        );
        assert_eq!(c.earnings().len(), 4);
        assert!(any(&c, |e| matches(
            e,
            today().plus_days(1),
            -5.0,
            7.5,
            -37.5,
            DAILY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn should_account_for_additional_earnings_in_weekly_if_shift_hours_in_the_week() {
        let mut c =
            card(vec![shift(2), shift(3), shift(4), shift(5)]).with_earnings(additional_earnings());
        let saved = run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_DAILY_HRS => "0", MAX_DAILY_HRS => "36", ADDITIONAL_HOURS => "[25]"
            },
        );
        assert_eq!(c.earnings().len(), 4);
        assert!(any(&c, |e| matches(
            e,
            week().end_date(),
            -2.0,
            7.5,
            -15.0,
            WEEKLY_PREFIX
        )));
        assert_eq!(saved.0.borrow().len(), 1);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_closed_day_is_skipped() {
        let mut c = card(vec![shift(1)]).with_calculation_start_date(today().plus_days(2));
        run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_DAILY_HRS => "10", MAX_DAILY_HRS => "99", MIN_WEEKLY_HRS => "0"
            },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_day_with_no_home_job_is_skipped() {
        let no_home = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                today().minus_days(100),
                max_date(),
                EmployeePayType::Hourly,
                7.5,
                false,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            today().minus_days(100),
            max_date(),
            EmployeeStatusType::Active,
        )]);
        let mut c = card(vec![shift(1)]).with_employee(no_home);
        run(
            &mut c,
            rule_params! { EARNING => "21", MIN_DAILY_HRS => "10", MAX_DAILY_HRS => "99" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn exactly_at_the_minimum_or_maximum_pays_nothing() {
        let mut c = card(vec![shift(1)]);
        run(
            &mut c,
            rule_params! { EARNING => "21", MIN_DAILY_HRS => "8", MAX_DAILY_HRS => "8", MIN_WEEKLY_HRS => "8", MAX_WEEKLY_HRS => "8" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_weekly_pass_counts_the_daily_earnings_just_written() {
        // 8h day, minDaily 10 -> +2 daily. Weekly floor 10: 8 + 2 = 10, so no
        // weekly top-up, where ignoring the daily earning would pay 2 more.
        let mut c = card(vec![shift(1)]);
        run(
            &mut c,
            rule_params! {
                EARNING => "21", MIN_DAILY_HRS => "10", MAX_DAILY_HRS => "99", MIN_WEEKLY_HRS => "10"
            },
        );
        assert_eq!(c.earnings().len(), 1);
    }
}
