//! Port of `HolidayDifferentialRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/dailyearning/HolidayDifferentialRuleImpl.java`.
//!
//! The family's largest rule. On a date that is a configured holiday at the
//! employee's **home job's property**, it pays a premium on the hours the
//! employee worked, in two tiers, with a daily cap and a daily minimum.
//!
//! # Which shifts
//!
//! The shifts on the date (or, with `countEntireShiftAsHoliday`, every shift
//! that *starts or ends* on it), kept only if they are not adjustment-only,
//! have no persisted errors, sit in an open period and belong to a job the
//! employee holds that day; then ordered by start time. A holiday is "on" the
//! date when the home property's calendar has one dated that day whose type is
//! among `holidayTypes`.
//!
//! # The two tiers and the cap
//!
//! Shift by shift, in start order, a running `dailyTotal`:
//!
//! - **primary hours** are the shift's hours up to `secondaryFactorThreshold`
//!   minus the running total (zero once the total has reached the threshold),
//!   paid at `primaryFactor`;
//! - **secondary hours** are the rest of the shift, paid at `secondaryFactor`,
//!   written only if the cap has not already been reached;
//! - both are clipped so the running total never exceeds `maxHrsPaid`, and the
//!   loop stops as soon as it is reached.
//!
//! A shift's hours are `ShiftUtil.getNetHours(shift, actualPunchTime)`: the
//! stored net hours, or rebuilt from the raw punch times.
//!
//! # The minimum
//!
//! If anything was paid but the total is under `minHrsPaid`, one more earning
//! pays the **difference at the primary factor**. It is attached to the first
//! valid shift on a job *other than* the home job, and when there is none it
//! has no shift and is priced off the home job instead.
//!
//! # Pricing one earning
//!
//! Rate comes from the earning's shift's job (`getEarningRate`), or the home
//! job when there is no shift. For a `DOLLARS` earning type the earning is
//! `round_currency(round_currency(rate × hours) × factor)` dollars with zero
//! hours and rate; otherwise it is `hours` at `round_currency(rate × factor)`.
//! An earning of zero or fewer hours is not written.
//!
//! Each earning is appended to the card and saved. `holidayMap` caches a
//! property's holidays for the life of the rule instance, so a run asks the
//! calendar port once per property, not once per day.

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_currency;
use crate::entity::earning_type::EarningType;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::holiday::Holiday;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::config::{
    ACTUAL_PUNCH_PROP, COUNT_ENTIRE_SHIFT_AS_HOLIDAY, HOLIDAY_EARNING_PROP, HOLIDAY_TYPES_PROP,
    HolidayDifferentialRuleConfig, MAX_HRS_PAID, MIN_HRS_PAID, PRIMARY_FACTOR, SECONDARY_FACTOR,
    SECONDARY_FACTOR_THRESHOLD,
};
use crate::rules::algorithm::dailyearning::{
    DailyEarningRule, earning_rate_for_shift, earning_rate_home_job,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{
    AssignmentPort, EarningTypePort, EmployeeEarningPort, HolidayPort, MinWagePort,
};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::HashMap;

/// `HolidayDifferentialRuleImpl`.
pub struct HolidayDifferentialRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    H: HolidayPort,
    A: AssignmentPort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
    holidays: H,
    assignments: A,
    /// `holidayMap`: holidays by property id, filled on first use.
    holiday_cache: RefCell<HashMap<i32, Vec<Holiday>>>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    HolidayDifferentialRule<T, E, M, H, A>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M, holidays: H, assignments: A) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
            holidays,
            assignments,
            holiday_cache: RefCell::new(HashMap::new()),
        }
    }

    /// The home job's property's holiday on `date`, if any (`getHoliday`).
    fn holiday_on(&self, employee: &Employee, date: LocalDate) -> Option<Holiday> {
        let home = employee.home_employee_job_status(date)?;
        let property_id = self.assignments.find_by_id(home.job_id())?.property_id();
        let mut cache = self.holiday_cache.borrow_mut();
        let holidays = cache
            .entry(property_id)
            .or_insert_with(|| self.holidays.find_all_for_property(property_id));
        holidays
            .iter()
            .find(|holiday| holiday.holiday_date() == date)
            .cloned()
    }

    fn holiday_applies(
        &self,
        time_card: &dyn TimeCard,
        date: LocalDate,
        params: &RuleParams,
    ) -> bool {
        let Some(employee) = time_card.employee() else {
            return false;
        };
        if !employee.is_active_on_date(date) {
            return false;
        }
        let types = ids_for_key(HOLIDAY_TYPES_PROP, params);
        self.holiday_on(employee, date)
            .is_some_and(|holiday| types.contains(&holiday.holiday_type_id()))
    }

    /// `getValidShiftsOnHoliday`, as indices into `time_card.shifts()`,
    /// ordered by start time (stable).
    fn valid_shift_indices(
        &self,
        time_card: &dyn TimeCard,
        date: LocalDate,
        count_entire_shift: bool,
    ) -> Vec<usize> {
        let Some(employee) = time_card.employee() else {
            return Vec::new();
        };
        let mut indices: Vec<usize> = time_card
            .shifts()
            .iter()
            .enumerate()
            .filter(|(_, shift)| {
                let on_day = if count_entire_shift {
                    shift
                        .end_date_time()
                        .is_some_and(|t| t.to_local_date() == date)
                        || shift
                            .start_date_time()
                            .is_some_and(|t| t.to_local_date() == date)
                } else {
                    shift.shift_date() == date
                };
                on_day
                    && !shift.is_adjustment_only_shift()
                    && !shift.has_errors()
                    && time_card.is_open_for_editing_for_shift(shift)
                    && employee.is_active_job_on_date(shift.job_id(), date)
            })
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| time_card.shifts()[i].start_time_for_ordering());
        indices
    }

    #[allow(clippy::too_many_arguments)]
    fn create_earning(
        &self,
        time_card: &mut dyn TimeCard,
        shift: Option<&EmployeeShift>,
        rule_item: &RuleItem,
        params: &RuleParams,
        date: LocalDate,
        hours: f64,
        factor: f64,
    ) {
        if hours <= 0.0 {
            return;
        }
        let Some(earning_type) = self
            .earning_types
            .find_by_id(params.int_at(HOLIDAY_EARNING_PROP))
        else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        let rate = match shift {
            Some(shift) => earning_rate_for_shift(employee, shift, &earning_type, &self.min_wage),
            None => earning_rate_home_job(employee, date, &earning_type, &self.min_wage),
        };
        let Some(rate) = rate else {
            return;
        };
        let job_id = shift.map_or(home.job_id(), |s| s.job_id());

        let mut earning = EmployeeEarning::new(
            0,
            employee.id(),
            job_id,
            earning_type.id(),
            date,
            0.0,
            0.0,
            EarningSource::Rule,
        )
        .from_rule(rule_item.id(), shift.map(|s| s.id()));
        earning.set_pay_date(Some(date));
        price(&mut earning, &earning_type, rate, hours, factor);
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

/// The unit-dependent split between dollars, hours and rate.
fn price(
    earning: &mut EmployeeEarning,
    earning_type: &EarningType,
    rate: f64,
    hours: f64,
    factor: f64,
) {
    if earning_type.uom() == UOM::Dollars {
        earning.set_dollars(round_currency(round_currency(rate * hours) * factor));
        earning.set_rate(0.0);
        earning.set_hours(0.0);
    } else {
        earning.set_dollars(0.0);
        earning.set_rate(round_currency(rate * factor));
        earning.set_hours(hours);
    }
}

fn cap_hours_at_maximum(daily_total: f64, max_hours_paid: f64, hours_to_add: f64) -> f64 {
    if daily_total + hours_to_add > max_hours_paid {
        max_hours_paid - daily_total
    } else {
        hours_to_add
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    DailyEarningRule for HolidayDifferentialRule<T, E, M, H, A>
{
    fn execute(&self, time_card: &mut dyn TimeCard, date: LocalDate, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&HolidayDifferentialRuleConfig.default_values());

        if !self.holiday_applies(time_card, date, &params) {
            return;
        }

        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let primary_factor = params.double_at(PRIMARY_FACTOR);
        let secondary_factor = params.double_at(SECONDARY_FACTOR);
        let secondary_threshold = params.double_at(SECONDARY_FACTOR_THRESHOLD);
        let max_hours_paid = params.double_at(MAX_HRS_PAID);
        let min_hours_paid = params.double_at(MIN_HRS_PAID);

        // Snapshot the valid shifts: the card grows as earnings are added, but
        // the shifts themselves are not touched.
        let shifts: Vec<EmployeeShift> = self
            .valid_shift_indices(
                time_card,
                date,
                params.bool_at(COUNT_ENTIRE_SHIFT_AS_HOLIDAY),
            )
            .into_iter()
            .map(|i| time_card.shifts()[i].clone())
            .collect();

        let mut daily_total = 0.0;
        for shift in &shifts {
            let shift_hours = shift.net_hours_using(use_actual);

            let primary_hours = {
                let raw = if daily_total < secondary_threshold {
                    shift_hours.min(secondary_threshold - daily_total)
                } else {
                    0.0
                };
                cap_hours_at_maximum(daily_total, max_hours_paid, raw)
            };
            self.create_earning(
                time_card,
                Some(shift),
                rule_item,
                &params,
                date,
                primary_hours,
                primary_factor,
            );
            daily_total += primary_hours;

            if daily_total < max_hours_paid {
                let secondary_hours =
                    cap_hours_at_maximum(daily_total, max_hours_paid, shift_hours - primary_hours);
                self.create_earning(
                    time_card,
                    Some(shift),
                    rule_item,
                    &params,
                    date,
                    secondary_hours,
                    secondary_factor,
                );
                daily_total += secondary_hours;
            }

            if daily_total >= max_hours_paid {
                break;
            }
        }

        if daily_total > 0.0 && daily_total < min_hours_paid {
            let home_job_id = time_card
                .employee()
                .and_then(|e| e.home_employee_job_status(date))
                .map(|home| home.job_id());
            let secondary_shift = shifts.iter().find(|s| Some(s.job_id()) != home_job_id);
            self.create_earning(
                time_card,
                secondary_shift,
                rule_item,
                &params,
                date,
                min_hours_paid - daily_total,
                primary_factor,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::employee_status_type::EmployeeStatusType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalTime;
    use std::cell::Cell;
    use std::rc::Rc;

    const RATE: f64 = 8.0;
    const NOTE: &str = "Rule: ruleSet - Holiday Differential";
    // Jobs: A is the employee's home job at property 1 (which has a holiday
    // today); the others are jobs at properties without a matching holiday.
    const JOB_A: i32 = 10;
    const JOB_NO_HOLIDAYS: i32 = 11;
    const JOB_WRONG_DATE: i32 = 12;
    const JOB_INACTIVE: i32 = 13;
    const OTHER_JOB: i32 = 14;

    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            let property = match id {
                JOB_NO_HOLIDAYS => 2,
                JOB_WRONG_DATE => 3,
                _ => 1,
            };
            Some(Assignment::new(id, property, "job", "J", None))
        }
    }

    /// Counts how often a property's calendar is read.
    struct Calendar(Rc<Cell<u32>>);
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, property_id: i32) -> Vec<Holiday> {
            self.0.set(self.0.get() + 1);
            match property_id {
                1 => vec![Holiday::new(1, 1, today(), "Today", 2)],
                3 => vec![Holiday::new(2, 3, today().plus_days(1), "Tomorrow", 3)],
                _ => Vec::new(),
            }
        }
    }

    fn at(date: LocalDate, hour: i32) -> joda_rs::LocalDateTime {
        date.at_time(LocalTime::of(hour, 0, 0))
    }

    fn shift(id: i32, date: LocalDate, job: i32, start_hour: i32, net_hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, job, date, ShiftType::Actual, Vec::new())
            .with_times(Some(at(date, start_hour)), None)
            .with_net_hours(net_hours)
    }

    fn employee(
        statuses: Vec<EmployeeStatus>,
        job_start: LocalDate,
        job_end: LocalDate,
    ) -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB_A,
                job_start,
                job_end,
                EmployeePayType::Hourly,
                RATE,
                true,
            )],
        )
        .with_statuses(statuses)
    }

    fn params(max: &str) -> RuleParams {
        rule_params! {
            HOLIDAY_EARNING_PROP => "3", PRIMARY_FACTOR => "0.5", SECONDARY_FACTOR => "1.0",
            SECONDARY_FACTOR_THRESHOLD => "8.0", MIN_HRS_PAID => "6.0",
            HOLIDAY_TYPES_PROP => "[2,4]", ACTUAL_PUNCH_PROP => "false", MAX_HRS_PAID => max
        }
    }

    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, NOTE, RuleClass::HolidayDifDdf, params)
    }

    fn rule(
        saved: SavedEarnings,
    ) -> HolidayDifferentialRule<FixedEarningType, SavedEarnings, FixedMinWage, Calendar, Jobs>
    {
        HolidayDifferentialRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved,
            FixedMinWage(10.0),
            Calendar(Rc::new(Cell::new(0))),
            Jobs,
        )
    }

    fn active_today(status: EmployeeStatusType) -> Employee {
        employee(
            vec![EmployeeStatus::new(today(), today(), status)],
            today(),
            today(),
        )
    }

    /// Run on `date`; return the saved earnings in order.
    fn run(card: &mut TimeCardData, date: LocalDate, params: RuleParams) -> Vec<EmployeeEarning> {
        let saved = SavedEarnings::default();
        rule(saved.clone()).execute(card, date, &item(params));
        saved.0.borrow().clone()
    }

    fn assert_earning(
        e: &EmployeeEarning,
        shift: Option<i32>,
        hours: f64,
        factor: f64,
        date: LocalDate,
    ) {
        assert_eq!(e.earning_date(), date);
        assert_eq!(e.pay_date(), Some(date));
        assert_eq!(e.employee_id(), 1);
        assert_eq!(e.job_id(), JOB_A);
        assert_eq!(e.source(), EarningSource::Rule);
        assert_eq!(e.note(), NOTE);
        assert_eq!(e.rule_item_id(), Some(1));
        assert_eq!(e.shift_id(), shift);
        assert_eq!(e.dollars(), 0.0);
        assert_eq!(e.hours(), hours);
        assert_eq!(e.rate(), RATE * factor);
    }

    // The Groovy's static shift fixtures. Ids 1/2 as in the spec.
    fn shifts1() -> Vec<EmployeeShift> {
        vec![
            shift(1, today(), JOB_A, 8, 3.0),
            shift(2, today(), JOB_A, 20, 5.0),
        ]
    }
    fn shifts2() -> Vec<EmployeeShift> {
        vec![
            shift(2, today(), JOB_A, 20, 5.0),
            shift(1, today(), JOB_A, 8, 3.0),
        ]
    }
    fn shifts3() -> Vec<EmployeeShift> {
        vec![
            shift(1, today(), JOB_A, 8, 8.0),
            shift(2, today(), JOB_A, 20, 4.0),
        ]
    }
    fn shifts4() -> Vec<EmployeeShift> {
        vec![shift(1, today(), JOB_A, 8, 10.0)]
    }
    fn shifts5() -> Vec<EmployeeShift> {
        vec![shift(1, today(), JOB_A, 8, 4.0)]
    }
    fn shifts6() -> Vec<EmployeeShift> {
        vec![shift(1, today(), JOB_A, 14, 9.5)]
    }

    // ---- java_parity_tests: HolidayDifferentialRuleImplTest.groovy ----

    /// The "no earning" rows of `will create holiday earnings on valid shifts`.
    #[test]
    fn rows_that_create_no_earning() {
        let tomorrow = today().plus_days(1);
        let yesterday = today().minus_days(1);
        let err = || vec![ShiftErrorType::MissingOut];
        let cases: Vec<(&str, EmployeeStatusType, Vec<EmployeeShift>)> = vec![
            (
                "inactive employee",
                EmployeeStatusType::LeaveOfAbsence,
                shifts1(),
            ),
            (
                "shifts not on date",
                EmployeeStatusType::Active,
                vec![shift(1, tomorrow, JOB_A, 8, 3.0)],
            ),
            (
                "shifts have errors",
                EmployeeStatusType::Active,
                vec![
                    shift(1, today(), JOB_A, 8, 3.0).with_errors(err()),
                    shift(2, today(), JOB_A, 9, 3.0).with_errors(err()),
                ],
            ),
            (
                "shifts in closed pay period",
                EmployeeStatusType::Active,
                vec![shift(1, yesterday, JOB_A, 8, 3.0)],
            ),
            (
                "home property not on a holiday",
                EmployeeStatusType::Active,
                vec![shift(1, today(), JOB_NO_HOLIDAYS, 8, 3.0)],
            ),
            (
                "wrong holiday date",
                EmployeeStatusType::Active,
                vec![shift(1, today(), JOB_WRONG_DATE, 8, 3.0)],
            ),
            (
                "shifts with inactive jobs",
                EmployeeStatusType::Active,
                vec![shift(1, today(), JOB_INACTIVE, 8, 3.0)],
            ),
        ];
        for (scenario, status, shifts) in cases {
            let mut card = TimeCardData::new()
                .with_employee(active_today(status))
                .with_shifts(shifts)
                .with_calculation_start_date(today());
            let saved = run(&mut card, today(), params("999"));
            assert!(saved.is_empty(), "{scenario}");
            assert!(card.earnings().is_empty(), "{scenario}");
        }
    }

    /// The paying rows. `(shifts, [(shift id | None, hours, factor)], max)`.
    #[test]
    fn rows_that_create_earnings() {
        const P: f64 = 0.5;
        const S: f64 = 1.0;
        type Row = (
            &'static str,
            Vec<EmployeeShift>,
            Vec<(Option<i32>, f64, f64)>,
            &'static str,
        );
        let rows: Vec<Row> = vec![
            (
                "2 shifts",
                shifts1(),
                vec![(Some(1), 3.0, P), (Some(2), 5.0, P)],
                "999",
            ),
            (
                "2 shifts reverse order",
                shifts2(),
                vec![(Some(1), 3.0, P), (Some(2), 5.0, P)],
                "999",
            ),
            (
                "primary and secondary",
                shifts3(),
                vec![(Some(1), 8.0, P), (Some(2), 4.0, S)],
                "999",
            ),
            (
                "primary and secondary, one shift",
                shifts4(),
                vec![(Some(1), 8.0, P), (Some(1), 2.0, S)],
                "999",
            ),
            (
                "primary and earning for minimum",
                shifts5(),
                vec![(Some(1), 4.0, P), (None, 2.0, P)],
                "999",
            ),
            (
                "secondary limited by max",
                shifts4(),
                vec![(Some(1), 8.0, P), (Some(1), 1.0, S)],
                "9",
            ),
            (
                "secondary limited by max, 2 shifts",
                shifts3(),
                vec![(Some(1), 8.0, P), (Some(2), 1.0, S)],
                "9",
            ),
            (
                "two primaries cut off by max",
                shifts2(),
                vec![(Some(1), 3.0, P), (Some(2), 4.0, P)],
                "7",
            ),
            (
                "second tier cut off by max",
                shifts6(),
                vec![(Some(1), 8.0, P), (Some(1), 1.0, S)],
                "9",
            ),
        ];
        for (scenario, shifts, expected, max) in rows {
            let mut card = TimeCardData::new()
                .with_employee(active_today(EmployeeStatusType::Active))
                .with_shifts(shifts)
                .with_calculation_start_date(today());
            let saved = run(&mut card, today(), params(max));
            assert_eq!(saved.len(), expected.len(), "{scenario}");
            for (e, (shift_id, hours, factor)) in saved.iter().zip(&expected) {
                assert_earning(e, *shift_id, *hours, *factor, today());
            }
            assert_eq!(
                card.earnings().len(),
                expected.len(),
                "{scenario}: also added to the card"
            );
        }
    }

    fn wide_employee() -> Employee {
        employee(
            vec![EmployeeStatus::new(
                today().minus_days(1),
                today().plus_days(2),
                EmployeeStatusType::Active,
            )],
            today().minus_days(1),
            today().plus_days(2),
        )
    }

    fn overnight(
        id: i32,
        start: (LocalDate, i32),
        end: (LocalDate, i32),
        net: f64,
    ) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB_A, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(at(start.0, start.1)), Some(at(end.0, end.1)))
            .with_net_hours(net)
    }

    /// `count entire shift as holiday is true` rows.
    #[test]
    fn counting_the_entire_shift_as_holiday() {
        let tomorrow = today().plus_days(1);
        let rows = [
            (
                "capped below the threshold",
                overnight(1, (today(), 8), (tomorrow, 17), 9.0),
                "7",
                7.0,
            ),
            (
                "capped at the primary factor",
                overnight(1, (today(), 23), (tomorrow, 4), 5.0),
                "5",
                5.0,
            ),
        ];
        for (scenario, shift, max, hours) in rows {
            let mut card = TimeCardData::new()
                .with_employee(wide_employee())
                .with_shifts(vec![shift])
                .with_calculation_start_date(today().minus_days(1));
            let mut p = params(max);
            p.set(COUNT_ENTIRE_SHIFT_AS_HOLIDAY, "true");
            let saved = run(&mut card, today(), p);
            assert_earning(&saved[0], Some(1), hours, 0.5, today());
            // The spec asserts only the first earning. The second row's 5 paid
            // hours are still under `minHrsPaid` (6), so a make-up earning of
            // one more hour follows, taking the day *past* its cap of 5. The
            // spec's mock does not forbid it and the config validator would
            // reject max < min; the rule itself does not look.
            let expected_saves = if hours < 6.0 { 2 } else { 1 };
            assert_eq!(saved.len(), expected_saves, "{scenario}");
        }
    }

    /// `holiday earnings should only be created up to the primary factor when
    /// the max hours per day is less than secondary threshold`.
    #[test]
    fn max_hours_below_the_secondary_threshold() {
        let rows = [
            ("10h capped at 7", shifts4(), "7", Some(7.0)),
            ("4h capped at 3", shifts5(), "3", Some(3.0)),
            ("maximum of zero", shifts4(), "0", None),
        ];
        for (scenario, shifts, max, hours) in rows {
            let mut card = TimeCardData::new()
                .with_employee(active_today(EmployeeStatusType::Active))
                .with_shifts(shifts)
                .with_calculation_start_date(today());
            let mut p = params(max);
            p.set(MIN_HRS_PAID, "0.0");
            let saved = run(&mut card, today(), p);
            match hours {
                Some(h) => {
                    assert_eq!(saved.len(), 1, "{scenario}");
                    assert_earning(&saved[0], Some(1), h, 0.5, today());
                }
                None => assert!(saved.is_empty(), "{scenario}"),
            }
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn counting_the_entire_shift_finds_a_shift_ending_on_the_holiday() {
        let yesterday = today().minus_days(1);
        let mut card = TimeCardData::new()
            .with_employee(wide_employee())
            .with_shifts(vec![overnight(1, (yesterday, 22), (today(), 6), 8.0)])
            .with_calculation_start_date(yesterday);
        let mut p = params("999");
        p.set(COUNT_ENTIRE_SHIFT_AS_HOLIDAY, "true");
        let saved = run(&mut card, today(), p);
        // 8h is all primary (threshold 8) and clears the 6h minimum.
        assert_eq!(saved.len(), 1);
        assert_earning(&saved[0], Some(1), 8.0, 0.5, today());
    }

    #[test]
    fn without_counting_the_entire_shift_an_overnight_shift_dated_yesterday_is_not_found() {
        let yesterday = today().minus_days(1);
        let mut card = TimeCardData::new()
            .with_employee(wide_employee())
            .with_shifts(vec![overnight(1, (yesterday, 22), (today(), 6), 8.0)])
            .with_calculation_start_date(yesterday);
        // shift date is `today` in the fixture; move it to yesterday.
        let mut shifts = card.shifts().to_vec();
        shifts[0].set_shift_date(yesterday);
        *card.shifts_mut() = shifts;
        assert!(run(&mut card, today(), params("999")).is_empty());
    }

    #[test]
    fn the_make_up_earning_attaches_to_a_shift_on_a_non_home_job() {
        // 4h on the home job and 1h on another job: total 5 < 6, so one more
        // hour is paid, priced off the *other* job's shift.
        let other = EmployeeShift::new(2, 1, OTHER_JOB, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(at(today(), 14)), None)
            .with_net_hours(1.0);
        let job_status = |id, job, home| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                today(),
                today(),
                EmployeePayType::Hourly,
                RATE,
                home,
            )
        };
        let e = Employee::new(
            1,
            1,
            "Pat",
            vec![job_status(1, JOB_A, true), job_status(2, OTHER_JOB, false)],
        )
        .with_statuses(vec![EmployeeStatus::new(
            today(),
            today(),
            EmployeeStatusType::Active,
        )]);
        let mut card = TimeCardData::new()
            .with_employee(e)
            .with_shifts(vec![shift(1, today(), JOB_A, 8, 4.0), other])
            .with_calculation_start_date(today());
        let saved = run(&mut card, today(), params("999"));
        assert_eq!(saved.len(), 3);
        let make_up = &saved[2];
        assert_eq!(make_up.shift_id(), Some(2));
        assert_eq!(make_up.job_id(), OTHER_JOB);
        assert_eq!(make_up.hours(), 1.0);
        assert_eq!(make_up.rate(), RATE * 0.5);
    }

    #[test]
    fn a_dollars_earning_type_pays_rounded_dollars_and_no_hours() {
        let mut card = TimeCardData::new()
            .with_employee(active_today(EmployeeStatusType::Active))
            .with_shifts(vec![shift(1, today(), JOB_A, 8, 3.0)])
            .with_calculation_start_date(today());
        let saved = SavedEarnings::default();
        HolidayDifferentialRule::new(
            FixedEarningType(EarningType::new(
                3,
                1,
                "Holiday $",
                EarnType::Earning,
                UOM::Dollars,
            )),
            saved.clone(),
            FixedMinWage(10.0),
            Calendar(Rc::new(Cell::new(0))),
            Jobs,
        )
        .execute(&mut card, today(), &item(params("999")));
        let first = saved.0.borrow()[0].clone();
        // round_currency(round_currency(8 * 3) * 0.5)
        assert_eq!(first.dollars(), 12.0);
        assert_eq!(first.hours(), 0.0);
        assert_eq!(first.rate(), 0.0);
        assert_eq!(first.total_dollars(), 12.0);
    }

    #[test]
    fn a_property_calendar_is_read_once_per_property() {
        let reads = Rc::new(Cell::new(0));
        let r = HolidayDifferentialRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            SavedEarnings::default(),
            FixedMinWage(10.0),
            Calendar(reads.clone()),
            Jobs,
        );
        let mut card = TimeCardData::new()
            .with_employee(wide_employee())
            .with_calculation_start_date(today().minus_days(1));
        for offset in 0..3 {
            r.execute(
                &mut card,
                today().plus_days(offset - 1),
                &item(params("999")),
            );
        }
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn using_actual_punches_rebuilds_net_hours_from_the_punch_times() {
        // 8:00-17:30 with a 30 minute break from 12:00 to 12:30 is 9.0 worked.
        // The stored netHours (5.0) is deliberately wrong, so only the punches
        // can give 9.
        let punch = |id, t, hour, minute| {
            EmployeeShiftPunch::new(
                id,
                t,
                PunchSource::Clock,
                today().at_time(LocalTime::of(hour, minute, 0)),
            )
        };
        let s = EmployeeShift::new(
            1,
            1,
            JOB_A,
            today(),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, 8, 0),
                punch(2, PunchType::Break, 12, 0),
                punch(3, PunchType::Back, 12, 30),
                punch(4, PunchType::Out, 17, 30),
            ],
        )
        .with_net_hours(5.0);
        assert_eq!(s.net_hours_using(false), 5.0);
        assert_eq!(s.net_hours_using(true), 9.0);
    }
}
