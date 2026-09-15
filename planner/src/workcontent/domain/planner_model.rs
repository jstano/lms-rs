use crate::workcontent::common::dates;
use crate::workcontent::domain::business_driver::{BusinessDriver, BusinessDriverId};
use crate::workcontent::domain::business_driver_values::BusinessDriverValues;
use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::location::LocationId;
use crate::workcontent::domain::plan_type::PlanType;
use crate::workcontent::domain::planner_mode::PlannerMode;
use crate::workcontent::domain::standard_set::StandardSetId;
use date_range_rs::DateRange;
use joda_rs::{DayOfWeek, LocalDate};
use std::collections::HashMap;

pub struct PlannerModel {
    dates: DateRange,
    planner_mode: PlannerMode,
    location_id: LocationId,
    standard_set_id: StandardSetId,
    jobs: Vec<Job>,
    business_drivers: Vec<BusinessDriver>,
    business_driver_values: HashMap<BusinessDriverId, BusinessDriverValues>,
    /// How long a block of opening or closing work may run when the standard
    /// does not pin both ends itself. Zero means no ceiling is configured.
    max_duration_minutes_for_dynamic_work: i32,
    /// Whether flowed work rounds the way the engine used to, to a whole
    /// minute at the half, rather than to ten-thousandths.
    legacy_flow_pattern_rounding: bool,
    /// The day the property's week ends on, which decides what "this week"
    /// means to a weekly standard.
    week_ending_day: DayOfWeek,
}

impl PlannerModel {
    pub fn new(
        dates: DateRange,
        planner_mode: PlannerMode,
        location_id: LocationId,
        standard_set_id: StandardSetId,
        jobs: Vec<Job>,
        business_drivers: Vec<BusinessDriver>,
        business_driver_values: HashMap<BusinessDriverId, BusinessDriverValues>,
    ) -> Self {
        Self {
            dates,
            planner_mode,
            location_id,
            standard_set_id,
            jobs,
            business_drivers,
            business_driver_values,
            max_duration_minutes_for_dynamic_work: 0,
            legacy_flow_pattern_rounding: false,
            week_ending_day: DayOfWeek::Saturday,
        }
    }

    #[must_use]
    pub fn with_week_ending_day(mut self, week_ending_day: DayOfWeek) -> Self {
        self.week_ending_day = week_ending_day;
        self
    }

    pub fn week_ending_day(&self) -> DayOfWeek {
        self.week_ending_day
    }

    #[must_use]
    pub fn with_planner_mode(mut self, planner_mode: PlannerMode) -> Self {
        self.planner_mode = planner_mode;
        self
    }

    #[must_use]
    pub fn with_legacy_flow_pattern_rounding(mut self) -> Self {
        self.legacy_flow_pattern_rounding = true;
        self
    }

    pub fn legacy_flow_pattern_rounding(&self) -> bool {
        self.legacy_flow_pattern_rounding
    }

    /// Cap opening and closing work at `minutes` when the standard leaves one
    /// end of its window open.
    #[must_use]
    pub fn with_max_duration_minutes_for_dynamic_work(mut self, minutes: i32) -> Self {
        self.max_duration_minutes_for_dynamic_work = minutes;
        self
    }

    pub fn max_duration_minutes_for_dynamic_work(&self) -> i32 {
        self.max_duration_minutes_for_dynamic_work
    }

    pub fn dates(&self) -> DateRange {
        self.dates
    }

    pub fn planner_mode(&self) -> PlannerMode {
        self.planner_mode
    }

