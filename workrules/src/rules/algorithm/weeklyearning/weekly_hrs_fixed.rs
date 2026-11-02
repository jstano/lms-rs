//! Port of `WeeklyHrsFixedRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/WeeklyHrsFixedRuleImpl.java`.
//!
//! Pays a fixed dollar amount, dated the week's **last day**, when the week's
//! total hours fall in `[minHours, maxHours]` (both ends included).
//!
//! The total is the sum, over the week's shifts that have no persisted errors,
//! of [`net_hours_legacy`](super::net_hours_legacy), rounded to two places
//! after each shift.
//!
//! It does nothing unless the week's end date is open for editing **and** the
//! employee has a home job that day. The earning goes on that home job.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::earning_source::EarningSource;
use crate::common::numbers::round_to;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, EARNING, MAX_HRS_PROP, MIN_HRS_PROP, WeeklyHrsFixedRuleConfig,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, net_hours_legacy, rule_earning};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use std::cell::RefCell;

/// `WeeklyHrsFixedRuleImpl`.
pub struct WeeklyHrsFixedRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> WeeklyHrsFixedRule<E> {
    /// Build the rule over the earning writer. The earning type is only
    /// stored, never inspected, so no earning-type port is needed.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> WeeklyEarningRule for WeeklyHrsFixedRule<E> {
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        rule_description: &str,
    ) {
        let end = work_week.end_date();
        if !time_card.is_open_for_editing_on(end) {
            return;
        }
        let Some(employee) = time_card.employee() else {
            return;
        };
        let Some(home) = employee.home_employee_job_status(end) else {
            return;
        };
        let (employee_id, job_id) = (employee.id(), home.job_id());

        // Java reads these without `fixMap`; see divergence 95.
        let params = rule_item
            .params()
            .fixed(&WeeklyHrsFixedRuleConfig.default_values());
        let (min, max) = (
            params.double_at(MIN_HRS_PROP),
            params.double_at(MAX_HRS_PROP),
        );
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);

        let mut total = 0.0;
        for shift in time_card.shifts() {
            if work_week.contains_date(shift.shift_date()) && !shift.has_errors() {
                total = round_to(total + net_hours_legacy(shift, use_actual), 2);
            }
        }

        if total >= min && total <= max {
            let mut earning = rule_earning(
                employee_id,
                job_id,
                params.int_at(EARNING),
                end,
                rule_item,
                None,
            );
            earning.set_dollars(params.double_at(DOLLARS_PROP));
            earning.set_rate(0.0);
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_description);
            debug_assert_eq!(earning.source(), EarningSource::Rule);

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
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::algorithm::dailyearning::fixtures::SavedEarnings;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    const JOB: i32 = 7;

    fn week() -> DateRange {
        DateRange::new(LocalDate::of(2014, 5, 5), LocalDate::of(2014, 5, 11))
    }

    fn employee(job_start: LocalDate) -> Employee {
        Employee::new(
            1,
            1,
            "Pat",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                job_start,
                LocalDate::of(2015, 1, 1),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        )
    }

    fn shift(day: i32, hours: f64) -> EmployeeShift {
        EmployeeShift::new(
            day,
            1,
            JOB,
            LocalDate::of(2014, 5, day),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(hours)
    }

    fn params(min: &str, max: &str) -> RuleParams {
        rule_params! {
            DOLLARS_PROP => "25", EARNING => "4", MIN_HRS_PROP => min, MAX_HRS_PROP => max,
            ACTUAL_PUNCH_PROP => "false"
        }
    }

    fn run(card: &mut TimeCardData, p: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        WeeklyHrsFixedRule::new(saved.clone()).execute(
            card,
            &week(),
            &RuleItem::new(3, 1, "WHF", RuleClass::WklyHrsFixedWdf, p),
            "Rule: set - WHF",
        );
        saved
    }

    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee(LocalDate::of(2014, 1, 1)))
            .with_shifts(shifts)
    }

    #[test]
    fn a_week_inside_the_band_pays_on_the_last_day() {
        let mut c = card(vec![shift(5, 10.0), shift(6, 12.0)]);
        let saved = run(&mut c, params("20", "40"));
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), LocalDate::of(2014, 5, 11));
        assert_eq!(e.pay_date(), Some(LocalDate::of(2014, 5, 11)));
        assert_eq!(e.dollars(), 25.0);
        assert_eq!(e.total_dollars(), 25.0);
        assert_eq!(e.rate(), 0.0);
        assert_eq!(e.job_id(), JOB);
        assert_eq!(e.note(), "Rule: set - WHF");
        assert_eq!(e.shift_id(), None);
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn both_ends_of_the_band_are_included() {
        for hours in [20.0, 40.0] {
            let mut c = card(vec![shift(5, hours)]);
            run(&mut c, params("20", "40"));
            assert_eq!(c.earnings().len(), 1, "{hours}");
        }
        for hours in [19.99, 40.01] {
            let mut c = card(vec![shift(5, hours)]);
            run(&mut c, params("20", "40"));
            assert!(c.earnings().is_empty(), "{hours}");
        }
    }

    #[test]
    fn shifts_with_errors_and_outside_the_week_do_not_count() {
        let bad = shift(6, 30.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let outside = EmployeeShift::new(
            9,
            1,
            JOB,
            LocalDate::of(2014, 5, 12),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(30.0);
        let mut c = card(vec![shift(5, 5.0), bad, outside]);
        run(&mut c, params("20", "40"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn a_closed_week_end_pays_nothing() {
        let mut c =
            card(vec![shift(5, 25.0)]).with_calculation_start_date(LocalDate::of(2014, 5, 12));
        run(&mut c, params("20", "40"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn no_home_job_on_the_end_date_pays_nothing() {
        let mut c = TimeCardData::new()
            .with_employee(employee(LocalDate::of(2014, 5, 12)))
            .with_shifts(vec![shift(5, 25.0)]);
        run(&mut c, params("20", "40"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn the_legacy_net_hours_are_unrounded_for_stored_hours() {
        // 20 x 0.999 = 19.98 < 20, but each shift's contribution is rounded
        // to two places in the running total: 0.999 -> 1.0, so 20 shifts of
        // 0.999h reach exactly 20.0 if rounded per step.
        let shifts: Vec<_> = (5..=11).map(|d| shift(d, 2.857)).collect();
        let total_unrounded: f64 = shifts.iter().map(|s| s.net_hours()).sum();
        assert!(total_unrounded < 20.0);
        let mut c = card(shifts);
        run(&mut c, params("19.99", "40"));
        assert_eq!(c.earnings().len(), 1);
    }
}
