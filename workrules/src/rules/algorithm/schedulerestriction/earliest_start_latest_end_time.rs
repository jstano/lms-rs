//! Port of `EarliestStartLatestEndTimeRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulerestriction/EarliestStartLatestEndTimeRuleImpl.java`.
//!
//! On a configured day of week, a shift must start no earlier than
//! `earliestStartTime` and end no later than `latestEndTime` — a window that
//! may itself cross midnight, in which case the acceptable range is extended
//! a day. Days not in `dows` are unrestricted entirely.
//!
//! `dows` values follow the engine's own Sun=1..Sat=7 convention
//! ([`translate_dow_from_iso`](crate::common::dates::translate_dow_from_iso)),
//! not a 0-based day index, despite the spec's `[0,1,2,3,5,6]`-style
//! fixtures — `0` is simply never produced by the translation and so never
//! matches anything.
//!
//! Ported cases: `EarliestStartLatestEndTimeRuleImplTest.groovy` (four cases,
//! two of them `where:` tables).

use crate::common::dates::translate_dow_from_iso;
use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::schedulerestriction::config::{
    DOW_MULTIPLE_PROP, EARLIEST_START_TIME_PROP, EarliestStartLatestEndTimeRuleConfig,
    LATEST_END_TIME_PROP,
};
use crate::rules::algorithm::schedulerestriction::{
    ScheduleRestrictionResult, ScheduleRestrictionRule,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime, LocalTime};

/// `EarliestStartLatestEndTimeRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarliestStartLatestEndTimeRule;

impl EarliestStartLatestEndTimeRule {
    fn shift_falls_on_included_day_of_week(
        &self,
        shift_date: LocalDate,
        params: &RuleParams,
    ) -> bool {
        let shift_day_of_week = translate_dow_from_iso(shift_date.day_of_week().value());
        crate::common::json_ids::ids_for_key(DOW_MULTIPLE_PROP, params).contains(&shift_day_of_week)
    }

    fn shift_falls_within_configured_times(
        &self,
        shift: &EmployeeShift,
        params: &RuleParams,
    ) -> bool {
        let earliest_start_time =
            LocalTime::parse(params.get(EARLIEST_START_TIME_PROP).unwrap_or("00:00:00"));
        let latest_end_time =
            LocalTime::parse(params.get(LATEST_END_TIME_PROP).unwrap_or("00:00:00"));
        let acceptable_range =
            acceptable_schedule_range(shift.shift_date(), earliest_start_time, latest_end_time);

        let shift_range = DateTimeRange::of(
            shift.start_date_time().expect("shift has no start time"),
            shift.end_date_time().expect("shift has no end time"),
        );
        acceptable_range.overlaps_completely(&shift_range)
    }
}

/// `createAcceptableScheduleDateTimeRange`.
fn acceptable_schedule_range(
    shift_date: LocalDate,
    earliest_start_time: LocalTime,
    latest_end_time: LocalTime,
) -> DateTimeRange {
    let earliest_start = shift_date.at_time(earliest_start_time);
    let latest_end = latest_shift_end_time(shift_date, earliest_start_time, latest_end_time);
    DateTimeRange::of(earliest_start, latest_end)
}

/// `determineLatestShiftEndTime`.
fn latest_shift_end_time(
    shift_date: LocalDate,
    earliest_start_time: LocalTime,
    latest_end_time: LocalTime,
) -> LocalDateTime {
    let latest_end = shift_date.at_time(latest_end_time);
    if latest_end_time.is_before(earliest_start_time) {
        latest_end.plus_days(1)
    } else {
        latest_end
    }
}

impl ScheduleRestrictionRule for EarliestStartLatestEndTimeRule {
    fn can_employee_work_shift(
        &self,
        _dataset: &dyn TimeCard,
        shift: &EmployeeShift,
        rule_item: &RuleItem,
    ) -> ScheduleRestrictionResult {
        let params = rule_item
            .params()
            .fixed(&EarliestStartLatestEndTimeRuleConfig.default_values());

        if !self.shift_falls_on_included_day_of_week(shift.shift_date(), &params) {
            return ScheduleRestrictionResult::ok();
        }

        if self.shift_falls_within_configured_times(shift, &params) {
            ScheduleRestrictionResult::ok()
        } else {
            ScheduleRestrictionResult::error(ShiftErrorType::EarliestStartLatestEnd)
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

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            1,
            1,
            "Earliest start / latest end",
            RuleClass::EarliestStartLatestEndSrr,
            params,
        )
    }

