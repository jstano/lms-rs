//! Port of `BreakPeriodPenaltyHoursRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/BreakPeriodPenaltyHoursRuleImpl.java`.
//!
//! A penalty for **short breaks**, paid as the time the employee actually spent
//! on them: on a shift of at least `minShiftLength` hours, if the employee took
//! at least one break but **none** reached `minBreakLength`, pays the total
//! break time (summed, each addition rounded to two places) as hours at the
//! shift's job rate. One long-enough break anywhere in the shift cancels the
//! penalty; no breaks at all pays nothing (nothing to repay).
//!
//! Needs both times, no persisted errors and an open period.
//!
//! Java reads its parameters without `fixMap`; the port backfills (divergence
//! 103). No Groovy spec.

use crate::common::numbers::round_to;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, BreakPeriodPenaltyHoursRuleConfig, EARNING, MIN_BREAK_LENGTH,
    MIN_SHIFT_LENGTH,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `BreakPeriodPenaltyHoursRuleImpl`.
pub struct BreakPeriodPenaltyHoursRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort>
    BreakPeriodPenaltyHoursRule<T, E, M>
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
    for BreakPeriodPenaltyHoursRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) || !shift.has_both_times() {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&BreakPeriodPenaltyHoursRuleConfig.default_values());
        let min_break = params.double_at(MIN_BREAK_LENGTH);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        if shift.net_hours_using(use_actual) < params.double_at(MIN_SHIFT_LENGTH) {
            return;
        }

        let mut break_time = 0.0;
        for period in shift.breaks_using(use_actual) {
            let duration = period.duration().fractional_hours();
            if duration >= min_break {
                return;
            }
            break_time = round_to(break_time + duration, 2);
        }
        if break_time <= 0.0 {
            return;
        }

        let earning_type_id = params.int_at(EARNING);
        let Some(earning_type) = self.earning_types.find_by_id(earning_type_id) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(rate) = earning_rate_for_job(
            employee,
            shift.job_id(),
            shift.shift_date(),
            &earning_type,
            &self.min_wage,
        ) else {
            return;
        };

        let mut earning = new_shift_earning(shift, earning_type_id, rule_item, true);
        earning.set_hours(break_time);
        earning.set_rate(rate);
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
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    const JOB: i32 = 9;

    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, h, m, 0)
    }
    fn punch(id: i32, t: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, when)
    }
    /// 8:00-17:00 with the given `(start minute-of-day offset from 10:00, length)` breaks.
    fn shift(breaks: &[(i64, i64)], net: f64) -> EmployeeShift {
        let mut punches = vec![punch(1, PunchType::In, at(8, 0))];
        let mut id = 2;
        for (offset, len) in breaks {
            let start = at(10, 0).plus_minutes(*offset);
            punches.push(punch(id, PunchType::Break, start));
            punches.push(punch(id + 1, PunchType::Back, start.plus_minutes(*len)));
            id += 2;
        }
        punches.push(punch(id, PunchType::Out, at(17, 0)));
        EmployeeShift::new(
            3,
            1,
            JOB,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            punches,
        )
        .with_times(Some(at(8, 0)), Some(at(17, 0)))
        .with_net_hours(net)
    }
    fn run(s: &EmployeeShift) -> TimeCardData {
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                LocalDate::of(2014, 1, 1),
                LocalDate::of(2015, 1, 1),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        );
        let mut c = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![s.clone()]);
        BreakPeriodPenaltyHoursRule::new(FixedEarningType(earning_type(UOM::Hours)), SavedEarnings::default(), FixedMinWage(0.0))
            .execute(
                s, &mut c,
                &RuleItem::new(2, 1, "BPPH", RuleClass::BreakPerPenaltyHrsDif,
                    rule_params! { EARNING => "3", MIN_SHIFT_LENGTH => "5", MIN_BREAK_LENGTH => "0.5", ACTUAL_PUNCH_PROP => "false" }),
            );
        c
    }

    #[test]
    fn short_breaks_only_are_paid_back_as_the_total_break_time() {
        let c = run(&shift(&[(0, 10), (120, 15)], 8.5)); // 10 + 15 minutes
        let e = &c.earnings()[0];
        assert_eq!(
            e.hours(),
            0.42,
            "0.17 + 0.25 = 0.42, each addition rounded to 2 places"
        );
        assert_eq!((e.rate(), e.total_dollars()), (10.0, 4.2));
        assert_eq!(e.shift_id(), Some(3));
    }

    #[test]
    fn one_long_enough_break_cancels_the_penalty() {
        for breaks in [
            vec![(0, 30)],
            vec![(0, 10), (120, 30)],
            vec![(0, 30), (120, 10)],
        ] {
            assert!(
                run(&shift(&breaks, 8.0)).earnings().is_empty(),
                "{breaks:?}"
            );
        }
    }

    #[test]
    fn a_shift_with_no_breaks_pays_nothing() {
        assert!(run(&shift(&[], 9.0)).earnings().is_empty());
    }

    #[test]
    fn a_shift_shorter_than_the_minimum_pays_nothing() {
        assert!(run(&shift(&[(0, 10)], 4.99)).earnings().is_empty());
    }

    #[test]
    fn exactly_the_minimum_break_length_counts_as_long_enough() {
        assert!(run(&shift(&[(0, 30)], 8.0)).earnings().is_empty());
        assert_eq!(run(&shift(&[(0, 29)], 8.0)).earnings().len(), 1);
    }
}
