//! Port of `MinimumHoursWorkedShiftDifferentialRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/MinimumHoursWorkedShiftDifferentialRuleImpl.java`.
//!
//! The per-shift sibling of `dailyearning`'s `MinimumHoursWorkedDifferential`:
//! tops a short day up to `minDailyHrs`, but is driven by a shift and priced off
//! **that shift**.
//!
//! - It runs only for the day's **latest** shift (by start time; ties keep
//!   the listed order, and the last of them is "latest"), so the day is paid
//!   once. It also needs the employee active that day and the shift's period
//!   open.
//! - The day's hours are the same helper as the daily rule: the net hours of the
//!   day's error-free shifts. It pays the shortfall when that is above zero
//!   and under `minDailyHrs` — there is no `minQualifyingHours` here.
//! - The rate is the shift's job's rate, or with `MinimumWage` on the minimum
//!   wage for that job and date; × `rateFactor`. Hours / dollars / rate split
//!   by the earning type's unit exactly as in the daily rule.
//!
//! # Backfilled from the *daily* rule's config
//!
//! Java calls `fixMap` on a `MinimumHoursWorkedDifferentialRuleConfig` (the
//! `dailyearning` one) rather than this rule's own config. The defaults overlap
//! except `MinimumWage`, which the daily config lacks — so a missing
//! `MinimumWage` reads as null → `false` instead of the `"false"` the rule's
//! own config would supply. The result is the same; the port uses the daily
//! defaults to stay faithful.

use crate::common::enums::uom::UOM;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    MinimumHoursWorkedDifferentialRuleConfig, RATE_FACTOR,
};
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::dailyearning::minimum_hours_worked::total_hours_worked_on_day;
use crate::rules::algorithm::shiftearning::config::{EARNING, MIN_DAILY_HRS, MINIMUM_WAGE_PROP};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `MinimumHoursWorkedShiftDifferentialRuleImpl`.
pub struct MinimumHoursWorkedShiftDifferentialRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    MinimumHoursWorkedShiftDifferentialRule<T, E, M>
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

