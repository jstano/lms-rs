//! Port of `HolidayRateFactorRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/HolidayRateFactorRuleImpl.java`.
//!
//! Pays a shift worked **on a holiday date** at `rate × factor`.
//!
//! - The holiday is the shift's job's property's holiday whose date **equals the
//!   shift date** (the first listed, if the calendar has two); it counts when its
//!   type is among `holidayTypes`. There is no time-of-day window, unlike
//!   `HolidayTimeOfDay` and `DOWHoliday`: a shift starting the evening before
//!   gets nothing.
//! - `homeDeptOnly`: the employee needs a home job on the shift date whose
//!   parent assignment is the shift job's parent (`None == None` counts as the
//!   same department, as two null references do in Java).
//! - The hours are `min(net hours, maxHrsPaid)` using the shift's net hours
//!   (`getNetHours(shift, useActual)`, adjustments included); nothing is paid for
//!   zero or fewer.
//! - The rate: `homeAsBase` — the home job's rate (floored at minimum wage when
//!   the earning type says so); else `homeDeptRate` — the home job's hourly rate
//!   when the shift job is in the home job's department, otherwise the job's
//!   effective pay rate (no minimum-wage floor on either); else the shift job's
//!   own rate (floored).
//! - A `DOLLARS` earning type pays `round_currency(round_currency(rate × hours) ×
//!   factor)` dollars; otherwise `hours` at `round_currency(rate × factor)`.

use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_currency;
use crate::entity::earning_type::EarningType;
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::holiday::Holiday;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::{earning_rate_for_job, earning_rate_home_job};
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, EARNING, FACTOR_PROP, HOLIDAY_TYPES_PROP, HOME_AS_BASE_PROP,
    HOME_DEPT_ONLY_PROP, HOME_DEPT_RATE_PROP, HolidayRateFactorRuleConfig, MAX_HRS_PAID,
};
use crate::rules::algorithm::shiftearning::utility::holiday_matcher::HolidayMatcher;
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{
    AssignmentPort, EarningTypePort, EmployeeEarningPort, HolidayPort, MinWagePort,
};
use crate::rules::rule_config::RuleConfig;
use std::cell::RefCell;

/// `HolidayRateFactorRuleImpl`.
pub struct HolidayRateFactorRule<
    T: EarningTypePort,
    E: EmployeeEarningPort,
    M: MinWagePort,
    H: HolidayPort,
    A: AssignmentPort,
