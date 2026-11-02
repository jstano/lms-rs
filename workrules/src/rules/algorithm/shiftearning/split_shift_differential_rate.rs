//! Port of `SplitShiftDifferentialRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/SplitShiftDifferentialRateRuleImpl.java`.
//!
//! A premium for a **split shift**: two shifts the same employee works with a
//! gap between them that is long enough to be a real break but short enough to
//! be a split.
//!
//! - The *second* shift is the one the rule runs for. It must have no errors,
//!   both times, be open for editing and not exceed `secondShiftMax` net hours.
//! - Its *prior* shift is, among the card's other error-free shifts with both
//!   times whose OUT punch is **strictly before** this shift's IN punch, the one
//!   whose OUT is **latest** (the first on a tie). It must not exceed
//!   `firstShiftMax`, and the gap between its OUT and this IN must satisfy
//!   `minTime <= gap < maxTime`.
//! - The premium hours are the sum, over the configured `splitShiftPayTypes` in
//!   order (a type listed twice counts twice), of: the prior shift's hours, this
//!   shift's hours, the gap, or `fixedAmount`; then raised to `minAmount` and
//!   capped at `maxAmount`, in that order.
//! - The rate is `diffRate`, or with `deductMinWage` the part of the premium not
//!   already covering minimum wage on both shifts:
//!   `max(0, premium × diffRate − (prior pay + this pay − total hours × min wage)) / premium`
//!   where each shift's pay is its net hours at the employee's rate for that
//!   job. The minimum wage is the shift job's own; when that is zero, the
//!   **current property's**.
//! - `HOURS`, `HOURS_DOLLARS` and `UNITS` types pay hours at the rate; `DOLLARS`
//!   pays `round_display_currency(rate × hours)`. An earning with neither is
//!   discarded. It is added to the card and saved through the earning DAO.
//!
//! `SplitShiftPayTypes` is reconstructed (`common/enums`); the
//! `CurrentProperty` / job minimum wage is behind [`JobMinWagePort`].

use crate::common::dates::duration_in_fractional_hours;
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::split_shift_pay_types::{
    SplitShiftPayTypes, parse_split_shift_pay_types,
};
use crate::common::enums::uom::UOM;
use crate::common::numbers::{
    round_currency, round_display_currency, round_hours, safe_divide_default,
};
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, DEDUCT_MIN_WAGE, DIFF_RATE_PROP, EARNING, FIRST_SHIFT_MAX, FIXED_AMOUNT,
    MAX_HOURS_PAID, MAX_TIME_PROP, MIN_HOURS_PAID, MIN_TIME_PROP, SECOND_SHIFT_MAX,
    SPLIT_SHIFT_PAY_TYPES, SplitShiftDifferentialRateRuleConfig,
};
use crate::rules::algorithm::shiftearning::{ShiftEarningRule, new_shift_earning};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, JobMinWagePort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDateTime;
use std::cell::RefCell;

