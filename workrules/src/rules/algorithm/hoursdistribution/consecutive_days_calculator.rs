//! Port of `ConsecutiveDaysCalculator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/ConsecutiveDaysCalculator.java`.
//!
//! How many consecutive days the employee had already worked before the day a
//! rule is about to calculate — the number
//! [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator) is seeded
//! with, which decides whether the daily limits apply. Only
//! `MinHrsForFullTimeOTRuleImpl` uses this one.
//!
//! It walks backwards from the day before `start_date` to the dataset start
//! date. The first day with no distribution ends the run and the answer is the
//! count so far. If the whole window is worked, the run continues into data the
//! time card does not hold, and the DAO
//! ([`EmployeeShiftConsecutiveDaysPort`]) is asked how far back it goes.
//! Either way the answer is taken modulo `consecutiveDaysLimit +
//! maxConsecutiveDays - 1`, the same modifier
//! [`WeeklyAccumulator`](super::weekly_accumulator::WeeklyAccumulator) wraps on.
//!
//! # It is not the same calculation as [`PriorDaysCalculator`]
//!
//! [`PriorDaysCalculator`](super::prior_days_calculator::PriorDaysCalculator)
//! answers the same question for five other rules, and the two disagree:
//!
//! | | this | `PriorDaysCalculator` |
//! |---|---|---|
//! | shifts with errors | excluded, unless run from scheduling | counted |
//! | salaried-exempt job statuses | excluded | counted |
//! | earnings | never counted | counted when the rule asks |
//! | week starting on the dataset start date | falls through the loop to the DAO | short-circuits to the DAO value |
//! | the DAO call | only when the whole window is worked | always, before anything else |
//!
//! Neither is a simplification of the other. Port a rule against the one it
//! actually calls.
//!
//! # A shift with errors still counts when scheduling
//!
//! `getErrors().isEmpty() || isRunFromScheduling` — under
//! [`EmployeeCalculationMode::AutoSchedule`] or
//! [`EmployeeCalculationMode::EditSchedule`] the error filter is switched off
//! entirely, because a schedule being built has not been validated yet. Note it
//! reads the **persisted** error set, the one Wave 0's correction separated from
//! the derived one.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::time_card::TimeCard;
use crate::rules::ports::EmployeeShiftConsecutiveDaysPort;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashSet;

/// Consecutive days worked before a date. `ConsecutiveDaysCalculator`.
///
/// `calcConsecDaysPriorToStartDate`. Java returns `double` although the count
/// is an `int` and the DAO's value is an `int` widened to `double`; the modulo
/// is taken in floating point. Reproduced, because the caller feeds the result
/// straight into arithmetic that expects it.
pub fn calc_consec_days_prior_to_start_date(
    time_card: &dyn TimeCard,
    consecutive_days: &dyn EmployeeShiftConsecutiveDaysPort,
    start_date: LocalDate,
    consecutive_days_limit: i32,
    max_consecutive_days: i32,
) -> f64 {
    let days_before_count_resets = f64::from(consecutive_days_limit + max_consecutive_days - 1);
    let period_start = time_card.dataset_start_date();
    let period_end = start_date.minus_days(1);
    let dates_with_distributions =
        dates_with_hours_distributions(time_card, &DateRange::new(period_start, period_end));

    let mut count = 0.0;
    let mut date = period_end;
    // An inverted window — the dataset starting on or after `start_date` —
    // never enters the loop, exactly as Java's `!date.isBefore(startDate)`
    // guard does not, and falls through to the DAO.
    while date >= period_start {
        if !dates_with_distributions.contains(&date) {
            return count % days_before_count_resets;
        }
        count += 1.0;
        date = date.minus_days(1);
    }

    let stat_value = f64::from(consecutive_days.prior_consecutive_days_worked(time_card));
    (count + stat_value) % days_before_count_resets
}