> {
    earning_types: T,
    earnings: RefCell<E>,
    min_wage: M,
    /// Supplies the per-property holiday cache (`holidayMap`) and the job lookup.
    calendar: HolidayMatcher<H, A>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    HolidayRateFactorRule<T, E, M, H, A>
{
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, min_wage: M, holidays: H, assignments: A) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            min_wage,
            calendar: HolidayMatcher::new(holidays, assignments),
        }
    }

    /// `getHoliday`: the first holiday on the shift's date, if any.
    fn holiday_on(&self, shift: &EmployeeShift) -> Option<Holiday> {
        self.calendar
            .holidays_for_job(shift.job_id())
            .into_iter()
            .find(|holiday| holiday.holiday_date() == shift.shift_date())
    }

    /// `isInvalidHomeJob`.
    fn is_invalid_home_job(
        &self,
        shift: &EmployeeShift,
        employee: &Employee,
        params: &RuleParams,
    ) -> bool {
        if !params.bool_at(HOME_DEPT_ONLY_PROP) {
            return false;
        }
        let Some(home) = employee.home_employee_job_status(shift.shift_date()) else {
            return true;
        };
        self.parent_of(shift.job_id()) != self.parent_of(home.job_id())
    }

    fn parent_of(&self, job_id: i32) -> Option<i32> {
        self.calendar
            .assignments()
            .find_by_id(job_id)
            .and_then(|job| job.parent_assignment_id())
    }

    /// `getEarningRate`.
    fn rate(
        &self,
        shift: &EmployeeShift,
        employee: &Employee,
        earning_type: &EarningType,
        params: &RuleParams,
    ) -> Option<f64> {
        let date = shift.shift_date();
        if params.bool_at(HOME_AS_BASE_PROP) {
            earning_rate_home_job(employee, date, earning_type, &self.min_wage)
        } else if params.bool_at(HOME_DEPT_RATE_PROP) {
            let job = self.calendar.assignments().find_by_id(shift.job_id())?;
            match employee.home_employee_job_status(date) {
                Some(home) if job.parent_assignment_id() == self.parent_of(home.job_id()) => {
                    Some(home.hourly_rate())
                }
                _ => Some(job.effective_hourly_pay_rate(date, self.calendar.assignments())),
            }
        } else {
            earning_rate_for_job(employee, shift.job_id(), date, earning_type, &self.min_wage)
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    ShiftEarningRule for HolidayRateFactorRule<T, E, M, H, A>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&HolidayRateFactorRuleConfig.default_values());

        let holiday_types = ids_for_key(HOLIDAY_TYPES_PROP, &params);
        match self.holiday_on(shift) {
            Some(holiday) if holiday_types.contains(&holiday.holiday_type_id()) => {}
            _ => return,
        }
        let Some(employee) = time_card.employee() else {
            return;
        };
        if self.is_invalid_home_job(shift, employee, &params) {
            return;
        }

        let hours = shift
            .net_hours_using(params.bool_at(ACTUAL_PUNCH_PROP))
            .min(params.double_at(MAX_HRS_PAID));
        if hours <= 0.0 {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let Some(rate) = self.rate(shift, employee, &earning_type, &params) else {
            return;
        };
        let factor = params.double_at(FACTOR_PROP);

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
    use crate::entity::assignment_pay_rate::AssignmentPayRate;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings,
    };
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    const NOTE: &str = "Rule: testName - Holiday Rate Factor Earning";
    const JOB: i32 = 5;
    const PARENT: i32 = 50;

    struct Jobs(Vec<Assignment>);
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            self.0.iter().find(|a| a.id() == id).cloned()
        }
    }
    struct Calendar(Vec<Holiday>);
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            self.0.clone()
        }
    }

    fn d(y: i32, m: i32, day: i32) -> LocalDate {
        LocalDate::of(y, m, day)
    }
    fn holidays() -> Vec<Holiday> {
        vec![
            Holiday::new(1, 1, d(2013, 1, 1), "New Year", 1),
            Holiday::new(2, 1, d(2013, 7, 4), "Fourth", 2),
            Holiday::new(3, 1, d(2013, 11, 11), "Veterans", 3),
            Holiday::new(4, 1, d(2013, 12, 25), "Christmas", 4),
        ]
    }
    fn job() -> Assignment {
        Assignment::new(JOB, 1, "job", "J", Some(PARENT))
            .with_pay_rates(vec![AssignmentPayRate::new(d(2012, 12, 31), 11.5)])
    }
    fn status(home: bool, job: i32, rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            if home { 2 } else { 1 },
            1,
            job,
            d(2011, 12, 31),
            d(2013, 12, 31),
            EmployeePayType::Hourly,
            rate,
            home,
        )
    }
    fn employee() -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![status(false, JOB, 10.5), status(true, JOB, 9.5)],
        )
    }
    /// A SCHEDULE shift: netHours 10, adjHours 10.5, 7 punched hours (8 less a
    /// one-hour break) — 17.5 using actual punches.
    fn shift(date: LocalDate) -> EmployeeShift {
        let at = |h, m| LocalDateTime::of(2013, 1, 1, h, m, 0);
        let p = |id, t, when| EmployeeShiftPunch::new(id, t, PunchSource::Clock, when);
        EmployeeShift::new(
            1,
            1,
            JOB,
            date,
            ShiftType::Schedule,
            vec![
                p(1, PunchType::In, at(1, 1)),
                p(2, PunchType::Break, at(4, 1)),
                p(3, PunchType::Back, at(5, 1)),
                p(4, PunchType::Out, at(9, 1)),
            ],
        )
        .with_net_hours(10.0)
        .with_adj_hours(10.5)
    }
    fn item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, NOTE, RuleClass::HolidayFactorDif, params)
    }
    #[allow(clippy::too_many_arguments)]
    fn params(
        earning: &str,
        factor: &str,
        actual: &str,
        home_base: &str,
        home_dept_rate: &str,
        home_dept_only: &str,
        types: &str,
        max: &str,
    ) -> RuleParams {
        rule_params! {
            EARNING => earning, FACTOR_PROP => factor, ACTUAL_PUNCH_PROP => actual,
            HOME_AS_BASE_PROP => home_base, HOME_DEPT_RATE_PROP => home_dept_rate,
            HOME_DEPT_ONLY_PROP => home_dept_only, HOLIDAY_TYPES_PROP => types, MAX_HRS_PAID => max
        }
    }
    fn run_with(
        employee: Employee,
        jobs: Vec<Assignment>,
        s: EmployeeShift,
        uom: UOM,
        p: RuleParams,
    ) -> (TimeCardData, SavedEarnings) {
        let mut card = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(d(2012, 12, 31));
        let saved = SavedEarnings::default();
        let earning_type = EarningType::new(31, 1, "Hol", EarnType::Regular, uom)
            .with_flags(true, false, false, false);
        HolidayRateFactorRule::new(
            FixedEarningType(earning_type),
            saved.clone(),
            FixedMinWage(7.79),
            Calendar(holidays()),
            Jobs(jobs),
        )
        .execute(&s, &mut card, &item(p));
        (card, saved)
    }
    fn run(s: EmployeeShift, uom: UOM, p: RuleParams) -> (TimeCardData, SavedEarnings) {
        run_with(employee(), vec![job()], s, uom, p)
    }

    // ---- java_parity_tests: HolidayRateFactorRuleImplTest.groovy ----

    #[test]
    #[allow(clippy::type_complexity)]
    fn rule_creates_appropriate_earning_with_correct_hours_rate_and_dollars() {
        use UOM::{Dollars as D, Hours as H};
        // (params, shift date, uom, expected (dollars, rate, hours) or None for no earning)
        // Row 1 of the Groovy table expects zero calls only because its expected
        // earning is all zero; a real earning *is* created — see the next test.
        let rows: Vec<(RuleParams, LocalDate, UOM, Option<(f64, f64, f64)>)> = vec![
            (
                params("1", "0.5", "true", "true", "true", "true", "[1]", "99"),
                d(2012, 1, 1),
                D,
                None,
            ),
            (
                params("1", "0.5", "true", "true", "true", "true", "[1]", "99"),
                d(2013, 7, 4),
                D,
                None,
            ),
            (
                params("1", "0.5", "true", "true", "true", "false", "[1]", "99"),
                d(2013, 1, 1),
                D,
                Some((83.125, 0.0, 0.0)),
            ),
            (
                params("1", "0.5", "true", "true", "false", "false", "[2]", "99"),
                d(2013, 7, 4),
                D,
                Some((83.125, 0.0, 0.0)),
            ),
            (
                params("1", "0.5", "true", "false", "false", "false", "[2]", "99"),
                d(2013, 7, 4),
                D,
                Some((91.875, 0.0, 0.0)),
            ),
            (
                params("1", "0.5", "false", "false", "false", "false", "[2]", "99"),
                d(2013, 7, 4),
                D,
                Some((52.5, 0.0, 0.0)),
            ),
            (
                params("1", "0.75", "false", "false", "false", "false", "[3]", "99"),
                d(2013, 11, 11),
                D,
                Some((78.75, 0.0, 0.0)),
            ),
            (
                params("1", "0.75", "true", "true", "true", "false", "[3]", "99"),
                d(2013, 11, 11),
                H,
                Some((0.0, 7.125, 17.5)),
            ),
            (
                params("1", "0.75", "true", "true", "false", "false", "[3]", "99"),
                d(2013, 11, 11),
                H,
                Some((0.0, 7.125, 17.5)),
            ),
            (
                params("1", "0.75", "true", "false", "false", "false", "[4]", "99"),
                d(2013, 12, 25),
                H,
                Some((0.0, 7.875, 17.5)),
            ),
            (
                params("1", "0.75", "false", "false", "false", "false", "[4]", "99"),
                d(2013, 12, 25),
                H,
                Some((0.0, 7.875, 10.0)),
            ),
            (
                params("1", "0.75", "true", "true", "true", "false", "[3]", "10"),
                d(2013, 11, 11),
                H,
                Some((0.0, 7.125, 10.0)),
            ),
            (
                params("1", "0.75", "true", "true", "true", "false", "[3]", "10"),
                d(2013, 11, 11),
                D,
                Some((71.25, 0.0, 0.0)),
            ),
        ];
        for (i, (p, date, uom, expected)) in rows.into_iter().enumerate() {
            let (card, saved) = run(shift(date), uom, p);
            match expected {
                None => assert!(saved.0.borrow().is_empty(), "row {i}"),
                Some((dollars, rate, hours)) => {
                    assert_eq!(saved.0.borrow().len(), 1, "row {i}");
                    let e = &card.earnings()[0];
                    assert_eq!(e.earning_date(), date, "row {i}");
                    assert_eq!(e.pay_date(), Some(date));
                    assert_eq!(e.employee_id(), 1);
                    assert_eq!(e.job_id(), JOB);
                    assert_eq!(e.source(), EarningSource::Rule);
                    assert_eq!(e.note(), NOTE);
                    assert_eq!(e.rule_item_id(), Some(1));
                    assert_eq!(e.shift_id(), Some(1));
                    assert_eq!(
                        (e.dollars(), e.rate(), e.hours()),
                        (dollars, rate, hours),
                        "row {i}"
                    );
                }
            }
        }
    }

    /// The Groovy table's first row — earning "0", type 1, a New Year's shift,
    /// HOURS — expects an all-zero earning and zero saves. The mock matcher only
    /// counts saves *equal to* that all-zero earning, so the spec passes while the
    /// rule really does pay: 17.5 hours at 9.5 × 0.5. Preserved, not "fixed".
    #[test]
    fn the_first_groovy_row_is_vacuous_the_rule_really_pays() {
        let (card, saved) = run(
            shift(d(2013, 1, 1)),
            UOM::Hours,
            params("0", "0.5", "true", "true", "false", "false", "[1]", "99"),
        );
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.rate(), e.hours()), (0.0, 4.75, 17.5));
    }

    // ---- behaviour tests from the Java ----

    fn new_years() -> EmployeeShift {
        shift(d(2013, 1, 1))
    }
    fn base() -> RuleParams {
        params("1", "1", "false", "true", "false", "false", "[1]", "99")
    }

    #[test]
    fn a_shift_with_errors_or_in_a_closed_period_earns_nothing() {
        let (_, saved) = run(
            new_years().with_errors(vec![ShiftErrorType::MissingOut]),
            UOM::Hours,
            base(),
        );
        assert!(saved.0.borrow().is_empty());
        let s = new_years();
        let mut card = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(d(2013, 1, 2));
        let saved = SavedEarnings::default();
        HolidayRateFactorRule::new(
            FixedEarningType(EarningType::new(
                31,
                1,
                "Hol",
                EarnType::Regular,
                UOM::Hours,
            )),
            saved.clone(),
            FixedMinWage(0.0),
            Calendar(holidays()),
            Jobs(vec![job()]),
        )
        .execute(&s, &mut card, &item(base()));
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn the_holiday_type_must_be_configured() {
        let (_, saved) = run(
            new_years(),
            UOM::Hours,
            params("1", "1", "false", "true", "false", "false", "[2,3]", "99"),
        );
        assert!(saved.0.borrow().is_empty());
        let (_, saved) = run(
            new_years(),
            UOM::Hours,
            params("1", "1", "false", "true", "false", "false", "[]", "99"),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_shift_with_no_hours_earns_nothing() {
        let (_, saved) = run(new_years().with_net_hours(0.0), UOM::Hours, base());
        assert!(saved.0.borrow().is_empty());
        let (_, saved) = run(new_years().with_net_hours(-1.0), UOM::Hours, base());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn home_dept_only_needs_a_home_job_in_the_shift_jobs_department() {
        let only = params("1", "1", "false", "true", "false", "true", "[1]", "99");
        // Same department: paid.
        let (_, saved) = run(new_years(), UOM::Hours, only.clone());
        assert_eq!(saved.0.borrow().len(), 1);

        // Home job in another department: not paid.
        let other_job = Assignment::new(6, 1, "other", "O", Some(PARENT + 1));
        let away = Employee::new(
            1,
            1,
            "Pat",
            vec![status(true, 6, 9.5), status(false, JOB, 10.5)],
        );
        let (_, saved) = run_with(
            away,
            vec![job(), other_job],
            new_years(),
            UOM::Hours,
            only.clone(),
        );
        assert!(saved.0.borrow().is_empty());

        // No home job at all: not paid.
        let no_home = Employee::new(1, 1, "Pat", vec![status(false, JOB, 10.5)]);
        let (_, saved) = run_with(no_home, vec![job()], new_years(), UOM::Hours, only);
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn home_dept_rate_uses_the_home_rate_in_the_same_department_else_the_jobs_pay_rate() {
        let dept_rate = params("1", "1", "false", "false", "true", "false", "[1]", "99");
        let (card, _) = run(new_years(), UOM::Hours, dept_rate.clone());
        assert_eq!(card.earnings()[0].rate(), 9.5);

        let other_job = Assignment::new(6, 1, "other", "O", Some(PARENT + 1));
        let away = Employee::new(
            1,
            1,
            "Pat",
            vec![status(true, 6, 9.5), status(false, JOB, 10.5)],
        );
        let (card, _) = run_with(
            away,
            vec![job(), other_job],
            new_years(),
            UOM::Hours,
            dept_rate,
        );
        assert_eq!(card.earnings()[0].rate(), 11.5);
    }

    #[test]
    fn the_home_rate_is_floored_at_minimum_wage_for_a_type_that_asks() {
        let cheap = Employee::new(1, 1, "Pat", vec![status(true, JOB, 5.0)]);
        let (card, _) = run_with(cheap, vec![job()], new_years(), UOM::Hours, base());
        assert_eq!(card.earnings()[0].rate(), 7.79);
    }

    #[test]
    fn max_hours_caps_the_hours_paid() {
        let (card, _) = run(
            new_years(),
            UOM::Hours,
            params("1", "1", "true", "true", "false", "false", "[1]", "4"),
        );
        assert_eq!(card.earnings()[0].hours(), 4.0);
    }
}