/// `SplitShiftDifferentialRateRuleImpl`.
pub struct SplitShiftDifferentialRateRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    W: JobMinWagePort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: W,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, W: JobMinWagePort>
    SplitShiftDifferentialRateRule<T, E, W>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: W) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
        }
    }

    /// `getRateForShift`: the employee's hourly rate for the shift's job, `0` with none.
    fn rate_for_shift(employee: &Employee, shift: &EmployeeShift) -> f64 {
        employee
            .effective_job_status(shift.shift_date(), shift.job_id())
            .map_or(0.0, |status| status.hourly_rate())
    }

    /// `determineDifferentialRateWithMinWageAdjustment`.
    fn rate_with_min_wage_adjustment(
        &self,
        employee: &Employee,
        prior: &EmployeeShift,
        shift: &EmployeeShift,
        premium_hours: f64,
        use_actual: bool,
        diff_rate: f64,
    ) -> f64 {
        let job_wage = self
            .min_wage
            .effective_min_wage(shift.job_id(), shift.shift_date());
        let min_wage = if job_wage == 0.0 {
            self.min_wage.current_property_min_wage()
        } else {
            job_wage
        };
        let prior_hours = prior.net_hours_using(use_actual);
        let prior_total = round_currency(prior_hours * Self::rate_for_shift(employee, prior));
        let shift_hours = shift.net_hours_using(use_actual);
        let shift_total = round_currency(shift_hours * Self::rate_for_shift(employee, shift));

        let total_hours = round_hours(prior_hours + shift_hours);
        let deduction = round_currency(
            (premium_hours * diff_rate) - ((prior_total + shift_total) - (total_hours * min_wage)),
        );
        safe_divide_default(deduction.max(0.0), premium_hours)
    }

    /// `calculatePremiumHours`.
    fn premium_hours(
        prior: &EmployeeShift,
        shift: &EmployeeShift,
        gap: f64,
        use_actual: bool,
        params: &RuleParams,
    ) -> f64 {
        let sum: f64 =
            parse_split_shift_pay_types(params.get(SPLIT_SHIFT_PAY_TYPES).unwrap_or("[]"))
                .into_iter()
                .map(|kind| match kind {
                    SplitShiftPayTypes::PayFirstShiftHours => prior.net_hours_using(use_actual),
                    SplitShiftPayTypes::PaySecondShiftHours => shift.net_hours_using(use_actual),
                    SplitShiftPayTypes::PayTimeBetweenShifts => gap,
                    SplitShiftPayTypes::FixedAmount => params.double_at(FIXED_AMOUNT),
                })
                .sum();
        sum.max(params.double_at(MIN_HOURS_PAID))
            .min(params.double_at(MAX_HOURS_PAID))
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, W: JobMinWagePort> ShiftEarningRule
    for SplitShiftDifferentialRateRule<T, E, W>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&SplitShiftDifferentialRateRuleConfig.default_values());
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let qualifies = !shift.has_errors()
            && shift.has_both_times()
            && time_card.is_open_for_editing_for_shift(shift)
            && shift.net_hours_using(use_actual) <= params.double_at(SECOND_SHIFT_MAX);
        if !qualifies {
            return;
        }
        let Some(second_in) = shift.punch_time_of_type(PunchType::In, use_actual) else {
            return;
        };

        // The closest prior shift: latest OUT before this IN, first on a tie.
        let mut closest: Option<(&EmployeeShift, LocalDateTime)> = None;
        for other in time_card.shifts() {
            if other.id() == shift.id() || other.has_errors() || !other.has_both_times() {
                continue;
            }
            let Some(out) = other.punch_time_of_type(PunchType::Out, use_actual) else {
                continue;
            };
            if out < second_in && closest.is_none_or(|(_, best)| out > best) {
                closest = Some((other, out));
            }
        }
        let Some((prior, prior_out)) = closest else {
            return;
        };
        let gap = duration_in_fractional_hours(prior_out, second_in);
        if prior.net_hours_using(use_actual) > params.double_at(FIRST_SHIFT_MAX)
            || !(gap >= params.double_at(MIN_TIME_PROP) && gap < params.double_at(MAX_TIME_PROP))
        {
            return;
        }

        let premium_hours = Self::premium_hours(prior, shift, gap, use_actual, &params);
        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let diff_rate = params.double_at(DIFF_RATE_PROP);
        let rate = if params.bool_at(DEDUCT_MIN_WAGE) {
            self.rate_with_min_wage_adjustment(
                employee,
                prior,
                shift,
                premium_hours,
                use_actual,
                diff_rate,
            )
        } else {
            diff_rate
        };

        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        earning.set_note(rule_item.name());
        match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars | UOM::Units => {
                earning.set_hours(premium_hours);
                earning.set_rate(rate);
            }
            UOM::Dollars => earning.set_dollars(round_display_currency(rate * premium_hours)),
            _ => {}
        }
        earning.calc_and_set_total_dollars();

        if earning.dollars() != 0.0 || earning.hours() != 0.0 {
            time_card.earnings_mut().push(earning.clone());
            self.earnings.borrow_mut().save(earning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{FixedEarningType, SavedEarnings};
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const NOTE: &str = "Rule: ruleSetName - Split Shift Differential Rate";
    const JOB1: i32 = 1;
    const JOB2: i32 = 2;
    const JOB3: i32 = 3;

    struct Wages {
        property_asked: std::cell::Cell<u32>,
    }
    impl JobMinWagePort for Wages {
        fn effective_min_wage(&self, job_id: i32, _d: LocalDate) -> f64 {
            if job_id == JOB2 { 0.0 } else { 7.25 }
        }
        fn current_property_min_wage(&self) -> f64 {
            self.property_asked.set(self.property_asked.get() + 1);
            8.0
        }
    }

    fn date() -> LocalDate {
        LocalDate::of(2013, 3, 1)
    }
    fn at(h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2013, 3, 1, h, m, 0)
    }
    fn punch(kind: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(1, kind, PunchSource::Clock, when)
    }
    /// A shift 00:00-08:00 on the shift date with one punch and stored net hours.
    fn base(id: i32, job: i32, punches: Vec<EmployeeShiftPunch>, net: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, job, date(), ShiftType::Actual, punches)
            .with_times(Some(at(0, 0)), Some(at(8, 0)))
            .with_net_hours(net)
    }
    fn second(job: i32) -> EmployeeShift {
        base(2, job, vec![punch(PunchType::In, at(14, 0))], 3.0)
    }
    fn first() -> EmployeeShift {
        base(1, JOB1, vec![punch(PunchType::Out, at(11, 0))], 2.0)
    }
    fn employee() -> Employee {
        let status = |id, job, rate| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                date().minus_years(1),
                date().plus_years(1),
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
                status(1, JOB1, 8.0),
                status(2, JOB2, 9.0),
                status(3, JOB3, 9.0),
            ],
        )
    }
    fn item(extra: RuleParams) -> RuleItem {
        // The spec's shared properties: not actual punches, maxTime 4.
        let mut p = rule_params! { ACTUAL_PUNCH_PROP => "false", MAX_TIME_PROP => "4.0" };
        for (k, v) in extra.iter() {
            p.set(k, v);
        }
        RuleItem::new(1, 1, NOTE, RuleClass::SplitShiftDifRateDif, p)
    }
    fn run(
        card_shifts: Vec<EmployeeShift>,
        s: &EmployeeShift,
        uom: UOM,
        extra: RuleParams,
    ) -> (TimeCardData, SavedEarnings, u32) {
        let mut card = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(card_shifts)
            .with_calculation_start_date(LocalDate::of(2013, 2, 1));
        let saved = SavedEarnings::default();
        let rule = SplitShiftDifferentialRateRule::new(
            FixedEarningType(EarningType::new(0, 1, "Split", EarnType::Regular, uom)),
            saved.clone(),
            Wages {
                property_asked: std::cell::Cell::new(0),
            },
        );
        rule.execute(s, &mut card, &item(extra));
        let asked = rule.min_wage.property_asked.get();
        (card, saved, asked)
    }

    // ---- java_parity_tests: SplitShiftDifferentialRateRuleImplTest.groovy ----

    #[test]
    fn test_execute_where_employee_earning_is_not_set() {
        let with_errors = base(2, JOB1, vec![], 0.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let one_time_only = EmployeeShift::new(2, 1, JOB1, date(), ShiftType::Actual, vec![])
            .with_times(Some(at(0, 0)), None);
        let before_calc = EmployeeShift::new(
            2,
            1,
            JOB1,
            LocalDate::of(2013, 1, 1),
            ShiftType::Actual,
            vec![],
        )
        .with_times(
            Some(LocalDateTime::of(2013, 1, 1, 0, 0, 0)),
            Some(LocalDateTime::of(2013, 1, 1, 8, 0, 0)),
        );
        let too_many = base(2, JOB1, vec![], 100.0);
        let regular = second(JOB1);
        // (second, cards shifts) — none of these may save.
        let rows: Vec<(&str, EmployeeShift, Vec<EmployeeShift>)> = vec![
            ("second has errors", with_errors, vec![]),
            ("second lacks both times", one_time_only, vec![]),
            ("second before calculation start", before_calc, vec![]),
            ("second has too many hours", too_many, vec![]),
            ("no shifts in dataset", regular.clone(), vec![]),
            (
                "second and first match",
                regular.clone(),
                vec![regular.clone()],
            ),
            (
                "first has errors",
                regular.clone(),
                vec![first().with_errors(vec![ShiftErrorType::MissingOut])],
            ),
            (
                "first lacks both times",
                regular.clone(),
                vec![
                    EmployeeShift::new(1, 1, JOB1, date(), ShiftType::Actual, vec![])
                        .with_times(Some(at(0, 0)), None),
                ],
            ),
            (
                "first has too many hours",
                regular,
                vec![base(1, JOB1, vec![punch(PunchType::Out, at(11, 0))], 100.0)],
            ),
        ];
        for (name, s, shifts) in rows {
            let (_, saved, _) = run(shifts, &s, UOM::Hours, RuleParams::new());
            assert!(saved.0.borrow().is_empty(), "{name}");
        }
        // Zero premium (net -1, so max(-1, 0) = 0): nothing for any unit.
        let zero = base(2, JOB1, vec![punch(PunchType::In, at(14, 0))], -1.0);
        for uom in [UOM::Dollars, UOM::Hours, UOM::HoursDollars, UOM::Units] {
            let (_, saved, _) = run(vec![first()], &zero, uom, RuleParams::new());
            assert!(saved.0.borrow().is_empty(), "{uom:?}");
        }
    }

    #[test]
    fn test_execute_where_employee_earning_is_set() {
        use UOM::{Dollars as D, Hours as H, HoursDollars as HD, Units as U};
        // (second job, deductMinWage, calcSplitShift, property asked, uom, dollars, hours, rate)
        let rows = [
            (JOB1, "false", false, 0, D, 7.25, 0.0, 0.0),
            (JOB1, "false", false, 0, H, 0.0, 1.0, 7.25),
            (JOB1, "false", false, 0, HD, 0.0, 1.0, 7.25),
            (JOB1, "false", false, 0, U, 0.0, 1.0, 7.25),
            (JOB3, "true", false, 0, D, 0.5, 0.0, 0.0),
            (JOB3, "true", false, 0, H, 0.0, 1.0, 0.5),
            (JOB3, "true", false, 0, HD, 0.0, 1.0, 0.5),
            (JOB3, "true", false, 0, U, 0.0, 1.0, 0.5),
            (JOB3, "true", true, 0, D, 0.5, 0.0, 0.0),
            (JOB2, "true", false, 1, D, 4.25, 0.0, 0.0),
        ];
        for (i, (job, deduct, calc_second, asked, uom, dollars, hours, rate)) in
            rows.into_iter().enumerate()
        {
            let s = second(job);
            let (card, saved, property_asked) = run(
                vec![first()],
                &s,
                uom,
                rule_params! {
                    DIFF_RATE_PROP => "7.25", DEDUCT_MIN_WAGE => deduct, MAX_HOURS_PAID => "1.00",
                    SPLIT_SHIFT_PAY_TYPES => if calc_second { "[PAY_SECOND_SHIFT_HOURS]" } else { "[PAY_TIME_BETWEEN_SHIFTS]" }
                },
            );
            assert_eq!(saved.0.borrow().len(), 1, "row {i}");
            let e = &card.earnings()[0];
            assert_eq!(e.earning_date(), date(), "row {i}");
            assert_eq!(e.pay_date(), Some(date()));
            assert_eq!(e.employee_id(), 1);
            assert_eq!(e.job_id(), job);
            assert_eq!(e.source(), EarningSource::Rule);
            assert_eq!(e.note(), NOTE);
            assert_eq!(e.shift_id(), Some(2));
            assert_eq!(
                (e.dollars(), e.hours(), e.rate()),
                (dollars, hours, rate),
                "row {i}"
            );
            assert_eq!(property_asked, asked, "row {i}");
        }
    }

    fn today_shift(
        id: i32,
        from: (i32, i32),
        to: (i32, i32),
        net: f64,
        rounded: bool,
        day: i32,
    ) -> EmployeeShift {
        let t = |(h, m): (i32, i32), d: i32| LocalDateTime::of(2013, 3, d, h, m, 0);
        let punches = vec![
            punch(PunchType::In, t(from, day)),
            punch(PunchType::Out, t(to, day)),
        ];
        let _ = rounded;
        EmployeeShift::new(
            id,
            1,
            JOB1,
            LocalDate::of(2013, 3, day),
            ShiftType::Actual,
            punches,
        )
        .with_times(Some(t(from, 5)), Some(t(to, 5)))
        .with_net_hours(net)
    }

    #[test]
    fn split_shift_enhancements_pay_the_configured_combination() {
        // Prior 8-14 (6h), second 16-20 (4h), gap 2h, fixed 0.5; actual punches.
        let prior = today_shift(1, (8, 0), (14, 0), 6.0, false, 5);
        let s = today_shift(2, (16, 0), (20, 0), 4.0, false, 5);
        let rows = [
            ("[\"PAY_FIRST_SHIFT_HOURS\"]", 6.0),
            ("[\"PAY_SECOND_SHIFT_HOURS\"]", 4.0),
            ("[\"PAY_TIME_BETWEEN_SHIFTS\"]", 2.0),
            ("[\"FIXED_AMOUNT\"]", 0.5),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_SECOND_SHIFT_HOURS\"]",
                10.0,
            ),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\"]",
                8.0,
            ),
            ("[\"PAY_FIRST_SHIFT_HOURS\",\"FIXED_AMOUNT\"]", 6.5),
            (
                "[\"PAY_SECOND_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\"]",
                6.0,
            ),
            ("[\"PAY_SECOND_SHIFT_HOURS\",\"FIXED_AMOUNT\"]", 4.5),
            ("[\"PAY_TIME_BETWEEN_SHIFTS\",\"FIXED_AMOUNT\"]", 2.5),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_SECOND_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\"]",
                12.0,
            ),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_SECOND_SHIFT_HOURS\",\"FIXED_AMOUNT\"]",
                10.5,
            ),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\",\"FIXED_AMOUNT\"]",
                8.5,
            ),
            (
                "[\"PAY_SECOND_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\",\"FIXED_AMOUNT\"]",
                6.5,
            ),
            (
                "[\"PAY_FIRST_SHIFT_HOURS\", \"PAY_SECOND_SHIFT_HOURS\",\"PAY_TIME_BETWEEN_SHIFTS\",\"FIXED_AMOUNT\"]",
                12.5,
            ),
        ];
        for (types, expected) in rows {
            let (card, _, _) = run(
                vec![prior.clone(), s.clone()],
                &s,
                UOM::Hours,
                rule_params! {
                    ACTUAL_PUNCH_PROP => "true", MAX_TIME_PROP => "3.0", SPLIT_SHIFT_PAY_TYPES => types,
                    FIXED_AMOUNT => "0.5", EARNING => "12"
                },
            );
            assert_eq!(card.earnings()[0].hours(), expected, "{types}");
        }
    }

    #[test]
    fn valid_prior_shifts_are_correctly_sorted() {
        // Four shifts; the closest earlier OUT is the same-day 06-10 shift.
        let punches = |d: i32, from: (i32, i32), to: (i32, i32)| {
            vec![
                punch(
                    PunchType::In,
                    LocalDateTime::of(2013, 3, d, from.0, from.1, 0),
                ),
                punch(PunchType::Out, LocalDateTime::of(2013, 3, d, to.0, to.1, 0)),
            ]
        };
        let mk = |id: i32, shift_day: i32, punch_day: i32, from: (i32, i32), to: (i32, i32)| {
            EmployeeShift::new(
                id,
                1,
                JOB1,
                LocalDate::of(2013, 3, shift_day),
                ShiftType::Actual,
                punches(punch_day, from, to),
            )
            .with_times(
                Some(LocalDateTime::of(2013, 3, 5, from.0, from.1, 0)),
                Some(LocalDateTime::of(2013, 3, 5, to.0, to.1, 0)),
            )
            .with_net_hours(4.0)
        };
        let shifts = vec![
            mk(1, 4, 4, (7, 0), (11, 0)),
            mk(2, 4, 4, (15, 30), (19, 30)),
            mk(3, 5, 5, (6, 0), (10, 0)),
            mk(4, 5, 5, (15, 30), (19, 30)),
        ];
        let s = shifts[3].clone();
        let (card, _, _) = run(
            shifts,
            &s,
            UOM::Hours,
            rule_params! {
                DEDUCT_MIN_WAGE => "false", DIFF_RATE_PROP => "1.2", EARNING => "12",
                FIRST_SHIFT_MAX => "99", MAX_TIME_PROP => "6", MAX_HOURS_PAID => "99", MIN_TIME_PROP => "2",
                MIN_HOURS_PAID => "1", SECOND_SHIFT_MAX => "99",
                SPLIT_SHIFT_PAY_TYPES => "[\"PAY_FIRST_SHIFT_HOURS\",\"PAY_SECOND_SHIFT_HOURS\"]",
                ACTUAL_PUNCH_PROP => "false"
            },
        );
        assert!(card.earnings().iter().any(|e| e.shift_id() == Some(4)
            && e.earning_date() == LocalDate::of(2013, 3, 5)
            && e.rate() == 1.2
            && e.hours() == 8.0));
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn the_gap_must_be_at_least_min_time_and_below_max_time() {
        let s = second(JOB1); // gap from 11:00 to 14:00 is 3h
        let exactly_min = rule_params! { MIN_TIME_PROP => "3.0", MAX_TIME_PROP => "4.0" };
        let (_, saved, _) = run(vec![first()], &s, UOM::Hours, exactly_min);
        assert_eq!(saved.0.borrow().len(), 1);
        let exactly_max = rule_params! { MIN_TIME_PROP => "1.0", MAX_TIME_PROP => "3.0" };
        let (_, saved, _) = run(vec![first()], &s, UOM::Hours, exactly_max);
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn min_amount_raises_and_max_amount_caps_the_premium() {
        let s = second(JOB1);
        let (card, _, _) = run(
            vec![first()],
            &s,
            UOM::Hours,
            rule_params! { MIN_HOURS_PAID => "2.5", SPLIT_SHIFT_PAY_TYPES => "[FIXED_AMOUNT]", FIXED_AMOUNT => "0.5" },
        );
        assert_eq!(card.earnings()[0].hours(), 2.5);
        let (card, _, _) = run(
            vec![first()],
            &s,
            UOM::Hours,
            rule_params! { MAX_HOURS_PAID => "2.5", SPLIT_SHIFT_PAY_TYPES => "[PAY_FIRST_SHIFT_HOURS,PAY_SECOND_SHIFT_HOURS]" },
        );
        assert_eq!(card.earnings()[0].hours(), 2.5);
    }

    #[test]
    fn a_prior_shift_ending_exactly_when_this_one_starts_is_not_prior() {
        let touching = base(1, JOB1, vec![punch(PunchType::Out, at(14, 0))], 2.0);
        let (_, saved, _) = run(vec![touching], &second(JOB1), UOM::Hours, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_tie_between_prior_shifts_keeps_the_first() {
        let a = base(1, JOB1, vec![punch(PunchType::Out, at(11, 0))], 2.0);
        let b = base(5, JOB1, vec![punch(PunchType::Out, at(11, 0))], 3.5);
        let (card, _, _) = run(
            vec![a, b],
            &second(JOB1),
            UOM::Hours,
            rule_params! { SPLIT_SHIFT_PAY_TYPES => "[PAY_FIRST_SHIFT_HOURS]", MAX_HOURS_PAID => "99" },
        );
        assert_eq!(card.earnings()[0].hours(), 2.0);
    }
}
