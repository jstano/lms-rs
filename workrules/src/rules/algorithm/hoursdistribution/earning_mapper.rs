//! Port of `EarningMapper`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/EarningMapper.java`.
//!
//! Indexes a period's earnings twice — by date for the ones that stand alone,
//! by shift for the ones attached to one — so a rule walking a week can ask for
//! either without rescanning. Only `MinHrsForFullTimeOTRuleImpl` uses it;
//! `CaliforniaOTHrsRuleImpl` builds the same two maps inline with
//! `partitioningBy` rather than reaching for this class.
//!
//! # An earning goes in exactly one of the two maps
//!
//! `addEarning` branches on `earning.getShift() == null`: attached earnings go
//! only into the shift map, standalone ones only into the daily map. So
//! `getEarningsForDate(d)` does **not** return everything dated `d` — it
//! returns what is dated `d` and attached to no shift.
//!
//! # Three filters, all of which must pass
//!
//! `shouldAddEarning` requires the date to be in range, the earning type to be
//! one of the rule's configured ids, and the employee's job status on that date
//! to be not salaried-exempt. The last one dereferences
//! `getEmployeeJobStatus(...)` without a null check, so an earning against a
//! job the employee held no status for on that date throws. There is no reading
//! under which such an earning is known to be non-exempt, so it is dropped
//! here — the same call the `punchvalidation` port made for a missing employee
//! (divergence 20).
//!
//! # Keyed by shift id
//!
//! Java's `shiftEarningMap` is a `HashMap<EmployeeShift, …>` and `EmployeeShift`
//! overrides neither `equals` nor `hashCode`, so it keys on object identity.
//! Earnings reach this map through `earning.getShift()`, and
//! [`EmployeeEarning`](crate::entity::employee_earning::EmployeeEarning) carries
//! that as a `shift_id` here, so the id is the key. Within one time card the
//! two agree.

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// A period's earnings, indexed by date and by shift. `EarningMapper`.
///
/// Holds positions into [`TimeCard::earnings`], for the reason
/// [`DailyData`](super::daily_data::DailyData) does.
#[derive(Debug, Clone, Default)]
pub struct EarningMapper {
    daily_earning_map: HashMap<LocalDate, Vec<usize>>,
    shift_earning_map: HashMap<i32, Vec<usize>>,
}

impl EarningMapper {
    /// Index the time card's earnings over `period`, keeping only the
    /// configured earning types. `EarningMapper(TimeCard, DateRange, Set)`.
    pub fn new(time_card: &dyn TimeCard, period: &DateRange, selected_type_ids: &[i32]) -> Self {
        let mut mapper = Self::default();

        // Java seeds an empty list for every date in the range first, so a
        // lookup for a date in range hits rather than falling through to the
        // empty-list default. Nothing observable depends on the difference,
        // but it is why `addEarning` can `get` the daily list unguarded.
        for date in period.dates() {
            mapper.daily_earning_map.insert(date, Vec::new());
        }

        for (index, earning) in time_card.earnings().iter().enumerate() {
            if should_add_earning(time_card, period, selected_type_ids, earning) {
                mapper.add_earning(index, earning);
            }
        }

        mapper
    }

    /// The earnings dated `date` that are attached to no shift.
    /// `getEarningsForDate()`.
    pub fn earnings_for_date(&self, date: LocalDate) -> &[usize] {
        self.daily_earning_map
            .get(&date)
            .map_or(&[][..], Vec::as_slice)
    }

    /// The earnings attached to this shift. `getEarningsForShift()`.
    pub fn earnings_for_shift(&self, shift_id: i32) -> &[usize] {
        self.shift_earning_map
            .get(&shift_id)
            .map_or(&[][..], Vec::as_slice)
    }

    fn add_earning(&mut self, index: usize, earning: &EmployeeEarning) {
        match earning.shift_id() {
            Some(shift_id) => self
                .shift_earning_map
                .entry(shift_id)
                .or_default()
                .push(index),
            None => self
                .daily_earning_map
                .entry(earning.earning_date())
                .or_default()
                .push(index),
        }
    }
}

