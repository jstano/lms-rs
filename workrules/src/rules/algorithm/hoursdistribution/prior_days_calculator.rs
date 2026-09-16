//! Port of `priordayscalculator.PriorDaysCalculator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/priordayscalculator/PriorDaysCalculator.java`.
//!
//! How many consecutive days the employee had already worked before a work
//! week starts — the number five of the overtime rules seed their
//! [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator) with.
//!
//! Same question as
//! [`consecutive_days_calculator`](super::consecutive_days_calculator), a
//! different answer; that module's table sets out where the two part company.
//! The differences that matter here:
//!
//! - **No shift filtering at all.** Shifts with errors count, and so do
//!   salaried-exempt ones. The other calculator excludes both.
//! - **Earnings can count as worked days**, when the caller passes
//!   `include_configured_earning_types`. A day carrying a configured earning
//!   and no shift then continues the run rather than breaking it.
//! - **The DAO is consulted first, always**, even on the path that never uses
//!   its answer.
//! - **A work week starting on or before the dataset start date short-circuits**
//!   to the DAO value, because there is no window of held data to walk.
//!
//! # Which DAO method, and which anchor date
//!
//! `include_configured_earning_types` also picks the query:
//! `getPriorConsecutiveDaysWorkedWithCacheIncludingEarnings` when set,
//! `getPriorConsecutiveDayWorkedWithCache` when not. Those count back from
//! **different dates** — the calculation start date and the dataset start date
//! respectively, ten days apart. See
//! [`EmployeeShiftConsecutiveDaysPort`](crate::rules::ports::EmployeeShiftConsecutiveDaysPort).
//!
//! # The modifier is passed in, not derived
//!
//! Java takes `consecDaysModifier` as an argument where
//! `ConsecutiveDaysCalculator` computes it from the limit and the max. Callers
//! read it off the
//! [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator) they are
//! about to seed, so the two always agree — but the signature does not enforce
//! that.

use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::time_card::TimeCard;
use crate::rules::ports::EmployeeShiftConsecutiveDaysPort;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashSet;

/// Consecutive days worked before `work_week` began.
/// `calculatePriorConsecutiveDays`.
pub fn calculate_prior_consecutive_days(
    time_card: &dyn TimeCard,
    consecutive_days: &dyn EmployeeShiftConsecutiveDaysPort,
    work_week: &DateRange,
    configured_earning_type_ids: &[i32],
    consec_days_modifier: i32,
    include_configured_earning_types: bool,
) -> f64 {
    // Java fetches this before the branch, so it is fetched even when the
    // short-circuit below makes the loop unnecessary.
    let stat_value = f64::from(if include_configured_earning_types {
        consecutive_days.prior_consecutive_days_worked_including_earnings(
            time_card,
            configured_earning_type_ids,
        )
    } else {
        consecutive_days.prior_consecutive_days_worked(time_card)
    });
    let modifier = f64::from(consec_days_modifier);

    if work_week.start_date() <= time_card.dataset_start_date() {
        return stat_value % modifier;
    }

    let period = DateRange::new(
        time_card.dataset_start_date(),
        work_week.start_date().minus_days(1),
    );
    let distribution_dates = extract_distribution_days(time_card, &period);
    let earning_dates = extract_earning_dates(time_card, configured_earning_type_ids, &period);

    let mut count = 0.0;
    for date in period.dates().into_iter().rev() {
        if date_is_included_in_count(
            date,
            &distribution_dates,
            &earning_dates,
            include_configured_earning_types,
        ) {
            count += 1.0;
        } else {
            return count % modifier;
        }
    }

    (count + stat_value) % modifier
}

/// `dateIsIncludedInCount`.
fn date_is_included_in_count(
    date: LocalDate,
    distribution_dates: &HashSet<LocalDate>,
    earning_dates: &HashSet<LocalDate>,
    include_configured_earning_types: bool,
) -> bool {
    distribution_dates.contains(&date)
        || (include_configured_earning_types && earning_dates.contains(&date))
}

/// `extractDistributionDays` — every date carrying hours, with no filtering of
/// the shift that carried them.
fn extract_distribution_days(time_card: &dyn TimeCard, period: &DateRange) -> HashSet<LocalDate> {
    time_card
        .shifts_with_distributions_for_period(period)
        .into_iter()
        .flat_map(|shift| shift.hours_distributions())
        .map(HoursDistribution::date)
        .collect()
}

