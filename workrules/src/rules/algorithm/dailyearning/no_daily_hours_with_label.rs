//! Port of `NoDailyHoursWithLabelEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/NoDailyHoursWithLabelEarningRuleImpl.java`.
//!
//! The mirror image of the min-hours rules: pays `amount` hours at
//! `round_currency(homeRate * rateFactor)` on a day with **exactly zero**
//! distributed hours whose schedule label matches. Unlike the other two the
//! label check is not optional.
//!
//! Unlike [`MinDailyHoursEarningRule`](super::min_daily_hours::MinDailyHoursEarningRule)
//! this rule calls `calcAndSetTotalDollars` explicitly after setting the
//! hours and rate — a no-op here, since both setters already recompute.

use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    AMOUNT, EARNING, NoDailyHoursWithLabelEarningRuleConfig, RATE_FACTOR, SCHEDULE_LABEL,
};
use crate::rules::algorithm::dailyearning::{
    DailyEarningRule, earning_rate_home_job, original_hours_on_day, schedule_label_matches,
    should_not_run_for_date,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `NoDailyHoursWithLabelEarningRuleImpl`.
pub struct NoDailyHoursWithLabelEarningRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    NoDailyHoursWithLabelEarningRule<T, E, M>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> DailyEarningRule
    for NoDailyHoursWithLabelEarningRule<T, E, M>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        if should_not_run_for_date(time_card, date) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&NoDailyHoursWithLabelEarningRuleConfig.default_values());

        // `totalHoursWorkedOnDay == 0.0` — exact, so a day of 0.0001 hours
        // does not qualify.
        let total = original_hours_on_day(time_card, date);
        let label = params.get(SCHEDULE_LABEL).unwrap_or("");
        if !(total == 0.0 && schedule_label_matches(time_card, date, label)) {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(rate) = earning_rate_home_job(employee, date, &earning_type, &self.min_wage)
        else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };

        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            home.job_id(),
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
        earning.set_note(rule_item.name());
        earning.set_hours(params.double_at(AMOUNT));
        earning.set_rate(round_currency(rate * params.double_at(RATE_FACTOR)));
        earning.calc_and_set_total_dollars();

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::label_source::LabelSource;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    fn label(text: &str) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(1, today(), text, LabelSource::Manual)
    }

    fn run(card: &mut TimeCardData, min_wage: f64, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        NoDailyHoursWithLabelEarningRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            FixedMinWage(min_wage),
        )
        .execute(
            card,
            today(),
            &RuleItem::new(1, 1, "No Hours", RuleClass::NoDailyHrsDdf, params),
        );
        saved
    }

    // ---- java_parity_tests: NoDailyHoursWithLabelEarningRuleImplTest.groovy ----

    #[test]
    fn earning_is_created_on_a_day_with_a_label_match_and_no_hours() {
        let mut c = card(employee(8.0, EmployeeStatusType::Active), Vec::new())
            .with_schedule_labels(vec![label("FIXED")]);
        let saved = run(
            &mut c,
            8.0,
            rule_params! { SCHEDULE_LABEL => "FIXED", AMOUNT => "7.5", RATE_FACTOR => "2.0" },
        );
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 7.5);
        assert_eq!(e.rate(), 16.0);
        assert_eq!(e.earning_date(), today());
        assert_eq!(e.rule_item_id(), Some(1));
        assert_eq!(
            e.source(),
            crate::common::enums::earning_source::EarningSource::Rule
        );
    }

    #[test]
    fn no_earning_is_created_if_there_are_worked_hours_on_the_day() {
        let mut c = card(
            employee(8.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 8.0)],
        )
        .with_schedule_labels(vec![label("FIXED")]);
        let saved = run(
            &mut c,
            8.0,
            rule_params! { SCHEDULE_LABEL => "FIXED", AMOUNT => "7.5" },
        );
        assert!(saved.0.borrow().is_empty());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_earning_is_created_if_there_is_no_label_match() {
        let mut c = card(employee(8.0, EmployeeStatusType::Active), Vec::new())
            .with_schedule_labels(vec![label("NOMATCH")]);
        let saved = run(
            &mut c,
            8.0,
            rule_params! { SCHEDULE_LABEL => "FIXED", AMOUNT => "7.5" },
        );
        assert!(saved.0.borrow().is_empty());
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_label_comparison_ignores_case_and_surrounding_whitespace() {
        let mut c = card(employee(8.0, EmployeeStatusType::Active), Vec::new())
            .with_schedule_labels(vec![label("  fixed ")]);
        run(
            &mut c,
            8.0,
            rule_params! { SCHEDULE_LABEL => " FIXED", AMOUNT => "1", RATE_FACTOR => "1" },
        );
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn a_day_with_no_label_never_matches_a_non_empty_configured_label() {
        let mut c = card(employee(8.0, EmployeeStatusType::Active), Vec::new());
        run(&mut c, 8.0, rule_params! { SCHEDULE_LABEL => "FIXED" });
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn an_unconfigured_label_matches_a_day_with_no_label() {
        // Default scheduleLabel is "": "" equals the no-label sentinel "".
        let mut c = card(employee(8.0, EmployeeStatusType::Active), Vec::new());
        run(&mut c, 8.0, rule_params! { RATE_FACTOR => "1" });
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn a_fraction_of_an_hour_disqualifies() {
        let mut c = card(
            employee(8.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 0.0001)],
        )
        .with_schedule_labels(vec![label("FIXED")]);
        run(&mut c, 8.0, rule_params! { SCHEDULE_LABEL => "FIXED" });
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn an_inactive_employee_is_skipped() {
        let mut c = card(employee(8.0, EmployeeStatusType::Terminated), Vec::new())
            .with_schedule_labels(vec![label("FIXED")]);
        let saved = SavedEarnings::default();
        NoDailyHoursWithLabelEarningRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            FixedMinWage(8.0),
        )
        .execute(
            &mut c,
            today().plus_days(1),
            &RuleItem::new(
                1,
                1,
                "x",
                RuleClass::NoDailyHrsDdf,
                rule_params! { SCHEDULE_LABEL => "FIXED" },
            ),
        );
        assert!(saved.0.borrow().is_empty());
    }
}
