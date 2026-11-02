//! Port of `MinimumHoursWorkedDifferentialRuleImpl` and the
//! `MinimumHoursWorkedRuleHelper` it leans on.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/MinimumHoursWorkedDifferentialRuleImpl.java`
//! and `.../algorithm/MinimumHoursWorkedRuleHelper.java`.
//!
//! Tops a short day up to `minDailyHrs`: when the employee worked more than
//! zero, fewer than `minDailyHrs`, and at least `minQualifyingHours`, it pays
//! the **shortfall** (`minDailyHrs - worked`).
//!
//! The day's hours are the `netHours` of that date's shifts that have no
//! persisted errors — unlike the min-hours rules, which sum distributions.
//!
//! # Priced off the "primary" shift
//!
//! The earning takes its job and rate from the day's *primary* shift: among
//! the day's shifts that have an OUT punch, the one whose OUT is latest; if
//! none has an OUT punch, the first shift. It is **not** the home job — the
//! spec's "punched job not the home job" case pins that. On a tie for latest
//! OUT, Java's `Stream.max` keeps the **first**.
//!
//! The shortfall becomes hours, dollars or both by the earning type's unit:
//! hours are zero for `DOLLARS`; dollars are zero for `HOURS`
//! (`round_currency(shortfall × rate × factor)` otherwise); the stored rate is
//! zero for `DOLLARS` and `round_hours(rate × factor)` otherwise.

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::uom::UOM;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    EARNING, MIN_DAILY_HRS, MIN_QUALIFYING_HOURS, MinimumHoursWorkedDifferentialRuleConfig,
    RATE_FACTOR,
};
use crate::rules::algorithm::dailyearning::{
    DailyEarningRule, earning_rate_for_shift, should_not_run_for_date,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `MinimumHoursWorkedRuleHelper.getTotalHoursWorkedOnDay`: net hours of the
/// day's shifts with no persisted errors.
pub fn total_hours_worked_on_day(time_card: &dyn TimeCard, date: LocalDate) -> f64 {
    time_card
        .shifts_for_day(date)
        .iter()
        .filter(|shift| !shift.has_errors())
        .map(|shift| shift.net_hours())
        .sum()
}

/// The shift the earning is priced off. See the module doc.
fn primary_shift<'a>(shifts: &[&'a EmployeeShift]) -> Option<&'a EmployeeShift> {
    let mut best: Option<(&'a EmployeeShift, joda_rs::LocalDateTime)> = None;
    for shift in shifts {
        let Some(out) = shift
            .punch_index_of_type(PunchType::Out)
            .and_then(|i| shift.punch(i).rounded_time())
        else {
            continue;
        };
        // Strictly later replaces; a tie keeps the earlier-listed shift.
        if best.is_none_or(|(_, latest)| out > latest) {
            best = Some((shift, out));
        }
    }
    best.map(|(shift, _)| shift)
        .or_else(|| shifts.first().copied())
}