/// `extractEarningDates` — dates carrying an earning of a configured type.
///
/// Note it scans `getEarnings()` and filters on the period itself rather than
/// calling `getEarningsForPeriod`; same answer.
fn extract_earning_dates(
    time_card: &dyn TimeCard,
    configured_earning_type_ids: &[i32],
    period: &DateRange,
) -> HashSet<LocalDate> {
    time_card
        .earnings()
        .iter()
        .filter(|earning| configured_earning_type_ids.contains(&earning.earning_type_id()))
        .filter(|earning| period.contains_date(earning.earning_date()))
        .map(|earning| earning.earning_date())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::time_card::TimeCardData;
    use std::cell::Cell;

    const JOB: i32 = 200;
    const REGULAR: i32 = 1;
    const HOLIDAY_TYPE: i32 = 7;
    /// California's limit of 7 and max of 7.
    const MODIFIER: i32 = 13;

    /// The DAO, recording which method was asked.
    struct PriorDays {
        shifts_only: i32,
        including_earnings: i32,
        asked: Cell<&'static str>,
    }

    impl PriorDays {
        fn new(shifts_only: i32, including_earnings: i32) -> Self {
            Self {
                shifts_only,
                including_earnings,
                asked: Cell::new("nothing"),
            }
        }
    }

    impl EmployeeShiftConsecutiveDaysPort for PriorDays {
        fn prior_consecutive_days_worked(&self, _time_card: &dyn TimeCard) -> i32 {
            self.asked.set("shifts only");
            self.shifts_only
        }
        fn prior_consecutive_days_worked_including_earnings(
            &self,
            _time_card: &dyn TimeCard,
            _earning_type_ids: &[i32],
        ) -> i32 {
            self.asked.set("including earnings");
            self.including_earnings
        }
    }

    fn shift(day: u32, date_of_hours: u32) -> EmployeeShift {
        EmployeeShift::new(
            day as i32,
            100,
            JOB,
            LocalDate::of(2010, 1, day.try_into().unwrap()),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_hours_distributions(vec![HoursDistribution::new(
            11,
            LocalDate::of(2010, 1, date_of_hours.try_into().unwrap()),
            Some(REGULAR),
            8.0,
            10.0,
        )])
    }

    fn worked(days: &[u32]) -> Vec<EmployeeShift> {
        days.iter().map(|&day| shift(day, day)).collect()
    }

    fn earning(id: i32, day: u32, type_id: i32) -> EmployeeEarning {
        EmployeeEarning::new(
            id,
            100,
            JOB,
            type_id,
            LocalDate::of(2010, 1, day.try_into().unwrap()),
            8.0,
            10.0,
            EarningSource::Manual,
        )
    }

    /// A card whose dataset starts on the 1st.
    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_dataset_start_date(LocalDate::of(2010, 1, 1))
    }

    /// The week beginning the 11th.
    fn week() -> DateRange {
        DateRange::new(LocalDate::of(2010, 1, 11), LocalDate::of(2010, 1, 17))
    }

    fn count(card: &TimeCardData, dao: &PriorDays) -> f64 {
        calculate_prior_consecutive_days(card, dao, &week(), &[HOLIDAY_TYPE], MODIFIER, false)
    }

    fn count_with_earnings(card: &TimeCardData, dao: &PriorDays) -> f64 {
        calculate_prior_consecutive_days(card, dao, &week(), &[HOLIDAY_TYPE], MODIFIER, true)
    }

    #[test]
    fn a_gap_ends_the_run() {
        let card = card(worked(&[1, 2, 3, 4, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays::new(99, 99)), 3.0);
    }

    #[test]
    fn an_unbroken_window_adds_what_the_dao_knows() {
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays::new(0, 0)), 10.0);
        assert_eq!(count(&card, &PriorDays::new(2, 0)), 12.0);
    }

    #[test]
    fn the_count_wraps_at_the_modifier() {
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays::new(4, 0)), 1.0);
        assert_eq!(count(&card, &PriorDays::new(3, 0)), 0.0);
    }

    #[test]
    fn a_shift_with_errors_still_counts() {
        // Where ConsecutiveDaysCalculator would drop it.
        let mut shifts = worked(&[9, 10]);
        shifts[1] = shifts[1]
            .clone()
            .with_errors(vec![ShiftErrorType::MissingOut]);

        assert_eq!(count(&card(shifts), &PriorDays::new(99, 99)), 2.0);
    }

    #[test]
    fn an_earning_does_not_count_unless_the_caller_asks() {
        // Worked the 9th; the 10th has only a holiday earning.
        let card = card(worked(&[9])).with_earnings(vec![earning(1, 10, HOLIDAY_TYPE)]);

        assert_eq!(
            count(&card, &PriorDays::new(99, 99)),
            0.0,
            "the 10th breaks the run"
        );
        assert_eq!(
            count_with_earnings(&card, &PriorDays::new(99, 99)),
            2.0,
            "unless earnings count, and then the 9th continues it"
        );
    }

    #[test]
    fn only_configured_earning_types_count() {
        let card = card(worked(&[9])).with_earnings(vec![earning(1, 10, 999)]);

        assert_eq!(count_with_earnings(&card, &PriorDays::new(99, 99)), 0.0);
    }

    #[test]
    fn the_earnings_switch_also_picks_the_dao_method() {
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));

        let dao = PriorDays::new(2, 5);
        assert_eq!(count(&card, &dao), 12.0);
        assert_eq!(dao.asked.get(), "shifts only");

        let dao = PriorDays::new(2, 5);
        assert_eq!(count_with_earnings(&card, &dao), 2.0, "(10 + 5) % 13");
        assert_eq!(dao.asked.get(), "including earnings");
    }

    #[test]
    fn a_week_starting_on_the_dataset_start_date_short_circuits() {
        let card = card(worked(&[1, 2, 3]));
        let week = DateRange::new(LocalDate::of(2010, 1, 1), LocalDate::of(2010, 1, 7));

        assert_eq!(
            calculate_prior_consecutive_days(
                &card,
                &PriorDays::new(5, 0),
                &week,
                &[HOLIDAY_TYPE],
                MODIFIER,
                false
            ),
            5.0,
            "there is no window of held data to walk"
        );
    }

    #[test]
    fn the_dao_is_consulted_even_when_the_run_breaks() {
        // Java fetches the stat value before deciding it does not need it.
        let card = card(worked(&[9, 10]));
        let dao = PriorDays::new(99, 99);

        assert_eq!(count(&card, &dao), 2.0);
        assert_eq!(dao.asked.get(), "shifts only");
    }

    #[test]
    fn distribution_dates_decide_the_run_not_shift_dates() {
        // A shift dated the 11th distributing its hours into the 10th.
        let card = card(vec![shift(11, 10)]);

        assert_eq!(count(&card, &PriorDays::new(99, 99)), 1.0);
    }

    #[test]
    fn a_run_reaching_the_dataset_start_uses_the_whole_window() {
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));
        let dao = PriorDays::new(0, 0);

        assert_eq!(count(&card, &dao), 10.0, "the 1st through the 10th");
    }
}

