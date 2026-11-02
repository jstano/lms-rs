//! Port of `DOWHrsFixedRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/weeklyearning/DOWHrsFixedRuleImpl.java`.
//!
//! For each day of the week whose weekday is among the configured `dow`s and
//! which is open for editing, sums that day's shift hours (every shift,
//! **errors included**, via [`net_hours_legacy`](super::net_hours_legacy)) and,
//! when the total is within `[minHours, maxHours]`, pays a fixed dollar amount
//! dated that day. A configured, open day with **no** shifts totals zero, so
//! with `minHours` 0 it would pay — config validation forbids `minHours <= 0`.
//!
//! The earning goes on the home job of that day.
//!
//! # Order
//!
//! Java iterates a `HashMap<LocalDate, Double>`, so the order the days' earnings
//! are appended in is unspecified. The port appends in date order (divergence 96).
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::dates::translate_dow_from_iso;
use crate::common::json_ids::ids_for_key;
use crate::common::numbers::round_to;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::weeklyearning::config::{
    ACTUAL_PUNCH_PROP, DOLLARS_PROP, DOW_PROP, DOWHrsFixedRuleConfig, EARNING, MAX_HRS_PROP,
    MIN_HRS_PROP,
};
use crate::rules::algorithm::weeklyearning::{WeeklyEarningRule, net_hours_legacy, rule_earning};
use crate::rules::ports::EmployeeEarningPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// `DOWHrsFixedRuleImpl`.
pub struct DOWHrsFixedRule<E: EmployeeEarningPort> {
    earnings: RefCell<E>,
}

impl<E: EmployeeEarningPort> DOWHrsFixedRule<E> {
    /// Build the rule over the earning writer.
    pub fn new(earnings: E) -> Self {
        Self {
            earnings: RefCell::new(earnings),
        }
    }
}

impl<E: EmployeeEarningPort> WeeklyEarningRule for DOWHrsFixedRule<E> {
    fn execute(
        &self,
        time_card: &mut dyn TimeCard,
        work_week: &DateRange,
        rule_item: &RuleItem,
        rule_description: &str,
    ) {
        let params = rule_item
            .params()
            .fixed(&DOWHrsFixedRuleConfig.default_values());
        let (min, max) = (
            params.double_at(MIN_HRS_PROP),
            params.double_at(MAX_HRS_PROP),
        );
        let use_actual = params.bool_at(ACTUAL_PUNCH_PROP);
        let dows = ids_for_key(DOW_PROP, &params);

        let mut daily_hours: BTreeMap<LocalDate, f64> = BTreeMap::new();
        for date in work_week.dates() {
            if dows.contains(&translate_dow_from_iso(date.day_of_week().value()))
                && time_card.is_open_for_editing_on(date)
            {
                daily_hours.insert(date, 0.0);
            }
        }
        for shift in time_card.shifts() {
            if let Some(hours) = daily_hours.get_mut(&shift.shift_date()) {
                *hours = round_to(*hours + net_hours_legacy(shift, use_actual), 2);
            }
        }

        for (date, hours) in daily_hours {
            if !(hours >= min && hours <= max) {
                continue;
            }
            let Some(employee) = time_card.employee() else {
                return;
            };
            let Some(home) = employee.home_employee_job_status(date) else {
                continue;
            };
            let mut earning = rule_earning(
                employee.id(),
                home.job_id(),
                params.int_at(EARNING),
                date,
                rule_item,
                None,
            );
            earning.set_dollars(params.double_at(DOLLARS_PROP));
            earning.set_rate(0.0);
            earning.calc_and_set_total_dollars();
            earning.set_note(rule_description);

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

    const JOB: i32 = 7;

    fn week() -> DateRange {
        // Mon 2014-05-05 .. Sun 2014-05-11; Tuesday is database weekday 3.
        DateRange::new(LocalDate::of(2014, 5, 5), LocalDate::of(2014, 5, 11))
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

    fn params(dows: &str) -> RuleParams {
        rule_params! {
            DOLLARS_PROP => "15", EARNING => "4", MIN_HRS_PROP => "4", MAX_HRS_PROP => "10",
            ACTUAL_PUNCH_PROP => "false", DOW_PROP => dows
        }
    }

    fn run(card: &mut TimeCardData, p: RuleParams) -> SavedEarnings {
        let saved = SavedEarnings::default();
        DOWHrsFixedRule::new(saved.clone()).execute(
            card,
            &week(),
            &RuleItem::new(3, 1, "DOW", RuleClass::DowHrsFixedWdf, p),
            "desc",
        );
        saved
    }

    #[test]
    fn a_day_in_the_band_on_a_configured_weekday_pays() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(6, 6.0)]);
        let saved = run(&mut c, params("[3]"));
        let e = &c.earnings()[0];
        assert_eq!(e.earning_date(), LocalDate::of(2014, 5, 6));
        assert_eq!(e.dollars(), 15.0);
        assert_eq!(e.note(), "desc");
        assert_eq!(saved.0.borrow().len(), 1);
    }

    #[test]
    fn other_weekdays_and_out_of_band_totals_do_not_pay() {
        let mut other_day = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(7, 6.0)]);
        run(&mut other_day, params("[3]"));
        assert!(other_day.earnings().is_empty());

        for hours in [3.99, 10.01] {
            let mut c = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![shift(6, hours)]);
            run(&mut c, params("[3]"));
            assert!(c.earnings().is_empty(), "{hours}");
        }
        for hours in [4.0, 10.0] {
            let mut c = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![shift(6, hours)]);
            run(&mut c, params("[3]"));
            assert_eq!(c.earnings().len(), 1, "{hours}");
        }
    }

    #[test]
    fn several_shifts_on_the_day_are_summed_errors_included() {
        let bad = shift(6, 3.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(6, 2.0), bad]);
        run(&mut c, params("[3]"));
        assert_eq!(
            c.earnings().len(),
            1,
            "2 + 3 = 5 is in the band; the errored shift counts"
        );
    }

    #[test]
    fn a_closed_day_is_skipped() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(6, 6.0)])
            .with_calculation_start_date(LocalDate::of(2014, 5, 7));
        run(&mut c, params("[3]"));
        assert!(c.earnings().is_empty());
    }

    #[test]
    fn several_configured_weekdays_pay_in_date_order() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(8, 5.0), shift(6, 5.0)]);
        run(&mut c, params("[3,5]"));
        let dates: Vec<_> = c
            .earnings()
            .iter()
            .map(|e| e.earning_date().day_of_month())
            .collect();
        assert_eq!(dates, vec![6, 8]);
    }

    #[test]
    fn no_configured_weekdays_pays_nothing() {
        let mut c = TimeCardData::new()
            .with_employee(employee())
            .with_shifts(vec![shift(6, 6.0)]);
        run(&mut c, params("[]"));
        assert!(c.earnings().is_empty());
    }
}
