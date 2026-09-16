//! Port of `HolidayDTHrsRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/HolidayDTHrsRuleImpl.java`.
//!
//! `HOLIDAY_DT_HDR`. Every regular hour worked on one of the configured holiday
//! types becomes double time: a new double-time distribution is added carrying
//! the whole amount, and the regular one is **set to zero** rather than
//! removed. The shift keeps both rows, so the reader can still see where the
//! hours came from.
//!
//! The simplest real rule in the family, and the shape the rest follow: select
//! distributions, create a premium row from each, reduce the regular one.
//!
//! # It matches buckets by **id**, not by name
//!
//! `timeCard.distributionsWithShiftDuringPeriodMatchingType(workWeek, REGULAR_ID)`
//! and `createPremiumDistribution(distribution, DT_ID, …)` — the constants
//! `HoursDistributionType.REGULAR_ID` (1) and `DT_ID` (3), straight off the
//! class. Nothing here goes through `TimeCard.getDTHoursDistributionTypeId()`,
//! so this rule is *immune* to the renamed-bucket problem the name-based
//! lookups have, and equally blind to a property that configured its double
//! time under a different id. The two mechanisms coexist in one family; the
//! overtime rules use the name lookup and this one does not.
//!
//! # Three filters
//!
//! A distribution pays double time when it is regular, dated inside the work
//! week (both from the selector above), **open for editing**, and dated on a
//! holiday whose type the rule item selected. A closed pay period silently
//! leaves the hours alone.
//!
//! # The holiday lookup is per **distribution property**, not per shift
//!
//! `distribution.getPropertyID()` — the property that owns the *job the hours
//! were worked at*, which for a multi-property employee is not necessarily the
//! employee's own. Java memoizes `holidayDAO.findAllForProperty` into a
//! `Map<Integer, Map<LocalDate, Integer>>` field on the request-scoped bean;
//! see the divergence note on [`HolidayDTHrsRule`] for what that becomes here.
//!
//! One sharp edge inherited as-is: Java builds that inner map with
//! `toMap(Holiday::getHolidayDate, holidayTypeIDMapper)`, which **throws** on a
//! duplicate key. Two holidays of different types on one date at one property
//! is a configuration the DAO can return and this collector cannot survive.

use crate::common::json_ids::ids_for_key;
use crate::entity::hours_distribution_type::HoursDistributionType;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::hoursdistribution::HoursDistributionRule;
use crate::rules::algorithm::hoursdistribution::config::{
    HOLIDAY_TYPES_PROP, HolidayDTHrsRuleConfig,
};
use crate::rules::algorithm::utility::hours_distribution_factory::create_premium_distribution;
use crate::rules::ports::HolidayPort;
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// Pay double time for hours worked on a configured holiday.
/// `HolidayDTHrsRuleImpl`.
///
/// # The holiday cache is per `execute`, not per request
///
/// Java's `holidays` map is a field on a `@Scope("request")` bean, so it
/// survives across the work weeks of one calculation. Here it is built once
/// inside each `execute` for the properties that week's distributions actually
/// name. Same DAO answers, same result — a memo over a pure lookup, as
/// divergences 15 and 28 already settled — and it keeps `execute` taking
/// `&self` rather than needing interior mutability for a cache nothing reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct HolidayDTHrsRule<P: HolidayPort> {
    holidays: P,
}

impl<P: HolidayPort> HolidayDTHrsRule<P> {
    /// Build the rule over its holiday lookup.
    pub fn new(holidays: P) -> Self {
        Self { holidays }
    }

    /// `holidayInjector` — the property's holidays as date to holiday type id.
    ///
    /// Java's `toMap` throws when a property has two holidays on one date;
    /// here the last one read wins. Neither behaviour is specified, and a
    /// panic in a rule engine is worse than an arbitrary pick.
    fn holidays_for_property(&self, property_id: i32) -> HashMap<LocalDate, i32> {
        self.holidays
            .find_all_for_property(property_id)
            .into_iter()
            .map(|holiday| (holiday.holiday_date(), holiday.holiday_type_id()))
            .collect()
    }
}

impl<P: HolidayPort> HoursDistributionRule for HolidayDTHrsRule<P> {
    fn execute(&self, time_card: &mut dyn TimeCard, work_week: &DateRange, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&HolidayDTHrsRuleConfig.default_values());
        let configured_holiday_ids = ids_for_key(HOLIDAY_TYPES_PROP, &params);

