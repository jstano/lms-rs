//! Port of `MaxDaysWorkedPerWeekRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/MaxDaysWorkedPerWeekRuleImpl.java`.
//!
//! Caps both the number of distinct scheduled days in a week and the longest
//! run of consecutive scheduled days within it.
//!
//! # Two different weeks, by configuration
//!
//! `useCalendarWeek` picks between two entirely different windows to check:
//! a single fixed calendar week (`dow` names which day of week it starts on,
//! walked back from the shift's own date) when `true`, or **every** rolling
//! seven-day window that could contain the shift's date — `shiftDate` through
//! `shiftDate + 6`, one `WeeklyDateRange` per offset — when `false`. The
//! rolling-window form returns a violation if *any* of those seven windows is
//! over the limit, not just the one containing the shift most naturally.
//!
//! Ported cases: `MaxDaysWorkedPerWeekRuleImplTest.groovy` (three cases).

use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::config::{
    DOW_PROP, MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP, MAX_DAYS_PER_WEEK_PROP,
    MaxDaysWorkedPerWeekRuleConfig, USE_CALENDAR_WEEK_PROP,
};
use crate::rules::algorithm::schedulerestriction::{
    ScheduleRestrictionResult, ScheduleRestrictionRule,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `MaxDaysWorkedPerWeekRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaxDaysWorkedPerWeekRule;

impl MaxDaysWorkedPerWeekRule {
    fn validate_shifts(
        &self,
        work_week: &DateRange,
        dataset: &dyn TimeCard,
        params: &RuleParams,
    ) -> bool {
        let mut scheduled_dates: Vec<LocalDate> = dataset
            .shifts_for_period(work_week)
            .iter()
            .map(|shift| shift.shift_date())
            .collect();
        scheduled_dates.sort_unstable();

        self.is_max_days_per_week_reached(params, &scheduled_dates)
            || self.is_max_consecutive_days_scheduled(params, &scheduled_dates)
    }

    fn is_max_days_per_week_reached(
        &self,
        params: &RuleParams,
        scheduled_dates: &[LocalDate],
    ) -> bool {
        scheduled_dates.len() as i32 > params.int_at(MAX_DAYS_PER_WEEK_PROP)
    }

    fn is_max_consecutive_days_scheduled(
        &self,
        params: &RuleParams,
        scheduled_dates: &[LocalDate],
    ) -> bool {
        let max_consecutive_days = params.int_at(MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP);
        let mut consecutive_days = 1;

        for window in scheduled_dates.windows(2) {
            let (current, next) = (window[0], window[1]);
            if current.plus_days(1) == next {
                consecutive_days += 1;
                if consecutive_days > max_consecutive_days {
                    return true;
                }
            } else {
                consecutive_days = 1;
            }
        }
        false
    }
}

impl ScheduleRestrictionRule for MaxDaysWorkedPerWeekRule {
    fn can_employee_work_shift(
        &self,
        dataset: &dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult {
        let params = rule_item
            .params()
            .fixed(&MaxDaysWorkedPerWeekRuleConfig.default_values());
        let shift_date = shift.shift_date();
        let use_calendar_week = params.bool_at(USE_CALENDAR_WEEK_PROP);

        let violated = if use_calendar_week {
            let dow = params.int_at(DOW_PROP);
            let mut days_to_add =
                crate::common::dates::translate_dow_from_iso(shift_date.day_of_week().value())
                    - dow;
            if days_to_add < 0 {
                days_to_add += 7;
            }
            let week = WeeklyDateRange::with_start_date(shift_date.minus_days(days_to_add.into()));
            self.validate_shifts(&week, dataset, &params)
        } else {
            (0i64..7).any(|offset| {
                let week = WeeklyDateRange::with_end_date(shift_date.plus_days(offset));
                self.validate_shifts(&week, dataset, &params)
            })
        };

        if violated {
            ScheduleRestrictionResult::error(ShiftErrorType::MaxDaysWorkedPerWeek)
        } else {
            ScheduleRestrictionResult::ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;

    fn shift(net_hours: f64, shift_date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new())
            .with_net_hours(net_hours)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "Max days worked per week",
            RuleClass::MaxDaysWorkedPerWeek,
            params,
        )
    }

    mod java_parity_tests {
        use super::*;

        /// `MaxDaysWorkedPerWeekRuleImplTest`: "if a shifts are not
        /// exceeding max days and consecutive days".
        #[test]
        fn shifts_not_exceeding_max_days_and_consecutive_days() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![
                target.clone(),
                shift(11.0, LocalDate::of(2018, 2, 20)),
                shift(11.0, LocalDate::of(2018, 2, 19)),
                shift(10.0, LocalDate::of(2018, 2, 24)),
            ]);
            let params = rule_params! {
                MAX_DAYS_PER_WEEK_PROP => "5",
                MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP => "5"
            };

            let result = MaxDaysWorkedPerWeekRule.can_employee_work_shift(
                &dataset,
                &target,
                &rule_item(params),
            );

            assert!(result.is_ok());
        }

        /// `MaxDaysWorkedPerWeekRuleImplTest`: "it should return an error if
        /// shifts exceeds max days in a week".
        #[test]
        fn an_error_when_shifts_exceed_max_days_in_a_week() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![
                target.clone(),
                shift(11.0, LocalDate::of(2018, 2, 19)),
                shift(11.0, LocalDate::of(2018, 2, 20)),
                shift(11.0, LocalDate::of(2018, 2, 23)),
                shift(11.0, LocalDate::of(2018, 2, 24)),
            ]);
            let params = rule_params! {
                MAX_DAYS_PER_WEEK_PROP => "4",
                MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP => "4",
                USE_CALENDAR_WEEK_PROP => "false"
            };

            let result = MaxDaysWorkedPerWeekRule.can_employee_work_shift(
                &dataset,
                &target,
                &rule_item(params),
            );

            assert_eq!(
                result.shift_error_type(),
                Some(ShiftErrorType::MaxDaysWorkedPerWeek)
            );
        }

        /// `MaxDaysWorkedPerWeekRuleImplTest`: "it should return an error if
        /// shifts exceeds max consecutive days in a week".
        #[test]
        fn an_error_when_shifts_exceed_max_consecutive_days_in_a_week() {
            let target = shift(8.0, LocalDate::of(2018, 2, 23));
            let dataset = TimeCardData::new().with_shifts(vec![
                target.clone(),
                shift(11.0, LocalDate::of(2018, 2, 19)),
                shift(11.0, LocalDate::of(2018, 2, 20)),
                shift(11.0, LocalDate::of(2018, 2, 21)),
                shift(11.0, LocalDate::of(2018, 2, 22)),
            ]);
            let params = rule_params! {
                MAX_DAYS_PER_WEEK_PROP => "5",
                MAX_CONSECUTIVE_DAYS_PER_WEEK_PROP => "4"
            };

            let result = MaxDaysWorkedPerWeekRule.can_employee_work_shift(
                &dataset,
                &target,
                &rule_item(params),
            );

            assert_eq!(
                result.shift_error_type(),
                Some(ShiftErrorType::MaxDaysWorkedPerWeek)
            );
        }
    }
}
