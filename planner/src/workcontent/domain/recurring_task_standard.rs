use crate::id_type;
use crate::workcontent::common::dates;
use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::job_shift::JobShiftId;
use crate::workcontent::domain::standard_set::StandardSetId;
use joda_rs::{DayOfWeek, LocalDate, Month};

id_type!(RecurringTaskStandardId, uuid_v4);

const LAST_DAY_OF_THE_MONTH: i32 = -1;
const DAYS_PER_WEEK: i64 = joda_rs::constants::DAYS_PER_WEEK;

/// How often a recurring task comes round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyType {
    Daily,
    Weekly,
    Monthly,
}

/// Which way a monthly recurrence picks its day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthlyIntervalType {
    /// The same numbered day every month, or the last day when the day is -1.
    DayNOfEveryMonth,
    /// The nth occurrence of a weekday, such as the second Tuesday.
    NthDayOfEveryNMonths,
}

/// Whether a task always takes the same time or scales with volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationType {
    Fixed,
    Variable,
}

/// A task that recurs on a schedule, adding work to whichever shift it falls in.
pub struct RecurringTaskStandard {
    id: RecurringTaskStandardId,
    job_id: JobId,
    standard_set_id: StandardSetId,
    business_driver_id: Option<BusinessDriverId>,
    name: String,
    /// The first date the task can occur on; earlier dates never match.
    initial_date: LocalDate,
    frequency_type: FrequencyType,
    daily_interval: i32,
    weekly_interval: i32,
    weekly_days_of_week: Vec<DayOfWeek>,
    monthly_interval_type: MonthlyIntervalType,
    monthly_day_of_month: i32,
    monthly_every_nth_week: i32,
    monthly_day_of_week: Option<DayOfWeek>,
    monthly_selected_months: Vec<Month>,
    occurs_during_shift_id: Option<JobShiftId>,
    duration_type: DurationType,
    fixed_hours: f64,
    variable_works: Vec<RecurringVariableWork>,
}

/// What a recurring task works out to on one date, with the reasoning.
#[derive(Debug, Clone, PartialEq)]
pub struct RecurringTaskCalculation {
    pub total_work_hours: f64,
    pub formula_description: String,
}

