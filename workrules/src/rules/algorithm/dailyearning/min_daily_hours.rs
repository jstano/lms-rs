//! Port of `MinDailyHoursEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/MinDailyHoursEarningRuleImpl.java`.
//!
//! Grants a fixed amount for a day on which the employee worked at least
//! `minQualifyingHours` (and more than zero), optionally only when the day's
//! schedule label matches. The day's hours are the sum of `originalHours`
//! over the hours distributions dated that day.
//!
//! The grant is `amount` hours at `round_currency(homeRate * rateFactor)` for
//! any unit but `DOLLARS`, and `amount` dollars (no hours, no rate) for
//! `DOLLARS`. It is attributed to the employee's **home** job on that date.
//!
//! The earning is appended to the card and then saved — in that order.

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    AMOUNT, EARNING, MIN_QUALIFYING_HOURS, MinDailyHoursEarningRuleConfig, RATE_FACTOR,
    SCHEDULE_LABEL, VALIDATE_SCHEDULE_LABEL,
};
use crate::rules::algorithm::dailyearning::{
    DailyEarningRule, earning_rate_home_job, original_hours_on_day, schedule_label_matches,
    should_not_run_for_date,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `MinDailyHoursEarningRuleImpl`.
pub struct MinDailyHoursEarningRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> MinDailyHoursEarningRule<T, E, M> {
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
    for MinDailyHoursEarningRule<T, E, M>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        if should_not_run_for_date(time_card, date) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&MinDailyHoursEarningRuleConfig.default_values());

        let total = original_hours_on_day(time_card, date);
        let qualifies = total > 0.0 && total >= params.double_at(MIN_QUALIFYING_HOURS);
        let label_ok = !params.bool_at(VALIDATE_SCHEDULE_LABEL)
            || schedule_label_matches(time_card, date, params.get(SCHEDULE_LABEL).unwrap_or(""));
        if !(qualifies && label_ok) {
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

        let amount = params.double_at(AMOUNT);
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
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(amount);
        } else {
            earning.set_rate(round_currency(rate * params.double_at(RATE_FACTOR)));
            earning.set_hours(amount);
        }

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::label_source::LabelSource;
    use crate::entity::employee_schedule_label::EmployeeScheduleLabel;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(7, 1, "Lieu Day", RuleClass::MinDailyHrsDdf, params)
    }

    fn run(card: &mut TimeCardData, uom: UOM, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        MinDailyHoursEarningRule::new(
            FixedEarningType(earning_type(uom)),
            saved.clone(),
            FixedMinWage(10.0),
        )
        .execute(card, today(), &item(params));
        saved
    }

    fn worked_16h(rate: f64) -> TimeCardData {
        card(
            employee(rate, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 16.0)],
        )
    }

    // ---- java_parity_tests: MinDailyHoursEarningRuleImplTest.groovy ----

    #[test]
    fn rule_is_not_run_if_employee_is_not_active_on_date() {
        let mut c = card(
            employee(10.0, EmployeeStatusType::Terminated),
            vec![shift_with_hours(today(), 16.0)],
        );
        // The Groovy is vacuous: it terminates the employee on `today` and runs
        // `today`, which `isActiveOnDate` counts as active; the card simply has
        // no shifts, so no earning follows either way. Running the day *after*
        // the termination date makes the inactivity do the work.
        let saved = SavedEarnings::default();
        MinDailyHoursEarningRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            FixedMinWage(10.0),
        )
        .execute(&mut c, today().plus_days(1), &item(RuleParams::new()));
        assert!(c.earnings().is_empty());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn earning_is_paid_over_the_threshold_with_labels_off_and_hours_uom() {
        let mut c = worked_16h(10.0);
        let saved = run(
            &mut c,
            UOM::Hours,
            rule_params! { RATE_FACTOR => "1.2", AMOUNT => "15" },
        );
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 15.0);
        assert_eq!(e.rate(), 12.0);
        assert_eq!(e.dollars(), 0.0);
        assert_eq!(e.earning_date(), today());
        assert_eq!(e.job_id(), JOB);
        assert_eq!(e.employee_id(), 1);
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn earning_is_paid_over_the_threshold_with_labels_off_and_dollars_uom() {
        let mut c = worked_16h(10.0);
        run(
            &mut c,
            UOM::Dollars,
            rule_params! { RATE_FACTOR => "1.2", AMOUNT => "15" },
        );
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 0.0);
        assert_eq!(e.rate(), 0.0);
        assert_eq!(e.dollars(), 15.0);
        assert_eq!(e.total_dollars(), 15.0);
    }

    fn label(text: &str) -> EmployeeScheduleLabel {
        EmployeeScheduleLabel::new(1, today(), text, LabelSource::Manual)
    }

    #[test]
    fn earning_is_not_paid_if_validate_labels_is_on_and_there_is_no_match() {
        let mut c = worked_16h(10.0).with_schedule_labels(vec![label("THE LABEL")]);
        run(
            &mut c,
            UOM::Hours,
            rule_params! { VALIDATE_SCHEDULE_LABEL => "true", SCHEDULE_LABEL => "Never Match" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn earning_is_paid_if_there_is_a_label_match_on_the_day() {
        let mut c = worked_16h(10.0).with_schedule_labels(vec![label("MATCH")]);
        run(
            &mut c,
            UOM::Hours,
            rule_params! {
                VALIDATE_SCHEDULE_LABEL => "true", SCHEDULE_LABEL => "Match",
                AMOUNT => "15", RATE_FACTOR => "1.2"
            },
        );
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 15.0);
        assert_eq!(e.rate(), 12.0);
        assert_eq!(e.dollars(), 0.0);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn under_the_qualifying_threshold_pays_nothing() {
        // default minQualifyingHours is 6.
        let mut c = card(
            employee(10.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 5.99)],
        );
        run(&mut c, UOM::Hours, RuleParams::new());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn exactly_the_qualifying_threshold_pays() {
        let mut c = card(
            employee(10.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), 6.0)],
        );
        run(&mut c, UOM::Hours, RuleParams::new());
        assert_eq!(c.earnings().len(), 1);
    }

    #[test]
    fn a_zero_hour_day_pays_nothing_even_with_a_zero_threshold() {
        let mut c = card(employee(10.0, EmployeeStatusType::Active), Vec::new());
        run(
            &mut c,
            UOM::Hours,
            rule_params! { MIN_QUALIFYING_HOURS => "0" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn only_distributions_dated_that_day_count() {
        let mut c = card(
            employee(10.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today().plus_days(1), 16.0)],
        );
        run(&mut c, UOM::Hours, RuleParams::new());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_closed_period_pays_nothing() {
        let mut c = worked_16h(10.0).with_calculation_start_date(today().plus_days(1));
        run(&mut c, UOM::Hours, RuleParams::new());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_default_rate_factor_is_zero_so_the_rate_is_zero() {
        let mut c = worked_16h(10.0);
        run(&mut c, UOM::Hours, rule_params! { AMOUNT => "4" });
        assert_eq!(c.earnings()[0].rate(), 0.0);
        assert_eq!(c.earnings()[0].hours(), 4.0);
    }

    #[test]
    fn the_rate_is_floored_at_minimum_wage_when_the_earning_type_says_so() {
        let mut c = worked_16h(8.0);
        let saved = SavedEarnings::default();
        let pays_min = earning_type(UOM::Hours).with_flags(true, false, false, false);
        MinDailyHoursEarningRule::new(FixedEarningType(pays_min), saved, FixedMinWage(10.0))
            .execute(
                &mut c,
                today(),
                &item(rule_params! { AMOUNT => "1", RATE_FACTOR => "1" }),
            );
        assert_eq!(c.earnings()[0].rate(), 10.0);
    }

    #[test]
    fn no_home_job_pays_nothing() {
        let mut c = TimeCardData::new()
            .with_employee(
                crate::entity::employee::Employee::new(1, 1, "Pat", Vec::new()).with_statuses(
                    vec![crate::entity::employee_status::EmployeeStatus::new(
                        today(),
                        today(),
                        EmployeeStatusType::Active,
                    )],
                ),
            )
            .with_shifts(vec![shift_with_hours(today(), 16.0)]);
        run(&mut c, UOM::Hours, RuleParams::new());
        assert!(c.earnings().is_empty());
    }
}