/// `isLatestShiftWorked`: the shift is the last of the day's shifts when sorted
/// by start time (stable). Shifts are matched by id.
fn is_latest_shift_worked(time_card: &dyn TimeCard, shift: &EmployeeShift) -> bool {
    let mut day = time_card.shifts_for_day(shift.shift_date());
    day.sort_by_key(|s| s.start_time_for_ordering());
    day.last().is_some_and(|last| last.id() == shift.id())
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftEarningRule
    for MinimumHoursWorkedShiftDifferentialRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let date = shift.shift_date();
        let active = time_card
            .employee()
            .is_some_and(|e| e.is_active_on_date(date));
        if !(active
            && time_card.is_open_for_editing_for_shift(shift)
            && is_latest_shift_worked(time_card, shift))
        {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&MinimumHoursWorkedDifferentialRuleConfig.default_values());
        let minimum_daily_hours = params.double_at(MIN_DAILY_HRS);
        let total = total_hours_worked_on_day(time_card, date);
        if !(total > 0.0 && total < minimum_daily_hours) {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let earning_rate = if params.bool_at(MINIMUM_WAGE_PROP) {
            Some(
                self.min_wage
                    .min_wage(employee.property_id(), Some(shift.job_id()), date),
            )
        } else {
            earning_rate_for_job(
                employee,
                shift.job_id(),
                date,
                &earning_type,
                &self.min_wage,
            )
        };
        let Some(earning_rate) = earning_rate else {
            return;
        };

        let shortfall = minimum_daily_hours - total;
        let factor = params.double_at(RATE_FACTOR);
        let uom = earning_type.uom();

        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, false);
        earning.set_note(rule_item.name());
        earning.set_hours(if uom == UOM::Dollars { 0.0 } else { shortfall });
        earning.set_dollars(if uom == UOM::Hours {
            0.0
        } else {
            round_currency(shortfall * earning_rate * factor)
        });
        earning.set_rate(if uom == UOM::Dollars {
            0.0
        } else {
            round_hours(earning_rate * factor)
        });

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const HOME_JOB: i32 = 1;
    const RECENT_JOB: i32 = 2;
    const MIN_WAGE_JOB: i32 = 3;
    const NOTE: &str = "Rule: testSet - Minimum Hours Worked";

    fn date() -> LocalDate {
        LocalDate::of(2015, 2, 17)
    }

    fn shift(id: i32, hours: i64, job: i32) -> EmployeeShift {
        let t = date().at_start_of_day().plus_hours(hours);
        EmployeeShift::new(id, 1, job, date(), ShiftType::Actual, Vec::new())
            .with_times(Some(t), Some(t))
            .with_net_hours(hours as f64)
    }

    fn employee(home_job: i32) -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                home_job,
                date().minus_years(1),
                date().plus_years(1),
                EmployeePayType::Hourly,
                10.5,
                true,
            )],
        )
        .with_statuses(vec![EmployeeStatus::new(
            date().minus_years(1),
            date().plus_years(1),
            EmployeeStatusType::Active,
        )])
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(5, 1, NOTE, RuleClass::MinimumHoursWorkedShiftDif, params)
    }

    fn run(
        s: &EmployeeShift,
        card: &mut TimeCardData,
        uom: UOM,
        min_wage: f64,
        p: RuleParams,
    ) -> SavedEarnings {
        let saved = SavedEarnings::default();
        MinimumHoursWorkedShiftDifferentialRule::new(
            FixedEarningType(earning_type(uom)),
            saved.clone(),
            FixedMinWage(min_wage),
        )
        .execute(s, card, &item(p));
        saved
    }

    fn pay_params() -> RuleParams {
        rule_params! { MIN_DAILY_HRS => "6", EARNING => "1", RATE_FACTOR => "1" }
    }

    // ---- java_parity_tests: MinimumHoursWorkedShiftDifferentialRuleImplTest.groovy ----

    #[test]
    fn someone_inactive_on_the_date_is_not_paid() {
        let s = shift(1, 3, HOME_JOB);
        let mut c = TimeCardData::new()
            .with_employee(Employee::new(1, 1, "Pat", Vec::new()))
            .with_shifts(vec![s.clone()]);
        let saved = run(&s, &mut c, UOM::Hours, 0.0, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn shifts_outside_the_calculation_period_are_not_paid() {
        let s = shift(1, 3, HOME_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(date().plus_days(1));
        let saved = run(&s, &mut c, UOM::Hours, 0.0, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_shift_that_is_not_the_latest_of_the_day_is_not_paid() {
        let early = shift(1, 2, HOME_JOB);
        let late = shift(2, 3, RECENT_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![early.clone(), late]);
        let saved = run(&early, &mut c, UOM::Hours, 0.0, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn someone_who_did_not_work_hours_is_not_paid() {
        let s = shift(1, 0, HOME_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![s.clone()]);
        let saved = run(&s, &mut c, UOM::Hours, 0.0, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn someone_who_worked_more_than_the_minimum_is_not_paid() {
        let s = shift(1, 10, HOME_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![s.clone()]);
        let saved = run(
            &s,
            &mut c,
            UOM::Hours,
            0.0,
            rule_params! { MIN_DAILY_HRS => "5" },
        );
        assert!(saved.0.borrow().is_empty());
    }

    fn two_shift_card(home: i32) -> (EmployeeShift, TimeCardData) {
        let early = shift(1, 2, HOME_JOB);
        let late = shift(2, 3, home);
        let c = TimeCardData::new()
            .with_employee(employee(home))
            .with_shifts(vec![early, late.clone()]);
        (late, c)
    }

    fn verify(c: &TimeCardData, s: &EmployeeShift, dollars: f64, hours: f64, rate: f64) {
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), date());
        assert_eq!(e.pay_date(), Some(date()));
        assert_eq!(e.source(), EarningSource::Rule);
        assert_eq!(e.earning_type_id(), 0);
        assert_eq!(e.rule_item_id(), Some(5));
        assert_eq!(e.note(), NOTE);
        assert_eq!(e.employee_id(), 1);
        assert_eq!(e.job_id(), s.job_id());
        assert_eq!((e.dollars(), e.hours(), e.rate()), (dollars, hours, rate));
        assert_eq!(e.total_dollars(), dollars + hours * rate);
    }

    #[test]
    fn worked_but_under_the_minimum_gets_an_earning_for_the_correct_job() {
        for (uom, dollars, hours, rate) in
            [(UOM::Dollars, 10.5, 0.0, 0.0), (UOM::Hours, 0.0, 1.0, 10.5)]
        {
            let (late, mut c) = two_shift_card(RECENT_JOB);
            let saved = run(&late, &mut c, uom, 0.0, pay_params());
            assert_eq!(saved.0.borrow().len(), 1);
            verify(&c, &late, dollars, hours, rate);
        }
    }

    #[test]
    fn at_minimum_wage_when_the_minimum_wage_flag_is_true() {
        for (uom, dollars, hours, rate) in
            [(UOM::Dollars, 15.0, 0.0, 0.0), (UOM::Hours, 0.0, 1.0, 15.0)]
        {
            let early = shift(1, 2, HOME_JOB);
            let late = shift(2, 3, MIN_WAGE_JOB);
            let mut c = TimeCardData::new()
                .with_employee(employee(MIN_WAGE_JOB))
                .with_shifts(vec![early, late.clone()]);
            let mut p = pay_params();
            p.set(MINIMUM_WAGE_PROP, "true");
            let saved = run(&late, &mut c, uom, 15.0, p);
            assert_eq!(saved.0.borrow().len(), 1);
            let e = &c.earnings()[0];
            assert_eq!(
                (e.dollars(), e.hours(), e.rate()),
                (dollars, hours, rate),
                "{uom:?}"
            );
            assert_eq!(e.job_id(), MIN_WAGE_JOB);
        }
    }

    #[test]
    fn at_the_employee_rate_when_the_minimum_wage_flag_is_false() {
        for (uom, dollars, hours, rate) in
            [(UOM::Dollars, 10.5, 0.0, 0.0), (UOM::Hours, 0.0, 1.0, 10.5)]
        {
            let (late, mut c) = two_shift_card(RECENT_JOB);
            let mut p = pay_params();
            p.set(MINIMUM_WAGE_PROP, "false");
            run(&late, &mut c, uom, 15.0, p);
            verify(&c, &late, dollars, hours, rate);
        }
    }

    #[test]
    fn minimum_wage_earning_uses_the_rate_factor() {
        let late = shift(2, 2, MIN_WAGE_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(MIN_WAGE_JOB))
            .with_shifts(vec![late.clone()]);
        let mut p = rule_params! { MIN_DAILY_HRS => "6", EARNING => "1", RATE_FACTOR => "1.5" };
        p.set(MINIMUM_WAGE_PROP, "true");
        let saved = run(&late, &mut c, UOM::Hours, 15.0, p);
        let e = &saved.0.borrow()[0];
        assert_eq!(
            (e.hours(), e.rate(), e.dollars()),
            (4.0, round_hours(15.0 * 1.5), 0.0)
        );
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn shifts_with_errors_do_not_count_toward_the_day() {
        let bad = shift(1, 2, HOME_JOB).with_errors(vec![ShiftErrorType::MissingOut]);
        let late = shift(2, 3, RECENT_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![bad, late.clone()]);
        run(&late, &mut c, UOM::Hours, 0.0, pay_params());
        assert_eq!(
            c.earnings()[0].hours(),
            3.0,
            "6 - 3, the errored 2h is not counted"
        );
    }

    #[test]
    fn exactly_the_minimum_is_not_paid() {
        let s = shift(1, 6, RECENT_JOB);
        let mut c = TimeCardData::new()
            .with_employee(employee(RECENT_JOB))
            .with_shifts(vec![s.clone()]);
        let saved = run(&s, &mut c, UOM::Hours, 0.0, pay_params());
        assert!(saved.0.borrow().is_empty());
    }
}