/// `shouldAddEarning` — in range, of a configured type, and against a job the
/// employee was not salaried-exempt in on that date.
fn should_add_earning(
    time_card: &dyn TimeCard,
    period: &DateRange,
    selected_type_ids: &[i32],
    earning: &EmployeeEarning,
) -> bool {
    period.contains_date(earning.earning_date())
        && selected_type_ids.contains(&earning.earning_type_id())
        && time_card
            .employee()
            .and_then(|employee| {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            })
            .is_some_and(|status| status.pay_type().is_not_salaried_exempt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;

    const HOURLY_JOB: i32 = 200;
    const EXEMPT_JOB: i32 = 300;
    const HOLIDAY: i32 = 7;
    const BONUS: i32 = 8;
    const UNCONFIGURED: i32 = 9;

    fn week() -> DateRange {
        DateRange::new(LocalDate::of(2010, 1, 3), LocalDate::of(2010, 1, 9))
    }

    fn earning(id: i32, job_id: i32, type_id: i32, date: LocalDate) -> EmployeeEarning {
        EmployeeEarning::new(
            id,
            100,
            job_id,
            type_id,
            date,
            8.0,
            10.0,
            EarningSource::Manual,
        )
    }

    fn employee() -> Employee {
        let start = LocalDate::of(2009, 1, 1);
        let end = LocalDate::of(2011, 12, 31);

        Employee::new(
            100,
            11,
            "Someone",
            vec![
                EmployeeJobStatus::new(
                    1,
                    100,
                    HOURLY_JOB,
                    start,
                    end,
                    EmployeePayType::Hourly,
                    10.0,
                    true,
                ),
                EmployeeJobStatus::new(
                    2,
                    100,
                    EXEMPT_JOB,
                    start,
                    end,
                    EmployeePayType::SalariedExempt,
                    0.0,
                    false,
                ),
            ],
        )
    }

    fn card(earnings: Vec<EmployeeEarning>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_earnings(earnings)
    }

    #[test]
    fn a_standalone_earning_lands_in_the_daily_map() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![earning(1, HOURLY_JOB, HOLIDAY, date)]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY]);

        assert_eq!(mapper.earnings_for_date(date), &[0]);
    }

    #[test]
    fn an_attached_earning_lands_only_in_the_shift_map() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![
            earning(1, HOURLY_JOB, HOLIDAY, date).from_rule(5, Some(42)),
        ]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY]);

        assert_eq!(mapper.earnings_for_shift(42), &[0]);
        assert!(
            mapper.earnings_for_date(date).is_empty(),
            "the daily map holds only earnings with no shift"
        );
    }

    #[test]
    fn several_earnings_on_one_shift_keep_their_order() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![
            earning(1, HOURLY_JOB, HOLIDAY, date).from_rule(5, Some(42)),
            earning(2, HOURLY_JOB, BONUS, date).from_rule(5, Some(42)),
        ]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY, BONUS]);

        assert_eq!(mapper.earnings_for_shift(42), &[0, 1]);
    }

    #[test]
    fn an_earning_outside_the_period_is_dropped() {
        let card = card(vec![earning(
            1,
            HOURLY_JOB,
            HOLIDAY,
            LocalDate::of(2010, 1, 20),
        )]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY]);

        assert!(
            mapper
                .earnings_for_date(LocalDate::of(2010, 1, 20))
                .is_empty()
        );
    }

    #[test]
    fn an_earning_of_an_unconfigured_type_is_dropped() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![earning(1, HOURLY_JOB, UNCONFIGURED, date)]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY, BONUS]);

        assert!(mapper.earnings_for_date(date).is_empty());
    }

    #[test]
    fn a_salaried_exempt_earning_is_dropped() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![earning(1, EXEMPT_JOB, HOLIDAY, date)]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY]);

        assert!(mapper.earnings_for_date(date).is_empty());
    }

    #[test]
    fn an_earning_against_a_job_with_no_status_is_dropped() {
        // Java throws here; nothing says such an earning is non-exempt.
        let date = LocalDate::of(2010, 1, 4);
        let card = card(vec![earning(1, 999, HOLIDAY, date)]);

        let mapper = EarningMapper::new(&card, &week(), &[HOLIDAY]);

        assert!(mapper.earnings_for_date(date).is_empty());
    }

    #[test]
    fn a_date_in_range_with_no_earnings_answers_empty() {
        let mapper = EarningMapper::new(&card(Vec::new()), &week(), &[HOLIDAY]);

        assert!(
            mapper
                .earnings_for_date(LocalDate::of(2010, 1, 4))
                .is_empty()
        );
        assert!(
            mapper
                .earnings_for_date(LocalDate::of(2010, 6, 1))
                .is_empty()
        );
        assert!(mapper.earnings_for_shift(42).is_empty());
    }
}

