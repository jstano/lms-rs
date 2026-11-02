//! Port of `AnniversaryEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/AnniversaryEarningRuleImpl.java`.
//!
//! Pays on an employee's hire-date, seniority-date or birthday anniversary:
//! the configured date types are each moved to the processing date's **year**,
//! and each one that lands exactly on the processing date earns once. An
//! employee missing a date simply never matches that type.
//!
//! # One earning per matching type, in enum order
//!
//! Java gathers matches in an `EnumMap`, so they come out in declaration order
//! (hire, seniority, birthday) whatever order the rule is configured in, and a
//! type listed twice earns once. All the earnings are appended together at the
//! end, with a note `"<rule description> (<Type>)"`.
//!
//! # Hours or dollars
//!
//! A dollars-type earning (`isDollarsType`) is `round_currency(hours × factor ×
//! homeRate)` in dollars; any other is `hours` at `round_currency(factor ×
//! homeRate)`. The rate is the home job's raw hourly rate — **no minimum-wage
//! floor**, unlike its siblings.
//!
//! # February 29
//!
//! `date.withYear(y)` in Joda clamps a Feb-29 anniversary in a non-leap year to
//! Feb 28; the port does the same by hand, because `joda_rs::LocalDate::with_year`
//! panics there. So a leap-day birthday pays on Feb 28 in common years and on
//! Feb 29 in leap years.
//!
//! The earnings are **not** saved through a DAO — this rule only appends to the
//! card, so it takes no `EmployeeEarningPort`.

use crate::common::enums::anniversary_date_types::{AnniversaryDateTypes, parse_anniversary_types};
use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_currency;
use crate::entity::earning_type::EarningType;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::DailyEarningRule;
use crate::rules::algorithm::dailyearning::config::{
    ANNIVERSARY_DATE_TYPE, AnniversaryEarningRuleConfig, EARNING, EARNING_FACTOR, HOURS,
};
use crate::rules::ports::EarningTypePort;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::collections::BTreeSet;

/// `date.withYear(year)` with Joda's Feb-29 clamping.
pub(crate) fn with_year_clamped(date: LocalDate, year: i32) -> LocalDate {
    if date.month_value() == 2
        && date.day_of_month() == 29
        && !LocalDate::of(year, 1, 1).is_leap_year()
    {
        LocalDate::of(year, 2, 28)
    } else {
        date.with_year(year)
    }
}

pub(crate) fn date_for_type(
    kind: AnniversaryDateTypes,
    employee: &Employee,
    date: LocalDate,
) -> Option<LocalDate> {
    let base = match kind {
        AnniversaryDateTypes::HireDate => employee.hire_date(),
        AnniversaryDateTypes::SeniorityDate => employee.seniority_date(),
        AnniversaryDateTypes::Birthday => employee.birth_date(),
    }?;
    Some(with_year_clamped(base, date.year()))
}

/// `AnniversaryEarningRuleImpl`.
pub struct AnniversaryEarningRule<T: EarningTypePort> {
    earning_types: T,
}

impl<T: EarningTypePort> AnniversaryEarningRule<T> {
    /// Build the rule over the port that resolves its earning type.
    pub fn new(earning_types: T) -> Self {
        Self { earning_types }
    }
}

#[allow(clippy::too_many_arguments)]
fn create_earning(
    date: LocalDate,
    employee: &Employee,
    earning_type: &EarningType,
    factor: f64,
    hours: f64,
    home_rate: f64,
    home_job_id: i32,
    rule_item: &RuleItem,
    kind: AnniversaryDateTypes,
) -> EmployeeEarning {
    let mut earning = EmployeeEarning::new(
        0,
        employee.id(),
        home_job_id,
        earning_type.id(),
        date,
        0.0,
        0.0,
        EarningSource::Rule,
    )
    .from_rule(rule_item.id(), None);
    earning.set_pay_date(Some(date));
    if earning_type.is_dollars_type() {
        earning.set_dollars(round_currency(hours * factor * home_rate));
    } else {
        earning.set_hours(hours);
        earning.set_rate(round_currency(factor * home_rate));
    }
    earning.set_note(format!("{} ({})", rule_item.name(), kind.display_name()));
    earning.calc_and_set_total_dollars();
    earning
}

