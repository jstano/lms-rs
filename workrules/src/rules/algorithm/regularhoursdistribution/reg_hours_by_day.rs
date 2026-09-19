//! Port of `RegularHoursByDayRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularhoursdistribution/RegularHoursByDayRuleImpl.java`.
//!
//! Splits a shift into two regular distributions when it crosses a configured
//! "day start/end time" (default midnight): one dated the shift's own date,
//! one dated the day after. A shift that does not cross the boundary gets the
//! plain single distribution, same as
//! [`RegularHoursOnShiftDateRule`](super::reg_hours_on_shift_date::RegularHoursOnShiftDateRule).
//!
//! Ported cases: `RegularHoursByDayRuleImplTest.groovy` (282 lines).

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::regularhoursdistribution::RegularHoursDistributionRule;
use crate::rules::algorithm::regularhoursdistribution::config::{
    DAY_START_AND_END_TIME, RegularHoursByDayRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::{
    create_distribution_of_configured_type, create_split_distributions,
};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalTime;

/// `RegularHoursByDayRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegularHoursByDayRule;

impl RegularHoursDistributionRule for RegularHoursByDayRule {
    fn execute(&self, shift: &mut EmployeeShift, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&RegularHoursByDayRuleConfig.default_values());
        let day_start_and_end_time = day_start_and_end_time(&params);

        if shift_spans_day(shift, day_start_and_end_time) {
            let day_split_time = shift
                .shift_date()
                .plus_days(1)
                .at_time(day_start_and_end_time);
            let distributions =
                create_split_distributions(shift, day_split_time, &params, Some(rule_item.id()));
            shift.add_hours_distributions(distributions);
        } else {
            let distribution = create_distribution_of_configured_type(
                shift.shift_date(),
                shift.property_id(),
                shift.net_hours(),
                &params,
                Some(rule_item.id()),
            );
            shift.add_hours_distribution(distribution);
        }
    }
}

fn day_start_and_end_time(params: &RuleParams) -> LocalTime {
    LocalTime::parse(params.get(DAY_START_AND_END_TIME).unwrap_or("00:00:00"))
}

/// `shiftSpansDay`.
fn shift_spans_day(shift: &EmployeeShift, day_start_and_end_time: LocalTime) -> bool {
    shift.has_both_times()
        && shift.end_date_time().unwrap()
            > shift
                .shift_date()
                .plus_days(1)
                .at_time(day_start_and_end_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            9,
            1,
            "Regular hours by day",
            RuleClass::RegHoursByDayRhd,
            params,
        )
    }

    fn shift_from_to(start: joda_rs::LocalDateTime, end: joda_rs::LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            start.to_local_date(),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(start), Some(end))
    }

    #[test]
    fn a_shift_inside_one_day_gets_a_single_distribution() {
        let mut shift = shift_from_to(
            LocalDate::of(2010, 1, 4).at_time(LocalTime::of(8, 0, 0)),
            LocalDate::of(2010, 1, 4).at_time(LocalTime::of(16, 0, 0)),
        )
        .with_net_hours(8.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item(RuleParams::new()));

        assert_eq!(shift.hours_distributions().len(), 1);
        assert_eq!(
            shift.hours_distributions()[0].date(),
            LocalDate::of(2010, 1, 4)
        );
        assert_eq!(shift.hours_distributions()[0].hours(), 8.0);
    }

    #[test]
    fn a_shift_spanning_midnight_is_split_across_the_two_dates() {
        let mut shift = shift_from_to(
            LocalDate::of(2010, 1, 4).at_time(LocalTime::of(22, 0, 0)),
            LocalDate::of(2010, 1, 5).at_time(LocalTime::of(2, 0, 0)),
        );

        RegularHoursByDayRule.execute(&mut shift, &rule_item(RuleParams::new()));

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_eq!(distributions[0].date(), LocalDate::of(2010, 1, 4));
        assert_eq!(distributions[0].hours(), 2.0);
        assert_eq!(distributions[1].date(), LocalDate::of(2010, 1, 5));
        assert_eq!(distributions[1].hours(), 2.0);
    }

    #[test]
    fn a_configured_split_time_moves_the_boundary() {
        // The boundary is always the day *after* the shift date, at the
        // configured time — 15:00 the next day here, not 15:00 today.
        let params = crate::rule_params! { DAY_START_AND_END_TIME => "15:00:00" };
        let mut shift = shift_from_to(
            LocalDate::of(2010, 1, 4).at_time(LocalTime::of(20, 0, 0)),
            LocalDate::of(2010, 1, 5).at_time(LocalTime::of(18, 0, 0)),
        );

        RegularHoursByDayRule.execute(&mut shift, &rule_item(params));

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_eq!(distributions[0].date(), LocalDate::of(2010, 1, 4));
        assert_eq!(distributions[0].hours(), 19.0);
        assert_eq!(distributions[1].date(), LocalDate::of(2010, 1, 5));
        assert_eq!(distributions[1].hours(), 3.0);
    }

    #[test]
    fn a_shift_with_no_end_time_does_not_span_a_day() {
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(4.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item(RuleParams::new()));

        assert_eq!(shift.hours_distributions().len(), 1);
    }
}

