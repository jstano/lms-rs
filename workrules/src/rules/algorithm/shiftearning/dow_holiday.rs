//! Port of `DOWHolidayRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/DOWHolidayRuleImpl.java`.
//!
//! A premium for time worked on a holiday **or** on a weekday, where the day is
//! a configurable window (`Start Time`–`End Time`) rather than a calendar date
//! — so a night shift's hours after midnight belong to the previous day's
//! window.
//!
//! The rule looks at three dates around the shift: the day before, the shift
//! date and the day after. For each:
//!
//! - if a holiday of a configured type dated that day overlaps the shift, the
//!   holiday branch pays `time inside the window × holidayHoursFactor` hours at
//!   `rate × holidayFactor` under `holidayEarning`;
//! - otherwise, if that date's weekday (database numbering, Sunday = 1) is in
//!   `dow`, the same arithmetic with `dowHoursFactor`, `dowFactor` and
//!   `dowEarning` — a holiday on a date **replaces** its weekday premium.
//!
//! "Time inside the window" is [`calculate_time_in_period`] with breaks
//! deducted, plus — only when the window contains `shift start + adjThreshold
//! minutes` — the shift's worked adjustments not reallocated to an earning:
//! `WORKED` adjustments count, `BREAK` ones (reallocated or not) are subtracted,
//! everything else is ignored. The threshold keeps an adjustment (a late edit
//! to a long shift, say) from being counted in the wrong day's window.
//!
//! The earning is dated the shift date and attached to the shift. A `DOLLARS`
//! type pays `round_currency(round_currency(rate × hours) × factor)` dollars,
//! otherwise hours at `round_currency(rate × factor)`; `homeAsBase` selects the
//! home job's rate over the shift job's. Nothing is written for zero or fewer
//! hours.

use crate::common::dates::translate_dow_from_iso;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::{earning_rate_for_job, earning_rate_home_job};
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, ADJUSTMENT_THRESHOLD_PROP, DOW_EARNING_PROP, DOW_FACTOR_PROP,
    DOW_HOURS_FACTOR_PROP, DOW_PROP, DowHolidayRuleConfig, END_TIME_PROP, HOLIDAY_EARNING_PROP,
    HOLIDAY_FACTOR_PROP, HOLIDAY_HOURS_FACTOR_PROP, HOLIDAY_TYPES_PROP, HOME_AS_BASE_PROP,
    START_TIME_PROP,
};
use crate::rules::algorithm::shiftearning::utility::holiday_matcher::{
    HolidayMatcher, holiday_period,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_in_period_calculator::calculate_time_in_period;
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{
    AssignmentPort, EarningTypePort, EmployeeEarningPort, HolidayPort, MinWagePort,
};
use crate::rules::rule_config::RuleConfig;
use joda_rs::{LocalDate, LocalTime};
use std::cell::RefCell;

/// `DOWHolidayRuleImpl`.
pub struct DOWHolidayRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    H: HolidayPort,
    A: AssignmentPort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
    matcher: HolidayMatcher<H, A>,
}

/// Everything one execution reads from the parameters.
struct Settings {
    holiday_earning_id: i32,
    dow_earning_id: i32,
    dow_factor: f64,
    holiday_factor: f64,
    dow_hours_factor: f64,
    holiday_hours_factor: f64,
    use_actual: bool,
    home_job_rate: bool,
    start_time: LocalTime,
    end_time: LocalTime,
    holiday_types: Vec<i32>,
    dow_ids: Vec<i32>,
    adjustment_threshold: joda_rs::LocalDateTime,
}

