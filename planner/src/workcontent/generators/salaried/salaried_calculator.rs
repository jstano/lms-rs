use crate::workcontent::common::numbers;
use crate::workcontent::domain::salaried_standard::SalariedStandard;
use crate::workcontent::domain::salary_mode::SalaryMode;
use joda_rs::constants::{DAYS_PER_WEEK, MONTHS_PER_YEAR};
use joda_rs::{DayOfWeek, LocalDate};

/// Turns a salaried entitlement into the hours owed on one date.
pub trait SalariedCalculator {
    fn calculate_hours(&self, standard: &SalariedStandard, date: LocalDate) -> f64;
}

pub struct SalariedCalculatorImpl;

impl SalariedCalculatorImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl SalariedCalculator for SalariedCalculatorImpl {
    fn calculate_hours(&self, standard: &SalariedStandard, date: LocalDate) -> f64 {
        match standard.salary_mode() {
            SalaryMode::MONTHLY => monthly_hours(standard, date),
            SalaryMode::WEEKLY => weekly_hours(standard, date),
        }
    }
}

/// A monthly salary spreads the annual entitlement evenly across the days of
/// the month, so the daily figure varies with month length.
fn monthly_hours(standard: &SalariedStandard, date: LocalDate) -> f64 {
    let hours_per_year = standard.hours_per_year();

    numbers::round_hours(hours_per_year / MONTHS_PER_YEAR as f64 / date.length_of_month() as f64)
}

