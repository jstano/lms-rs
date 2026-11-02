//! Port of `DailySpreadShiftDifferentialRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/DailySpreadShiftDifferentialRuleImpl.java`.
//!
//! The per-shift form of New York's "spread of hours" rule (compare
//! `weeklyearning`'s `DailySpread`). When the time from the **first IN to the last
//! OUT across all of a day's shifts** exceeds `minHours` (strictly), one extra
//! `hours` is paid — at the minimum wage with `useMinWage`, else at the rate of
//! the job the running shift worked.
//!
//! # Runs once a day, for the right shift
//!
//! The day's shifts (by shift date, errors excluded) are ordered by **net
//! hours, then start time**, and the rule runs only for the **last** of them —
//! the shift with the most hours, the most recent among ties — so the day is
//! paid once and the earning lands on that shift's job. The period must be open
//! for that shift. Shifts are matched by id.
//!
//! The earning is attached to **no** shift. For a `DOLLARS` earning type it is
//! `round(hours × rate, 2)` dollars; otherwise `hours` at `rate`.
//!
//! A shift with no IN or OUT punch makes Java throw; the port skips such shifts
//! when finding the day's span (divergence 105).

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::punch_type::PunchType;
use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DailySpreadShiftDifferentialRuleConfig, EARNING, HOURS, MIN_HRS_SPREAD,
    USE_MIN_WAGE,
};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `DailySpreadShiftDifferentialRuleImpl`.
pub struct DailySpreadShiftDifferentialRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    DailySpreadShiftDifferentialRule<T, E, M>
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

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> ShiftEarningRule
    for DailySpreadShiftDifferentialRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let date = shift.shift_date();

        // `sort(shifts, byPrecedence(netHours, startTime))`, then drop errors.
        let mut day: Vec<&EmployeeShift> = time_card.shifts_for_day(date);
        day.sort_by(|a, b| {
            a.net_hours().total_cmp(&b.net_hours()).then_with(|| {
                a.start_time_for_ordering()
                    .cmp(&b.start_time_for_ordering())
            })
        });
        day.retain(|s| !s.has_errors());

        let is_last = day.last().is_some_and(|last| last.id() == shift.id());
        if !(is_last && time_card.is_open_for_editing_for_shift(shift)) {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&DailySpreadShiftDifferentialRuleConfig.default_values());
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let mut first_in = None;
        let mut last_out = None;
        for s in &day {
            if let Some(t) = s.punch_time_of_type(PunchType::In, use_actual)
                && first_in.is_none_or(|current| t < current)
            {
                first_in = Some(t);
            }
            if let Some(t) = s.punch_time_of_type(PunchType::Out, use_actual)
                && last_out.is_none_or(|current| t > current)
            {
                last_out = Some(t);
            }
        }
        let (Some(first), Some(last)) = (first_in, last_out) else {
            return;
        };
        if duration_in_fractional_hours(first, last) <= params.double_at(MIN_HRS_SPREAD) {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(job_status) = employee.employee_job_status(shift.job_id(), date) else {
            return;
        };
        let rate = if params.bool_at(USE_MIN_WAGE) {
            self.min_wage
                .min_wage(employee.property_id(), Some(job_status.job_id()), date)
        } else {
            job_status.hourly_rate()
        };

        let hours = params.double_at(HOURS);
        let is_dollars = earning_type.uom().is_dollars();
        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, false);
        earning.set_hours(if is_dollars { 0.0 } else { hours });
        earning.set_dollars(if is_dollars {
            round_to(hours * rate, 2)
        } else {
            0.0
        });
        earning.set_rate(if is_dollars { 0.0 } else { rate });
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
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
    use joda_rs::{LocalDate, LocalDateTime, LocalTime};

    const EMP_MIN_WAGE: f64 = 5.0;
    const JOB_STATUS_RATE: f64 = 10.0;
    const HOME_JOB: i32 = 1;
    const MOST_HOURS_JOB: i32 = 2;
    const OTHER_JOB: i32 = 3;

    fn today() -> LocalDate {
        LocalDate::of(2015, 2, 16)
    }
    fn now() -> LocalDateTime {
        today().at_time(LocalTime::of(0, 0, 0))
    }

    fn employee() -> Employee {
        let status = |id, job, rate| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                today().minus_weeks(1).minus_years(1),
                today().plus_weeks(1).plus_years(1),
                EmployeePayType::Hourly,
                rate,
                false,
            )
        };
        Employee::new(
            1,
            1,
            "Pat",
            vec![
                status(1, HOME_JOB, 0.0),
                status(2, OTHER_JOB, 0.0),
                status(3, MOST_HOURS_JOB, JOB_STATUS_RATE),
            ],
        )
    }

    /// `getShiftWithPunches`: the punches are always on `today`, whatever the
    /// shift's own date; net hours is the nonsensical `out + in` that fixes the
    /// sort order.
    fn shift(
        id: i32,
        shift_date_time: LocalDateTime,
        in_hour: i32,
        out_hour: i32,
        job: i32,
    ) -> EmployeeShift {
        let at = |h| today().at_time(LocalTime::of(h, 0, 0));
        EmployeeShift::new(
            id,
            1,
            job,
            shift_date_time.to_local_date(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, at(in_hour)),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, at(out_hour)),
            ],
        )
        .with_times(Some(shift_date_time), None)
        .with_net_hours(f64::from(out_hour + in_hour))
    }

    // ---- java_parity_tests: DailySpreadShiftDifferentialRuleImplTest.groovy ----

    #[test]
    fn correct_earning_is_created_based_on_the_configured_earning_type_and_shift() {
        // (uom, useMinWage, in, out, count, hours, rate, dollars, total, calc start)
        let rows = [
            (
                UOM::Hours,
                "true",
                7,
                18,
                1,
                1.0,
                EMP_MIN_WAGE,
                0.0,
                EMP_MIN_WAGE,
                today().minus_days(3),
            ),
            (
                UOM::Dollars,
                "true",
                7,
                18,
                1,
                0.0,
                0.0,
                EMP_MIN_WAGE,
                EMP_MIN_WAGE,
                today().minus_days(3),
            ),
            (
                UOM::Hours,
                "true",
                9,
                2,
                0,
                0.0,
                0.0,
                0.0,
                0.0,
                today().minus_days(3),
            ),
            (
                UOM::Dollars,
                "true",
                9,
                2,
                0,
                0.0,
                0.0,
                0.0,
                0.0,
                today().minus_days(3),
            ),
            (
                UOM::Hours,
                "false",
                7,
                18,
                1,
                1.0,
                JOB_STATUS_RATE,
                0.0,
                JOB_STATUS_RATE,
                today().minus_days(3),
            ),
            (
                UOM::Dollars,
                "false",
                7,
                18,
                1,
                0.0,
                0.0,
                JOB_STATUS_RATE,
                JOB_STATUS_RATE,
                today().minus_days(3),
            ),
            (
                UOM::Dollars,
                "false",
                7,
                18,
                0,
                0.0,
                0.0,
                JOB_STATUS_RATE,
                JOB_STATUS_RATE,
                today().plus_days(3),
            ),
        ];
        for (uom, use_min_wage, in_h, out_h, count, hours, rate, dollars, total, calc_start) in rows
        {
            let shifts = vec![
                shift(1, now(), in_h, out_h, HOME_JOB),
                shift(2, now().plus_hours(2), in_h, out_h - 1, OTHER_JOB),
                shift(3, now().plus_days(1), in_h, out_h, OTHER_JOB),
                shift(4, now().plus_hours(8), in_h, out_h + 1, MOST_HOURS_JOB),
                shift(5, now().plus_hours(4), in_h, out_h + 1, OTHER_JOB),
                shift(6, now().minus_days(1), in_h, out_h, OTHER_JOB),
            ];
            let target = shifts[3].clone();
            let mut card = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(shifts)
                .with_calculation_start_date(calc_start);
            let mut params = DailySpreadShiftDifferentialRuleConfig.default_values();
            params.set(USE_MIN_WAGE, use_min_wage);
            let saved = SavedEarnings::default();
            DailySpreadShiftDifferentialRule::new(
                FixedEarningType(earning_type(uom)),
                saved.clone(),
                FixedMinWage(EMP_MIN_WAGE),
            )
            .execute(
                &target,
                &mut card,
                &RuleItem::new(1, 1, "Spread", RuleClass::DailySpreadShiftDif, params),
            );

            assert_eq!(
                card.earnings().len(),
                count,
                "{uom:?} {use_min_wage} {in_h}-{out_h}"
            );
            assert_eq!(saved.0.borrow().len(), count);
            for e in card.earnings() {
                assert_eq!(
                    (e.hours(), e.rate(), e.dollars(), e.total_dollars()),
                    (hours, rate, dollars, total)
                );
                assert_eq!(e.job_id(), MOST_HOURS_JOB);
                assert_eq!(e.shift_id(), None);
            }
        }
    }

    // ---- behaviour tests from the Java ----

    fn run_for(
        target: &EmployeeShift,
        shifts: Vec<EmployeeShift>,
        params: RuleParams,
    ) -> TimeCardData {
        let mut card = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts);
        DailySpreadShiftDifferentialRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            SavedEarnings::default(),
            FixedMinWage(EMP_MIN_WAGE),
        )
        .execute(
            target,
            &mut card,
            &RuleItem::new(1, 1, "Spread", RuleClass::DailySpreadShiftDif, params),
        );
        card
    }

    #[test]
    fn only_the_last_shift_of_the_day_by_net_hours_then_start_runs() {
        let small = shift(1, now(), 7, 17, HOME_JOB);
        let big = shift(2, now().plus_hours(4), 7, 19, MOST_HOURS_JOB);
        assert!(
            run_for(&small, vec![small.clone(), big.clone()], RuleParams::new())
                .earnings()
                .is_empty()
        );
        assert_eq!(
            run_for(&big, vec![small, big.clone()], RuleParams::new())
                .earnings()
                .len(),
            1
        );
    }

    #[test]
    fn a_tie_on_net_hours_goes_to_the_later_starting_shift() {
        let a = shift(1, now().plus_hours(1), 7, 19, HOME_JOB);
        let b = shift(2, now().plus_hours(5), 7, 19, MOST_HOURS_JOB);
        assert!(
            run_for(&a, vec![a.clone(), b.clone()], RuleParams::new())
                .earnings()
                .is_empty()
        );
        assert_eq!(
            run_for(&b, vec![a, b.clone()], RuleParams::new())
                .earnings()
                .len(),
            1
        );
    }

    #[test]
    fn shifts_with_errors_do_not_count_and_cannot_be_the_last() {
        let bad_big = shift(2, now().plus_hours(4), 7, 21, OTHER_JOB)
            .with_errors(vec![ShiftErrorType::MissingOut]);
        let ok = shift(1, now(), 7, 19, MOST_HOURS_JOB);
        let card = run_for(&ok, vec![ok.clone(), bad_big.clone()], RuleParams::new());
        assert_eq!(card.earnings().len(), 1);
        assert!(
            run_for(&bad_big, vec![ok, bad_big.clone()], RuleParams::new())
                .earnings()
                .is_empty()
        );
    }

    #[test]
    fn exactly_the_minimum_spread_pays_nothing() {
        let s = shift(1, now(), 8, 18, MOST_HOURS_JOB);
        assert!(
            run_for(&s, vec![s.clone()], rule_params! { MIN_HRS_SPREAD => "10" })
                .earnings()
                .is_empty()
        );
    }
}