impl Settings {
    fn read(params: &RuleParams, shift_start: joda_rs::LocalDateTime) -> Self {
        Self {
            holiday_earning_id: params.int_at(HOLIDAY_EARNING_PROP),
            dow_earning_id: params.int_at(DOW_EARNING_PROP),
            dow_factor: params.double_at(DOW_FACTOR_PROP),
            holiday_factor: params.double_at(HOLIDAY_FACTOR_PROP),
            dow_hours_factor: params.double_at(DOW_HOURS_FACTOR_PROP),
            holiday_hours_factor: params.double_at(HOLIDAY_HOURS_FACTOR_PROP),
            use_actual: params.bool_at(ACTUAL_PUNCH_PROP),
            home_job_rate: params.bool_at(HOME_AS_BASE_PROP),
            start_time: LocalTime::parse(params.get(START_TIME_PROP).unwrap_or("00:00:00")),
            end_time: LocalTime::parse(params.get(END_TIME_PROP).unwrap_or("00:00:00")),
            holiday_types: ids_for_key(HOLIDAY_TYPES_PROP, params),
            dow_ids: ids_for_key(DOW_PROP, params),
            adjustment_threshold: shift_start
                .plus_minutes(i64::from(params.int_at(ADJUSTMENT_THRESHOLD_PROP))),
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    DOWHolidayRule<T, E, M, H, A>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M, holidays: H, assignments: A) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
            matcher: HolidayMatcher::new(holidays, assignments),
        }
    }

    /// `computeTimeInHolidayPeriod` for holidays on the given dates.
    fn time_in_holiday_periods(
        shift: &EmployeeShift,
        settings: &Settings,
        holiday_dates: &[LocalDate],
    ) -> f64 {
        let mut premium_time = 0.0;
        for date in holiday_dates {
            let period = holiday_period(*date, settings.start_time, settings.end_time);
            if period.contains(settings.adjustment_threshold) {
                premium_time =
                    round_hours(premium_time + worked_adjustments_not_reallocated(shift));
            }
            premium_time = round_hours(
                calculate_time_in_period(shift, &period, settings.use_actual, true) + premium_time,
            );
        }
        premium_time
    }

    /// `createPremiumEarning`.
    #[allow(clippy::too_many_arguments)]
    fn create_premium_earning(
        &self,
        shift: &EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
        settings: &Settings,
        hours: f64,
        factor: f64,
        earning_id: i32,
    ) {
        if hours <= 0.0 {
            return;
        }
        let Some(earning_type) = self.earning_types.find_by_id(earning_id) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let date = shift.shift_date();
        let rate = if settings.home_job_rate {
            earning_rate_home_job(employee, date, &earning_type, &self.min_wage)
        } else {
            earning_rate_for_job(
                employee,
                shift.job_id(),
                date,
                &earning_type,
                &self.min_wage,
            )
        };
        let Some(rate) = rate else {
            return;
        };

        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        earning.set_note(rule_item.name());
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(round_currency(round_currency(rate * hours) * factor));
            earning.set_rate(0.0);
            earning.set_hours(0.0);
        } else {
            earning.set_dollars(0.0);
            earning.set_rate(round_currency(rate * factor));
            earning.set_hours(hours);
        }
        earning.calc_and_set_total_dollars();

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

