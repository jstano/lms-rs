//! Port of `HolidayTimeOfDayRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/HolidayTimeOfDayRuleImpl.java`.
//!
//! Pays the **hours of a shift that fall inside a holiday's time-of-day
//! window**. The holiday is any of the property's holidays of a configured type
//! that the shift overlaps ([`HolidayMatcher`]); each holiday's window is its
//! date from `Start Time` to `End Time` (midnight to midnight, a full day, by
//! default). The hours are the shift's span inside each window less the breaks
//! inside it, summed with `round_hours` at each step, so a shift crossing
//! midnight into a holiday pays only the holiday side.
//!
//! The rate is the shift's rate (home job's with `homeAsBase`) **plus, with
//! `includeDiffs`, every other differential already earned on this shift**:
//! each rule-created earning of an `EARNING`-kind type attached to this shift
//! adds `round_currency(total dollars / net hours)` per hour. The rule therefore
//! depends on its position among the other shift rules. The earning pays
//! `hours` at `round_currency(rate × factor)` or, for a `DOLLARS` type,
//! `round_currency(round_currency(rate × hours) × factor)` dollars.
//!
//! A shift with errors or outside the open pay period is skipped before any
//! parameter is read.

use crate::common::enums::earning_source::EarningSource;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::{round_currency, round_hours};
use crate::entity::earning_type::EarningType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::dailyearning::{earning_rate_for_job, earning_rate_home_job};
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, END_TIME_PROP, FACTOR_PROP, HOLIDAY_TOD_EARNING_PROP, HOLIDAY_TYPES_PROP,
    HOME_AS_BASE_PROP, HolidayTimeOfDayRuleConfig, INCLUDE_DIFFS, START_TIME_PROP,
};
use crate::rules::algorithm::shiftearning::utility::holiday_matcher::{
    HolidayMatcher, holiday_period,
};
use crate::rules::algorithm::shiftearning::utility::shift_time_in_period_calculator::calculate_time_in_period;
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{
    AssignmentPort, EarningTypePort, EmployeeEarningPort, HolidayPort, MinWagePort,
};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalTime;
use std::cell::RefCell;