    pub fn location_id(&self) -> LocationId {
        self.location_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn jobs_iter(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter()
    }

    pub fn business_drivers(&self) -> &[BusinessDriver] {
        &self.business_drivers
    }

    /// The volume recorded for a business driver on a date, or zero when the
    /// driver has no values at all.
    pub fn business_driver_value(
        &self,
        business_driver_id: BusinessDriverId,
        date: LocalDate,
    ) -> i32 {
        self.business_driver_values
            .get(&business_driver_id)
            .map(|values| values.value_for_date(date))
            .unwrap_or(0)
    }

    /// The volume recorded for a business driver across the whole week `date`
    /// falls in.
    ///
    /// Weekly standards are stated per week rather than per day, so they read
    /// this instead of [`business_driver_value`](Self::business_driver_value).
    /// `week_ending_day` is the property's own week boundary; dates with no
    /// recorded volume contribute zero, so an unknown driver sums to zero
    /// rather than failing.
    pub fn business_driver_value_for_week(
        &self,
        business_driver_id: BusinessDriverId,
        date: LocalDate,
        week_ending_day: DayOfWeek,
    ) -> i32 {
        dates::week_containing(date, week_ending_day)
            .into_iter()
            .map(|day| self.business_driver_value(business_driver_id, day))
            .sum()
    }

    /// The plan type generated work is primarily written against.
    pub fn primary_plan_type(&self) -> Option<PlanType> {
        match self.planner_mode {
            PlannerMode::Projected => Some(PlanType::Forecast),
            PlannerMode::ReProject => Some(PlanType::Original),
            PlannerMode::Standard => Some(PlanType::Standard),
            // Java throws here; there is no primary type for a forecast-only run.
            PlannerMode::ProjectedForecastOnly => None,
        }
    }

    /// Every plan type generated work is written against. A projected run
    /// writes both the forecast and the original plan.
    pub fn plan_types(&self) -> Vec<PlanType> {
        match self.planner_mode {
            PlannerMode::Projected => vec![PlanType::Forecast, PlanType::Original],
            PlannerMode::ProjectedForecastOnly => vec![PlanType::Forecast],
            PlannerMode::ReProject => vec![PlanType::Original],
            PlannerMode::Standard => vec![PlanType::Standard],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn model(planner_mode: PlannerMode) -> PlannerModel {
        PlannerModel::new(
            DateRange::new(LocalDate::new(2025, 10, 1), LocalDate::new(2025, 10, 31)),
            planner_mode,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        )
    }

    #[rstest]
    #[case(PlannerMode::Projected, vec![PlanType::Forecast, PlanType::Original])]
    #[case(PlannerMode::ProjectedForecastOnly, vec![PlanType::Forecast])]
    #[case(PlannerMode::ReProject, vec![PlanType::Original])]
    #[case(PlannerMode::Standard, vec![PlanType::Standard])]
    fn plan_types_follow_the_planner_mode(
        #[case] planner_mode: PlannerMode,
        #[case] expected: Vec<PlanType>,
    ) {
        assert_eq!(model(planner_mode).plan_types(), expected);
    }

    #[rstest]
    #[case(PlannerMode::Projected, Some(PlanType::Forecast))]
    #[case(PlannerMode::ReProject, Some(PlanType::Original))]
    #[case(PlannerMode::Standard, Some(PlanType::Standard))]
    #[case(PlannerMode::ProjectedForecastOnly, None)]
    fn the_primary_plan_type_follows_the_planner_mode(
        #[case] planner_mode: PlannerMode,
        #[case] expected: Option<PlanType>,
    ) {
        assert_eq!(model(planner_mode).primary_plan_type(), expected);
    }

    #[test]
    fn a_weekly_business_driver_value_sums_the_whole_week() {
        let business_driver_id = BusinessDriverId::new();
        // 2025-10-06 is a Monday; with weeks ending Saturday its week runs
        // Sunday the 5th through Saturday the 11th. The 12th is the next week.
        let planner_model = PlannerModel::new(
            DateRange::new(LocalDate::new(2025, 10, 1), LocalDate::new(2025, 10, 31)),
            PlannerMode::Standard,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                business_driver_id,
                BusinessDriverValues::new(
                    business_driver_id,
                    HashMap::from([
                        (LocalDate::new(2025, 10, 5), 100),
                        (LocalDate::new(2025, 10, 6), 200),
                        (LocalDate::new(2025, 10, 11), 300),
                        (LocalDate::new(2025, 10, 12), 999),
                    ]),
                ),
            )]),
        );

        assert_eq!(
            planner_model.business_driver_value_for_week(
                business_driver_id,
                LocalDate::new(2025, 10, 6),
                DayOfWeek::Saturday
            ),
            600
        );
    }

    #[test]
    fn a_weekly_value_for_an_unknown_driver_is_zero() {
        assert_eq!(
            model(PlannerMode::Standard).business_driver_value_for_week(
                BusinessDriverId::new(),
                LocalDate::new(2025, 10, 6),
                DayOfWeek::Saturday
            ),
            0
        );
    }

    #[test]
    fn a_business_driver_value_is_read_for_its_date() {
        let business_driver_id = BusinessDriverId::new();
        let date = LocalDate::new(2025, 10, 6);
        let planner_model = PlannerModel::new(
            DateRange::new(LocalDate::new(2025, 10, 1), LocalDate::new(2025, 10, 31)),
            PlannerMode::Standard,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::from([(
                business_driver_id,
                BusinessDriverValues::new(business_driver_id, HashMap::from([(date, 400)])),
            )]),
        );

        assert_eq!(planner_model.business_driver_value(business_driver_id, date), 400);
        assert_eq!(
            planner_model.business_driver_value(business_driver_id, LocalDate::new(2025, 10, 7)),
            0
        );
    }

    #[test]
    fn an_unknown_business_driver_has_no_value() {
        assert_eq!(
            model(PlannerMode::Standard)
                .business_driver_value(BusinessDriverId::new(), LocalDate::new(2025, 10, 6)),
            0
        );
    }
}