/// `RegularHoursByDayRuleImplTest.groovy` — every case.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    /// `static today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    /// `today.plusDays(1).toLocalDateTime(MIDNIGHT)`.
    fn midnight_tomorrow() -> joda_rs::LocalDateTime {
        today().plus_days(1).at_start_of_day()
    }

    fn punch(punch_type: PunchType, time: joda_rs::LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(
            0,
            punch_type,
            crate::common::enums::punch_source::PunchSource::Clock,
            time,
        )
    }

    fn shift(
        start: joda_rs::LocalDateTime,
        end: joda_rs::LocalDateTime,
        punches: Vec<EmployeeShiftPunch>,
    ) -> EmployeeShift {
        EmployeeShift::new(1, 1, 1, today(), ShiftType::Actual, punches)
            .with_property_id(1)
            .with_times(Some(start), Some(end))
    }

    fn rule_item() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::RegHoursByDayRhd,
            RegularHoursByDayRuleConfig.default_values(),
        )
    }

    fn assert_distribution(
        distribution: &crate::entity::hours_distribution::HoursDistribution,
        date: LocalDate,
        hours: f64,
    ) {
        assert_eq!(distribution.base_rate(), 0.0);
        assert_eq!(distribution.premium_rate(), 0.0);
        assert_eq!(distribution.date(), date);
        assert_eq!(distribution.hours(), hours);
        assert_eq!(distribution.original_hours(), hours);
        assert_eq!(distribution.hours_rule_item_id(), Some(1));
        assert_eq!(
            distribution.hours_distribution_type_id(),
            Some(HoursDistributionType::REGULAR_ID)
        );
        assert_eq!(distribution.rate_rule_item_id(), None);
        assert_eq!(distribution.property_id(), 1);
    }

    #[test]
    fn hours_should_be_distributed_to_shift_date_if_shift_does_not_span_days() {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(10),
            midnight_tomorrow().minus_hours(2),
            Vec::new(),
        )
        .with_net_hours(8.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        assert_eq!(shift.hours_distributions().len(), 1);
        assert_distribution(&shift.hours_distributions()[0], today(), 8.0);
    }

    #[test]
    fn hours_should_be_distributed_proportionately_to_the_dates_they_are_worked_on() {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(4),
            midnight_tomorrow().plus_hours(6),
            Vec::new(),
        )
        .with_net_hours(10.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_distribution(&distributions[0], today(), 4.0);
        assert_distribution(&distributions[1], today().plus_days(1), 6.0);
    }

    #[test]
    fn adjustments_should_be_distributed_proportionately_to_the_amount_of_the_shift_worked_each_day()
     {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(4),
            midnight_tomorrow().plus_hours(6),
            Vec::new(),
        )
        .with_net_hours(10.0)
        .with_worked_hours(10.0)
        .with_adj_hours(-1.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_distribution(&distributions[0], today(), 3.6);
        assert_distribution(&distributions[1], today().plus_days(1), 5.4);
    }

    #[test]
    fn breaks_should_be_distributed_based_on_the_day_they_fall_on() {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(4),
            midnight_tomorrow().plus_hours(6),
            vec![
                punch(PunchType::In, midnight_tomorrow().minus_hours(4)),
                punch(PunchType::Break, midnight_tomorrow().minus_minutes(30)),
                punch(PunchType::Back, midnight_tomorrow().plus_minutes(30)),
                punch(PunchType::Break, midnight_tomorrow().plus_hours(4)),
                punch(PunchType::Back, midnight_tomorrow().plus_hours(5)),
                punch(PunchType::Out, midnight_tomorrow().plus_hours(6)),
            ],
        )
        .with_net_hours(10.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_distribution(&distributions[0], today(), 3.5);
        assert_distribution(&distributions[1], today().plus_days(1), 4.5);
    }

    #[test]
    fn irrational_numbers_are_rounded_correctly() {
        let start = midnight_tomorrow().minus_hours(4).minus_minutes(2);
        let end = midnight_tomorrow().plus_hours(6);
        let mut shift = EmployeeShift::new(
            1,
            1,
            1,
            today(),
            ShiftType::Schedule,
            vec![
                punch(PunchType::In, start),
                punch(PunchType::Break, midnight_tomorrow().minus_minutes(31)),
                punch(PunchType::Back, midnight_tomorrow().plus_minutes(30)),
                punch(PunchType::Out, end),
            ],
        )
        .with_property_id(1)
        .with_times(Some(start), Some(end));
        shift.calc_worked_hours();

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_distribution(&distributions[0], today(), 3.52);
        assert_distribution(&distributions[1], today().plus_days(1), 5.5);
        assert_eq!(
            shift.net_hours(),
            distributions[1].hours() + distributions[0].hours()
        );
    }

    #[test]
    fn proportions_follow_actual_worked_hours_not_including_break_time() {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(6),
            midnight_tomorrow().plus_hours(5),
            vec![
                punch(PunchType::In, midnight_tomorrow().minus_hours(6)),
                punch(PunchType::Break, midnight_tomorrow().minus_hours(1)),
                punch(PunchType::Back, midnight_tomorrow()),
                punch(PunchType::Out, midnight_tomorrow().plus_hours(5)),
            ],
        )
        .with_net_hours(12.0)
        .with_worked_hours(10.0)
        .with_adj_hours(-1.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        let distributions = shift.hours_distributions();
        assert_eq!(distributions.len(), 2);
        assert_distribution(&distributions[0], today(), 4.5);
        assert_distribution(&distributions[1], today().plus_days(1), 4.5);
    }

    #[test]
    fn hours_should_only_be_distributed_to_the_start_date_if_the_shift_ends_at_the_day_cut() {
        let mut shift = shift(
            midnight_tomorrow().minus_hours(8),
            midnight_tomorrow(),
            Vec::new(),
        )
        .with_net_hours(8.0);

        RegularHoursByDayRule.execute(&mut shift, &rule_item());

        assert_eq!(shift.hours_distributions().len(), 1);
        assert_distribution(&shift.hours_distributions()[0], today(), 8.0);
    }
}