/// `EarningMapperTest.groovy`, transcribed.
///
/// All seven cases. Two adjustments, neither touching what is asserted: the
/// Groovy anchors its week on `WeeklyDateRange.withStartDate(new LocalDate())`
/// — today — and this pins it to a fixed date, as the `punchvalidation` port
/// did for the same reason; and the Groovy's `Mock(Employee)` answering
/// `getEmployeeJobStatus(hourlyJob, _)` becomes a real employee holding a
/// status for each of the two jobs over a span that covers the week.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::time_card::TimeCardData;
    use date_range_rs::WeeklyDateRange;
    use rstest::rstest;

    /// `hourlyJob = new Assignment(id: 32)`.
    const HOURLY_JOB: i32 = 32;
    /// `salaryJob = new Assignment(id: 33)`.
    const SALARY_JOB: i32 = 33;
    /// `selectedTypeIDs = [5,6,7] as Set`.
    const SELECTED_TYPE_IDS: &[i32] = &[5, 6, 7];
    /// `shift = new EmployeeShift(id: 10)`.
    const SHIFT: i32 = 10;

    /// `WeeklyDateRange.withStartDate(new LocalDate())`, pinned.
    fn date_range() -> DateRange {
        WeeklyDateRange::with_start_date(LocalDate::of(2010, 11, 22))
    }

    fn employee() -> Employee {
        let start = LocalDate::of(2000, 1, 1);
        let end = LocalDate::of(2020, 1, 1);

        Employee::new(
            1,
            11,
            "",
            vec![
                EmployeeJobStatus::new(
                    1,
                    1,
                    HOURLY_JOB,
                    start,
                    end,
                    EmployeePayType::Hourly,
                    10.0,
                    true,
                ),
                EmployeeJobStatus::new(
                    2,
                    1,
                    SALARY_JOB,
                    start,
                    end,
                    EmployeePayType::SalariedExempt,
                    0.0,
                    false,
                ),
            ],
        )
    }

    /// `createDefaultEarning(shift)` — the week's start date, earning type 5,
    /// the hourly job.
    fn default_earning(shift_id: Option<i32>) -> EmployeeEarning {
        earning_of(shift_id, 5, HOURLY_JOB, date_range().start_date())
    }

    /// The same with the field each `setX` case overrides spelled out, since
    /// the entity carries no setter for the three the Groovy reaches for.
    fn earning_of(
        shift_id: Option<i32>,
        earning_type_id: i32,
        job_id: i32,
        earning_date: LocalDate,
    ) -> EmployeeEarning {
        let earning = EmployeeEarning::new(
            1,
            1,
            job_id,
            earning_type_id,
            earning_date,
            0.0,
            0.0,
            EarningSource::Manual,
        );

        match shift_id {
            Some(id) => earning.from_rule(0, Some(id)),
            None => earning,
        }
    }

    fn card(earnings: Vec<EmployeeEarning>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_earnings(earnings)
    }

    fn mapper(card: &TimeCardData) -> EarningMapper {
        EarningMapper::new(card, &date_range(), SELECTED_TYPE_IDS)
    }

    /// `should always return at least an empty list for any date regardless if
    /// in the date range` — the three days spanning the week's start.
    #[rstest]
    #[case(-1)]
    #[case(0)]
    #[case(1)]
    fn should_always_return_at_least_an_empty_list_for_any_date(#[case] offset: i64) {
        let card = card(Vec::new());

        assert!(
            mapper(&card)
                .earnings_for_date(date_range().start_date().plus_days(offset))
                .is_empty()
        );
    }

    #[test]
    fn should_always_return_at_least_an_empty_list_for_any_shift() {
        let card = card(Vec::new());

        assert!(mapper(&card).earnings_for_shift(12).is_empty());
    }

    #[test]
    fn should_put_earnings_without_shift_in_date_map() {
        let card = card(vec![default_earning(None)]);

        assert_eq!(
            mapper(&card).earnings_for_date(date_range().start_date()),
            &[0]
        );
    }

    #[test]
    fn should_put_earning_with_shift_in_shift_map() {
        let card = card(vec![
            default_earning(Some(SHIFT)),
            default_earning(Some(SHIFT)),
        ]);
        let mapper = mapper(&card);

        assert!(
            mapper
                .earnings_for_date(date_range().start_date())
                .is_empty()
        );
        assert_eq!(mapper.earnings_for_shift(SHIFT), &[0, 1]);
        assert!(mapper.earnings_for_shift(77).is_empty());
    }

    #[test]
    fn should_not_put_earnings_in_map_if_outside_date_range() {
        let before = date_range().start_date().minus_days(1);
        let after = date_range().end_date().plus_days(1);

        let card = card(vec![
            earning_of(None, 5, HOURLY_JOB, before),
            earning_of(Some(SHIFT), 5, HOURLY_JOB, after),
        ]);
        let mapper = mapper(&card);

        assert!(mapper.earnings_for_date(before).is_empty());
        assert!(mapper.earnings_for_date(after).is_empty());
        assert!(mapper.earnings_for_shift(SHIFT).is_empty());
    }

    #[test]
    fn should_not_include_earnings_that_are_not_selected() {
        let start = date_range().start_date();
        let card = card(vec![
            earning_of(None, 1, HOURLY_JOB, start),
            earning_of(Some(SHIFT), 2, HOURLY_JOB, start),
        ]);
        let mapper = mapper(&card);

        assert!(
            mapper
                .earnings_for_date(date_range().start_date())
                .is_empty()
        );
        assert!(mapper.earnings_for_shift(SHIFT).is_empty());
    }

    #[test]
    fn should_not_include_salary_exempt_earnings() {
        let start = date_range().start_date();
        let card = card(vec![
            earning_of(None, 5, SALARY_JOB, start),
            earning_of(Some(SHIFT), 5, SALARY_JOB, start),
        ]);
        let mapper = mapper(&card);

        assert!(
            mapper
                .earnings_for_date(date_range().start_date())
                .is_empty()
        );
        assert!(mapper.earnings_for_shift(SHIFT).is_empty());
    }
}
