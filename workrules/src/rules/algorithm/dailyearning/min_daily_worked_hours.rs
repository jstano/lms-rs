//! Port of `MinDailyWorkedHoursEarningRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/MinDailyWorkedHoursEarningRuleImpl.java`.
//!
//! Same gate as [`MinDailyHoursEarningRule`](super::min_daily_hours::MinDailyHoursEarningRule),
//! but the earning is the **hours actually worked that day** (not a fixed
//! amount) at `round_currency(homeRate * rateFactor)`, always hours-and-rate
//! regardless of the earning type's unit.
//!
//! # It backfills from the wrong config
//!
//! `execute` calls `fixMap` on a **`MinDailyHoursEarningRuleConfig`**, not its
//! own `MinDailyWorkedHoursEarningRuleConfig`. The defaults differ where it
//! matters: a rule item with no `rateFactor` reads **`0`** here, not the `1`
//! its own config advertises, so the earning is paid at a rate of zero. This is
//! reproduced, not fixed — see the audit's `dailyearning` section.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_currency;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    EARNING, MIN_QUALIFYING_HOURS, MinDailyHoursEarningRuleConfig, RATE_FACTOR, SCHEDULE_LABEL,
    VALIDATE_SCHEDULE_LABEL,
};
use crate::rules::algorithm::dailyearning::{
    DailyEarningRule, earning_rate_home_job, original_hours_on_day, schedule_label_matches,
    should_not_run_for_date,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `MinDailyWorkedHoursEarningRuleImpl`.
pub struct MinDailyWorkedHoursEarningRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    MinDailyWorkedHoursEarningRule<T, E, M>
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
    for MinDailyWorkedHoursEarningRule<T, E, M>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        if should_not_run_for_date(time_card, date) {
            return;
        }
        // Deliberately the *other* rule's defaults — see the module doc.
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
        earning.set_rate(round_currency(rate * params.double_at(RATE_FACTOR)));
        earning.set_hours(total);

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::uom::UOM;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    fn run(card: &mut TimeCardData, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        MinDailyWorkedHoursEarningRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            FixedMinWage(10.0),
        )
        .execute(
            card,
            today(),
            &RuleItem::new(3, 1, "Worked", RuleClass::MinDailyWrkdHrsDdf, params),
        );
        saved
    }

    fn worked(hours: f64) -> TimeCardData {
        card(
            employee(10.0, EmployeeStatusType::Active),
            vec![shift_with_hours(today(), hours)],
        )
    }

    #[test]
    fn it_pays_the_hours_actually_worked_at_the_factored_rate() {
        let mut c = worked(7.5);
        let saved = run(&mut c, rule_params! { RATE_FACTOR => "1.5" });
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 7.5);
        assert_eq!(e.rate(), 15.0);
        assert_eq!(e.total_dollars(), 112.5);
        assert_eq!(saved.0.borrow().len(), 1);
    }

    /// The Java quirk: the missing `rateFactor` backfills to 0, not to this
    /// rule's own default of 1.
    #[test]
    fn an_unset_rate_factor_prices_at_zero_not_one() {
        let mut c = worked(7.5);
        run(&mut c, RuleParams::new());
        assert_eq!(c.earnings()[0].rate(), 0.0);
        assert_eq!(c.earnings()[0].hours(), 7.5);
        assert_eq!(c.earnings()[0].total_dollars(), 0.0);
    }

    #[test]
    fn under_the_qualifying_hours_pays_nothing() {
        let mut c = worked(5.0);
        run(&mut c, RuleParams::new());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_label_mismatch_pays_nothing() {
        let mut c = worked(8.0);
        run(
            &mut c,
            rule_params! { VALIDATE_SCHEDULE_LABEL => "true", SCHEDULE_LABEL => "OFF" },
        );
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn an_inactive_employee_is_skipped() {
        let mut c = card(
            employee(10.0, EmployeeStatusType::LeaveOfAbsence),
            vec![shift_with_hours(today(), 8.0)],
        );
        run(&mut c, rule_params! { RATE_FACTOR => "1" });
        assert!(c.earnings().is_empty());
    }
}
