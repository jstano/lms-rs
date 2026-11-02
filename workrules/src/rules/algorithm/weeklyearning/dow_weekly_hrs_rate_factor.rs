//! Port of `DOWWeeklyHrsRateFactorRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/DOWWeeklyHrsRateFactorRuleImpl.java`.
//!
//! Pays the **whole week's hours** at `rateFactor × homeRate`, as a single
//! earning dated the configured day of the week. (A "weekly overtime paid on
//! Monday" shape.)
//!
//! - The hours are the week's valid shifts (no persisted errors) at
//!   `ShiftUtil.getNetHours`, plus the hours of the week's `REGULAR` and
//!   `PREMIUM` earnings, rounded to two places at each step.
//! - The earning's date is the **first** date, scanning shifts and then earnings
//!   in card order, that falls on `dow` (database numbering: Sunday = 1). No
//!   hours on that weekday means no earning, however many hours were worked
//!   on others.
//! - It needs hours above zero, an open date and a home job on that date.
//! - For an `HOURS` earning type it pays `hours` at `round_currency(rate ×
//!   factor)`; for any other unit `round_currency(factor × hours × rate)`
//!   dollars.
//!
//! The Java class keeps `dowDate`/`hours` in instance fields and resets them on
//! each `execute`; the port uses locals.

use crate::common::dates::translate_dow_from_iso;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ACTUAL_PUNCH_PROP, DOW_EARNING_PROP, DOW_PROP, DOWWeeklyHrsRateFactorRuleConfig, FACTOR_PROP,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, rule_earning};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `DOWWeeklyHrsRateFactorRuleImpl`.
pub struct DOWWeeklyHrsRateFactorRule<T: EarningTypePort, E: EmployeeEarningPort> {
    earning_types: T,
    earnings: RefCell<E>,
}