/// `HolidayTimeOfDayRuleImpl`.
pub struct HolidayTimeOfDayRule<
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

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    HolidayTimeOfDayRule<T, E, M, H, A>
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

    /// `computeOtherDifferentialRates`.
    fn other_differential_rates(
        &self,
        shift: &EmployeeShift,
        time_card: &dyn TimeCard,
        use_actual: bool,
    ) -> f64 {
        let net_hours = shift.net_hours_using(use_actual);
        if net_hours == 0.0 {
            return 0.0;
        }
        time_card
            .earnings()
            .iter()
            .filter(|earning| {
                earning.shift_id() == Some(shift.id())
                    && earning.source() == EarningSource::Rule
                    && self
                        .earning_types
                        .find_by_id(earning.earning_type_id())
                        .is_some_and(|kind| kind.earn_type().is_earning())
            })
            .map(|earning| round_currency(earning.total_dollars() / net_hours))
            .sum()
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, M: MinWagePort, H: HolidayPort, A: AssignmentPort>
    ShiftEarningRule for HolidayTimeOfDayRule<T, E, M, H, A>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&HolidayTimeOfDayRuleConfig.default_values());
        let earning_id = params.int_at(HOLIDAY_TOD_EARNING_PROP);
        let factor = params.double_at(FACTOR_PROP);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let start_time = LocalTime::parse(params.get(START_TIME_PROP).unwrap_or("00:00:00"));
        let end_time = LocalTime::parse(params.get(END_TIME_PROP).unwrap_or("00:00:00"));
        let holiday_types = ids_for_key(HOLIDAY_TYPES_PROP, &params);

        // The matcher always uses the default midnight window; `Start Time` and
        // `End Time` only shape the period the hours are measured in.
        let holidays = self
            .matcher
            .find_overlapping_holidays(shift, &holiday_types, use_actual);
        let mut premium_time = 0.0;
        for holiday in &holidays {
            let period = holiday_period(holiday.holiday_date(), start_time, end_time);
            premium_time = round_hours(
                calculate_time_in_period(shift, &period, use_actual, true) + premium_time,
            );
        }
        if premium_time <= 0.0 {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(earning_id) else {
            return;
        };
        let Some(employee) = time_card.employee() else {
            return;
        };
        let date = shift.shift_date();
        let base_rate = if params.bool_at(HOME_AS_BASE_PROP) {
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
        let Some(base_rate) = base_rate else {
            return;
        };
        let diffs = if params.bool_at(INCLUDE_DIFFS) {
            self.other_differential_rates(shift, time_card, use_actual)
        } else {
            0.0
        };
        let premium_rate = base_rate + diffs;

        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        earning.set_note(rule_item.name());
        pay(
            &mut earning,
            &earning_type,
            premium_rate,
            premium_time,
            factor,
        );
        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

fn pay(
    earning: &mut crate::entity::employee_earning::EmployeeEarning,
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
    earning.calc_and_set_total_dollars();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee::Employee;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::holiday::Holiday;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, FixedMinWage, SavedEarnings,
    };
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    const JOB: i32 = 1;

    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            Some(Assignment::new(id, 1, "job", "J", None))
        }
    }
    struct Calendar;
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            vec![Holiday::new(
                1,
                1,
                LocalDate::of(2012, 11, 22),
                "Thanksgiving",
                5,
            )]
        }
    }

    fn at(day: i32, h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2012, 11, day, h, m, 0)
    }
    fn shift(punches: &[(PunchType, LocalDateTime)]) -> EmployeeShift {
        let list: Vec<EmployeeShiftPunch> = punches
            .iter()
            .enumerate()
            .map(|(i, (t, when))| {
                EmployeeShiftPunch::new(i as i32 + 1, *t, PunchSource::Clock, *when)
            })
            .collect();
        let first = punches.first().unwrap().1;
        let last = punches.last().unwrap().1;
        EmployeeShift::new(1, 1, JOB, first.to_local_date(), ShiftType::Actual, list)
            .with_times(Some(first), Some(last))
    }
    fn employee(rate: f64) -> Employee {
        let day = LocalDate::of(2012, 11, 22);
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                day.minus_years(1),
                day.plus_years(1),
                EmployeePayType::Hourly,
                rate,
                true,
            )],
        )
    }
    fn item(extra: RuleParams) -> RuleItem {
        let mut p =
            rule_params! { HOLIDAY_TOD_EARNING_PROP => "22", HOLIDAY_TYPES_PROP => "[5,6]" };
        for (k, v) in extra.iter() {
            p.set(k, v);
        }
        RuleItem::new(1, 1, "holiday rule", RuleClass::HolidayTodDif, p)
    }
    fn run(
        s: &EmployeeShift,
        existing: Vec<EmployeeEarning>,
        extra: RuleParams,
    ) -> (TimeCardData, SavedEarnings) {
        let mut card = TimeCardData::new()
            .with_employee(employee(20.0))
            .with_shifts(vec![s.clone()])
            .with_earnings(existing)
            .with_calculation_start_date(LocalDate::of(2012, 11, 15));
        let saved = SavedEarnings::default();
        HolidayTimeOfDayRule::new(
            FixedEarningType(EarningType::new(
                22,
                1,
                "Hol",
                EarnType::Earning,
                UOM::Hours,
            )),
            saved.clone(),
            FixedMinWage(0.0),
            Calendar,
            Jobs,
        )
        .execute(s, &mut card, &item(extra));
        (card, saved)
    }

    // ---- java_parity_tests: HolidayTimeOfDayRuleImplTest.groovy ----

    #[test]
    fn errors_do_not_create_earnings() {
        let s = shift(&[
            (PunchType::In, at(22, 8, 0)),
            (PunchType::Out, at(22, 16, 0)),
        ])
        .with_errors(vec![ShiftErrorType::MissingOut]);
        let (card, saved) = run(&s, vec![], RuleParams::new());
        assert!(saved.0.borrow().is_empty());
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn shifts_before_the_calculation_start_date_do_not_create_earnings() {
        let s = shift(&[(PunchType::In, at(1, 8, 0)), (PunchType::Out, at(1, 16, 0))]);
        let (card, saved) = run(&s, vec![], RuleParams::new());
        assert!(saved.0.borrow().is_empty());
        assert!(card.earnings().is_empty());
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn test_acceptance_criteria() {
        use PunchType::{Back, Break, In, Out};
        // (scenario, punches, holiday hours)
        let scenarios: Vec<(&str, Vec<(PunchType, LocalDateTime)>, f64)> = vec![
            ("1", vec![(In, at(21, 23, 0)), (Out, at(22, 7, 0))], 7.0),
            ("2", vec![(In, at(22, 20, 0)), (Out, at(23, 5, 0))], 4.0),
            ("3", vec![(In, at(22, 8, 0)), (Out, at(22, 17, 0))], 9.0),
            ("4a", vec![(In, at(21, 22, 0)), (Out, at(22, 5, 0))], 5.0),
            ("4b", vec![(In, at(22, 14, 0)), (Out, at(22, 20, 0))], 6.0),
            (
                "5",
                vec![
                    (In, at(21, 20, 0)),
                    (Break, at(21, 23, 30)),
                    (Back, at(22, 0, 45)),
                    (Out, at(22, 4, 0)),
                ],
                3.25,
            ),
            (
                "6",
                vec![
                    (In, at(22, 20, 0)),
                    (Break, at(22, 23, 30)),
                    (Back, at(23, 0, 45)),
                    (Out, at(23, 4, 0)),
                ],
                3.5,
            ),
            (
                "7",
                vec![
                    (In, at(21, 20, 0)),
                    (Break, at(22, 1, 0)),
                    (Back, at(22, 1, 45)),
                    (Out, at(22, 4, 0)),
                ],
                3.25,
            ),
            (
                "8",
                vec![
                    (In, at(22, 16, 0)),
                    (Break, at(22, 20, 30)),
                    (Back, at(22, 22, 0)),
                    (Out, at(23, 4, 0)),
                ],
                6.5,
            ),
            (
                "9",
                vec![
                    (In, at(22, 8, 0)),
                    (Break, at(22, 12, 0)),
                    (Back, at(22, 12, 30)),
                    (Out, at(22, 16, 0)),
                ],
                7.5,
            ),
        ];
        for (name, punches, hours) in scenarios {
            let (card, saved) = run(&shift(&punches), vec![], RuleParams::new());
            assert_eq!(saved.0.borrow().len(), 1, "scenario {name}");
            assert_eq!(card.earnings().len(), 1, "scenario {name}");
            assert_eq!(card.earnings()[0].hours(), hours, "scenario {name}");
        }
    }

    #[test]
    fn test_rate_calculation_hourly_earning() {
        use PunchType::{In, Out};
        // (include diffs, punches, diff dollars, diff is a rule earning on the shift, hours, rate, total)
        let rows = [
            (
                true,
                (at(22, 15, 0), at(22, 23, 0)),
                16.0,
                true,
                8.0,
                11.0,
                88.0,
            ),
            (
                true,
                (at(21, 23, 0), at(22, 7, 0)),
                16.0,
                true,
                7.0,
                11.0,
                77.0,
            ),
            (
                true,
                (at(22, 4, 0), at(22, 12, 0)),
                8.0,
                true,
                8.0,
                10.5,
                84.0,
            ),
            (
                true,
                (at(22, 15, 0), at(22, 23, 0)),
                16.0,
                false,
                8.0,
                10.0,
                80.0,
            ),
            (
                false,
                (at(22, 15, 0), at(22, 23, 0)),
                16.0,
                true,
                8.0,
                10.0,
                80.0,
            ),
        ];
        for (i, (include, (from, to), diff_dollars, rule_based, hours, rate, total)) in
            rows.into_iter().enumerate()
        {
            let s = shift(&[(In, from), (Out, to)]);
            let mut diff = EmployeeEarning::new(
                0,
                1,
                JOB,
                99,
                from.to_local_date(),
                0.0,
                0.0,
                if rule_based {
                    EarningSource::Rule
                } else {
                    EarningSource::Manual
                },
            );
            if rule_based {
                diff = diff.from_rule(7, Some(s.id()));
            }
            diff.set_total_dollars(diff_dollars);
            let (card, saved) = run(
                &s,
                vec![diff],
                rule_params! { INCLUDE_DIFFS => if include { "true" } else { "false" } },
            );
            assert_eq!(saved.0.borrow().len(), 1, "row {i}");
            assert_eq!(card.earnings().len(), 2, "row {i}");
            let e = &card.earnings()[1];
            assert_eq!(
                (e.hours(), e.rate(), e.total_dollars()),
                (hours, rate, total),
                "row {i}"
            );
        }
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_dollars_type_pays_dollars_with_no_hours_or_rate() {
        let s = shift(&[
            (PunchType::In, at(22, 8, 0)),
            (PunchType::Out, at(22, 16, 0)),
        ]);
        let mut card = TimeCardData::new()
            .with_employee(employee(20.0))
            .with_shifts(vec![s.clone()])
            .with_calculation_start_date(LocalDate::of(2012, 11, 15));
        HolidayTimeOfDayRule::new(
            FixedEarningType(EarningType::new(
                22,
                1,
                "Hol",
                EarnType::Earning,
                UOM::Dollars,
            )),
            SavedEarnings::default(),
            FixedMinWage(0.0),
            Calendar,
            Jobs,
        )
        .execute(&s, &mut card, &item(RuleParams::new()));
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.rate(), e.hours()), (80.0, 0.0, 0.0));
    }

    #[test]
    fn no_holiday_of_a_configured_type_means_no_earning() {
        let s = shift(&[
            (PunchType::In, at(22, 8, 0)),
            (PunchType::Out, at(22, 16, 0)),
        ]);
        let (card, _) = run(&s, vec![], rule_params! { HOLIDAY_TYPES_PROP => "[6,7]" });
        assert!(card.earnings().is_empty());
    }

    #[test]
    fn the_configured_window_narrows_the_hours_counted() {
        let s = shift(&[
            (PunchType::In, at(22, 8, 0)),
            (PunchType::Out, at(22, 16, 0)),
        ]);
        let (card, _) = run(
            &s,
            vec![],
            rule_params! { START_TIME_PROP => "10:00:00", END_TIME_PROP => "14:00:00" },
        );
        assert_eq!(card.earnings()[0].hours(), 4.0);
    }

    #[test]
    fn with_no_other_differentials_the_rate_is_the_plain_shift_rate() {
        let s = shift(&[
            (PunchType::In, at(22, 8, 0)),
            (PunchType::Out, at(22, 16, 0)),
        ]);
        let (card, _) = run(&s, vec![], RuleParams::new());
        assert_eq!(card.earnings()[0].rate(), 10.0);
    }
}
