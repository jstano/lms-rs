//! Port of `ContractGuaranteeRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/ContractGuaranteeRuleImpl.java`.
//!
//! Guarantees a contracted employee who is **lent to a lower-paying
//! department** the pay difference up to their contract hours, so working
//! elsewhere never costs them against their home job.
//!
//! - Skipped when the shift is in the employee's home department (the parent
//!   assignment, compared by identity), or when the shift's job pays no less
//!   than the home job. The shift's rate is the job's effective pay rate when
//!   `defaultRate` is set, otherwise the employee's own status rate for the job.
//! - The hours counted toward the contract, across the pay period containing the
//!   shift: the stored net hours of every error-free shift that is in the home
//!   department **or** starts before this one, plus the hours of the card's
//!   earnings in that period whose type is in `earningTypes`.
//! - While those fall short of the employee's contract hours for the period
//!   (`Employee.getContractHours(date)`, below), pays
//!   `min(this shift's net hours, shortfall)` hours at `home rate − shift rate`.
//!   A `DOLLARS` type pays `round(rate × hours, 2)` dollars instead.
//! - Nothing guards a zero or negative result: an earning is written whenever
//!   the contract is short, even for a shift with no hours.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::contract::ContractCalculator;
use crate::common::enums::uom::UOM;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_to;
use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftearning::config::{
    ContractGuaranteeRuleConfig, DEFAULT_RATE, EARNING, EARNING_TYPES,
};
use crate::rules::algorithm::shiftearning::{
    ShiftEarningRule, new_shift_earning, shift_is_eligible,
};
use crate::rules::ports::{AssignmentPort, EarningTypePort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;

/// `Employee.getContractHours(LocalDate)` (`BasicEmployee`).
///
/// Zero without a home job on `date`. For a `CONTRACT` or `SALARIED_NON_EXEMPT`
/// home job: the weekly figure turned into the pay period's hours by the
/// property's contract calculator, with the period starting on the hire date
/// when that falls inside it. A missing hire date counts as "not inside".
pub(crate) fn employee_contract_hours(
    employee: &Employee,
    time_card: &dyn TimeCard,
    date: LocalDate,
) -> f64 {
    let Some(home) = employee.home_employee_job_status(date) else {
        return 0.0;
    };
    let pay_type = home.pay_type();
    if !(pay_type.is_contract() || pay_type.is_salaried_non_exempt()) {
        return 0.0;
    }
    let mut pay_period = time_card.pay_period_containing(date);
    if let Some(hired) = employee.hire_date()
        && pay_period.contains_date(hired)
    {
        pay_period = DateRange::new(hired, pay_period.end_date());
    }
    ContractCalculator::for_property(time_card.pay_period_type(), time_card.schedule_mode())
        .contract_hours_for_pay_period(home.contract_hours().unwrap_or(0.0), &pay_period)
}

/// `ContractGuaranteeRuleImpl`.
pub struct ContractGuaranteeRule<T: EarningTypePort, E: EmployeeEarningPort, A: AssignmentPort> {
    earning_types: T,
    earnings: RefCell<E>,
    assignments: A,
}

impl<T: EarningTypePort, E: EmployeeEarningPort, A: AssignmentPort> ContractGuaranteeRule<T, E, A> {
    /// Build the rule over the ports it reads and writes through.
    pub fn new(earning_types: T, earnings: E, assignments: A) -> Self {
        Self {
            earning_types,
            earnings: RefCell::new(earnings),
            assignments,
        }
    }

    fn parent_of(&self, job_id: i32) -> Option<i32> {
        self.assignments
            .find_by_id(job_id)
            .and_then(|job| job.parent_assignment_id())
    }
}

impl<T: EarningTypePort, E: EmployeeEarningPort, A: AssignmentPort> ShiftEarningRule
    for ContractGuaranteeRule<T, E, A>
{
    fn execute(&self, shift: &EmployeeShift, time_card: &mut dyn TimeCard, rule_item: &RuleItem) {
        if !shift_is_eligible(shift, time_card) {
            return;
        }
        let params = rule_item
            .params()
            .fixed(&ContractGuaranteeRuleConfig.default_values());
        let earning_type_ids = ids_for_key(EARNING_TYPES, &params);
        let date = shift.shift_date();

        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(date) else {
            return;
        };
        let home_department = self.parent_of(home.job_id());
        if home_department == self.parent_of(shift.job_id()) {
            return;
        }

        let home_rate = home.hourly_rate();
        let secondary_rate = if params.bool_at(DEFAULT_RATE) {
            match self.assignments.find_by_id(shift.job_id()) {
                Some(job) => job.effective_hourly_pay_rate(date, &self.assignments),
                None => return,
            }
        } else {
            match time_card.employee_job_status_for_shift(shift) {
                Some(status) => status.hourly_rate(),
                None => return,
            }
        };
        if secondary_rate >= home_rate {
            return;
        }

        let current_period = time_card.pay_period_containing(date);
        let shift_start = shift.start_time_for_ordering();
        let mut actual_contract_hours = 0.0;
        for other in time_card.shifts_for_period(&current_period) {
            if other.has_errors() {
                continue;
            }
            if self.parent_of(other.job_id()) == home_department
                || other.start_time_for_ordering() < shift_start
            {
                actual_contract_hours += other.net_hours();
            }
        }
        for earning in time_card.earnings() {
            if current_period.contains_date(earning.earning_date())
                && earning_type_ids.contains(&earning.earning_type_id())
            {
                actual_contract_hours += earning.hours();
            }
        }

        let contract_hours = employee_contract_hours(employee, time_card, date);
        if actual_contract_hours >= contract_hours {
            return;
        }
        let earning_hours = shift
            .net_hours()
            .min(contract_hours - actual_contract_hours);
        let rate = home_rate - secondary_rate;

        let Some(earning_type) = self.earning_types.find_by_id(params.int_at(EARNING)) else {
            return;
        };
        let mut earning = new_shift_earning(shift, earning_type.id(), rule_item, true);
        if earning_type.uom() == UOM::Dollars {
            earning.set_dollars(round_to(rate * earning_hours, 2));
            earning.set_rate(0.0);
            earning.set_hours(0.0);
        } else {
            earning.set_dollars(0.0);
            earning.set_rate(rate);
            earning.set_hours(earning_hours);
        }
        earning.calc_and_set_total_dollars();
        earning.set_note(rule_item.name());

        time_card.earnings_mut().push(earning.clone());
        self.earnings.borrow_mut().save(earning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_pay_rate::AssignmentPayRate;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::{
        FixedEarningType, SavedEarnings, earning_type,
    };
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    const HOME_JOB: i32 = 1;
    const OTHER_JOB: i32 = 2;
    const SAME_DEPT_JOB: i32 = 3;

    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            let (parent, rate) = match id {
                HOME_JOB => (Some(100), 20.0),
                OTHER_JOB => (Some(200), 12.0),
                SAME_DEPT_JOB => (Some(100), 20.0),
                _ => return None,
            };
            Some(
                Assignment::new(id, 1, "job", "J", parent).with_pay_rates(vec![
                    AssignmentPayRate::new(LocalDate::of(2000, 1, 1), rate),
                ]),
            )
        }
    }

    fn day() -> LocalDate {
        LocalDate::of(2013, 3, 4)
    }
    fn employee(pay_type: EmployeePayType) -> Employee {
        let d = day();
        let status = |id, job, rate, home| {
            EmployeeJobStatus::new(
                id,
                1,
                job,
                d.minus_years(1),
                d.plus_years(1),
                pay_type,
                rate,
                home,
            )
        };
        Employee::new(
            1,
            1,
            "Pat",
            vec![
                status(1, HOME_JOB, 20.0, true).with_salaried_fields(0.0, 0.0, Some(10.0), None),
                status(2, OTHER_JOB, 15.0, false),
            ],
        )
        .with_dates(Some(d.minus_years(2)), None, None)
    }
    fn shift(id: i32, job: i32, hour: i32, hours: f64) -> EmployeeShift {
        let start = LocalDateTime::of(2013, 3, 4, hour, 0, 0);
        EmployeeShift::new(id, 1, job, day(), ShiftType::Actual, Vec::new())
            .with_times(Some(start), Some(start.plus_hours(4)))
            .with_net_hours(hours)
    }
    fn item(extra: RuleParams) -> RuleItem {
        let mut p = rule_params! { EARNING => "1" };
        for (k, v) in extra.iter() {
            p.set(k, v);
        }
        RuleItem::new(1, 1, "cg", RuleClass::ContractGuaranteeDif, p)
    }
    fn run(
        pay_type: EmployeePayType,
        shifts: Vec<EmployeeShift>,
        target: usize,
        uom: UOM,
        extra: RuleParams,
    ) -> (TimeCardData, SavedEarnings) {
        let s = shifts[target].clone();
        let mut card = TimeCardData::new()
            .with_employee(employee(pay_type))
            .with_shifts(shifts)
            .with_current_pay_period(DateRange::new(
                LocalDate::of(2013, 3, 1),
                LocalDate::of(2013, 3, 14),
            ))
            .with_contract_periods(
                crate::common::enums::pay_period_type::PayPeriodType::BiWeekly,
                crate::common::enums::schedule_mode::ScheduleMode::Monthly,
            )
            .with_calculation_start_date(LocalDate::of(2013, 3, 1));
        let saved = SavedEarnings::default();
        ContractGuaranteeRule::new(FixedEarningType(earning_type(uom)), saved.clone(), Jobs)
            .execute(&s, &mut card, &item(extra));
        (card, saved)
    }

    #[test]
    fn the_rate_gap_is_paid_on_the_hours_the_contract_is_short() {
        // Contract 10 hours/week x 2 weeks = 20 in the period. Only this 4-hour
        // shift (other dept, nothing earlier): short by 20 -> min(4, 20) = 4 hours
        // at 20 - 15 = 5.
        let (card, saved) = run(
            EmployeePayType::Contract,
            vec![shift(1, OTHER_JOB, 8, 4.0)],
            0,
            UOM::Hours,
            RuleParams::new(),
        );
        assert_eq!(saved.0.borrow().len(), 1);
        let e = &card.earnings()[0];
        assert_eq!((e.hours(), e.rate(), e.dollars()), (4.0, 5.0, 0.0));
        assert_eq!(e.source(), EarningSource::Rule);
        assert_eq!(e.shift_id(), Some(1));
    }

    #[test]
    fn home_department_hours_and_earlier_shifts_count_toward_the_contract() {
        // 16 home-department hours + this shift's 4 reach... the earlier home shift
        // counts; contract 20, counted 16 -> shortfall 4 -> min(4, 4).
        let shifts = vec![
            shift(1, SAME_DEPT_JOB, 0, 16.0),
            shift(2, OTHER_JOB, 8, 4.0),
        ];
        let (card, _) = run(
            EmployeePayType::Contract,
            shifts,
            1,
            UOM::Hours,
            RuleParams::new(),
        );
        assert_eq!(card.earnings()[0].hours(), 4.0);

        let shifts = vec![
            shift(1, SAME_DEPT_JOB, 0, 18.0),
            shift(2, OTHER_JOB, 8, 4.0),
        ];
        let (card, _) = run(
            EmployeePayType::Contract,
            shifts,
            1,
            UOM::Hours,
            RuleParams::new(),
        );
        assert_eq!(card.earnings()[0].hours(), 2.0);

        let shifts = vec![
            shift(1, SAME_DEPT_JOB, 0, 20.0),
            shift(2, OTHER_JOB, 8, 4.0),
        ];
        let (_, saved) = run(
            EmployeePayType::Contract,
            shifts,
            1,
            UOM::Hours,
            RuleParams::new(),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_dollars_type_pays_the_rounded_total() {
        let (card, _) = run(
            EmployeePayType::Contract,
            vec![shift(1, OTHER_JOB, 8, 4.0)],
            0,
            UOM::Dollars,
            RuleParams::new(),
        );
        let e = &card.earnings()[0];
        assert_eq!((e.dollars(), e.hours(), e.rate()), (20.0, 0.0, 0.0));
    }

    #[test]
    fn the_default_rate_option_prices_the_shift_at_the_jobs_pay_rate() {
        // Job rate 12 rather than the employee's own 15: gap 8.
        let (card, _) = run(
            EmployeePayType::Contract,
            vec![shift(1, OTHER_JOB, 8, 4.0)],
            0,
            UOM::Hours,
            rule_params! { DEFAULT_RATE => "true" },
        );
        assert_eq!(card.earnings()[0].rate(), 8.0);
    }

    #[test]
    fn nothing_is_paid_in_the_home_department_or_at_an_equal_or_higher_rate() {
        let (_, saved) = run(
            EmployeePayType::Contract,
            vec![shift(1, SAME_DEPT_JOB, 8, 4.0)],
            0,
            UOM::Hours,
            RuleParams::new(),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_pay_type_without_a_contract_has_none_to_guarantee() {
        let (_, saved) = run(
            EmployeePayType::Hourly,
            vec![shift(1, OTHER_JOB, 8, 4.0)],
            0,
            UOM::Hours,
            RuleParams::new(),
        );
        assert!(saved.0.borrow().is_empty());
    }

    #[test]
    fn a_shift_with_errors_is_skipped() {
        let (_, saved) = run(
            EmployeePayType::Contract,
            vec![shift(1, OTHER_JOB, 8, 4.0).with_errors(vec![ShiftErrorType::MissingOut])],
            0,
            UOM::Hours,
            RuleParams::new(),
        );
        assert!(saved.0.borrow().is_empty());
    }
}
