//! Port of `AnniversaryWorkedShiftDifferentialRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/AnniversaryWorkedShiftDifferentialRuleImpl.java`.
//!
//! Pays a premium on a shift worked on the employee's hire-date,
//! seniority-date or birthday anniversary.
//!
//! - Each configured date type is resolved to the employee's date with the
//!   **shift date's year** substituted (a 29 February date lands on the 28th in
//!   a non-leap year, as Joda's `withYear` does); a type is *eligible* when
//!   that date equals the shift date. An employee lacking the date is not
//!   eligible for it.
//! - One earning is created per eligible occurrence, **in configured order**,
//!   and a type configured twice pays twice (unlike the daily
//!   `AnniversaryEarning`, which de-duplicates). That is the observable
//!   difference between the two rules.
//! - The only gate is `isOpenForEditingFor(shift)`: unlike its siblings this
//!   rule does not look at the shift's errors, nor at the employee's status.
//! - The rate is the shift's **job** rate (floored at minimum wage when the
//!   earning type says so). A `DOLLARS` type pays
//!   `round_currency(round_currency(rate × net hours) × factor)` dollars;
//!   otherwise `net hours` at `round_currency(rate × factor)`. The hours are the
//!   shift's stored `netHours` field, not recomputed from punches.
//! - The earning takes the id of the earning type that was **found** (the
//!   spec's DAO returns type 31 for configured id 23).
//! - The earnings are added to the time card **only**: Java never calls the
//!   earning DAO here (divergence 109 in the audit).
//! - The note is `"<description> (<date type>)"`, with the type's display name.

use crate::common::enums::anniversary_date_types::parse_anniversary_types;
use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::anniversary_earning::date_for_type;
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::shiftearning::config::{
    ANNIVERSARY_DATE_TYPE, AnniversaryWorkedShiftDifferentialRuleConfig, EARNING, EARNING_FACTOR,
};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::ports::{EarningTypePort, MinWagePort};
use crate::rules::rule_config::RuleConfig;

/// `AnniversaryWorkedShiftDifferentialRuleImpl`.
pub struct AnniversaryWorkedShiftDifferentialRule<T: EarningTypePort, M: MinWagePort> {
    earning_types: T,
    min_wage: M,
}

impl<T: EarningTypePort, M: MinWagePort> AnniversaryWorkedShiftDifferentialRule<T, M> {
    /// Build the rule over the earning-type lookup and the minimum-wage source.
    pub fn new(earning_types: T, min_wage: M) -> Self {
        Self {
            earning_types,
            min_wage,
        }
    }
}