/// `getDatesWithHoursDistributions` — every date carrying hours from a shift
/// that counts.
fn dates_with_hours_distributions(
    time_card: &dyn TimeCard,
    period: &DateRange,
) -> HashSet<LocalDate> {
    let run_from_scheduling = time_card.is_run_from_scheduling();

    time_card
        .shifts_with_distributions_for_period(period)
        .into_iter()
        .filter(|shift| shift.errors().is_empty() || run_from_scheduling)
        .filter(|shift| time_card.shift_is_not_salaried_exempt(shift))
        .flat_map(EmployeeShift::hours_distributions)
        .map(|distribution| distribution.date())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;

    const HOURLY_JOB: i32 = 200;
    const EXEMPT_JOB: i32 = 300;
    const REGULAR: i32 = 1;

    /// The DAO, answering a fixed number of days already running.
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

    fn shift(id: i32, job_id: i32, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(id, 100, job_id, date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![HoursDistribution::new(
                11,
                date,
                Some(REGULAR),
                8.0,
                10.0,
            )])
    }

    /// Shifts on each of `days` in January 2010, all hourly.
    fn worked(days: &[u32]) -> Vec<EmployeeShift> {
        days.iter()
            .map(|&day| {
                shift(
                    day as i32,
                    HOURLY_JOB,
                    LocalDate::of(2010, 1, day.try_into().unwrap()),
                )
            })
            .collect()
    }

    /// A card whose dataset starts on the 1st.
    fn card(shifts: Vec<EmployeeShift>) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_dataset_start_date(LocalDate::of(2010, 1, 1))
    }

    /// Ask about the 11th, with California's limit of 7 and max of 7.
    fn count(card: &TimeCardData, dao: &PriorDays) -> f64 {
        calc_consec_days_prior_to_start_date(card, dao, LocalDate::of(2010, 1, 11), 7, 7)
    }

    #[test]
    fn a_gap_ends_the_run() {
        // Worked the 1st to the 4th and the 8th to the 10th: the 7th is the gap.
        let card = card(worked(&[1, 2, 3, 4, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays(99)), 3.0);
    }

    #[test]
    fn the_day_before_the_start_date_is_where_counting_begins() {
        // Nothing on the 10th, so the run is empty however much came before.
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9]));

        assert_eq!(count(&card, &PriorDays(99)), 0.0);
    }

    #[test]
    fn an_unbroken_window_reaches_for_the_dao() {
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays(0)), 10.0, "the ten days held");
        assert_eq!(
            count(&card, &PriorDays(2)),
            12.0,
            "plus what the DAO knows about"
        );
    }

    #[test]
    fn the_count_wraps_at_the_modifier() {
        // Ten days held plus four before them is 14, and the modifier is 13.
        let card = card(worked(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));

        assert_eq!(count(&card, &PriorDays(4)), 1.0);
        assert_eq!(count(&card, &PriorDays(3)), 0.0, "exactly the modifier");
    }

    #[test]
    fn the_dao_is_not_consulted_when_the_run_breaks() {
        let card = card(worked(&[9, 10]));

        assert_eq!(
            count(&card, &PriorDays(99)),
            2.0,
            "the gap on the 8th settles it"
        );
    }

    #[test]
    fn a_shift_with_errors_does_not_count() {
        let mut shifts = worked(&[9, 10]);
        shifts[1] = shifts[1]
            .clone()
            .with_errors(vec![ShiftErrorType::MissingOut]);

        assert_eq!(count(&card(shifts), &PriorDays(99)), 0.0);
    }

    #[test]
    fn a_shift_with_errors_counts_when_run_from_scheduling() {
        let mut shifts = worked(&[9, 10]);
        shifts[1] = shifts[1]
            .clone()
            .with_errors(vec![ShiftErrorType::MissingOut]);
        let card = card(shifts).with_calculation_mode(EmployeeCalculationMode::EditSchedule);

        assert_eq!(count(&card, &PriorDays(99)), 2.0);
    }

    #[test]
    fn a_salaried_exempt_shift_does_not_count() {
        let card = card(vec![
            shift(9, HOURLY_JOB, LocalDate::of(2010, 1, 9)),
            shift(10, EXEMPT_JOB, LocalDate::of(2010, 1, 10)),
        ]);

        assert_eq!(count(&card, &PriorDays(99)), 0.0);
    }

    #[test]
    fn a_shift_against_a_job_with_no_status_does_not_count() {
        // Java throws on the null job status; dropping the shift is the only
        // reading under which it is not known to be non-exempt.
        let card = card(vec![shift(10, 999, LocalDate::of(2010, 1, 10))]);

        assert_eq!(count(&card, &PriorDays(99)), 0.0);
    }

    #[test]
    fn a_window_that_inverts_falls_through_to_the_dao() {
        // The dataset starts after the day before the start date, so there is
        // nothing to walk.
        let card = card(Vec::new()).with_dataset_start_date(LocalDate::of(2010, 2, 1));

        assert_eq!(
            calc_consec_days_prior_to_start_date(
                &card,
                &PriorDays(5),
                LocalDate::of(2010, 1, 11),
                7,
                7
            ),
            5.0
        );
    }

    #[test]
    fn a_shift_dated_outside_the_window_still_counts_by_its_distributions() {
        // The filter is on distribution dates, so a shift dated the 11th that
        // distributed hours into the 10th counts for the 10th.
        let overnight = EmployeeShift::new(
            1,
            100,
            HOURLY_JOB,
            LocalDate::of(2010, 1, 11),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_hours_distributions(vec![HoursDistribution::new(
            11,
            LocalDate::of(2010, 1, 10),
            Some(REGULAR),
            4.0,
            10.0,
        )]);

        assert_eq!(count(&card(vec![overnight]), &PriorDays(99)), 1.0);
    }
}