/// `priordayscalculator/PriorDaysCalculatorTest.groovy`, transcribed.
///
/// All three `where:` tables, fourteen rows. As in the sibling calculator's
/// parity module, the Groovy drives the dataset start date through a mocked
/// `PayGroup` whose current pay period *is* the work week; that makes the
/// dataset start date the work week's start minus ten days, which is set on the
/// card directly here.
///
/// One thing to know about the first table: the Groovy passes
/// `earnings.collect { it.id }` as the configured **earning type** ids, and the
/// fixture's single earning has id 1 and an earning type whose id is also 1. So
/// the row that looks like it filters on the earning's own id is filtering on
/// its type, and matches by coincidence. Transcribed as the type id, which is
/// what the production code reads.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_earning::EmployeeEarning;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;
    use rstest::rstest;

    const JOB: i32 = 1;
    const EARNING_TYPE: i32 = 1;
    const REGULAR: i32 = 1;

    /// `static def shiftDate = new LocalDate(2010, 11, 20)`.
    fn shift_date() -> LocalDate {
        LocalDate::of(2010, 11, 20)
    }

    /// `new LegacyDatePeriod(new LocalDate(2010, 11, 22), new LocalDate(2010, 11, 28))`.
    fn work_week() -> DateRange {
        DateRange::new(LocalDate::of(2010, 11, 22), LocalDate::of(2010, 11, 28))
    }

    struct PriorDays(i32);

    impl EmployeeShiftConsecutiveDaysPort for PriorDays {
        fn prior_consecutive_days_worked(&self, _time_card: &dyn TimeCard) -> i32 {
            self.0
        }
        fn prior_consecutive_days_worked_including_earnings(
            &self,
            _time_card: &dyn TimeCard,
            _earning_type_ids: &[i32],
        ) -> i32 {
            self.0
        }
    }

    /// One eight-hour regular shift, offset from `shiftDate`. The Groovy names
    /// these `shift10` (eight days before) through `shift2` (one day after).
    fn shift(offset_days: i64) -> EmployeeShift {
        let date = shift_date().plus_days(offset_days);

        EmployeeShift::new(1, 1, JOB, date, ShiftType::Actual, Vec::new()).with_hours_distributions(
            vec![HoursDistribution::new(11, date, Some(REGULAR), 8.0, 0.0)],
        )
    }

    /// `static def earning = ... earningDate: new LocalDate(2010, 11, 21)`.
    fn earning() -> EmployeeEarning {
        EmployeeEarning::new(
            1,
            1,
            JOB,
            EARNING_TYPE,
            LocalDate::of(2010, 11, 21),
            2.0,
            0.0,
            EarningSource::Manual,
        )
    }

    fn card(shifts: Vec<EmployeeShift>, earnings: Vec<EmployeeEarning>) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_earnings(earnings)
            // The pay period is the work week, so the dataset starts ten days
            // before it: 2010-11-12.
            .with_dataset_start_date(work_week().start_date().minus_days(10))
    }

    /// `consecutive days is correct given shifts and earnings when
    /// includeEarningPayMap is true`.
    #[rstest]
    #[case(false, false, 0.0)]
    #[case(true, false, 0.0)]
    #[case(false, true, 1.0)]
    #[case(true, true, 2.0)]
    fn consecutive_days_including_earnings(
        #[case] with_shift: bool,
        #[case] with_earning: bool,
        #[case] expected: f64,
    ) {
        let shifts = if with_shift {
            vec![shift(0)]
        } else {
            Vec::new()
        };
        let earnings = if with_earning {
            vec![earning()]
        } else {
            Vec::new()
        };
        let card = card(shifts, earnings);

        assert_eq!(
            calculate_prior_consecutive_days(
                &card,
                &PriorDays(0),
                &work_week(),
                &[EARNING_TYPE],
                1000,
                true
            ),
            expected
        );
    }

    /// `consecutive days is correct given shifts and earnings when
    /// includeEarningPayMap is false` — the same fixtures, earnings ignored.
    #[rstest]
    #[case(&[], false, 0.0)]
    #[case(&[0], false, 0.0)]
    #[case(&[], true, 0.0)]
    #[case(&[0], true, 0.0)]
    #[case(&[0, 1], false, 2.0)]
    #[case(&[0, 1], true, 2.0)]
    #[case(&[1], false, 1.0)]
    fn consecutive_days_excluding_earnings(
        #[case] shift_offsets: &[i64],
        #[case] with_earning: bool,
        #[case] expected: f64,
    ) {
        let shifts = shift_offsets.iter().map(|&offset| shift(offset)).collect();
        let earnings = if with_earning {
            vec![earning()]
        } else {
            Vec::new()
        };
        let card = card(shifts, earnings);

        assert_eq!(
            calculate_prior_consecutive_days(
                &card,
                &PriorDays(0),
                &work_week(),
                &[EARNING_TYPE],
                1000,
                false
            ),
            expected
        );
    }

    /// `consecutive days prior and consecutive days modifier correctly affects
    /// the consecutive days worked` — the ten-day window, unbroken, so the
    /// DAO's value is added and the modifier of 20 applies.
    #[rstest]
    #[case(0, 10.0)]
    #[case(4, 14.0)]
    #[case(12, 2.0)]
    fn prior_days_and_the_modifier_affect_the_count(
        #[case] consec_days_prior_to_time_card: i32,
        #[case] expected: f64,
    ) {
        // shift10 through shift2: 2010-11-12 to 2010-11-21, the whole window.
        let card = card((-8..=1).map(shift).collect(), Vec::new());

        assert_eq!(
            calculate_prior_consecutive_days(
                &card,
                &PriorDays(consec_days_prior_to_time_card),
                &work_week(),
                &[],
                20,
                false
            ),
            expected
        );
    }
}