impl<T: EarningTypePort, M: MinWagePort> ShiftEarningRule
    for AnniversaryWorkedShiftDifferentialRule<T, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !time_card.is_open_for_editing_for_shift(shift) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&AnniversaryWorkedShiftDifferentialRuleConfig.default_values());
        let Some(employee) = time_card.employee() else {
            return;
        };
        let eligible: Vec<_> =
            parse_anniversary_types(params.get(ANNIVERSARY_DATE_TYPE).unwrap_or("[]"))
                .into_iter()
                .filter(|kind| {
                    date_for_type(*kind, employee, shift.shift_date()) == Some(shift.shift_date())
                })
                .collect();
        if eligible.is_empty() {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let factor = params.double_at(EARNING_FACTOR);
        let Some(rate) = earning_rate_for_job(
            employee,
            shift.job_id(),
            shift.shift_date(),
            &earning_type,
            &self.min_wage,
        ) else {
            return;
        };

        let mut created: Vec<EmployeeEarning> = Vec::new();
        for kind in eligible {
            let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
            if earning_type.uom() == UOM::Dollars {
                earning.set_dollars(round_currency(
                    round_currency(rate * shift.net_hours()) * factor,
                ));
                earning.set_rate(0.0);
                earning.set_hours(0.0);
            } else {
                earning.set_dollars(0.0);
                earning.set_rate(round_currency(rate * factor));
                earning.set_hours(shift.net_hours());
            }
            earning.calc_and_set_total_dollars();
            earning.set_note(format!("{} ({})", rule_item.name(), kind.display_name()));
            created.push(earning);
        }
        time_card.earnings_mut().extend(created);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{FixedEarningType, FixedMinWage};
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const JOB: i32 = 19;
    const DESCRIPTION: &str = "description";

    fn day() -> LocalDate {
        LocalDate::of(2024, 2, 28)
    }
    fn found(uom: UOM) -> EarningType {
        EarningType::new(31, 1, "Anniv", EarnType::Regular, uom)
    }
    fn employee(
        dates: (Option<LocalDate>, Option<LocalDate>, Option<LocalDate>),
        rate: f64,
        date: LocalDate,
    ) -> Employee {
        Employee::new(
            1,
            99,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                date,
                date,
                EmployeePayType::Hourly,
                rate,
                true,
            )],
        )
        .with_dates(dates.0, dates.1, dates.2)
    }
    fn shift(date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new())
    }
    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, DESCRIPTION, RuleClass::AnnWrkdDif, params)
    }
    fn run(
        employee: Employee,
        shift: EmployeeShift,
        calc_start: LocalDate,
        uom: UOM,
        params: RuleParams,
    ) -> TimeCardData {
        let mut card = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift.clone()])
            .with_calculation_start_date(calc_start);
        AnniversaryWorkedShiftDifferentialRule::new(
            FixedEarningType(found(uom)),
            FixedMinWage(0.0),
        )
        .execute(&shift, &mut card, &item(params));
        card
    }

    // ---- java_parity_tests: AnniversaryWorkedShiftDifferentialRuleImplTest.groovy ----

    #[test]
    fn an_earning_is_not_generated_if_the_timecard_is_not_open_for_editing() {
        let d = day();
        let all = Some(d);
        let card = run(
            employee((all, all, all), 10.0, d),
            shift(d),
            d.plus_days(1),
            UOM::Dollars,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE,SENIORITY_DATE,BIRTHDAY]", EARNING => "23" },
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn an_earning_is_not_generated_if_no_eligible_dates_are_set_on_the_employee() {
        let d = day();
        let card = run(
            employee((None, None, None), 10.0, d),
            shift(d),
            d,
            UOM::Dollars,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE,SENIORITY_DATE,BIRTHDAY]" },
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn an_earning_is_not_generated_if_a_date_does_not_match_a_day_worked() {
        // The birthday is today but only hire and seniority (yesterday) are configured.
        let d = day();
        let yesterday = Some(d.minus_days(1).with_year(2010));
        let card = run(
            employee((yesterday, yesterday, Some(d.with_year(1986))), 10.0, d),
            shift(d),
            d,
            UOM::Dollars,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE,SENIORITY_DATE]" },
        );
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn an_earning_is_paid_for_each_configured_type_that_falls_on_the_shift_date() {
        let d = day();
        let card = run(
            employee(
                (
                    Some(d.with_year(2010)),
                    Some(d.with_year(2010)),
                    Some(d.with_year(1986)),
                ),
                0.0,
                d,
            ),
            shift(d),
            d,
            UOM::Dollars,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE,SENIORITY_DATE,BIRTHDAY]", EARNING => "23" },
        );
        assert_eq!(card.earnings().len(), 3);
    }

    #[test]
    fn the_paid_earning_has_its_fields_set() {
        let d = LocalDate::of(2014, 5, 6);
        for uom in [UOM::Dollars, UOM::Hours] {
            let s = shift(d).with_net_hours(8.0);
            let card = run(
                employee(
                    (
                        Some(d.with_year(2012)),
                        Some(d.with_year(2012)),
                        Some(d.with_year(1986)),
                    ),
                    10.0,
                    d,
                ),
                s,
                d,
                uom,
                rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE]", EARNING => "23" },
            );
            let e = &card.earnings()[0];
            assert_eq!(e.earning_type_id(), 31, "{uom:?}");
            assert_eq!(e.earning_date(), d);
            assert_eq!(e.pay_date(), Some(d));
            assert_eq!(e.employee_id(), 1);
            assert_eq!(e.job_id(), JOB);
            assert_eq!(e.source(), EarningSource::Rule);
            assert_eq!(e.note(), "description (Hire Date)");
            assert_eq!(e.rule_item_id(), Some(1));
            assert_eq!(e.shift_id(), Some(1));
            assert_eq!(
                e.dollars(),
                if uom == UOM::Dollars { 80.0 } else { 0.0 },
                "{uom:?}"
            );
            assert_eq!(
                e.rate(),
                if uom == UOM::Hours { 10.0 } else { 0.0 },
                "{uom:?}"
            );
            assert_eq!(
                e.hours(),
                if uom == UOM::Hours { 8.0 } else { 0.0 },
                "{uom:?}"
            );
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn earnings_follow_the_configured_order_and_a_repeated_type_pays_twice() {
        let d = day();
        let card = run(
            employee(
                (Some(d.with_year(2010)), None, Some(d.with_year(1986))),
                10.0,
                d,
            ),
            shift(d).with_net_hours(8.0),
            d,
            UOM::Hours,
            rule_params! {
                ANNIVERSARY_DATE_TYPE => "[BIRTHDAY,HIRE_DATE,BIRTHDAY]", EARNING => "23"
            },
        );
        let notes: Vec<&str> = card.earnings().iter().map(|e| e.note()).collect();
        assert_eq!(
            notes,
            [
                "description (Birthday)",
                "description (Hire Date)",
                "description (Birthday)"
            ]
        );
    }

    #[test]
    fn the_factor_scales_the_rate_and_the_dollars() {
        let d = day();
        let hire = Some(d.with_year(2010));
        let make = |uom| {
            run(
                employee((hire, None, None), 10.0, d),
                shift(d).with_net_hours(8.0),
                d,
                uom,
                rule_params! {
                    ANNIVERSARY_DATE_TYPE => "[HIRE_DATE]", EARNING => "23", EARNING_FACTOR => "1.5"
                },
            )
        };
        let hours = make(UOM::Hours);
        assert_eq!(
            (hours.earnings()[0].rate(), hours.earnings()[0].hours()),
            (15.0, 8.0)
        );
        assert_eq!(hours.earnings()[0].total_dollars(), 120.0);
        let dollars = make(UOM::Dollars);
        assert_eq!(dollars.earnings()[0].dollars(), 120.0);
    }

    #[test]
    fn a_leap_day_anniversary_falls_on_the_28th_in_a_common_year() {
        let leap_hire = Some(LocalDate::of(2008, 2, 29));
        let d = LocalDate::of(2023, 2, 28);
        let card = run(
            employee((leap_hire, None, None), 10.0, d),
            shift(d),
            d,
            UOM::Hours,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE]", EARNING => "23" },
        );
        assert_eq!(card.earnings().len(), 1);
    }

    #[test]
    fn a_shift_with_errors_is_still_paid() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let d = day();
        let card = run(
            employee((Some(d.with_year(2010)), None, None), 10.0, d),
            shift(d).with_errors(vec![ShiftErrorType::MissingOut]),
            d,
            UOM::Hours,
            rule_params! { ANNIVERSARY_DATE_TYPE => "[HIRE_DATE]", EARNING => "23" },
        );
        assert_eq!(card.earnings().len(), 1);
    }
}