impl<T: EarningTypePort, E: EmployeeEarningPort> DOWWeeklyHrsRateFactorRule<T, E> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort> WeeklyEarningRule
    for DOWWeeklyHrsRateFactorRule<T, E>
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
            .fixed(&DOWWeeklyHrsRateFactorRuleConfig.default_values());
        let dow = params.int_at(DOW_PROP);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let factor = params.double_at(FACTOR_PROP);

        let mut dow_date: Option<LocalDate> = None;
        let mut hours = 0.0;
        let note_dow = |date: LocalDate, dow_date: &mut Option<LocalDate>| {
            if dow_date.is_none() && translate_dow_from_iso(date.day_of_week().value()) == dow {
                *dow_date = Some(date);
            }
        };

        for shift in time_card.shifts() {
            let date = shift.shift_date();
            if work_week.contains_date(date) && !shift.has_errors() {
                note_dow(date, &mut dow_date);
                hours = round_hours(hours + shift.net_hours_using(use_actual));
            }
        }
        for earning in time_card.earnings() {
            let date = earning.earning_date();
            let counts = work_week.contains_date(date)
                && self
                    .earning_types
                    .find_by_id(earning.earning_type_id())
                    .is_some_and(|t| t.earn_type().is_regular() || t.earn_type().is_premium());
            if counts {
                note_dow(date, &mut dow_date);
                hours = round_hours(hours + earning.hours());
            }
        }

        let Some(date) = dow_date else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        if !(hours > 0.0 && time_card.is_open_for_editing_on(date)) {
            return;
        }
        let Some(earning_type) = self
            .earning_types
            .find_by_id(params.int_at(DOW_EARNING_PROP))
        else {
            return;
        };

        let mut earning = rule_earning(
            employee.id(),
            home.job_id(),
            earning_type.id(),
            date,
            rule_item,
            None,
        );
        if earning_type.uom().is_hours() {
            earning.set_rate(round_currency(home.hourly_rate() * factor));
            earning.set_hours(hours);
        } else {
            earning.set_dollars(round_currency(factor * hours * home.hourly_rate()));
        }
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_description);

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
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::entity::earning_type::EarningType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use std::collections::HashMap;

    const JOB: i32 = 1;
    const DESC: &str = "rule description";

    fn d(m: i32, day: i32) -> LocalDate {
        LocalDate::of(2013, m, day)
    }
    fn dow_date() -> LocalDate {
        d(7, 1)
    }
    fn week() -> DateRange {
        DateRange::new(d(7, 1), d(7, 7))
    }

    /// Ids: 2 is the configured earning (HOURS); 101/102/103 are the
    /// REGULAR/PREMIUM/EARNING types the fixture earnings carry.
    struct Types(HashMap<i32, EarningType>);
    impl EarningTypePort for Types {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }
    fn types() -> Types {
        let t = |id, et| EarningType::new(id, 1, "t", et, UOM::Hours);
        Types(HashMap::from([
            (2, t(2, EarnType::Earning)),
            (101, t(101, EarnType::Regular)),
            (102, t(102, EarnType::Premium)),
            (103, t(103, EarnType::Earning)),
        ]))
    }

    /// `LaborTestUtils.shiftWithPunches`: an 8h / 4h shift whose stored net
    /// hours are the punch span.
    fn shift(id: i32, date: LocalDate, hours: f64) -> EmployeeShift {
        EmployeeShift::new(id, 1, JOB, date, ShiftType::Actual, Vec::new()).with_net_hours(hours)
    }

    fn earning(type_id: i32, date: LocalDate) -> EmployeeEarning {
        EmployeeEarning::new(0, 1, JOB, type_id, date, 5.0, 0.0, EarningSource::Auto)
    }

    fn params(factor: &str) -> RuleParams {
        rule_params! { DOW_EARNING_PROP => "2", DOW_PROP => "2", FACTOR_PROP => factor, ACTUAL_PUNCH_PROP => "false" }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_row(
        rate: f64,
        calc_date: LocalDate,
        shifts: Vec<EmployeeShift>,
        earnings: Vec<EmployeeEarning>,
        factor: &str,
        scenario: &str,
        pay: f64,
        saves: usize,
    ) {
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                week().start_date().minus_years(1),
                week().start_date().plus_years(1),
                EmployeePayType::Hourly,
                rate,
                true,
            )],
        );
        let mut card = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(shifts)
            .with_earnings(earnings)
            .with_calculation_start_date(calc_date);
        let before = card.earnings().len();
        let saved = SavedEarnings::default();
        let item = RuleItem::new(1, 1, DESC, RuleClass::DowWeeklyHrsFactorWdf, params(factor));
        DOWWeeklyHrsRateFactorRule::new(types(), saved.clone()).execute(
            &mut card,
            &week(),
            &item,
            DESC,
        );

        assert_eq!(saved.0.borrow().len(), saves, "{scenario}");
        if saves == 1 {
            let e = card.earnings().last().unwrap();
            assert_eq!(card.earnings().len(), before + 1, "{scenario}");
            assert_eq!(e.earning_date(), dow_date(), "{scenario}");
            assert_eq!(e.pay_date(), Some(dow_date()), "{scenario}");
            assert_eq!(e.earning_type_id(), 2);
            assert_eq!(e.job_id(), JOB);
            assert_eq!(e.source(), EarningSource::Rule);
            assert_eq!(e.rule_item_id(), Some(1));
            assert_eq!(e.total_dollars(), pay, "{scenario}");
            assert!(e.shift_id().is_none());
        } else {
            assert_eq!(card.earnings().len(), before, "{scenario}");
        }
    }

    // ---- java_parity_tests: DOWWeeklyHrsRateFactorRuleImplTest.groovy ----

    #[test]
    fn test_execute_rows() {
        let dow1 = shift(1, dow_date(), 8.0);
        let dow2 = shift(2, dow_date(), 4.0);
        let before = shift(3, d(6, 30), 8.0);
        let after = shift(4, d(7, 8), 8.0);
        let extra1 = shift(5, d(7, 2), 8.0);
        let extra2 = shift(6, d(7, 6), 8.0);
        let dow_error = shift(7, dow_date(), 8.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let extra_error = shift(8, d(7, 7), 8.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let (dow_reg, dow_prem, dow_earn) = (
            earning(101, dow_date()),
            earning(102, dow_date()),
            earning(103, dow_date()),
        );
        let (extra_reg, extra_prem, extra_earn) = (
            earning(101, d(7, 2)),
            earning(102, d(7, 2)),
            earning(103, d(7, 2)),
        );

        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            LocalDate,
            Vec<EmployeeShift>,
            Vec<EmployeeEarning>,
            &str,
            f64,
            usize,
            &str,
        )> = vec![
            (
                dow_date(),
                vec![dow1.clone()],
                vec![],
                ".5",
                40.0,
                1,
                "default scenario",
            ),
            (dow_date(), vec![], vec![], ".5", 0.0, 0, "no shifts"),
            (
                dow_date(),
                vec![extra1.clone()],
                vec![],
                ".5",
                0.0,
                0,
                "no shift on date",
            ),
            (
                dow_date(),
                vec![dow1.clone(), extra1.clone(), extra2.clone()],
                vec![],
                ".5",
                120.0,
                1,
                "shifts through week",
            ),
            (
                dow_date(),
                vec![dow1.clone(), before.clone(), after.clone()],
                vec![],
                ".5",
                40.0,
                1,
                "shifts outside week",
            ),
            (
                dow_date(),
                vec![dow1.clone(), dow2],
                vec![],
                ".5",
                60.0,
                1,
                "multiple on date",
            ),
            (
                dow_date(),
                vec![before, after],
                vec![],
                ".5",
                0.0,
                0,
                "no shifts in week",
            ),
            (
                dow_date(),
                vec![dow1.clone(), extra1.clone(), extra2.clone()],
                vec![],
                ".25",
                60.0,
                1,
                "change factor",
            ),
            (
                dow_date(),
                vec![dow_error, extra1.clone(), extra2],
                vec![],
                ".5",
                0.0,
                0,
                "error on dow",
            ),
            (
                dow_date(),
                vec![dow1.clone(), extra1.clone(), extra_error],
                vec![],
                ".5",
                80.0,
                1,
                "error on extra day",
            ),
            (
                d(7, 5),
                vec![dow1.clone()],
                vec![],
                ".5",
                0.0,
                0,
                "before calc start",
            ),
            (
                dow_date(),
                vec![],
                vec![dow_reg],
                ".5",
                25.0,
                1,
                "dow regular earning",
            ),
            (
                dow_date(),
                vec![],
                vec![dow_prem],
                ".5",
                25.0,
                1,
                "dow premium earning",
            ),
            (
                dow_date(),
                vec![],
                vec![dow_earn],
                ".5",
                0.0,
                0,
                "dow earning",
            ),
            (
                dow_date(),
                vec![dow1.clone()],
                vec![extra_reg.clone()],
                ".5",
                65.0,
                1,
                "extra reg earning",
            ),
            (
                dow_date(),
                vec![dow1.clone()],
                vec![extra_prem],
                ".5",
                65.0,
                1,
                "extra prem earning",
            ),
            (
                dow_date(),
                vec![dow1],
                vec![extra_earn],
                ".5",
                40.0,
                1,
                "extra earning",
            ),
            (
                dow_date(),
                vec![],
                vec![extra_reg],
                ".5",
                0.0,
                0,
                "extra reg earning only",
            ),
        ];
        for (calc, shifts, earnings, factor, pay, saves, scenario) in rows {
            run_row(10.0, calc, shifts, earnings, factor, scenario, pay, saves);
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_non_hours_earning_type_pays_dollars() {
        let employee = Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                d(1, 1),
                d(12, 31),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        );
        let mut card = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift(1, dow_date(), 8.0)]);
        let dollars_type = Types(HashMap::from([(
            2,
            EarningType::new(2, 1, "$", EarnType::Earning, UOM::Dollars),
        )]));
        let saved = SavedEarnings::default();
        DOWWeeklyHrsRateFactorRule::new(dollars_type, saved).execute(
            &mut card,
            &week(),
            &RuleItem::new(1, 1, DESC, RuleClass::DowWeeklyHrsFactorWdf, params(".5")),
            DESC,
        );
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.hours(), e.rate()), (40.0, 0.0, 0.0));
    }
}