/// `MinimumHoursWorkedDifferentialRuleImpl`.
pub struct MinimumHoursWorkedDifferentialRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    MinimumHoursWorkedDifferentialRule<T, E, M>
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
    for MinimumHoursWorkedDifferentialRule<T, E, M>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        if should_not_run_for_date(time_card, date) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&MinimumHoursWorkedDifferentialRuleConfig.default_values());

        let minimum_daily_hours = params.double_at(MIN_DAILY_HRS);
        let minimum_qualifying_hours = params.double_at(MIN_QUALIFYING_HOURS);
        let total = total_hours_worked_on_day(time_card, date);
        if !(total > 0.0 && total < minimum_daily_hours && total >= minimum_qualifying_hours) {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let day_shifts = time_card.shifts_for_day(date);
        let Some(primary) = primary_shift(&day_shifts) else {
            return;
        };
        let Some(earning_rate) =
            earning_rate_for_shift(employee, primary, &earning_type, &self.min_wage)
        else {
            return;
        };

        let factor = params.double_at(RATE_FACTOR);
        let shortfall = minimum_daily_hours - total;
        let uom = earning_type.uom();

        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            primary.job_id(),
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), None);
        earning.set_pay_date(Some(date));
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
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    const HOME_JOB: i32 = 0;
    const PUNCHED_JOB: i32 = 99;

    fn midnight(date: LocalDate) -> LocalDateTime {
        date.at_start_of_day()
    }

    /// `getShiftOnDateWithHoursAndJob`: a shift starting at midnight with an
    /// OUT punch `hours` later and `netHours = hours`.
    fn shift(date: LocalDate, hours: f64, job: i32) -> EmployeeShift {
        let out = midnight(date).plus_hours(hours as i64);
        EmployeeShift::new(
            1,
            1,
            job,
            date,
            ShiftType::Actual,
            vec![EmployeeShiftPunch::new(
                1,
                PunchType::Out,
                PunchSource::Clock,
                out,
            )],
        )
        .with_times(Some(midnight(date)), Some(out))
        .with_net_hours(hours)
    }

    fn employee_with_two_jobs() -> Employee {
        let d = today();
        let status = |id, job, home| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                d.minus_years(1),
                d.plus_years(1),
                EmployeePayType::Hourly,
                10.0,
                home,
            )
        };
        Employee::new(
            1,
            1,
            "Pat",
            vec![status(1, HOME_JOB, true), status(2, PUNCHED_JOB, false)],
        )
        .with_statuses(vec![EmployeeStatus::new(
            d.minus_years(1),
            d.plus_years(1),
            EmployeeStatusType::Active,
        )])
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(5, 1, "testSet", RuleClass::MinHoursWorkedDdf, params)
    }

    fn run(card: &mut TimeCardData, uom: UOM, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        MinimumHoursWorkedDifferentialRule::new(
            FixedEarningType(earning_type(uom)),
            saved.clone(),
            FixedMinWage(10.0),
        )
        .execute(card, today(), &item(params));
        saved
    }

    // ---- java_parity_tests: MinimumHoursWorkedDifferentialRuleImplTest.groovy ----

    #[test]
    fn someone_inactive_on_the_date_is_not_paid() {
        let inactive = Employee::new(1, 1, "Pat", Vec::new());
        let mut c = card(inactive, vec![shift(today(), 2.0, HOME_JOB)]);
        let saved = run(&mut c, UOM::Hours, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn shifts_outside_the_calculation_period_are_not_paid() {
        let date = today().minus_days(5);
        let mut c = card(employee_with_two_jobs(), vec![shift(date, 2.0, HOME_JOB)])
            .with_calculation_start_date(today());
        let saved = SavedEarnings::default();
        MinimumHoursWorkedDifferentialRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            FixedMinWage(10.0),
        )
        .execute(&mut c, date, &item(RuleParams::new()));
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn someone_who_did_not_work_is_not_paid() {
        let mut c = card(employee_with_two_jobs(), Vec::new());
        let saved = run(&mut c, UOM::Hours, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn someone_who_worked_more_than_the_minimum_is_not_paid() {
        let mut c = card(
            employee_with_two_jobs(),
            vec![shift(today(), 10.0, HOME_JOB)],
        );
        let saved = run(&mut c, UOM::Hours, rule_params! { MIN_DAILY_HRS => "5" });
        assert!(saved.0.borrow().is_empty());
    }

    fn assert_shortfall_earning(uom: UOM, dollars: f64, hours: f64, rate: f64) {
        let mut c = card(
            employee_with_two_jobs(),
            vec![shift(today(), 4.0, PUNCHED_JOB)],
        );
        let saved = run(
            &mut c,
            uom,
            rule_params! { MIN_DAILY_HRS => "6", EARNING => "1" },
        );
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), today());
        assert_eq!(e.pay_date(), Some(today()));
        assert_eq!(e.job_id(), PUNCHED_JOB, "the punched job, not the home job");
        assert_eq!(e.note(), "testSet");
        assert_eq!(e.dollars(), dollars);
        assert_eq!(e.hours(), hours);
        assert_eq!(e.rate(), rate);
        assert_eq!(e.total_dollars(), dollars + hours * rate);
    }

    #[test]
    fn someone_who_worked_but_not_the_minimum_is_paid_with_dollars_uom() {
        assert_shortfall_earning(UOM::Dollars, 20.0, 0.0, 0.0);
    }

    #[test]
    fn someone_who_worked_but_not_the_minimum_is_paid_with_hours_uom() {
        assert_shortfall_earning(UOM::Hours, 0.0, 2.0, 10.0);
    }

    #[test]
    fn must_work_over_the_minimum_qualifying_hours() {
        let mut c = card(
            employee_with_two_jobs(),
            vec![shift(today(), 3.0, HOME_JOB)],
        );
        run(
            &mut c,
            UOM::Hours,
            rule_params! { MIN_DAILY_HRS => "6", EARNING => "1", MIN_QUALIFYING_HOURS => "4" },
        );
        assert!(c.earnings().is_empty());
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_shift_with_errors_does_not_count_toward_the_day() {
        let bad = shift(today(), 4.0, HOME_JOB).with_errors(vec![ShiftErrorType::MissingIn]);
        let c = card(employee_with_two_jobs(), vec![bad]);
        assert_eq!(total_hours_worked_on_day(&c, today()), 0.0);
    }

    #[test]
    fn the_primary_shift_is_the_one_with_the_latest_out_punch() {
        let early = shift(today(), 2.0, HOME_JOB);
        let late = shift(today(), 3.0, PUNCHED_JOB);
        let mut c = card(employee_with_two_jobs(), vec![early, late]);
        run(&mut c, UOM::Hours, rule_params! { MIN_DAILY_HRS => "6" });
        assert_eq!(c.earnings()[0].job_id(), PUNCHED_JOB);
    }

    #[test]
    fn a_tie_for_the_latest_out_keeps_the_first_listed_shift() {
        let first = shift(today(), 2.0, HOME_JOB);
        let second = shift(today(), 2.0, PUNCHED_JOB);
        let mut c = card(employee_with_two_jobs(), vec![first, second]);
        run(&mut c, UOM::Hours, rule_params! { MIN_DAILY_HRS => "6" });
        assert_eq!(c.earnings()[0].job_id(), HOME_JOB);
    }

    #[test]
    fn with_no_out_punches_the_first_shift_is_primary() {
        let no_out = |job| {
            EmployeeShift::new(1, 1, job, today(), ShiftType::Actual, Vec::new())
                .with_net_hours(2.0)
        };
        let mut c = card(
            employee_with_two_jobs(),
            vec![no_out(PUNCHED_JOB), no_out(HOME_JOB)],
        );
        run(&mut c, UOM::Hours, rule_params! { MIN_DAILY_HRS => "10" });
        assert_eq!(c.earnings()[0].job_id(), PUNCHED_JOB);
    }

    #[test]
    fn the_factor_scales_the_rate_and_the_dollars() {
        let mut c = card(
            employee_with_two_jobs(),
            vec![shift(today(), 4.0, HOME_JOB)],
        );
        run(
            &mut c,
            UOM::HoursDollars,
            rule_params! { MIN_DAILY_HRS => "6", RATE_FACTOR => "1.5" },
        );
        let e = &c.earnings()[0];
        assert_eq!(e.hours(), 2.0);
        assert_eq!(e.rate(), 15.0);
        assert_eq!(e.dollars(), 30.0);
    }

    #[test]
    fn exactly_the_minimum_is_not_paid() {
        let mut c = card(
            employee_with_two_jobs(),
            vec![shift(today(), 6.0, HOME_JOB)],
        );
        run(&mut c, UOM::Hours, rule_params! { MIN_DAILY_HRS => "6" });
        assert!(c.earnings().is_empty());
    }
}