/// `calcWorkedAdjustmentsNotReallocatedToEarning`.
///
/// `adj_hours` is the sum of every `WORKED` adjustment less every `BREAK` one;
/// the reallocated `WORKED` hours come back out, which leaves exactly the Java
/// loop's result (reallocated `BREAK` adjustments are still subtracted, and
/// `OT`/`DT` adjustments never counted).
fn worked_adjustments_not_reallocated(shift: &EmployeeShift) -> f64 {
    round_hours(shift.adj_hours() - shift.reallocated_to_earning_hours())
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    ShiftEarningRule for DOWHolidayRule<T, E, M, H, A>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift.has_both_times() || !shift_is_eligible(shift, time_card) {
            return;
        }
        let Some(start) = shift.start_date_time() else {
            return;
        };
        let params = rule_item
            .params()
            .fixed(&DowHolidayRuleConfig.default_values());
        let settings = Settings::read(&params, start);

        let dates = [
            shift.shift_date().minus_days(1),
            shift.shift_date(),
            shift.shift_date().plus_days(1),
        ];
        let overlapping = self.matcher.find_overlapping_holidays_between(
            shift,
            &settings.holiday_types,
            settings.use_actual,
            settings.start_time,
            settings.end_time,
        );

        for date in dates {
            let holiday_dates: Vec<LocalDate> = overlapping
                .iter()
                .filter(|holiday| holiday.holiday_date() == date)
                .map(|holiday| holiday.holiday_date())
                .collect();

            if !holiday_dates.is_empty() {
                let hours = round_hours(
                    Self::time_in_holiday_periods(shift, &settings, &holiday_dates)
                        * settings.holiday_hours_factor,
                );
                self.create_premium_earning(
                    shift,
                    time_card,
                    rule_item,
                    &settings,
                    hours,
                    settings.holiday_factor,
                    settings.holiday_earning_id,
                );
            } else {
                let hours = if settings
                    .dow_ids
                    .contains(&translate_dow_from_iso(date.day_of_week().value()))
                {
                    Self::time_in_holiday_periods(shift, &settings, &[date])
                } else {
                    0.0
                };
                let hours = round_hours(hours * settings.dow_hours_factor);
                self.create_premium_earning(
                    shift,
                    time_card,
                    rule_item,
                    &settings,
                    hours,
                    settings.dow_factor,
                    settings.dow_earning_id,
                );
            }
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
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::holiday::Holiday;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings,
    };
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    const JOB: i32 = 1;
    const NOTE: &str = "Rule: ruleSet - Day of Week with Holiday";
    const HOLIDAY_TYPE: i32 = 1;

    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            Some(Assignment::new(id, 1, "job", "J", None))
        }
    }
    struct Calendar(Vec<Holiday>);
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            self.0.clone()
        }
    }

    fn d(day: i32) -> LocalDate {
        LocalDate::of(2013, 4, day)
    }
    fn at(day: i32, h: i32) -> LocalDateTime {
        LocalDateTime::of(2013, 4, day, h, 0, 0)
    }
    fn holiday(day: i32) -> Holiday {
        Holiday::new(day, 1, d(day), "Holiday", HOLIDAY_TYPE)
    }
    fn punched(from: LocalDateTime, to: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            1,
            JOB,
            from.to_local_date(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, from),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, to),
            ],
        )
        .with_times(Some(from), Some(to))
    }
    fn shift1() -> EmployeeShift {
        punched(at(5, 9), at(5, 17))
    }
    fn shift2() -> EmployeeShift {
        punched(at(6, 0), at(6, 8))
    }
    fn shift3() -> EmployeeShift {
        punched(at(6, 0), at(6, 3))
    }
    /// WORKED -0.5, WORKED -0.5 (reallocated), OT -0.5.
    fn shift4() -> EmployeeShift {
        shift2()
            .with_adj_hours(-1.0)
            .with_reallocated_to_earning_hours(-0.5)
    }
    /// BREAK 0.5, BREAK 0.5 (reallocated), WORKED -0.5.
    fn shift5() -> EmployeeShift {
        shift2().with_adj_hours(-1.5)
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
                d(1).minus_years(1),
                d(1).plus_years(1),
                EmployeePayType::Hourly,
                8.0,
                true,
            )],
        )
    }
    fn params(dow: &str) -> RuleParams {
        rule_params! {
            HOLIDAY_FACTOR_PROP => "1", START_TIME_PROP => "02:00:00", END_TIME_PROP => "02:00:00",
            HOLIDAY_TYPES_PROP => "[1]", DOW_PROP => dow
        }
    }
    fn item(extra: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, NOTE, RuleClass::DowHolidayDif, extra)
    }
    fn run(
        s: &EmployeeShift,
        holidays: Vec<Holiday>,
        uom: UOM,
        p: RuleParams,
        calc_start: LocalDate,
    ) -> (TimeCardData, SavedEarnings) {
        let mut card = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(calc_start);
        let saved = SavedEarnings::default();
        DOWHolidayRule::new(
            FixedEarningType(EarningType::new(31, 1, "Hol", EarnType::Regular, uom)),
            saved.clone(),
            FixedMinWage(0.0),
            Calendar(holidays),
            Jobs,
        )
        .execute(s, &mut card, &item(p));
        (card, saved)
    }
    fn hours_and_rates(card: &TimeCardData) -> Vec<(f64, f64)> {
        card.earnings()
            .iter()
            .map(|e| (e.hours(), e.rate()))
            .collect()
    }

    // ---- java_parity_tests: DOWHolidayRuleImplTest.groovy ----

    #[test]
    fn execute_for_shift_with_errors_and_shift_date_before_calculation_date() {
        let with_errors = shift1().with_errors(vec![ShiftErrorType::MissingOut]);
        let before_calc = shift1();
        let adjustment_only = EmployeeShift::new(1, 1, JOB, d(5), ShiftType::Actual, Vec::new());
        for (s, calc_start) in [
            (with_errors, d(1)),
            (before_calc, d(30)),
            (adjustment_only, d(1)),
        ] {
            let (card, saved) = run(&s, vec![holiday(5)], UOM::Hours, params("[6]"), calc_start);
            assert!(saved.0.borrow().is_empty());
            assert!(card.earnings().is_empty());
        }
    }

    #[test]
    fn execute_for_no_holidays_or_dow_matching() {
        // "no holidays and no dows"; "has holidays and dows but do not fall on shift day".
        let (_, saved) = run(&shift1(), vec![], UOM::Hours, params("[]"), d(1));
        assert!(saved.0.borrow().is_empty());
        let (_, saved) = run(
            &shift1(),
            vec![holiday(1)],
            UOM::Hours,
            params("[1,2,3]"),
            d(1),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn execute() {
        // (name, shift, holidays, dow ids, home job rate, expected (hours, rate) in date order)
        let rows: Vec<(
            &str,
            EmployeeShift,
            Vec<Holiday>,
            &str,
            &str,
            Vec<(f64, f64)>,
        )> = vec![
            (
                "holidays and no dows using homeJobRate",
                shift1(),
                vec![holiday(5)],
                "[]",
                "true",
                vec![(4.0, 8.0)],
            ),
            (
                "holidays and no dows using earningRate",
                shift1(),
                vec![holiday(5)],
                "[]",
                "false",
                vec![(4.0, 8.0)],
            ),
            (
                "two hours on the day after the holiday",
                shift2(),
                vec![holiday(5)],
                "[]",
                "true",
                vec![(1.0, 8.0)],
            ),
            (
                "holiday and dows. Get holiday factor",
                shift1(),
                vec![holiday(5)],
                "[6]",
                "true",
                vec![(4.0, 8.0)],
            ),
            (
                "2 holidays back to back",
                shift3(),
                vec![holiday(5), holiday(6)],
                "[7]",
                "true",
                vec![(1.0, 8.0), (0.5, 8.0)],
            ),
            (
                "holiday and dow back to back",
                shift3(),
                vec![holiday(5)],
                "[7]",
                "true",
                vec![(1.0, 8.0), (0.5, 4.0)],
            ),
            (
                "no holiday but dow",
                shift1(),
                vec![],
                "[6]",
                "true",
                vec![(4.0, 4.0)],
            ),
            (
                "dow and holiday back to back",
                shift2(),
                vec![holiday(6)],
                "[6]",
                "true",
                vec![(1.0, 4.0), (3.0, 8.0)],
            ),
            (
                "dow without adjustment applied",
                shift4(),
                vec![],
                "[6]",
                "true",
                vec![(1.0, 4.0)],
            ),
            (
                "dow with adjustment applied",
                shift4(),
                vec![],
                "[7]",
                "true",
                vec![(2.75, 4.0)],
            ),
            (
                "dow with break adjustment applied",
                shift5(),
                vec![],
                "[7]",
                "true",
                vec![(2.25, 4.0)],
            ),
        ];
        for (name, s, holidays, dow, home, expected) in rows {
            let mut p = params(dow);
            p.set(HOME_AS_BASE_PROP, home);
            let (card, saved) = run(&s, holidays, UOM::Hours, p, d(1));
            assert_eq!(hours_and_rates(&card), expected, "{name}");
            assert_eq!(saved.0.borrow().len(), expected.len(), "{name}");
            for e in card.earnings() {
                assert_eq!(e.earning_date(), s.shift_date(), "{name}");
                assert_eq!(e.pay_date(), Some(s.shift_date()), "{name}");
                assert_eq!(e.source(), EarningSource::Rule);
                assert_eq!(e.note(), NOTE);
                assert_eq!(e.shift_id(), Some(1));
                assert_eq!(e.dollars(), 0.0);
            }
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_dollars_type_pays_dollars_with_no_hours_or_rate() {
        let mut p = params("[]");
        p.set(HOLIDAY_FACTOR_PROP, "0.5");
        let (card, _) = run(&shift1(), vec![holiday(5)], UOM::Dollars, p, d(1));
        // 4 hours × 8 = 32, × 0.5 factor.
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.rate(), e.hours()), (16.0, 0.0, 0.0));
    }

    #[test]
    fn a_holiday_of_an_unconfigured_type_does_not_count() {
        let mut p = params("[]");
        p.set(HOLIDAY_TYPES_PROP, "[2]");
        let (card, _) = run(&shift1(), vec![holiday(5)], UOM::Hours, p, d(1));
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn a_holiday_replaces_the_weekday_premium_for_its_date() {
        // 5 April is both a holiday and a Friday in `dow`: one holiday earning.
        let (card, _) = run(&shift1(), vec![holiday(5)], UOM::Hours, params("[6]"), d(1));
        assert_eq!(card.earnings().len(), 1);
        assert_eq!(card.earnings()[0].rate(), 8.0);
    }

    #[test]
    fn the_adjustment_is_ignored_when_the_threshold_falls_outside_the_window() {
        // Threshold 0 minutes: shift4 starts 00:00 on the 6th, before the 5th's window ends,
        // so the 5th's window contains it and counts the adjustment: 2 - 0.5 = 1.5 -> 0.75.
        let mut p = params("[6]");
        p.set(ADJUSTMENT_THRESHOLD_PROP, "0");
        let (card, _) = run(&shift4(), vec![], UOM::Hours, p, d(1));
        assert_eq!(hours_and_rates(&card), vec![(0.75, 4.0)]);
    }
}
