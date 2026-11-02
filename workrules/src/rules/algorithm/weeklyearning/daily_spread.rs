//! Port of `DailySpreadRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/DailySpreadRuleImpl.java`.
//!
//! Based on New York's "spread of hours" rule (12 N.Y.C.R.R. 137-1.7, 142-2.4):
//! on each day on which the time from the **first IN to the last OUT** exceeds
//! `minHours` (strictly), one extra hour is paid at the minimum wage — or, with
//! `useMinWage` off, at the home job's rate.
//!
//! The day's first IN is the earliest IN across that date's valid shifts (no
//! persisted errors, in the week, not adjustment-only) and its last OUT the
//! latest OUT, using adjusted times when `actualPunchTime` is on and rounded
//! times otherwise. The earning is attributed to the date's **home** job.
//!
//! For a `DOLLARS` earning type the earning is `round(hours × rate, 2)` dollars
//! with no hours or rate; any other unit pays `hours` at `rate`.
//!
//! Unlike most of the family it does not check the period is open.
//!
//! A day with no home job makes Java throw `HomeJobNotFoundException`; the port
//! skips that day (divergence 97).

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::punch_type::PunchType;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ACTUAL_PUNCH_PROP, DailySpreadRuleConfig, EARNING, HOURS, MIN_HRS_SPREAD, USE_MIN_WAGE,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, rule_earning};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::{LocalDate, LocalDateTime};
use std::cell::RefCell;
use std::collections::HashMap;

/// `ShiftUtil.getPunchTime(shift, type, useActual)`: the first punch of a type.
fn punch_time(shift: &EmployeeShift, kind: PunchType, use_actual: bool) -> Option<LocalDateTime> {
    let punch = shift.punch(shift.punch_index_of_type(kind)?);
    if use_actual {
        punch.adj_time()
    } else {
        punch.rounded_time()
    }
}