/// A weekly salary spreads the annual entitlement evenly across Monday to
/// Saturday, and puts whatever rounding leaves over onto Sunday.
fn weekly_hours(standard: &SalariedStandard, date: LocalDate) -> f64 {
    let number_not_sunday_days = (DAYS_PER_WEEK - 1) as f64;
    let days_per_week = DAYS_PER_WEEK as f64;
    // Java derives this as `daysPerYear / DAYS_PER_WEEK` with integer division,
    // which is 52 in a leap year (366 / 7) as well as a common one (365 / 7).
    let weeks_per_year = 52.0;

    let std_hours_per_week = standard.hours_per_week();
    let vacation_hours_per_year = standard.vacation_hours_per_year();

    let hours_per_year = (std_hours_per_week * weeks_per_year) - vacation_hours_per_year;
    let adj_hours_per_week = hours_per_year / weeks_per_year;

    let mon_sat_hours = numbers::round_hours(hours_per_year / weeks_per_year / days_per_week);
    let sun_hours =
        numbers::round_hours(adj_hours_per_week - (mon_sat_hours * number_not_sunday_days));

    if date.day_of_week() == DayOfWeek::Sunday {
        sun_hours
    } else {
        mon_sat_hours
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::job_shift::JobShiftId;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use rstest::rstest;

    fn monthly_standard(hours_per_year: Option<f64>) -> SalariedStandard {
        SalariedStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            SalaryMode::MONTHLY,
            None,
            None,
            hours_per_year,
        )
    }

    fn weekly_standard(
        hours_per_week: Option<f64>,
        vacation_hours_per_year: Option<f64>,
    ) -> SalariedStandard {
        SalariedStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            SalaryMode::WEEKLY,
            hours_per_week,
            vacation_hours_per_year,
            None,
        )
    }

    fn d(year: i32, month: i32, day: i32) -> LocalDate {
        LocalDate::new(year, month, day)
    }

    // Monthly mode divides by 12 and then by the length of the month, so the
    // same annual figure gives a different daily figure in a shorter month.
    #[rstest]
    // April has 30 days: 12 * 30 * 4 = 1440.
    #[case(1440.0, 2025, 4, 10, 4.0)]
    // January has 31 days: 12 * 31 * 4 = 1488.
    #[case(1488.0, 2025, 1, 10, 4.0)]
    // February 2025 has 28 days: 12 * 28 * 4 = 1344.
    #[case(1344.0, 2025, 2, 10, 4.0)]
    // February 2024 has 29 days: 12 * 29 * 4 = 1392.
    #[case(1392.0, 2024, 2, 10, 4.0)]
    fn monthly_mode_spreads_the_year_across_the_days_of_the_month(
        #[case] hours_per_year: f64,
        #[case] year: i32,
        #[case] month: i32,
        #[case] day: i32,
        #[case] expected: f64,
    ) {
        let calculator = SalariedCalculatorImpl::new();

        assert_eq!(
            calculator.calculate_hours(&monthly_standard(Some(hours_per_year)), d(year, month, day)),
            expected
        );
    }

    #[test]
    fn monthly_mode_gives_the_same_annual_figure_a_different_daily_figure_per_month() {
        let calculator = SalariedCalculatorImpl::new();
        let standard = monthly_standard(Some(2190.0));

        // 2190 / 12 / 31 = 5.89, 2190 / 12 / 30 = 6.08.
        assert_eq!(calculator.calculate_hours(&standard, d(2025, 1, 10)), 5.89);
        assert_eq!(calculator.calculate_hours(&standard, d(2025, 4, 10)), 6.08);
    }

    #[test]
    fn weekly_mode_spreads_the_week_evenly_when_it_divides_exactly() {
        // 42 hours a week over 7 days is 6.0 every day, Sunday included.
        let calculator = SalariedCalculatorImpl::new();
        let standard = weekly_standard(Some(42.0), None);

        // 2025-01-06 is a Monday, 2025-01-05 a Sunday.
        assert_eq!(calculator.calculate_hours(&standard, d(2025, 1, 6)), 6.0);
        assert_eq!(calculator.calculate_hours(&standard, d(2025, 1, 5)), 6.0);
    }

    #[test]
    fn weekly_mode_puts_the_rounding_remainder_on_sunday() {
        // 40 / 7 = 5.714285..., rounded to 5.71 for Mon-Sat; Sunday takes
        // 40 - (5.71 * 6) = 5.74 so the week still totals 40.
        let calculator = SalariedCalculatorImpl::new();
        let standard = weekly_standard(Some(40.0), None);

        let monday = calculator.calculate_hours(&standard, d(2025, 1, 6));
        let sunday = calculator.calculate_hours(&standard, d(2025, 1, 5));

        assert_eq!(monday, 5.71);
        assert_eq!(sunday, 5.74);
        assert!(((monday * 6.0 + sunday) - 40.0).abs() < 1e-9);
    }

    #[rstest]
    #[case(2025, 1, 6)] // Monday
    #[case(2025, 1, 7)] // Tuesday
    #[case(2025, 1, 8)] // Wednesday
    #[case(2025, 1, 9)] // Thursday
    #[case(2025, 1, 10)] // Friday
    #[case(2025, 1, 11)] // Saturday
    fn weekly_mode_gives_every_non_sunday_the_same_hours(
        #[case] year: i32,
        #[case] month: i32,
        #[case] day: i32,
    ) {
        let calculator = SalariedCalculatorImpl::new();

        assert_eq!(
            calculator.calculate_hours(&weekly_standard(Some(40.0), None), d(year, month, day)),
            5.71
        );
    }

    #[test]
    fn weekly_mode_deducts_vacation_hours_from_the_year() {
        // (40 * 52) - 520 = 1560 hours a year, so 30 hours a week over 7 days.
        let calculator = SalariedCalculatorImpl::new();
        let standard = weekly_standard(Some(40.0), Some(520.0));

        assert_eq!(calculator.calculate_hours(&standard, d(2025, 1, 6)), 4.29);
    }

    #[test]
    fn an_unset_entitlement_yields_no_hours() {
        let calculator = SalariedCalculatorImpl::new();

        assert_eq!(
            calculator.calculate_hours(&monthly_standard(None), d(2025, 1, 10)),
            0.0
        );
        assert_eq!(
            calculator.calculate_hours(&weekly_standard(None, None), d(2025, 1, 6)),
            0.0
        );
    }

    #[test]
    fn the_two_salary_modes_read_different_fields() {
        // A monthly standard carries only hours_per_year and a weekly standard
        // only hours_per_week; crossing them over would produce zero.
        let calculator = SalariedCalculatorImpl::new();

        assert!(calculator.calculate_hours(&monthly_standard(Some(1440.0)), d(2025, 4, 10)) > 0.0);
        assert!(calculator.calculate_hours(&weekly_standard(Some(42.0), None), d(2025, 1, 6)) > 0.0);
    }
}
