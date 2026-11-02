//! Port of `PaidBreakFixedHrsRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/PaidBreakFixedHrsRuleImpl.java`.
//!
//! Pays a fixed number of **hours** (`hoursPerPenalty`) at the shift's job
//! rate — compensation for a paid break the employee did not get — when the
//! shift is at least `minShiftLength` hours.
//!
//! With `checkMinBreak` off it pays whenever the shift is long enough. With it
//! on, it pays only if the shift has a break that lasts at least `minBreakLength`
//! hours **and starts less than `breakLimit` hours after the first IN** — i.e.
//! it is conditional on a qualifying break having been taken early enough.
//!
//! Needs both times, no persisted errors and an open period. Rate comes from
//! the shift's job, floored at minimum wage if the earning type says so;
//! breaks and times use adjusted or rounded punches per `actualPunchTime`.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::earning_rate_for_job;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, BREAK_LIMIT_PROP, CHECK_MIN_BREAK_PROP, EARNING, HOURS_PER_PENALTY,
    MIN_BREAK_LENGTH, MIN_SHIFT_LENGTH, PaidBreakFixedHrsRuleConfig,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, MinWagePort};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `PaidBreakFixedHrsRuleImpl`.
pub struct PaidBreakFixedHrsRule<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort> PaidBreakFixedHrsRule<T, E, M> {
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
    for PaidBreakFixedHrsRule<T, E, M>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) || !shift.has_both_times() {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&PaidBreakFixedHrsRuleConfig.default_values());
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        if shift.net_hours_using(use_actual) < params.double_at(MIN_SHIFT_LENGTH) {
            return;
        }

        let found = if params.bool_at(CHECK_MIN_BREAK_PROP) {
            let min_break = params.double_at(MIN_BREAK_LENGTH);
            let limit = params.double_at(BREAK_LIMIT_PROP);
            let in_time = shift.punch_time_of_type(PunchType::In, use_actual);
            shift.breaks_using(use_actual).iter().any(|period| {
                period.duration().fractional_hours() >= min_break
                    && in_time
                        .is_some_and(|t| duration_in_fractional_hours(t, period.start()) < limit)
            })
        } else {
            true
        };
        if !found {
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
        earning.set_dollars(0.0);
        earning.set_hours(params.double_at(HOURS_PER_PENALTY));
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
    use joda_rs::{LocalDate, LocalDateTime};

    const JOB: i32 = 9;

    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2014, 5, 6, h, m, 0)
    }
    fn p(id: i32, t: PunchType, h: i32, m: i32) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, at(h, m))
    }
    /// 8:00-16:00 with a break from `(bh, bm)` for `len_min` minutes.
    fn shift(bh: i32, bm: i32, len_min: i64, net: f64) -> EmployeeShift {
        let back = at(bh, bm).plus_minutes(len_min);
        EmployeeShift::new(
            3,
            1,
            JOB,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![
                p(1, PunchType::In, 8, 0),
                p(2, PunchType::Break, bh, bm),
                EmployeeShiftPunch::new(3, PunchType::Back, PunchSource::Clock, back),
                p(4, PunchType::Out, 16, 0),
            ],
        )
        .with_times(Some(at(8, 0)), Some(at(16, 0)))
        .with_net_hours(net)
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
                LocalDate::of(2014, 1, 1),
                LocalDate::of(2015, 1, 1),
                EmployeePayType::Hourly,
                11.0,
                true,
            )],
        )
    }
    fn run(s: &EmployeeShift, p: RuleParams) -> TimeCardData {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![s.clone()]);
        PaidBreakFixedHrsRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            SavedEarnings::default(),
            FixedMinWage(0.0),
        )
        .execute(
            s,
            &mut c,
            &RuleItem::new(2, 1, "PBF", RuleClass::PdBreakFixedHrsDif, p),
        );
        c
    }
    fn base() -> RuleParams {
        rule_params! {
            EARNING => "3", HOURS_PER_PENALTY => "0.5", MIN_SHIFT_LENGTH => "6", MIN_BREAK_LENGTH => "0.5",
            ACTUAL_PUNCH_PROP => "false", CHECK_MIN_BREAK_PROP => "false", BREAK_LIMIT_PROP => "5"
        }
    }

    #[test]
    fn a_long_enough_shift_is_paid_the_fixed_hours_at_its_job_rate() {
        let c = run(&shift(12, 0, 30, 8.0), base());
        let e = &c.earnings()[0];
        assert_eq!(
            (e.hours(), e.rate(), e.dollars(), e.total_dollars()),
            (0.5, 11.0, 0.0, 5.5)
        );
        assert_eq!(e.shift_id(), Some(3));
        assert_eq!(e.job_id(), JOB);
    }

    #[test]
    fn a_shorter_shift_is_not_paid() {
        let c = run(&shift(12, 0, 30, 5.99), base());
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn with_the_break_check_on_a_qualifying_early_break_is_required() {
        let mut p = base();
        p.set(CHECK_MIN_BREAK_PROP, "true");
        // 30 minute break starting 4h after the IN: qualifies (< 5h limit).
        assert_eq!(run(&shift(12, 0, 30, 8.0), p.clone()).earnings().len(), 1);
        // Too short.
        assert!(run(&shift(12, 0, 29, 8.0), p.clone()).earnings().is_empty());
        // Starts exactly at the limit: not strictly before it.
        assert!(run(&shift(13, 0, 30, 8.0), p.clone()).earnings().is_empty());
        // Starts just before it.
        assert_eq!(run(&shift(12, 59, 30, 8.0), p).earnings().len(), 1);
    }

    #[test]
    fn a_shift_without_a_break_fails_the_break_check() {
        let mut p = base();
        p.set(CHECK_MIN_BREAK_PROP, "true");
        let no_break = EmployeeShift::new(
            3,
            1,
            JOB,
            LocalDate::of(2014, 5, 6),
            ShiftType::Actual,
            vec![p_in(), p_out()],
        )
        .with_times(Some(at(8, 0)), Some(at(16, 0)))
        .with_net_hours(8.0);
        assert!(run(&no_break, p).earnings().is_empty());
    }
    fn p_in() -> EmployeeShiftPunch {
        p(1, PunchType::In, 8, 0)
    }
    fn p_out() -> EmployeeShiftPunch {
        p(2, PunchType::Out, 16, 0)
    }

    #[test]
    fn a_shift_missing_a_time_or_with_errors_is_not_paid() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let open = shift(12, 0, 30, 8.0).with_times(Some(at(8, 0)), None);
        assert!(run(&open, base()).earnings().is_empty());
        let bad = shift(12, 0, 30, 8.0).with_errors(vec![ShiftErrorType::MissingOut]);
        assert!(run(&bad, base()).earnings().is_empty());
    }

    #[test]
    fn the_rate_is_floored_at_minimum_wage_when_the_earning_type_says_so() {
        let s = shift(12, 0, 30, 8.0);
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![s.clone()]);
        let floors = earning_type(UOM::Hours).with_flags(true, false, false, false);
        PaidBreakFixedHrsRule::new(
            FixedEarningType(floors),
            SavedEarnings::default(),
            FixedMinWage(15.0),
        )
        .execute(
            &s,
            &mut c,
            &RuleItem::new(2, 1, "PBF", RuleClass::PdBreakFixedHrsDif, base()),
        );
        assert_eq!(c.earnings()[0].rate(), 15.0);
    }
}