/// `ConsecutiveDaysCalculatorTest.groovy`, transcribed.
///
/// All eight cases. The Groovy drives the dataset start date through a mocked
/// `PayGroup` — `currentPayPeriod().getStartDate()` minus ten days — which is
/// the derivation divergence 24 does not port; each case sets the resulting
/// date on the card directly and names the pay period start it came from.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::time_card::TimeCardData;

    const JOB: i32 = 1;

    /// `LocalDate startDate = ld(2015, 1, 12)`.
    fn start_date() -> LocalDate {
        LocalDate::of(2015, 1, 12)
    }

    fn minus(days: i64) -> LocalDate {
        start_date().minus_days(days)
    }

    fn plus(days: i64) -> LocalDate {
        start_date().plus_days(days)
    }

    struct NoPriorDays;

    impl EmployeeShiftConsecutiveDaysPort for NoPriorDays {
        fn prior_consecutive_days_worked(&self, _time_card: &dyn TimeCard) -> i32 {
            0
        }
        fn prior_consecutive_days_worked_including_earnings(
            &self,
            _time_card: &dyn TimeCard,
            _earning_type_ids: &[i32],
        ) -> i32 {
            0
        }
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

    /// The single hourly job status spanning ten years either side.
    fn employee() -> Employee {
        Employee::new(
            1,
            11,
            "",
            vec![EmployeeJobStatus::new(
                1,
                1,
                JOB,
                LocalDate::of(2005, 1, 12),
                LocalDate::of(2025, 1, 12),
                EmployeePayType::Hourly,
                10.0,
                true,
            )],
        )
    }

    /// `empShiftWithPunches` — one distribution, on the shift date.
    ///
    /// The punches the Groovy attaches are never read by this calculator; only
    /// the distribution dates and the job are.
    fn shift(shift_date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![HoursDistribution::new(11, shift_date, None, 8.0, 0.0)])
    }

    /// `empShiftWithPunchesSpanningDayCut` — a 20:00 start, so its hours land
    /// on the shift date **and** the next day.
    fn spanning_shift(shift_date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(1, 1, JOB, shift_date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(vec![
                HoursDistribution::new(11, shift_date, None, 4.0, 0.0),
                HoursDistribution::new(11, shift_date.plus_days(1), None, 4.0, 0.0),
            ])
    }

    /// A card whose pay period starts on `pay_period_start`, so its dataset
    /// start date is ten days before that.
    fn card(shifts: Vec<EmployeeShift>, pay_period_start: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_employee(employee())
            .with_shifts(shifts)
            .with_dataset_start_date(pay_period_start.minus_days(10))
    }

    #[test]
    fn should_return_zero_if_no_data() {
        let card = card(Vec::new(), plus(3));

        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 6, 2),
            0.0
        );
    }

    #[test]
    fn should_return_consecutive_days_back() {
        let card = card(
            vec![shift(minus(1)), shift(minus(2)), shift(minus(3))],
            plus(3),
        );

        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 6, 2),
            3.0
        );
    }

    #[test]
    fn should_not_count_shifts_on_or_after_the_start_date() {
        let card = card(
            vec![
                shift(start_date()),
                shift(minus(1)),
                shift(minus(2)),
                shift(minus(3)),
            ],
            plus(3),
        );

        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 6, 2),
            3.0
        );
    }

    #[test]
    fn should_stop_counting_back_if_there_is_a_gap() {
        let card = card(
            vec![
                shift(minus(1)),
                shift(minus(2)),
                shift(minus(3)),
                shift(minus(5)),
            ],
            plus(3),
        );

        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 6, 2),
            3.0
        );
    }

    #[test]
    fn should_adjust_the_count_of_consecutive_days_based_on_the_passed_in_limits() {
        let card = card(
            vec![
                shift(minus(1)),
                shift(minus(2)),
                shift(minus(3)),
                shift(minus(4)),
                shift(minus(5)),
            ],
            plus(3),
        );

        // Five days run, and a modifier of 2 + 3 - 1 = 4.
        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 2, 3),
            1.0
        );
    }

    #[test]
    fn should_adjust_the_count_taking_into_account_days_prior_to_the_dataset_start_date() {
        let card = card((1..=7).map(|day| shift(minus(day))).collect(), plus(3));

        // The whole seven-day window is worked, so the DAO's two days are
        // added: (7 + 2) % (5 + 2 - 1) = 3.
        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &PriorDays(2), start_date(), 5, 2),
            3.0
        );
    }

    #[test]
    fn should_account_for_the_start_date_being_before_the_dataset_start_date() {
        // The pay period starts twelve days out, so the dataset starts two days
        // *after* the start date and the window inverts.
        let card = card(vec![shift(plus(2)), shift(plus(4))], plus(12));

        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &PriorDays(10), start_date(), 5, 2),
            4.0,
            "nothing to walk, so 10 % 6"
        );
    }

    #[test]
    fn should_count_spanning_shifts_as_two_consecutive_days_worked() {
        let card = card(
            vec![spanning_shift(minus(2)), spanning_shift(minus(4))],
            start_date(),
        );

        // Two shifts, four distribution dates: the 8th through the 11th.
        assert_eq!(
            calc_consec_days_prior_to_start_date(&card, &NoPriorDays, start_date(), 10, 5),
            4.0
        );
    }
}