impl RecurringTaskStandard {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        business_driver_id: Option<BusinessDriverId>,
        name: String,
        initial_date: LocalDate,
        frequency_type: FrequencyType,
        daily_interval: i32,
        weekly_interval: i32,
        weekly_days_of_week: Vec<DayOfWeek>,
        monthly_interval_type: MonthlyIntervalType,
        monthly_day_of_month: i32,
        monthly_every_nth_week: i32,
        monthly_day_of_week: Option<DayOfWeek>,
        monthly_selected_months: Vec<Month>,
        occurs_during_shift_id: Option<JobShiftId>,
        duration_type: DurationType,
        fixed_hours: f64,
        variable_works: Vec<RecurringVariableWork>,
    ) -> Self {
        Self {
            id: RecurringTaskStandardId::new(),
            job_id,
            standard_set_id,
            business_driver_id,
            name,
            initial_date,
            frequency_type,
            daily_interval,
            weekly_interval,
            weekly_days_of_week,
            monthly_interval_type,
            monthly_day_of_month,
            monthly_every_nth_week,
            monthly_day_of_week,
            monthly_selected_months,
            occurs_during_shift_id,
            duration_type,
            fixed_hours,
            variable_works,
        }
    }

    pub fn id(&self) -> RecurringTaskStandardId {
        self.id
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn business_driver_id(&self) -> Option<BusinessDriverId> {
        self.business_driver_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn initial_date(&self) -> LocalDate {
        self.initial_date
    }

    pub fn frequency_type(&self) -> FrequencyType {
        self.frequency_type
    }

    pub fn occurs_during_shift_id(&self) -> Option<JobShiftId> {
        self.occurs_during_shift_id
    }

    pub fn duration_type(&self) -> DurationType {
        self.duration_type
    }

    pub fn fixed_hours(&self) -> f64 {
        self.fixed_hours
    }

    pub fn variable_works(&self) -> &[RecurringVariableWork] {
        &self.variable_works
    }

    /// Whether the task comes round on `date`.
    ///
    /// `week_ending_day` is the property's own week boundary, which the
    /// multi-week recurrence needs to line dates up against the initial week.
    /// It is passed in rather than stored because it is a property-level
    /// setting, not part of the standard.
    pub fn is_applicable_to_date(&self, date: LocalDate, week_ending_day: DayOfWeek) -> bool {
        if date.is_before(self.initial_date) {
            return false;
        }

        match self.frequency_type {
            FrequencyType::Daily => self.matches_daily_frequency(date),
            FrequencyType::Weekly => self.matches_weekly_frequency(date, week_ending_day),
            FrequencyType::Monthly => self.matches_monthly_frequency(date),
        }
    }

    fn matches_daily_frequency(&self, date: LocalDate) -> bool {
        if self.daily_interval == 1 {
            return true;
        }
        if self.daily_interval <= 0 {
            return false;
        }

        dates::days_between(self.initial_date, date) % self.daily_interval as i64 == 0
    }

    fn matches_weekly_frequency(&self, date: LocalDate, week_ending_day: DayOfWeek) -> bool {
        if !self.weekly_days_of_week.contains(&date.day_of_week()) {
            return false;
        }
        if self.weekly_interval == 1 {
            return true;
        }

        self.matches_two_week_interval(date, week_ending_day)
    }

    /// Lines `date` up against the same weekday in the standard's initial week.
    ///
    /// Java only ever applies a two-week cycle here, whatever `weekly_interval`
    /// says beyond 1 — replicated rather than generalized.
    fn matches_two_week_interval(&self, date: LocalDate, week_ending_day: DayOfWeek) -> bool {
        let initial_week = dates::week_containing(self.initial_date, week_ending_day);

        let Some(same_weekday_in_initial_week) = initial_week
            .iter()
            .find(|candidate| candidate.day_of_week() == date.day_of_week())
        else {
            return false;
        };

        dates::days_between(*same_weekday_in_initial_week, date) % (DAYS_PER_WEEK * 2) == 0
    }

    fn matches_monthly_frequency(&self, date: LocalDate) -> bool {
        if !self.monthly_selected_months.contains(&date.month()) {
            return false;
        }

        match self.monthly_interval_type {
            MonthlyIntervalType::DayNOfEveryMonth => self.matches_day_of_month(date),
            MonthlyIntervalType::NthDayOfEveryNMonths => self.matches_nth_week(date),
        }
    }

    fn matches_day_of_month(&self, date: LocalDate) -> bool {
        if self.monthly_day_of_month == LAST_DAY_OF_THE_MONTH {
            return date == date.last_day_of_month();
        }

        date.day_of_month() == self.monthly_day_of_month
    }

    fn matches_nth_week(&self, date: LocalDate) -> bool {
        let Some(monthly_day_of_week) = self.monthly_day_of_week else {
            return false;
        };

        let first_of_month = date.with_day_of_month(1);
        let days_to_first_matching =
            (monthly_day_of_week.value() - first_of_month.day_of_week().value()).rem_euclid(7);
        let first_matching = first_of_month.plus_days(days_to_first_matching as i64);

        date == first_matching.plus_days((self.monthly_every_nth_week as i64 - 1) * DAYS_PER_WEEK)
    }

    /// The hours a fixed-duration task takes.
    pub fn fixed_hours_with_formula(&self) -> RecurringTaskCalculation {
        RecurringTaskCalculation {
            total_work_hours: self.fixed_hours,
            formula_description: format!("Fixed Work: {:.2}", self.fixed_hours),
        }
    }

    /// The hours a variable-duration task takes at `driver_value`.
    ///
    /// When several tiers cover the value the last one wins, matching Java's
    /// loop, which keeps assigning rather than stopping at the first match.
    /// A value no tier covers yields zero hours rather than an error.
    pub fn total_variable_hours_with_formula(&self, driver_value: i32) -> RecurringTaskCalculation {
        let matching = self
            .variable_works
            .iter()
            .rfind(|work| work.contains_value(driver_value));

        match matching {
            None => RecurringTaskCalculation {
                total_work_hours: 0.0,
                formula_description: format!(
                    "Variable Work: Driver Value - {driver_value}, \
                     Volume Range : No Range Matched.  Calculated Hours: 0.00"
                ),
            },
            Some(work) => {
                let hours = work.hours_for(driver_value);

                RecurringTaskCalculation {
                    total_work_hours: hours,
                    formula_description: format!(
                        "Variable Work: Driver Value - {driver_value}, \
                         Volume Range - {} to {}, Base Hours - {:.2}, \
                         Additional Hours - {:.2} per {} units.  Calculated Hours: {hours:.2}",
                        work.from_volume(),
                        if work.to_volume() == i32::MAX {
                            "Max".to_string()
                        } else {
                            work.to_volume().to_string()
                        },
                        work.base_hours(),
                        work.additional_hours(),
                        work.per_number_units(),
                    ),
                }
            }
        }
    }
}

/// One volume tier of a variable-duration recurring task.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecurringVariableWork {
    from_volume: i32,
    to_volume: i32,
    base_hours: f64,
    additional_hours: f64,
    per_number_units: i32,
}

