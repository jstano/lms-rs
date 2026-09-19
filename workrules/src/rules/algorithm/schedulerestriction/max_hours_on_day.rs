//! Port of `MaxHoursOnDayRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/MaxHoursOnDayRuleImpl.java`.
//!
//! On a configured day of week, the summed net hours of every shift on that
//! day (the shift under test included) must not exceed `maxHours`. Days not
//! in `dows` are unrestricted.
//!
//! Ported cases: `MaxHoursOnDayRuleImplTest.groovy` (six cases).

use crate::common::dates::translate_dow_from_iso;
use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::common::json_ids::ids_for_key;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::config::{
    MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP, MAX_HOURS_ON_DAY_PROP, MaxHoursOnDayRuleConfig,
};
use crate::rules::algorithm::schedulerestriction::{
    ScheduleRestrictionResult, ScheduleRestrictionRule,
};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// `MaxHoursOnDayRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaxHoursOnDayRule;

impl ScheduleRestrictionRule for MaxHoursOnDayRule {
    fn can_employee_work_shift(
        &self,
        dataset: &dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult {
        let params = rule_item
            .params()
            .fixed(&MaxHoursOnDayRuleConfig.default_values());
        let shift_date = shift.shift_date();

        let included_days = ids_for_key(MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP, &params);
        let day_of_week = translate_dow_from_iso(shift_date.day_of_week().value());
        if !included_days.contains(&day_of_week) {
            return ScheduleRestrictionResult::ok();
        }

        let max_hours = params.double_at(MAX_HOURS_ON_DAY_PROP);
        let mut hours_on_day = 0.0;
        for shift_on_day in dataset.shifts_for_period(&DateRange::new(shift_date, shift_date)) {
            hours_on_day += shift_on_day.net_hours();
            if hours_on_day > max_hours {
                return ScheduleRestrictionResult::error(ShiftErrorType::MaxHoursOnDay);
            }
        }

        ScheduleRestrictionResult::ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn shift(net_hours: f64, shift_date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new())
            .with_net_hours(net_hours)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "Max hours on day",
            RuleClass::MaxHoursOnDaySrr,
            params,
        )
    }

    mod java_parity_tests {
        use super::*;

        /// `MaxHoursOnDayRuleImplTest`: "if a shift is equal to max hours an
        /// ok result should be returned".
        #[test]
        fn a_shift_equal_to_max_hours_is_ok() {
            let target = shift(8.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![target.clone()]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 4, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert!(result.is_ok());
        }

        /// `MaxHoursOnDayRuleImplTest`: "if all shifts in a day have hours
        /// equal to max hours an ok result should be returned".
        #[test]
        fn all_shifts_in_a_day_summing_to_max_hours_is_ok() {
            let target = shift(3.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![
                target.clone(),
                shift(3.0, LocalDate::of(2018, 2, 21)),
                shift(2.0, LocalDate::of(2018, 2, 21)),
            ]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 4, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert!(result.is_ok());
        }

        /// `MaxHoursOnDayRuleImplTest`: "a shift exceeding max hours on a
        /// not configured day should return an ok result".
        #[test]
        fn exceeding_max_hours_on_an_unconfigured_day_is_ok() {
            let target = shift(15.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![target.clone()]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert!(result.is_ok());
        }

        /// `MaxHoursOnDayRuleImplTest`: "a shift exceeding max hours on a
        /// configured day should return an error".
        #[test]
        fn exceeding_max_hours_on_a_configured_day_is_an_error() {
            let target = shift(15.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![target.clone()]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 4, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert_eq!(
                result.shift_error_type(),
                Some(ShiftErrorType::MaxHoursOnDay)
            );
        }

        /// `MaxHoursOnDayRuleImplTest`: "if shift hours on day are pushed
        /// over max hours by the new shift it should return an error".
        #[test]
        fn pushed_over_max_hours_by_the_new_shift_is_an_error() {
            let target = shift(2.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new()
                .with_shifts(vec![shift(7.0, LocalDate::of(2018, 2, 21)), target.clone()]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 4, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert_eq!(
                result.shift_error_type(),
                Some(ShiftErrorType::MaxHoursOnDay)
            );
        }

        /// `MaxHoursOnDayRuleImplTest`: "hours from other days should not be
        /// included in max hours calculation".
        #[test]
        fn hours_from_other_days_are_not_included() {
            let target = shift(7.0, LocalDate::of(2018, 2, 21));
            let dataset = TimeCardData::new().with_shifts(vec![
                target.clone(),
                shift(9.0, LocalDate::of(2018, 2, 20)),
                shift(9.0, LocalDate::of(2018, 2, 22)),
            ]);
            let params =
                rule_params! { MAX_HOURS_ON_DAY_DOW_MULTIPLE_PROP => "[0, 1, 2, 3, 4, 5, 6]" };

            let result =
                MaxHoursOnDayRule.can_employee_work_shift(&dataset, &target, &rule_item(params));

            assert!(result.is_ok());
        }
    }
}
