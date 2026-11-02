//! Port of `ContractToFullTimeRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/ContractToFullTimeRuleImpl.java`.
//!
//! Pays a differential on the hours a contracted employee works **between their
//! contract hours and full-time hours** in a week — hours that are neither
//! covered by the contract nor yet overtime-like full-time hours.
//!
//! - The week is the work week containing the shift date, anchored on the
//!   property's pay-period end date (`Weeks`).
//! - The hours before this shift are the net hours of every shift in the week that
//!   sorts earlier by start time (the loop stops at this shift); a shift not on
//!   the card is therefore preceded by all of them.
//! - `weekly = before + this shift`. Over contract = `max(0, weekly − contract)`
//!   with the contract taken from the **home** job status on the shift date (zero
//!   with no home job); over full time = `max(0, weekly − fullTimeHours)`.
//!   Premium hours = `min(over contract, this shift's hours) − over full time`.
//! - `HOURS`, `HOURS_DOLLARS` and `UNITS` earning types pay premium hours at
//!   `diffRate`; `DOLLARS` pays `round_currency(diffRate × premium)`. Any other
//!   unit leaves the earning empty, and an earning with neither dollars nor hours
//!   is discarded.
//! - Unlike the other rules there is no `isShiftInvalid` helper: the gate is no
//!   errors and open for editing, and the parameters are read afterwards.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::uom::UOM;
use crate::common::numbers::round_currency;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ACTUAL_PUNCH_PROP, ContractToFullTimeRuleConfig, DIFF_RATE_PROP, EARNING, FULL_TIME_HOURS,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{EarningTypePort, EmployeeEarningPort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use std::cell::RefCell;

/// `ContractToFullTimeRuleImpl`.
pub struct ContractToFullTimeRule<T: EarningTypePort, E: EmployeeEarningPort, P: PropertyPort> {
    earning_types: T,
    earnings: RefCell<E>,
    property: P,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, P: PropertyPort> ContractToFullTimeRule<T, E, P> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, property: P) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            property,
        }
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, P: PropertyPort> ShiftEarningRule
    for ContractToFullTimeRule<T, E, P>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&ContractToFullTimeRuleConfig.default_values());
        let diff_rate = params.double_at(DIFF_RATE_PROP);
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let full_time_hours = params.double_at(FULL_TIME_HOURS);

        let Some(employee) = time_card.employee() else {
            return;
        };
        let weekly_contract = employee
            .home_employee_job_status(shift.shift_date())
            .map_or(0.0, |home| home.contract_hours().unwrap_or(0.0));

        let week =
            WeeklyDateRange::with_end_date(self.property.period_end_date(employee.property_id()))
                .range_containing_date(shift.shift_date());
        let mut week_shifts = time_card.shifts_for_period(&week);
        week_shifts.sort_by_key(|s| s.start_time_for_ordering());

        let mut weekly_hours = 0.0;
        for earlier in week_shifts {
            if earlier.id() == shift.id() {
                break;
            }
            weekly_hours += earlier.net_hours_using(use_actual);
        }
        let shift_hours = shift.net_hours_using(use_actual);
        weekly_hours += shift_hours;

        let over_full_time = (weekly_hours - full_time_hours).max(0.0);
        let over_contract = (weekly_hours - weekly_contract).max(0.0);
        let premium_hours = over_contract.min(shift_hours) - over_full_time;
        if premium_hours <= 0.0 {
            return;
        }

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        earning.set_note(rule_item.name());
        match earning_type.uom() {
            UOM::Hours | UOM::HoursDollars | UOM::Units => {
                earning.set_dollars(0.0);
                earning.set_hours(premium_hours);
                earning.set_rate(diff_rate);
            }
            UOM::Dollars => {
                earning.set_dollars(round_currency(diff_rate * premium_hours));
                earning.set_hours(0.0);
                earning.set_rate(0.0);
            }
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
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, SavedEarnings, earning_type,
    };
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    const JOB: i32 = 3;

    /// Weeks end on Sunday 2013-03-10.
    struct WeekEnds;
    impl PropertyPort for WeekEnds {
        fn period_end_date(&self, _p: i32) -> LocalDate {
            LocalDate::of(2013, 3, 10)
        }
    }

    /// Monday 2013-03-04 onward, one shift per day starting at 08:00.
    fn day_shift(id: i32, day: i32, hours: f64) -> EmployeeShift {
        let start = LocalDateTime::of(2013, 3, day, 8, 0, 0);
        EmployeeShift::new(
            id,
            1,
            JOB,
            start.to_local_date(),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(start), Some(start.plus_hours(8)))
        .with_net_hours(hours)
    }
    fn employee(contract: Option<f64>) -> Employee {
        let d = LocalDate::of(2013, 3, 1);
        Employee::new(
            1,
            1,
            "Pat",
            vec![
                EmployeeJobStatus::new(
                    1,
                    1,
                    JOB,
                    d.minus_years(1),
                    d.plus_years(1),
                    EmployeePayType::Hourly,
                    10.0,
                    true,
                )
                .with_salaried_fields(0.0, 0.0, contract, None),
            ],
        )
    }
    fn item(extra: RuleParams) -> RuleItem {
        let mut p = rule_params! {
            EARNING => "1", DIFF_RATE_PROP => "2", ACTUAL_PUNCH_PROP => "false", FULL_TIME_HOURS => "40"
        };
        for (k, v) in extra.iter() {
            p.set(k, v);
        }
        RuleItem::new(1, 1, "ctf", RuleClass::ContractToFullDif, p)
    }
    fn run(
        contract: Option<f64>,
        shifts: Vec<EmployeeShift>,
        target: usize,
        uom: UOM,
        extra: RuleParams,
    ) -> (TimeCardData, SavedEarnings) {
        let s = shifts[target].clone();
        let mut card = TimeCardData::new()
            .with_employee(employee(contract))
            .with_shifts(shifts)
            .with_calculation_start_date(LocalDate::of(2013, 3, 1));
        let saved = SavedEarnings::default();
        ContractToFullTimeRule::new(FixedEarningType(earning_type(uom)), saved.clone(), WeekEnds)
            .execute(&s, &mut card, &item(extra));
        (card, saved)
    }
    /// Three 10-hour shifts Mon–Wed.
    fn week() -> Vec<EmployeeShift> {
        vec![
            day_shift(1, 4, 10.0),
            day_shift(2, 5, 10.0),
            day_shift(3, 6, 10.0),
        ]
    }

    #[test]
    fn hours_between_contract_and_full_time_are_paid() {
        // Contract 25: weekly before 20, this 10 -> 30; over contract 5, over full 0.
        let (card, saved) = run(Some(25.0), week(), 2, UOM::Hours, RuleParams::new());
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &card.earnings()[0];
        assert_eq!((e.hours(), e.rate(), e.dollars()), (5.0, 2.0, 0.0));
        assert_eq!(e.total_dollars(), 10.0);
        assert_eq!(e.shift_id(), Some(3));
    }

    #[test]
    fn hours_beyond_full_time_are_taken_back_out() {
        // Four shifts: 40 before... use full time 25 and contract 10.
        // weekly 30, over contract 20 -> min(20, 10) = 10; over full 5 -> 5 paid.
        let (card, _) = run(
            Some(10.0),
            week(),
            2,
            UOM::Hours,
            rule_params! { FULL_TIME_HOURS => "25" },
        );
        assert_eq!(card.earnings()[0].hours(), 5.0);
    }

    #[test]
    fn a_dollars_type_pays_rate_times_premium_hours() {
        let (card, _) = run(Some(25.0), week(), 2, UOM::Dollars, RuleParams::new());
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.hours(), e.rate()), (10.0, 0.0, 0.0));
    }

    #[test]
    fn only_shifts_sorting_earlier_count_before_this_one() {
        // The first shift of the week has nothing before it: 10 hours over a
        // contract of 4 -> 6 premium hours.
        let (card, _) = run(Some(4.0), week(), 0, UOM::Hours, RuleParams::new());
        assert_eq!(card.earnings()[0].hours(), 6.0);
    }

    #[test]
    fn no_home_job_means_a_zero_contract_so_every_hour_up_to_full_time_pays() {
        let (card, _) = run(None, week(), 0, UOM::Hours, RuleParams::new());
        assert_eq!(card.earnings()[0].hours(), 10.0);
    }

    #[test]
    fn nothing_is_paid_within_the_contract_or_for_an_unsupported_unit() {
        let (_, saved) = run(Some(40.0), week(), 2, UOM::Hours, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
        let (_, saved) = run(Some(25.0), week(), 2, UOM::Days, RuleParams::new());
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_shift_with_errors_or_in_a_closed_period_is_skipped() {
        let mut shifts = week();
        shifts[2] = shifts[2]
            .clone()
            .with_errors(vec![ShiftErrorType::MissingOut]);
        let (_, saved) = run(Some(25.0), shifts, 2, UOM::Hours, RuleParams::new());
        assert!(saved.0.borrow().is_empty());

        let s = week()[2].clone();
        let mut card = TimeCardData::new()
            .with_employee(employee(Some(25.0)))
            .with_shifts(week())
            .with_calculation_start_date(LocalDate::of(2013, 3, 20));
        let saved = SavedEarnings::default();
        ContractToFullTimeRule::new(
            FixedEarningType(earning_type(UOM::Hours)),
            saved.clone(),
            WeekEnds,
        )
        .execute(&s, &mut card, &item(RuleParams::new()));
        assert!(saved.0.borrow().is_empty());
    }
}