/// `DailySpreadRuleImpl`.
pub struct DailySpreadRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> DailySpreadRule<T, E, M> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> WeeklyEarningRule
    for DailySpreadRule<T, E, M>
{
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        rule_description: &str,
    ) {
        let params = rule_item
            .params()
            .fixed(&DailySpreadRuleConfig.default_values());
        let min_hours = params.double_at(MIN_HRS_SPREAD);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let mut first_in: HashMap<LocalDate, LocalDateTime> = HashMap::new();
        let mut last_out: HashMap<LocalDate, LocalDateTime> = HashMap::new();
        for shift in time_card.shifts() {
            let valid = !shift.has_errors()
                && work_week.contains_date(shift.shift_date())
                && !shift.is_adjustment_only_shift();
            if !valid {
                continue;
            }
            let date = shift.shift_date();
            if let Some(t) = punch_time(shift, PunchType::In, use_actual) {
                first_in
                    .entry(date)
                    .and_modify(|current| {
                        if t < *current {
                            *current = t;
                        }
                    })
                    .or_insert(t);
            }
            if let Some(t) = punch_time(shift, PunchType::Out, use_actual) {
                last_out
                    .entry(date)
                    .and_modify(|current| {
                        if t > *current {
                            *current = t;
                        }
                    })
                    .or_insert(t);
            }
        }

        for date in work_week.dates() {
            let (Some(first), Some(last)) = (first_in.get(&date), last_out.get(&date)) else {
                continue;
            };
            if duration_in_fractional_hours(*first, *last) <= min_hours {
                continue;
            }

            let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
                continue;
            };
            let Some(employee) = time_card.employee() else {
                return;
            };
            let Some(home) = employee.home_employee_job_status(date) else {
                continue;
            };
            let rate = if params.bool_at(USE_MIN_WAGE) {
                self.min_wage
                    .min_wage(employee.property_id(), Some(home.job_id()), date)
            } else {
                home.hourly_rate()
            };

            let hours = params.double_at(HOURS);
            let mut earning = rule_earning(
                employee.id(),
                home.job_id(),
                earning_type.id(),
                date,
                rule_item,
                None,
            );
            let is_dollars = earning_type.uom().is_dollars();
            earning.set_hours(if is_dollars { 0.0 } else { hours });
            earning.set_dollars(if is_dollars {
                round_to(hours * rate, 2)
            } else {
                0.0
            });
            earning.set_rate(if is_dollars { 0.0 } else { rate });
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_description);

            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;

    const MIN_WAGE: f64 = 5.0;

    fn day() -> LocalDate {
        today()
    }

    fn week() -> DateRange {
        DateRange::new(day().minus_days(7), day().plus_days(7))
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
                day().minus_years(1),
                day().plus_years(1),
                EmployeePayType::Hourly,
                12.0,
                true,
            )],
        )
    }

    fn shift_with_punches(in_hour: i32, out_hour: i32) -> EmployeeShift {
        let at = |h| day().at_time(joda_rs::LocalTime::of(h, 0, 0));
        EmployeeShift::new(
            1,
            1,
            JOB,
            day(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(in_hour)),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, at(out_hour)),
            ],
        )
    }

    /// The Groovy's adjustment-only shift: no punches, one WORKED adjustment.
    fn adjustment_only() -> EmployeeShift {
        EmployeeShift::new(2, 1, JOB, day(), ShiftType::Actual, Vec::new())
            .with_worked_adjustments()
    }

    fn run(card: &mut TimeCardData, uom: UOM, params: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        DailySpreadRule::new(
            FixedEarningType(earning_type(uom)),
            saved.clone(),
            FixedMinWage(MIN_WAGE),
        )
        .execute(
            card,
            &week(),
            &RuleItem::new(1, 1, "Spread", RuleClass::DailySpreadWdf, params),
            "Rule Description",
        );
        saved
    }

    // ---- java_parity_tests: DailySpreadRuleImplTest.groovy ----

    #[test]
    fn correct_earning_is_created_based_on_the_configured_earning_type() {
        // (uom, in, out, earnings, hours, rate, dollars, total)
        let rows = [
            (UOM::Hours, 7, 18, 1, 1.0, 5.0, 0.0, 5.0),
            (UOM::Dollars, 7, 18, 1, 0.0, 0.0, 5.0, 5.0),
            (UOM::Hours, 9, 2, 0, 0.0, 0.0, 0.0, 0.0),
            (UOM::Dollars, 9, 2, 0, 0.0, 0.0, 0.0, 0.0),
        ];
        for (uom, in_h, out_h, count, hours, rate, dollars, total) in rows {
            let mut c = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![shift_with_punches(in_h, out_h), adjustment_only()]);
            // The Groovy hands the rule the config's own defaults.
            run(&mut c, uom, DailySpreadRuleConfig.default_values());
            assert_eq!(c.earnings().len(), count, "{uom:?} {in_h}-{out_h}");
            if count > 0 {
                let e = &c.earnings()[0];
                assert_eq!(
                    (e.hours(), e.rate(), e.dollars(), e.total_dollars()),
                    (hours, rate, dollars, total)
                );
            }
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn exactly_the_minimum_spread_does_not_pay() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift_with_punches(8, 18)]);
        run(&mut c, UOM::Hours, rule_params! { MIN_HRS_SPREAD => "10" });
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn two_shifts_on_a_day_spread_from_the_earliest_in_to_the_latest_out() {
        let mut early = shift_with_punches(6, 9);
        let late = shift_with_punches(15, 17);
        early.set_id(5);
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![early, late]);
        run(&mut c, UOM::Hours, rule_params! { MIN_HRS_SPREAD => "10" });
        assert_eq!(c.earnings().len(), 1, "6:00 to 17:00 is 11h");
    }

    #[test]
    fn the_home_rate_is_used_when_min_wage_is_off() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift_with_punches(7, 18)]);
        run(&mut c, UOM::Hours, rule_params! { USE_MIN_WAGE => "false" });
        assert_eq!(c.earnings()[0].rate(), 12.0);
    }

    #[test]
    fn shifts_with_errors_and_outside_the_week_are_ignored() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let bad = shift_with_punches(7, 18).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![bad]);
        run(&mut c, UOM::Hours, RuleParams::new());
        assert!(c.earnings().is_empty());

        let mut outside = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift_with_punches(7, 18)]);
        let saved = SavedEarnings::default();
        DailySpreadRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved,
            FixedMinWage(5.0),
        )
        .execute(
            &mut outside,
            &DateRange::new(day().plus_days(1), day().plus_days(7)),
            &RuleItem::new(1, 1, "x", RuleClass::DailySpreadWdf, RuleParams::new()),
            "d",
        );
        assert!(outside.earnings().is_empty());
    }

    #[test]
    fn rounded_times_are_used_when_actual_punches_are_off() {
        // adjTime 7..18 (11h) but rounded 9..18 (9h): only the former pays.
        let at = |h| day().at_time(joda_rs::LocalTime::of(h, 0, 0));
        let mut in_punch = EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(7));
        in_punch.set_adj_time(Some(at(7)));
        let mut shift = EmployeeShift::new(
            1,
            1,
            JOB,
            day(),
            ShiftType::Actual,
            vec![
                in_punch,
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, at(18)),
            ],
        );
        shift.punch_cursor(0).set_rounded_time(Some(at(9)));

        let mut actual = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift.clone()]);
        run(
            &mut actual,
            UOM::Hours,
            rule_params! { ACTUAL_PUNCH_PROP => "true" },
        );
        assert_eq!(actual.earnings().len(), 1);

        let mut rounded = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift]);
        run(
            &mut rounded,
            UOM::Hours,
            rule_params! { ACTUAL_PUNCH_PROP => "false" },
        );
        assert!(rounded.earnings().is_empty());
    }
}