impl<T: EarningTypePort> DailyEarningRule for AnniversaryEarningRule<T> {
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        if !(employee.is_active_on_date(date) && time_card.is_open_for_editing_on(date)) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&AnniversaryEarningRuleConfig.default_values());
        let configured: BTreeSet<AnniversaryDateTypes> =
            parse_anniversary_types(params.get(ANNIVERSARY_DATE_TYPE).unwrap_or("[]"))
                .into_iter()
                .collect();
        let eligible: Vec<AnniversaryDateTypes> = configured
            .into_iter()
            .filter(|kind| date_for_type(*kind, employee, date) == Some(date))
            .collect();
        if eligible.is_empty() {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let factor = params.double_at(EARNING_FACTOR);
        let hours = params.double_at(HOURS);

        let earnings: Vec<EmployeeEarning> = eligible
            .into_iter()
            .map(|kind| {
                create_earning(
                    date,
                    employee,
                    &earning_type,
                    factor,
                    hours,
                    home.hourly_rate(),
                    home.job_id(),
                    rule_item,
                    kind,
                )
            })
            .collect();
        time_card.earnings_mut().extend(earnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{FixedEarningType, JOB};
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    const RATE: f64 = 10.0;
    const NOTE: &str = "Rule: Anniversary Earning - Anniversary Earning";

    fn earning_date() -> LocalDate {
        LocalDate::of(2017, 1, 1)
    }

    fn employee(
        job_start: LocalDate,
        status: EmployeeStatusType,
        dates: (Option<LocalDate>, Option<LocalDate>, Option<LocalDate>),
    ) -> Employee {
        let d = earning_date();
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                job_start,
                d.plus_years(1),
                EmployeePayType::Hourly,
                RATE,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            d.minus_days(5),
            LocalDate::of(9999, 12, 31),
            status,
        )])
        .with_dates(dates.0, dates.1, dates.2)
    }

    fn active(dates: (Option<LocalDate>, Option<LocalDate>, Option<LocalDate>)) -> Employee {
        employee(
            earning_date().minus_years(1),
            EmployeeStatusType::Active,
            dates,
        )
    }

    fn birthday(
        date: Option<LocalDate>,
    ) -> (Option<LocalDate>, Option<LocalDate>, Option<LocalDate>) {
        (None, None, date)
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, NOTE, RuleClass::AnniDdf, params)
    }

    fn rule(uom: UOM, earn_type: EarnType) -> AnniversaryEarningRule<FixedEarningType> {
        AnniversaryEarningRule::new(FixedEarningType(EarningType::new(
            1, 1, "Anniv", earn_type, uom,
        )))
    }

    fn run_on(
        employee: Employee,
        date: LocalDate,
        uom: UOM,
        earn_type: EarnType,
        params: RuleParams,
    ) -> TimeCardData {
        let mut c = TimeCardData::new().with_employee(employee);
        rule(uom, earn_type).execute(&mut c, date, &item(params));
        c
    }

    fn birthday_params() -> RuleParams {
        rule_params! { EARNING => "1", ANNIVERSARY_DATE_TYPE => "[BIRTHDAY]", EARNING_FACTOR => "1.5" }
    }

    // ---- java_parity_tests: AnniversaryEarningRuleImplTest.groovy ----

    #[test]
    fn nothing_if_the_employee_is_not_status_active_on_date() {
        let e = employee(
            earning_date().minus_years(1),
            EmployeeStatusType::Terminated,
            birthday(Some(earning_date())),
        );
        // Terminated counts on its own start date (the 27th); the 1st is later.
        let c = run_on(
            e,
            earning_date(),
            UOM::Hours,
            EarnType::Earning,
            birthday_params(),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn nothing_if_there_is_no_home_job_status_on_the_date() {
        let e = employee(
            earning_date().plus_days(1),
            EmployeeStatusType::Active,
            birthday(Some(earning_date())),
        );
        let c = run_on(
            e,
            earning_date(),
            UOM::Hours,
            EarnType::Earning,
            birthday_params(),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn creates_an_hours_earning_on_a_birthday() {
        let e = active(birthday(Some(earning_date().minus_years(3))));
        let c = run_on(
            e,
            earning_date(),
            UOM::Hours,
            EarnType::Regular,
            birthday_params(),
        );
        let r = &c.earnings()[0];
        assert_eq!(r.earning_date(), earning_date());
        assert_eq!(r.pay_date(), Some(earning_date()));
        assert_eq!(r.employee_id(), 1);
        assert_eq!(r.hours(), 8.0);
        assert_eq!(r.rate(), RATE * 1.5);
        assert_eq!(r.earning_type_id(), 1);
        assert_eq!(r.job_id(), JOB);
        assert_eq!(r.rule_item_id(), Some(1));
        assert_eq!(r.source(), EarningSource::Rule);
        assert_eq!(
            r.note(),
            "Rule: Anniversary Earning - Anniversary Earning (Birthday)"
        );
    }

    #[test]
    fn creates_a_dollars_earning_on_a_birthday() {
        let e = active(birthday(Some(earning_date())));
        let c = run_on(
            e,
            earning_date(),
            UOM::Dollars,
            EarnType::Earning,
            birthday_params(),
        );
        let r = &c.earnings()[0];
        assert_eq!(r.dollars(), 8.0 * 1.5 * RATE);
        assert_eq!(r.hours(), 0.0);
        assert_eq!(r.rate(), 0.0);
        assert_eq!(r.pay_date(), Some(earning_date()));
        assert_eq!(
            r.note(),
            "Rule: Anniversary Earning - Anniversary Earning (Birthday)"
        );
    }

    #[test]
    fn creates_several_earnings_when_several_anniversaries_coincide() {
        let d = earning_date();
        let e = active((Some(d), Some(d), Some(d)));
        let params = rule_params! {
            EARNING => "1",
            ANNIVERSARY_DATE_TYPE => "[BIRTHDAY,SENIORITY_DATE,HIRE_DATE]"
        };
        let c = run_on(e, d, UOM::Hours, EarnType::Regular, params);
        let notes: Vec<&str> = c.earnings().iter().map(|e| e.note()).collect();
        assert_eq!(
            notes,
            vec![
                "Rule: Anniversary Earning - Anniversary Earning (Hire Date)",
                "Rule: Anniversary Earning - Anniversary Earning (Seniority Date)",
                "Rule: Anniversary Earning - Anniversary Earning (Birthday)",
            ],
            "enum order, not configuration order"
        );
        for r in c.earnings() {
            assert_eq!(r.hours(), 8.0);
            assert_eq!(r.rate(), RATE);
            assert_eq!(r.job_id(), JOB);
            assert_eq!(r.source(), EarningSource::Rule);
        }
    }

    #[test]
    fn creates_nothing_when_the_employee_has_no_such_date() {
        let c = run_on(
            active(birthday(None)),
            earning_date(),
            UOM::Hours,
            EarnType::Regular,
            birthday_params(),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn creates_nothing_when_the_timecard_is_closed_for_edit() {
        let mut c = TimeCardData::new()
            .with_employee(active(birthday(Some(earning_date()))))
            .with_calculation_start_date(LocalDate::of(2017, 3, 1));
        rule(UOM::Hours, EarnType::Regular).execute(
            &mut c,
            earning_date(),
            &item(birthday_params()),
        );
        assert!(c.earnings().is_empty());
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_type_listed_twice_earns_once() {
        let params =
            rule_params! { EARNING => "1", ANNIVERSARY_DATE_TYPE => "[BIRTHDAY,BIRTHDAY]" };
        let c = run_on(
            active(birthday(Some(earning_date()))),
            earning_date(),
            UOM::Hours,
            EarnType::Regular,
            params,
        );
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn a_different_day_of_the_year_does_not_match() {
        let c = run_on(
            active(birthday(Some(earning_date().plus_days(1)))),
            earning_date(),
            UOM::Hours,
            EarnType::Regular,
            birthday_params(),
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_rate_is_not_floored_at_minimum_wage() {
        // No MinWagePort is even available to the rule; the raw job rate is used.
        let c = run_on(
            active(birthday(Some(earning_date()))),
            earning_date(),
            UOM::Hours,
            EarnType::Regular,
            rule_params! { EARNING => "1", ANNIVERSARY_DATE_TYPE => "[BIRTHDAY]" },
        );
        assert_eq!(c.earnings()[0].rate(), RATE);
    }

    #[test]
    fn a_leap_day_anniversary_falls_on_feb_28_in_a_common_year() {
        let leap_birthday = Some(LocalDate::of(2000, 2, 29));
        let d = LocalDate::of(2017, 2, 28);
        let e = employee(
            d.minus_years(1),
            EmployeeStatusType::Active,
            birthday(leap_birthday),
        )
        .with_statuses(vec![EmployeeStatus::new(
            LocalDate::of(2016, 1, 1),
            LocalDate::of(9999, 12, 31),
            EmployeeStatusType::Active,
        )]);
        let c = run_on(e, d, UOM::Hours, EarnType::Regular, birthday_params());
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn a_leap_day_anniversary_falls_on_feb_29_in_a_leap_year() {
        let d = LocalDate::of(2016, 2, 29);
        let e = employee(
            d.minus_years(1),
            EmployeeStatusType::Active,
            birthday(Some(LocalDate::of(2000, 2, 29))),
        )
        .with_statuses(vec![EmployeeStatus::new(
            LocalDate::of(2015, 1, 1),
            LocalDate::of(9999, 12, 31),
            EmployeeStatusType::Active,
        )]);
        let c = run_on(e, d, UOM::Hours, EarnType::Regular, birthday_params());
        assert_eq!(c.earnings().len(), 1);
        // ...and not on Feb 28 of that year.
    }
}