        let regular = time_card.distribution_indices_during_period_matching_type(
            work_week,
            HoursDistributionType::REGULAR_ID,
        );

        // One lookup per distinct property named by the selected distributions,
        // which is Java's `computeIfAbsent` over the same set.
        let mut holidays_by_property: HashMap<i32, HashMap<LocalDate, i32>> = HashMap::new();

        let on_a_configured_holiday: Vec<(usize, usize)> = regular
            .into_iter()
            .filter(|&(shift_index, distribution_index)| {
                let distribution =
                    &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
                time_card.is_open_for_editing_on(distribution.date())
            })
            .filter(|&(shift_index, distribution_index)| {
                let distribution =
                    &time_card.shifts()[shift_index].hours_distributions()[distribution_index];
                let property_id = distribution.property_id();

                holidays_by_property
                    .entry(property_id)
                    .or_insert_with(|| self.holidays_for_property(property_id))
                    .get(&distribution.date())
                    .is_some_and(|holiday_type_id| configured_holiday_ids.contains(holiday_type_id))
            })
            .collect();

        for (shift_index, distribution_index) in on_a_configured_holiday {
            let shift = &mut time_card.shifts_mut()[shift_index];
            let distribution = &shift.hours_distributions()[distribution_index];

            let double_time = create_premium_distribution(
                distribution,
                HoursDistributionType::DT_ID,
                distribution.hours(),
                Some(rule_item.id()),
            );

            shift.add_hours_distribution(double_time);
            // The regular row stays, at zero hours.
            shift.hours_distributions_mut()[distribution_index].set_hours(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::holiday::Holiday;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;

    const PROPERTY: i32 = 1;
    const OTHER_PROPERTY: i32 = 2;
    const FEDERAL: i32 = 1;

    struct Holidays(Vec<Holiday>);

    impl HolidayPort for Holidays {
        fn find_all_for_property(&self, property_id: i32) -> Vec<Holiday> {
            self.0
                .iter()
                .filter(|holiday| holiday.property_id() == property_id)
                .cloned()
                .collect()
        }
    }

    fn week() -> DateRange {
        DateRange::new(LocalDate::of(2010, 1, 3), LocalDate::of(2010, 1, 9))
    }

    fn rule_item(holiday_types: &str) -> RuleItem {
        RuleItem::new(
            7,
            1,
            "Holiday DT",
            RuleClass::HolidayDtHdr,
            rule_params! { HOLIDAY_TYPES_PROP => holiday_types },
        )
    }

    fn shift(property_id: i32, distributions: Vec<HoursDistribution>) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_property_id(property_id)
        .with_hours_distributions(distributions)
    }

    fn regular(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(
            PROPERTY,
            date,
            Some(HoursDistributionType::REGULAR_ID),
            hours,
            10.0,
        )
    }

    #[test]
    fn hours_on_a_configured_holiday_become_double_time() {
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(PROPERTY, vec![regular(date, 8.0)])]);

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(
            10, PROPERTY, date, "New Year", FEDERAL,
        )]))
        .execute(&mut card, &week(), &rule_item("[1,2,3]"));

        let distributions = card.shifts()[0].hours_distributions();

        assert_eq!(distributions.len(), 2);
        assert_eq!(
            distributions[0].hours(),
            0.0,
            "the regular row stays, at zero"
        );
        assert_eq!(
            distributions[0].hours_distribution_type_id(),
            Some(HoursDistributionType::REGULAR_ID)
        );
        assert_eq!(distributions[1].hours(), 8.0);
        assert_eq!(
            distributions[1].hours_distribution_type_id(),
            Some(HoursDistributionType::DT_ID)
        );
    }

    #[test]
    fn the_double_time_row_records_the_rule_item_that_made_it() {
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(PROPERTY, vec![regular(date, 8.0)])]);

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(
            10, PROPERTY, date, "", FEDERAL,
        )]))
        .execute(&mut card, &week(), &rule_item("[1]"));

        let double_time = &card.shifts()[0].hours_distributions()[1];

        assert_eq!(double_time.hours_rule_item_id(), Some(7));
        assert_eq!(double_time.original_hours(), 0.0);
        assert_eq!(
            double_time.base_rate(),
            10.0,
            "inherited from the regular row"
        );
    }

    #[test]
    fn a_premium_distribution_is_not_selected() {
        // The selector matches REGULAR_ID only, so re-running the rule does not
        // pay double time on the double time it already created.
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(PROPERTY, vec![regular(date, 8.0)])]);
        let rule = HolidayDTHrsRule::new(Holidays(vec![Holiday::new(
            10, PROPERTY, date, "", FEDERAL,
        )]));

        rule.execute(&mut card, &week(), &rule_item("[1]"));
        rule.execute(&mut card, &week(), &rule_item("[1]"));

        let distributions = card.shifts()[0].hours_distributions();

        assert_eq!(distributions.len(), 3, "a second, empty DT row");
        assert_eq!(distributions[1].hours(), 8.0);
        assert_eq!(
            distributions[2].hours(),
            0.0,
            "the regular row was already zero, so the second pass moves nothing"
        );
    }

    #[test]
    fn the_holidays_consulted_are_the_distributions_property_not_the_shifts() {
        // The distribution names PROPERTY; the shift names OTHER_PROPERTY.
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(OTHER_PROPERTY, vec![regular(date, 8.0)])]);

        HolidayDTHrsRule::new(Holidays(vec![
            Holiday::new(10, PROPERTY, date, "", FEDERAL),
            Holiday::new(11, OTHER_PROPERTY, LocalDate::of(2010, 7, 4), "", FEDERAL),
        ]))
        .execute(&mut card, &week(), &rule_item("[1]"));

        assert_eq!(
            card.shifts()[0].hours_distributions().len(),
            2,
            "PROPERTY's calendar decides it, not the shift's"
        );
    }

    #[test]
    fn an_unconfigured_rule_item_pays_nothing() {
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(PROPERTY, vec![regular(date, 8.0)])]);

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(
            10, PROPERTY, date, "", FEDERAL,
        )]))
        .execute(
            &mut card,
            &week(),
            &RuleItem::new(
                7,
                1,
                "Holiday DT",
                RuleClass::HolidayDtHdr,
                crate::rules::params::RuleParams::new(),
            ),
        );

        assert_eq!(
            card.shifts()[0].hours_distributions().len(),
            1,
            "holidayTypes defaults to []"
        );
    }

    #[test]
    fn two_holidays_on_one_date_do_not_panic() {
        // Java's toMap collector throws here; the last read wins instead.
        let date = LocalDate::of(2010, 1, 4);
        let mut card =
            TimeCardData::new().with_shifts(vec![shift(PROPERTY, vec![regular(date, 8.0)])]);

        HolidayDTHrsRule::new(Holidays(vec![
            Holiday::new(10, PROPERTY, date, "One", 9),
            Holiday::new(11, PROPERTY, date, "Two", FEDERAL),
        ]))
        .execute(&mut card, &week(), &rule_item("[1]"));

        assert_eq!(card.shifts()[0].hours_distributions().len(), 2);
    }

    #[test]
    fn every_regular_row_on_the_holiday_is_converted() {
        let date = LocalDate::of(2010, 1, 4);
        let mut card = TimeCardData::new().with_shifts(vec![shift(
            PROPERTY,
            vec![regular(date, 6.0), regular(date, 2.0)],
        )]);

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(
            10, PROPERTY, date, "", FEDERAL,
        )]))
        .execute(&mut card, &week(), &rule_item("[1]"));

        let distributions = card.shifts()[0].hours_distributions();

        assert_eq!(distributions.len(), 4);
        assert_eq!(distributions[0].hours(), 0.0);
        assert_eq!(distributions[1].hours(), 0.0);
        assert_eq!(distributions[2].hours(), 6.0);
        assert_eq!(distributions[3].hours(), 2.0);
    }
}

