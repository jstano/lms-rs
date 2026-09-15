use crate::workcontent::domain::meal_break::MealBreak;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::non_meal_break::NonMealBreak;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::standard_type::StandardType;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

pub struct PlannerSettings {
    pub standard_type: StandardType,
    /// Scheduling granularity in minutes; durations are rounded to it.
    pub period_length: u32,
    pub min_shift_length: f64,
    pub max_shift_length: f64,
    /// Fraction of a period below which a remainder is discarded when no full
    /// shift has been planned for the day.
    pub rounding_threshold_below_one: f64,
    /// The same, once at least one full shift has been planned.
    pub rounding_threshold_above_one: f64,
    pub meal_break: Option<MealBreak>,
    pub non_meal_break: Option<NonMealBreak>,
    /// The part of the year these settings apply to; the year itself is
    /// ignored and re-based onto each planned date.
    pub effective_dates: DateRange,
    pub generate_long_shifts: bool,
    pub limit_shift_to_max_shift: bool,
    pub truncate_max_coverage: bool,
    pub non_flowed_distribution_method: NonFlowedDistributionMethod,
}

impl PlannerSettings {
    pub fn default() -> PlannerSettings {
        Self {
            standard_type: StandardType::NONE,
            period_length: 30,
            min_shift_length: 4.0,
            max_shift_length: 8.0,
            rounding_threshold_below_one: 1.0,
            rounding_threshold_above_one: 1.0,
            meal_break: None,
            non_meal_break: None,
            effective_dates: DateRange::new(
                LocalDate::new(2021, 1, 1),
                LocalDate::new(2021, 12, 31),
            ),
            generate_long_shifts: false,
            limit_shift_to_max_shift: false,
            truncate_max_coverage: false,
            non_flowed_distribution_method: NonFlowedDistributionMethod::VARYING,
        }
    }

    /// The dates in the planning window these settings are effective on.
    ///
    /// The effective range is a recurring part of the year, so it is re-based
    /// onto the year of each candidate date before being tested.
    pub fn dates(&self, planner_model: &PlannerModel) -> Vec<LocalDate> {
        planner_model
            .dates()
            .iter()
            .filter(|date| self.is_effective(*date))
            .collect()
    }

    pub fn is_effective(&self, date: LocalDate) -> bool {
        let effective_dates = DateRange::new(
            with_year(self.effective_dates.start_date(), date.year()),
            with_year(self.effective_dates.end_date(), date.year()),
        );

        effective_dates.contains_date(date)
    }
}

/// Re-base a date onto `year`, pulling 29 February back to the 28th in a
/// non-leap year rather than overflowing into March.
fn with_year(date: LocalDate, year: i32) -> LocalDate {
    let candidate = LocalDate::new(year, 1, 1);

    if date.month_value() == 2 && date.day_of_month() == 29 && !candidate.is_leap_year() {
        return LocalDate::new(year, 2, 28);
    }

    date.with_year(year)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use rstest::rstest;
    use std::collections::HashMap;

    fn model(dates: DateRange) -> PlannerModel {
        PlannerModel::new(
            dates,
            PlannerMode::Standard,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        )
    }

    fn settings(effective_dates: DateRange) -> PlannerSettings {
        let mut settings = PlannerSettings::default();
        settings.effective_dates = effective_dates;
        settings
    }

    #[test]
    fn every_date_in_a_full_year_range_is_effective() {
        let settings = settings(DateRange::new(
            LocalDate::new(2021, 1, 1),
            LocalDate::new(2021, 12, 31),
        ));
        let planner_model = model(DateRange::new(
            LocalDate::new(2025, 10, 1),
            LocalDate::new(2025, 10, 31),
        ));

        assert_eq!(settings.dates(&planner_model).len(), 31);
    }

    #[test]
    fn dates_outside_a_seasonal_range_are_dropped() {
        // Effective June through August, planned across July and September.
        let settings = settings(DateRange::new(
            LocalDate::new(2021, 6, 1),
            LocalDate::new(2021, 8, 31),
        ));
        let planner_model = model(DateRange::new(
            LocalDate::new(2025, 7, 30),
            LocalDate::new(2025, 9, 2),
        ));

        let dates = settings.dates(&planner_model);

        assert_eq!(dates.len(), 33); // 2 in July + 31 in August
        assert_eq!(dates[0], LocalDate::new(2025, 7, 30));
        assert_eq!(dates[dates.len() - 1], LocalDate::new(2025, 8, 31));
    }

    #[rstest]
    // A leap-year 29 February re-bases onto the 28th in a non-leap year...
    #[case(2025, 2, 28)]
    // ...and stays on the 29th in a leap year.
    #[case(2028, 2, 29)]
    fn a_february_29_boundary_is_corrected_for_the_target_year(
        #[case] year: i32,
        #[case] month: i32,
        #[case] day: i32,
    ) {
        assert_eq!(
            with_year(LocalDate::new(2024, 2, 29), year),
            LocalDate::new(year, month, day)
        );
    }

    #[test]
    fn an_effective_range_ending_on_february_29_still_covers_february_28() {
        let settings = settings(DateRange::new(
            LocalDate::new(2024, 1, 1),
            LocalDate::new(2024, 2, 29),
        ));

        assert!(settings.is_effective(LocalDate::new(2025, 2, 28)));
        assert!(!settings.is_effective(LocalDate::new(2025, 3, 1)));
    }
}
