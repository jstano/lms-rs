//! Port of `DOWRateFactorRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/DOWRateFactorRuleImpl.java`.
//!
//! On a shift whose date falls on the configured weekday (database numbering,
//! Sunday = 1), pays the shift's hours at `rate × factor`, where `rate` is the
//! **home** job's rate (`homeAsBase`) or the shift's own job's.
//!
//! The hours are `ShiftUtil.getNetHoursWithReallocatedToEarningAdjustments`:
//! hours already turned into an earning by a reallocation are added back (a
//! reallocation of `-2` makes 8 net hours price as 10). A negative result
//! pays nothing.
//!
//! For a `DOLLARS` earning type: `round(round(rate × hours, 2) × factor, 2)`
//! dollars with no hours or rate. Otherwise `hours` at
//! `round_currency(rate × factor)`.

use crate::common::dates::translate_dow_to_iso;
use crate::common::enums::uom::UOM;
use crate::common::numbers::{round_currency, round_to};
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::{earning_rate_for_job, earning_rate_home_job};
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DOW_PROP, DOWRateFactorRuleConfig, EARNING, FACTOR_PROP, HOME_AS_BASE_PROP,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `DOWRateFactorRuleImpl`.
pub struct DOWRateFactorRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> DOWRateFactorRule<T, E, M> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftEarningRule
    for DOWRateFactorRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&DOWRateFactorRuleConfig.default_values());
        let factor = params.double_at(FACTOR_PROP);
        let dow = params.int_at(DOW_PROP);

        if shift.shift_date().day_of_week().value() != translate_dow_to_iso(dow) {
            return;
        }
        let shift_hours =
            shift.net_hours_with_reallocated_undone(params.bool_at(ACTUAL_PUNCH_PROP));
        if shift_hours < 0.0 {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let rate = if params.bool_at(HOME_AS_BASE_PROP) {
            earning_rate_home_job(employee, shift.shift_date(), &earning_type, &self.min_wage)
        } else {
            earning_rate_for_job(
                employee,
                shift.job_id(),
                shift.shift_date(),
                &earning_type,
                &self.min_wage,
            )
        };
        let Some(rate) = rate else {
            return;
        };

        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(round_to(round_to(rate * shift_hours, 2) * factor, 2));
            earning.set_rate(0.0);
            earning.set_hours(0.0);
        } else {
            earning.set_dollars(0.0);
            earning.set_rate(round_currency(rate * factor));
            earning.set_hours(shift_hours);
        }
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const NOTE: &str = "Rule: ruleSet - Day of Week Rate Factor Earning";
    const JOB: i32 = 12;

    /// 2013-09-05 is a Thursday: database weekday 5.
    fn date() -> LocalDate {
        LocalDate::of(2013, 9, 5)
    }

    fn shift() -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, date(), ShiftType::Actual, Vec::new())
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, NOTE, RuleClass::DowFactorDif, params)
    }

    // ---- java_parity_tests: DOWRateFactorRuleImplTest.groovy ----

    #[test]
    fn execute_does_not_run_because_of_invalid_shift() {
        let valid_negative = shift().with_net_hours(-1.0);
        let with_errors = shift()
            .with_errors(vec![ShiftErrorType::MissingOut])
            .with_net_hours(8.0);
        // (shift, calculation start date, dow)
        let rows = [
            (with_errors, date(), "0"),
            (shift().with_net_hours(-1.0), date().plus_days(1), "0"),
            (valid_negative.clone(), date(), "0"),
            (valid_negative, date(), "5"),
        ];
        for (s, calc_start, dow) in rows {
            let mut card = TimeCardData::new()
                .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
                .with_shifts(vec![s.clone()])
                .with_calculation_start_date(calc_start);
            let saved = SavedEarnings::default();
            DOWRateFactorRule::new(
                FixedEarningType(earning_type(UOM::Hours)),
                saved.clone(),
                FixedMinWage(0.0),
            )
            .execute(
                &s,
                &mut card,
                &item(rule_params! {
                    EARNING => "0", FACTOR_PROP => "1", DOW_PROP => dow,
                    ACTUAL_PUNCH_PROP => "false", HOME_AS_BASE_PROP => "true"
                }),
            );
            assert!(saved.0.borrow().is_empty(), "dow {dow}");
        }
    }

    #[test]
    fn execute_with_valid_shift() {
        // (uom, homeAsBase, reallocated adjHours, dollars, hours, rate)
        let rows = [
            (UOM::Dollars, "true", 0.0, 64.0, 0.0, 0.0),
            (UOM::Hours, "false", 0.0, 0.0, 8.0, 8.0),
            (UOM::Hours, "false", -2.0, 0.0, 10.0, 8.0),
        ];
        for (uom, home_as_base, adj, dollars, hours, rate) in rows {
            let s = shift()
                .with_net_hours(8.0)
                .with_reallocated_to_earning_hours(adj);
            let employee = Employee::new(
                1,
                1,
                "Pat",
                vec![EmployeeJobStatus::new(
                    1,
                    1,
                    JOB,
                    date(),
                    LocalDate::of(9999, 12, 31),
                    EmployeePayType::Hourly,
                    8.0,
                    true,
                )],
            );
            let mut card = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![s.clone()])
                .with_calculation_start_date(date());
            let saved = SavedEarnings::default();
            DOWRateFactorRule::new(
                FixedEarningType(earning_type(uom)),
                saved.clone(),
                FixedMinWage(0.0),
            )
            .execute(
                &s,
                &mut card,
                &item(rule_params! {
                    EARNING => "0", FACTOR_PROP => "1", DOW_PROP => "5",
                    ACTUAL_PUNCH_PROP => "false", HOME_AS_BASE_PROP => home_as_base
                }),
            );
            assert_eq!(saved.0.borrow().len(), 1, "{uom:?} {adj}");
            let e = &card.earnings()[0];
            assert_eq!(e.earning_date(), date());
            assert_eq!(e.pay_date(), Some(date()));
            assert_eq!(e.employee_id(), 1);
            assert_eq!(e.job_id(), JOB);
            assert_eq!(e.source(), EarningSource::Rule);
            assert_eq!(e.note(), NOTE);
            assert_eq!(e.rule_item_id(), Some(1));
            assert_eq!(e.shift_id(), Some(1));
            assert_eq!(
                (e.dollars(), e.hours(), e.rate()),
                (dollars, hours, rate),
                "{uom:?} {adj}"
            );
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_factor_scales_the_rate_and_the_dollars() {
        let s = shift().with_net_hours(8.0);
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                date(),
                LocalDate::of(9999, 12, 31),
                EmployeePayType::Hourly,
                8.0,
                true,
            )],
        );
        let run = |uom, factor: &str| {
            let mut c = TimeCardData::new()
                .with_employee(employee.clone())
                .with_shifts(vec![s.clone()]);
            DOWRateFactorRule::new(FixedEarningType(earning_type(uom)), SavedEarnings::default(), FixedMinWage(0.0))
                .execute(&s, &mut c, &item(rule_params! {
                    EARNING => "1", FACTOR_PROP => factor, DOW_PROP => "5", ACTUAL_PUNCH_PROP => "false"
                }));
            c
        };
        let hours = run(UOM::Hours, "1.5");
        assert_eq!(
            (hours.earnings()[0].rate(), hours.earnings()[0].hours()),
            (12.0, 8.0)
        );
        let dollars = run(UOM::Dollars, "1.5");
        assert_eq!(dollars.earnings()[0].dollars(), 96.0);
    }

    #[test]
    fn the_shifts_own_job_is_priced_when_home_as_base_is_off() {
        let other_job = 77;
        let s = EmployeeShift::new(1, 1, other_job, date(), ShiftType::Actual, Vec::new())
            .with_net_hours(2.0);
        let status = |job, rate, home| {
            EmployeeJobStatus::new(
                1,
                1,
                job,
                date(),
                LocalDate::of(9999, 12, 31),
                EmployeePayType::Hourly,
                rate,
                home,
            )
        };
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![status(JOB, 8.0, true), status(other_job, 20.0, false)],
        );
        let mut c = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![s.clone()]);
        DOWRateFactorRule::new(FixedEarningType(earning_type(UOM::Hours)), SavedEarnings::default(), FixedMinWage(0.0))
            .execute(&s, &mut c, &item(rule_params! {
                EARNING => "1", FACTOR_PROP => "1", DOW_PROP => "5", ACTUAL_PUNCH_PROP => "false", HOME_AS_BASE_PROP => "false"
            }));
        assert_eq!(c.earnings()[0].rate(), 20.0);
        assert_eq!(c.earnings()[0].job_id(), other_job);
    }
}