    fn shift_with_times(shift_date: LocalDate, start: &str, end: &str) -> EmployeeShift {
        EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new()).with_times(
            Some(shift_date.at_time(LocalTime::parse(start))),
            Some(shift_date.at_time(LocalTime::parse(end))),
        )
    }

    mod java_parity_tests {
        use super::*;

        /// `EarliestStartLatestEndTimeRuleImplTest`: "a shift that falls on
        /// a day that is not configured is not restricted".
        #[test]
        fn a_shift_that_falls_on_a_day_that_is_not_configured_is_not_restricted() {
            let dataset = TimeCardData::new();
            let shift_date = LocalDate::of(2018, 2, 28);
            let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new());
            let params = rule_params! { DOW_MULTIPLE_PROP => "[0,1,2,3,5,6]" };

            let result = EarliestStartLatestEndTimeRule.can_employee_work_shift(
                &dataset,
                &shift,
                &rule_item(params),
            );

            assert!(result.is_ok());
        }

        /// `EarliestStartLatestEndTimeRuleImplTest`: "a shift that falls on
        /// a day that is configured and starts or ends outside of the
        /// configured times is restricted".
        #[test]
        fn starting_or_ending_outside_the_configured_times_is_restricted() {
            let dataset = TimeCardData::new();
            let shift_date = LocalDate::of(2018, 2, 28);
            let params = rule_params! {
                EARLIEST_START_TIME_PROP => "05:00:00",
                LATEST_END_TIME_PROP => "22:00:00",
                DOW_MULTIPLE_PROP => "[0,1,2,3,4,5,6]"
            };

            for (start, end) in [
                ("00:00:00", "23:59:59"),
                ("04:59:59", "22:00:00"),
                ("05:00:00", "22:00:01"),
            ] {
                let shift = shift_with_times(shift_date, start, end);
                let result = EarliestStartLatestEndTimeRule.can_employee_work_shift(
                    &dataset,
                    &shift,
                    &rule_item(params.clone()),
                );

                assert_eq!(
                    result.shift_error_type(),
                    Some(ShiftErrorType::EarliestStartLatestEnd),
                    "start={start} end={end}"
                );
            }
        }

        /// `EarliestStartLatestEndTimeRuleImplTest`: "a shift that falls on
        /// a day that is configured and starts or ends within the configured
        /// times is not restricted".
        #[test]
        fn starting_and_ending_within_the_configured_times_is_not_restricted() {
            let dataset = TimeCardData::new();
            let shift_date = LocalDate::of(2018, 2, 28);
            let params = rule_params! {
                EARLIEST_START_TIME_PROP => "05:00:00",
                LATEST_END_TIME_PROP => "22:00:00",
                DOW_MULTIPLE_PROP => "[0,1,2,3,4,5,6]"
            };

            for (start, end) in [("05:00:00", "22:00:00"), ("05:00:01", "21:59:59")] {
                let shift = shift_with_times(shift_date, start, end);
                let result = EarliestStartLatestEndTimeRule.can_employee_work_shift(
                    &dataset,
                    &shift,
                    &rule_item(params.clone()),
                );

                assert!(result.is_ok(), "start={start} end={end}");
            }
        }

        /// `EarliestStartLatestEndTimeRuleImplTest`: "a shift that crosses
        /// midnight is not restricted, provided the rule is configured that
        /// way".
        #[test]
        fn a_shift_crossing_midnight_is_not_restricted_when_the_rule_is_configured_that_way() {
            let dataset = TimeCardData::new();
            let shift_date = LocalDate::of(2018, 2, 28);
            let params = rule_params! {
                EARLIEST_START_TIME_PROP => "20:00:00",
                LATEST_END_TIME_PROP => "05:00:00",
                DOW_MULTIPLE_PROP => "[0,1,2,3,4,5,6]"
            };
            // The Groovy fixture leaves the end time on the same calendar
            // date as the start time (`shiftDate.toLocalDateTime(...)` for
            // both), so this asserts the *acceptable window*'s own midnight
            // rollover, not anything about how the shift's own end date is
            // constructed.
            let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Schedule, Vec::new())
                .with_times(
                    Some(shift_date.at_time(LocalTime::parse("20:00:01"))),
                    Some(shift_date.at_time(LocalTime::parse("04:59:59"))),
                );

            let result = EarliestStartLatestEndTimeRule.can_employee_work_shift(
                &dataset,
                &shift,
                &rule_item(params),
            );

            assert!(result.is_ok());
        }
    }
}