impl RecurringVariableWork {
    pub fn new(
        from_volume: i32,
        to_volume: i32,
        base_hours: f64,
        additional_hours: f64,
        per_number_units: i32,
    ) -> Self {
        Self {
            from_volume,
            to_volume,
            base_hours,
            additional_hours,
            per_number_units,
        }
    }

    pub fn from_volume(&self) -> i32 {
        self.from_volume
    }

    pub fn to_volume(&self) -> i32 {
        self.to_volume
    }

    pub fn base_hours(&self) -> f64 {
        self.base_hours
    }

    pub fn additional_hours(&self) -> f64 {
        self.additional_hours
    }

    pub fn per_number_units(&self) -> i32 {
        self.per_number_units
    }

    pub fn contains_value(&self, value: i32) -> bool {
        value >= self.from_volume && value <= self.to_volume
    }

    /// Base hours plus one lot of additional hours per whole `per_number_units`.
    ///
    /// The multiplier is integer division, so a part-filled lot adds nothing —
    /// 150 units at "1 hour per 100" earns one additional hour, not one and a
    /// half.
    pub fn hours_for(&self, driver_value: i32) -> f64 {
        let additional = if driver_value > 0 && self.per_number_units > 0 {
            let multiplier = driver_value / self.per_number_units;
            self.additional_hours * multiplier as f64
        } else {
            0.0
        };

        self.base_hours + additional
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    struct Builder {
        initial_date: LocalDate,
        frequency_type: FrequencyType,
        daily_interval: i32,
        weekly_interval: i32,
        weekly_days_of_week: Vec<DayOfWeek>,
        monthly_interval_type: MonthlyIntervalType,
        monthly_day_of_month: i32,
        monthly_every_nth_week: i32,
        monthly_day_of_week: Option<DayOfWeek>,
        monthly_selected_months: Vec<Month>,
        duration_type: DurationType,
        fixed_hours: f64,
        variable_works: Vec<RecurringVariableWork>,
    }

    impl Builder {
        fn new() -> Self {
            Self {
                initial_date: LocalDate::new(2013, 7, 1),
                frequency_type: FrequencyType::Daily,
                daily_interval: 1,
                weekly_interval: 1,
                weekly_days_of_week: Vec::new(),
                monthly_interval_type: MonthlyIntervalType::DayNOfEveryMonth,
                monthly_day_of_month: 1,
                monthly_every_nth_week: 1,
                monthly_day_of_week: None,
                monthly_selected_months: Vec::new(),
                duration_type: DurationType::Fixed,
                fixed_hours: 0.0,
                variable_works: Vec::new(),
            }
        }

        fn build(self) -> RecurringTaskStandard {
            RecurringTaskStandard::new(
                JobId::new(),
                StandardSetId::new(),
                None,
                "Deep clean".to_string(),
                self.initial_date,
                self.frequency_type,
                self.daily_interval,
                self.weekly_interval,
                self.weekly_days_of_week,
                self.monthly_interval_type,
                self.monthly_day_of_month,
                self.monthly_every_nth_week,
                self.monthly_day_of_week,
                self.monthly_selected_months,
                Some(JobShiftId::new()),
                self.duration_type,
                self.fixed_hours,
                self.variable_works,
            )
        }
    }

    #[test]
    fn a_date_before_the_initial_date_never_applies() {
        let standard = Builder::new().build();

        assert!(!standard.is_applicable_to_date(LocalDate::new(2013, 6, 30), DayOfWeek::Saturday));
        assert!(standard.is_applicable_to_date(LocalDate::new(2013, 7, 1), DayOfWeek::Saturday));
    }

    #[rstest]
    // A daily interval of 1 matches every day.
    #[case(1, LocalDate::new(2013, 7, 2), true)]
    #[case(1, LocalDate::new(2013, 7, 3), true)]
    // Every third day, counted from the initial date.
    #[case(3, LocalDate::new(2013, 7, 1), true)]
    #[case(3, LocalDate::new(2013, 7, 2), false)]
    #[case(3, LocalDate::new(2013, 7, 4), true)]
    #[case(3, LocalDate::new(2013, 7, 7), true)]
    fn a_daily_task_repeats_on_its_interval(
        #[case] daily_interval: i32,
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            daily_interval,
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[rstest]
    // 2013-07-01 is a Monday, 07-03 a Wednesday, 07-04 a Thursday.
    #[case(LocalDate::new(2013, 7, 1), true)]
    #[case(LocalDate::new(2013, 7, 3), true)]
    #[case(LocalDate::new(2013, 7, 4), false)]
    fn a_weekly_task_only_applies_on_its_chosen_days(
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            frequency_type: FrequencyType::Weekly,
            weekly_days_of_week: vec![DayOfWeek::Monday, DayOfWeek::Wednesday],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[rstest]
    // Initial date 2013-07-01 (Monday). On a two-week cycle the Mondays that
    // match are the 1st, 15th, 29th — not the 8th or 22nd.
    #[case(LocalDate::new(2013, 7, 1), true)]
    #[case(LocalDate::new(2013, 7, 8), false)]
    #[case(LocalDate::new(2013, 7, 15), true)]
    #[case(LocalDate::new(2013, 7, 22), false)]
    #[case(LocalDate::new(2013, 7, 29), true)]
    fn a_weekly_task_on_a_longer_interval_skips_alternate_weeks(
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            frequency_type: FrequencyType::Weekly,
            weekly_interval: 2,
            weekly_days_of_week: vec![DayOfWeek::Monday],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[rstest]
    #[case(LocalDate::new(2013, 7, 15), true)]
    #[case(LocalDate::new(2013, 7, 16), false)]
    // August is not a selected month.
    #[case(LocalDate::new(2013, 8, 15), false)]
    fn a_monthly_task_applies_on_its_day_in_its_months(
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            frequency_type: FrequencyType::Monthly,
            monthly_day_of_month: 15,
            monthly_selected_months: vec![Month::July, Month::September],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[rstest]
    // July has 31 days, September 30.
    #[case(LocalDate::new(2013, 7, 31), true)]
    #[case(LocalDate::new(2013, 7, 30), false)]
    #[case(LocalDate::new(2013, 9, 30), true)]
    fn a_day_of_month_of_minus_one_means_the_last_day(
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            frequency_type: FrequencyType::Monthly,
            monthly_day_of_month: LAST_DAY_OF_THE_MONTH,
            monthly_selected_months: vec![Month::July, Month::September],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[rstest]
    // July 2013: Tuesdays fall on the 2nd, 9th, 16th, 23rd, 30th.
    #[case(1, LocalDate::new(2013, 7, 2), true)]
    #[case(1, LocalDate::new(2013, 7, 9), false)]
    #[case(2, LocalDate::new(2013, 7, 9), true)]
    #[case(3, LocalDate::new(2013, 7, 16), true)]
    fn an_nth_weekday_task_applies_on_that_occurrence(
        #[case] nth_week: i32,
        #[case] date: LocalDate,
        #[case] expected: bool,
    ) {
        let standard = Builder {
            frequency_type: FrequencyType::Monthly,
            monthly_interval_type: MonthlyIntervalType::NthDayOfEveryNMonths,
            monthly_every_nth_week: nth_week,
            monthly_day_of_week: Some(DayOfWeek::Tuesday),
            monthly_selected_months: vec![Month::July],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard.is_applicable_to_date(date, DayOfWeek::Saturday),
            expected
        );
    }

    #[test]
    fn fixed_hours_are_reported_with_their_formula() {
        let standard = Builder {
            fixed_hours: 2.5,
            ..Builder::new()
        }
        .build();

        let calculation = standard.fixed_hours_with_formula();

        assert_eq!(calculation.total_work_hours, 2.5);
        assert_eq!(calculation.formula_description, "Fixed Work: 2.50");
    }

    #[rstest]
    // Base 1.0 plus 0.5 per 100 units. The multiplier is integer division, so
    // 150 earns one lot and 199 still earns only one.
    #[case(0, 1.0)]
    #[case(100, 1.5)]
    #[case(150, 1.5)]
    #[case(199, 1.5)]
    #[case(200, 2.0)]
    fn variable_hours_add_one_lot_per_whole_unit_block(
        #[case] driver_value: i32,
        #[case] expected: f64,
    ) {
        let standard = Builder {
            duration_type: DurationType::Variable,
            variable_works: vec![RecurringVariableWork::new(0, 1000, 1.0, 0.5, 100)],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard
                .total_variable_hours_with_formula(driver_value)
                .total_work_hours,
            expected
        );
    }

    #[test]
    fn a_volume_no_tier_covers_earns_no_hours() {
        let standard = Builder {
            duration_type: DurationType::Variable,
            variable_works: vec![RecurringVariableWork::new(0, 99, 1.0, 0.5, 100)],
            ..Builder::new()
        }
        .build();

        let calculation = standard.total_variable_hours_with_formula(500);

        assert_eq!(calculation.total_work_hours, 0.0);
        assert!(calculation.formula_description.contains("No Range Matched"));
    }

    #[test]
    fn the_last_matching_tier_wins_when_tiers_overlap() {
        let standard = Builder {
            duration_type: DurationType::Variable,
            variable_works: vec![
                RecurringVariableWork::new(0, 1000, 1.0, 0.0, 0),
                RecurringVariableWork::new(0, 1000, 9.0, 0.0, 0),
            ],
            ..Builder::new()
        }
        .build();

        assert_eq!(
            standard
                .total_variable_hours_with_formula(50)
                .total_work_hours,
            9.0
        );
    }
}