/// `HolidayDTHrsRuleImplTest.groovy`, transcribed.
///
/// All four cases. The Groovy anchors on `LocalDate.now()`; pinned to a fixed
/// date, as the other transcriptions are. The dataset's calculation start date
/// again comes from a mocked `PayGroup` whose `currentPayPeriod()` is the work
/// week — which makes every date in the week open for editing, except in the
/// third case, where the mock shifts the period a day later on purpose.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::holiday::Holiday;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;

    /// `new Property(id: 1, name: 'jobProperty', ...)`.
    const PROPERTY: i32 = 1;
    /// `new Assignment(id: 2, name: 'job', property: property)`.
    const JOB: i32 = 2;

    /// `static today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    fn tomorrow() -> LocalDate {
        today().plus_days(1)
    }

    /// `new LegacyDatePeriod(today, today.plusWeeks(1))`.
    fn work_week() -> DateRange {
        DateRange::new(today(), today().plus_days(7))
    }

    struct Holidays(Vec<Holiday>);

    impl HolidayPort for Holidays {
        fn find_all_for_property(&self, property_id: i32) -> Vec<Holiday> {
            self.0
                .iter()
                .filter(|holiday| holiday.property_id() == property_id)
                .cloned()
                .collect()
        }
    }

    /// `new RuleItem(params: [(HOLIDAY_TYPES_PROP): '[1,2,3]'])`.
    fn rule_item() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "",
            RuleClass::HolidayDtHdr,
            rule_params! { HOLIDAY_TYPES_PROP => "[1,2,3]" },
        )
    }

    fn distribution(date: LocalDate, hours: f64) -> HoursDistribution {
        HoursDistribution::new(
            PROPERTY,
            date,
            Some(HoursDistributionType::REGULAR_ID),
            hours,
            0.0,
        )
    }

    fn shift(shift_date: LocalDate, distributions: Vec<HoursDistribution>) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_property_id(PROPERTY)
            .with_hours_distributions(distributions)
    }

    /// The pay period is the work week unless a case says otherwise, so the
    /// calculation start date is the week's own start.
    fn card(shift: EmployeeShift, pay_period_start: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(vec![shift])
            .with_calculation_start_date(pay_period_start)
    }

    #[test]
    fn shifts_that_do_not_fall_on_a_configured_holiday_do_not_pay_dt() {
        // The holiday is of type 10; the rule item selects 1, 2 and 3.
        let mut card = card(shift(today(), vec![distribution(today(), 8.0)]), today());

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(10, PROPERTY, today(), "", 10)])).execute(
            &mut card,
            &work_week(),
            &rule_item(),
        );

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
    }

    #[test]
    fn shifts_that_do_not_have_distributions_on_a_configured_holiday_do_not_pay_dt() {
        // The holiday is today; the shift's hours land tomorrow.
        let mut card = card(
            shift(tomorrow(), vec![distribution(tomorrow(), 8.0)]),
            today(),
        );

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(1, PROPERTY, today(), "", 1)])).execute(
            &mut card,
            &work_week(),
            &rule_item(),
        );

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
    }

    #[test]
    fn shifts_only_get_dt_paid_on_distributions_that_are_open_for_edit() {
        // The pay period starts a day after the work week does, so today is
        // closed even though the holiday matches.
        let mut card = card(shift(today(), vec![distribution(today(), 8.0)]), tomorrow());

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(1, PROPERTY, today(), "", 1)])).execute(
            &mut card,
            &work_week(),
            &rule_item(),
        );

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
    }

    #[test]
    fn shifts_only_get_dt_paid_on_distributions_that_fall_on_the_holiday() {
        // An overnight shift: six hours today, two tomorrow, and the holiday
        // is tomorrow.
        let mut card = card(
            shift(
                tomorrow(),
                vec![distribution(today(), 6.0), distribution(tomorrow(), 2.0)],
            ),
            today(),
        );

        HolidayDTHrsRule::new(Holidays(vec![Holiday::new(1, PROPERTY, tomorrow(), "", 1)]))
            .execute(&mut card, &work_week(), &rule_item());

        let distributions = card.shifts()[0].hours_distributions();

        assert_eq!(distributions.len(), 3);
        assert!(distributions.iter().any(|d| d.date() == today()
            && d.hours_distribution_type_id() == Some(HoursDistributionType::REGULAR_ID)
            && d.hours() == 6.0));
        assert!(distributions.iter().any(|d| d.date() == tomorrow()
            && d.hours_distribution_type_id() == Some(HoursDistributionType::REGULAR_ID)
            && d.hours() == 0.0));
        assert!(distributions.iter().any(|d| d.date() == tomorrow()
            && d.hours_distribution_type_id() == Some(HoursDistributionType::DT_ID)
            && d.hours() == 2.0));
    }
}
