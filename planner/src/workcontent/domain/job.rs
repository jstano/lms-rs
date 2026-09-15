use crate::id_type;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job_min_max_coverage::JobMinMaxCoverage;
use crate::workcontent::domain::job_shift::{JobShift, JobShiftId};
use crate::workcontent::domain::location::LocationId;
use crate::workcontent::domain::planner_settings::PlannerSettings;
use crate::workcontent::domain::recurring_task_standard::RecurringTaskStandard;
use crate::workcontent::domain::salaried_standard::SalariedStandard;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::domain::spread_standard::SpreadStandard;
use crate::workcontent::domain::standard_set::StandardSetId;
use crate::workcontent::domain::work_type::WorkType;
use joda_rs::{DayOfWeek, LocalDate};

id_type!(JobId, uuid_v4);

pub struct Job {
    id: JobId,
    property_id: LocationId,
    planner_settings: PlannerSettings,
    shifts: Vec<JobShift>,
    salaried_standards: Vec<SalariedStandard>,
    shift_standards: Vec<ShiftStandard>,
    /// Standards that follow a configured intraday shape. Only the flowed
    /// generator reads these; the non-flowed path leaves them empty.
    spread_standards: Vec<SpreadStandard>,
    recurring_task_standards: Vec<RecurringTaskStandard>,
    min_max_coverages: Vec<JobMinMaxCoverage>,
}

impl Job {
    pub fn new(
        property_id: LocationId,
        planner_settings: PlannerSettings,
        shifts: Vec<JobShift>,
        salaried_standards: Vec<SalariedStandard>,
        shift_standards: Vec<ShiftStandard>,
    ) -> Self {
        Self {
            id: JobId::new(),
            property_id,
            planner_settings,
            shifts,
            salaried_standards,
            shift_standards,
            spread_standards: Vec::new(),
            recurring_task_standards: Vec::new(),
            min_max_coverages: Vec::new(),
        }
    }

    /// Attach the standards only the flowed generator plans from.
    ///
    /// Kept apart from [`new`](Self::new) so the non-flowed path, which never
    /// has any of these, is not made to pass three empty vectors.
    #[must_use]
    pub fn with_flowed_standards(
        mut self,
        spread_standards: Vec<SpreadStandard>,
        recurring_task_standards: Vec<RecurringTaskStandard>,
        min_max_coverages: Vec<JobMinMaxCoverage>,
    ) -> Self {
        self.spread_standards = spread_standards;
        self.recurring_task_standards = recurring_task_standards;
        self.min_max_coverages = min_max_coverages;
        self
    }

    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn property_id(&self) -> LocationId {
        self.property_id
    }

    pub fn planner_settings(&self) -> &PlannerSettings {
        &self.planner_settings
    }

    pub fn shifts(&self) -> &[JobShift] {
        &self.shifts
    }

    pub fn shifts_for_standard_set(&self, standard_set_id: StandardSetId) -> Vec<&JobShift> {
        self.shifts
            .iter()
            .filter(|shift| shift.standard_set_id() == standard_set_id)
            .collect()
    }

    pub fn salaried_standards(&self) -> &[SalariedStandard] {
        &self.salaried_standards
    }

    pub fn salaried_standard_for_standard_set_and_shift(
        &self,
        standard_set_id: StandardSetId,
        shift: &JobShift,
    ) -> Option<&SalariedStandard> {
        self.salaried_standards.iter().find(|standard| {
            standard.standard_set_id() == standard_set_id && standard.shift_id() == shift.id()
        })
    }

    pub fn shift_standards(&self) -> &[ShiftStandard] {
        &self.shift_standards
    }

    /// The standards the non-flowed generators plan from: everything on this
    /// standard set and shift except staffing work, which the staffing
    /// generators own.
    pub fn non_staff_shift_standards_for_standard_set_and_shift(
        &self,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
    ) -> Vec<&ShiftStandard> {
        self.shift_standards
            .iter()
            .filter(|standard| {
                standard.standard_set_id() == standard_set_id
                    && standard.job_shift_id() == job_shift_id
                    && standard.work_type() != WorkType::Staff
            })
            .collect()
    }

    /// The standards the staffing generator plans from: the mirror of
    /// [`non_staff_shift_standards_for_standard_set_and_shift`](Self::non_staff_shift_standards_for_standard_set_and_shift).
    pub fn staff_shift_standards_for_standard_set_and_shift(
        &self,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
    ) -> Vec<&ShiftStandard> {
        self.shift_standards
            .iter()
            .filter(|standard| {
                standard.standard_set_id() == standard_set_id
                    && standard.job_shift_id() == job_shift_id
                    && standard.work_type() == WorkType::Staff
            })
            .collect()
    }

    pub fn spread_standards(&self) -> &[SpreadStandard] {
        &self.spread_standards
    }

    /// The spread standards configured for this standard set and shift.
    pub fn spread_standards_for_standard_set_and_shift(
        &self,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
    ) -> Vec<&SpreadStandard> {
        self.spread_standards
            .iter()
            .filter(|standard| {
                standard.standard_set_id() == standard_set_id
                    && standard.job_shift_id() == job_shift_id
            })
            .collect()
    }

    pub fn recurring_task_standards(&self) -> &[RecurringTaskStandard] {
        &self.recurring_task_standards
    }

    /// The recurring tasks that fall in this shift on this date.
    ///
    /// `week_ending_day` is the property's week boundary, needed to place
    /// multi-week recurrences; see
    /// [`RecurringTaskStandard::is_applicable_to_date`].
    pub fn recurring_task_standards_for_shift_and_date(
        &self,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        date: LocalDate,
        week_ending_day: DayOfWeek,
    ) -> Vec<&RecurringTaskStandard> {
        self.recurring_task_standards
            .iter()
            .filter(|standard| {
                standard.standard_set_id() == standard_set_id
                    && standard.occurs_during_shift_id() == Some(job_shift_id)
                    && standard.is_applicable_to_date(date, week_ending_day)
            })
            .collect()
    }

    pub fn min_max_coverages(&self) -> &[JobMinMaxCoverage] {
        &self.min_max_coverages
    }

    /// The staffing bounds for this standard set in this environment, if any
    /// were configured.
    pub fn min_max_coverage_for_standard_set_and_environment(
        &self,
        standard_set_id: StandardSetId,
        environment_id: EnvironmentId,
    ) -> Option<&JobMinMaxCoverage> {
        self.min_max_coverages.iter().find(|coverage| {
            coverage.standard_set_id() == standard_set_id
                && coverage.environment_id() == environment_id
        })
    }

    #[cfg(test)]
    pub fn test() -> Self {
        Self::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::business_driver::BusinessDriverId;
    use crate::workcontent::domain::job_min_max_coverage::JobMinMaxCoverage;
    use crate::workcontent::domain::recurring_task_standard::{
        DurationType, FrequencyType, MonthlyIntervalType,
    };
    use crate::workcontent::domain::salary_mode::SalaryMode;
    use crate::workcontent::domain::shift_standard::ShiftStandardRange;
    use crate::workcontent::domain::spread_standard::SpreadStandardType;
    use crate::workcontent::domain::standard_type::StandardType;
    use crate::workcontent::domain::units::Units;

    fn shift(standard_set_id: StandardSetId) -> JobShift {
        JobShift::new(
            JobId::new(),
            standard_set_id,
            "Day".to_string(),
            1,
            Vec::new(),
        )
    }

    #[test]
    fn planner_settings_returns_the_jobs_own_settings() {
        let mut settings = PlannerSettings::default();
        settings.standard_type = StandardType::BASIC;
        settings.min_shift_length = 3.0;

        let job = Job::new(
            LocationId::new(),
            settings,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(job.planner_settings().standard_type, StandardType::BASIC);
        assert_eq!(job.planner_settings().min_shift_length, 3.0);
    }

    #[test]
    fn shifts_are_filtered_by_standard_set() {
        let standard_set_id = StandardSetId::new();
        let other_standard_set_id = StandardSetId::new();
        let job = Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            vec![shift(standard_set_id), shift(other_standard_set_id)],
            Vec::new(),
            Vec::new(),
        );

        let shifts = job.shifts_for_standard_set(standard_set_id);

        assert_eq!(shifts.len(), 1);
        assert_eq!(shifts[0].standard_set_id(), standard_set_id);
    }

    #[test]
    fn a_salaried_standard_is_matched_on_standard_set_and_shift() {
        let standard_set_id = StandardSetId::new();
        let matching_shift = shift(standard_set_id);
        let other_shift = shift(standard_set_id);
        let standard = SalariedStandard::new(
            JobId::new(),
            standard_set_id,
            matching_shift.id(),
            SalaryMode::WEEKLY,
            Some(40.0),
            None,
            None,
        );
        let job = Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            vec![standard],
            Vec::new(),
        );

        assert!(
            job.salaried_standard_for_standard_set_and_shift(standard_set_id, &matching_shift)
                .is_some()
        );
        assert!(
            job.salaried_standard_for_standard_set_and_shift(standard_set_id, &other_shift)
                .is_none()
        );
        assert!(
            job.salaried_standard_for_standard_set_and_shift(StandardSetId::new(), &matching_shift)
                .is_none()
        );
    }

    #[test]
    fn staff_standards_are_excluded_from_the_non_flowed_standards() {
        let standard_set_id = StandardSetId::new();
        let job_shift_id = JobShiftId::new();

        let standard_of = |work_type: WorkType| {
            ShiftStandard::new(
                JobId::new(),
                standard_set_id,
                job_shift_id,
                BusinessDriverId::new(),
                work_type,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 100, 1.0)],
            )
        };

        let job = Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            vec![
                standard_of(WorkType::Variable),
                standard_of(WorkType::Staff),
                standard_of(WorkType::Daily),
            ],
        );

        let standards =
            job.non_staff_shift_standards_for_standard_set_and_shift(standard_set_id, job_shift_id);

        assert_eq!(standards.len(), 2);
        assert!(standards.iter().all(|s| s.work_type() != WorkType::Staff));
    }

    #[test]
    fn a_job_has_no_flowed_standards_until_they_are_attached() {
        let job = Job::test();

        assert!(job.spread_standards().is_empty());
        assert!(job.recurring_task_standards().is_empty());
        assert!(job.min_max_coverages().is_empty());
    }

    #[test]
    fn spread_standards_are_matched_on_standard_set_and_shift() {
        let standard_set_id = StandardSetId::new();
        let job_shift_id = JobShiftId::new();

        let spread_standard = |standard_set_id, job_shift_id| {
            SpreadStandard::new(
                JobId::new(),
                standard_set_id,
                job_shift_id,
                BusinessDriverId::new(),
                Units::MinutesPerUnit,
                SpreadStandardType::Fixed,
                Vec::new(),
            )
        };

        let job = Job::test().with_flowed_standards(
            vec![
                spread_standard(standard_set_id, job_shift_id),
                // Right standard set, wrong shift.
                spread_standard(standard_set_id, JobShiftId::new()),
                // Right shift, wrong standard set.
                spread_standard(StandardSetId::new(), job_shift_id),
            ],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            job.spread_standards_for_standard_set_and_shift(standard_set_id, job_shift_id)
                .len(),
            1
        );
    }

    #[test]
    fn recurring_tasks_are_matched_on_standard_set_shift_and_date() {
        let standard_set_id = StandardSetId::new();
        let job_shift_id = JobShiftId::new();
        let date = LocalDate::new(2013, 7, 24);

        let task = |standard_set_id, shift_id: Option<JobShiftId>, initial_date| {
            RecurringTaskStandard::new(
                JobId::new(),
                standard_set_id,
                None,
                "Task".to_string(),
                initial_date,
                FrequencyType::Daily,
                1,
                1,
                Vec::new(),
                MonthlyIntervalType::DayNOfEveryMonth,
                1,
                1,
                None,
                Vec::new(),
                shift_id,
                DurationType::Fixed,
                1.0,
                Vec::new(),
            )
        };

        let job = Job::test().with_flowed_standards(
            Vec::new(),
            vec![
                task(standard_set_id, Some(job_shift_id), LocalDate::new(2013, 7, 1)),
                // Right standard set and shift, but not yet started on this date.
                task(standard_set_id, Some(job_shift_id), LocalDate::new(2013, 8, 1)),
                // Right standard set and date, wrong shift.
                task(
                    standard_set_id,
                    Some(JobShiftId::new()),
                    LocalDate::new(2013, 7, 1),
                ),
                // Right shift and date, wrong standard set.
                task(
                    StandardSetId::new(),
                    Some(job_shift_id),
                    LocalDate::new(2013, 7, 1),
                ),
            ],
            Vec::new(),
        );

        assert_eq!(
            job.recurring_task_standards_for_shift_and_date(
                standard_set_id,
                job_shift_id,
                date,
                DayOfWeek::Saturday
            )
            .len(),
            1
        );
    }

    #[test]
    fn min_max_coverage_is_matched_on_standard_set_and_environment() {
        let standard_set_id = StandardSetId::new();
        let environment_id = EnvironmentId::new();

        let job = Job::test().with_flowed_standards(
            Vec::new(),
            Vec::new(),
            vec![JobMinMaxCoverage::new(
                JobId::new(),
                standard_set_id,
                environment_id,
                Vec::new(),
            )],
        );

        assert!(
            job.min_max_coverage_for_standard_set_and_environment(standard_set_id, environment_id)
                .is_some()
        );
        assert!(
            job.min_max_coverage_for_standard_set_and_environment(
                standard_set_id,
                EnvironmentId::new()
            )
            .is_none()
        );
    }

    #[test]
    fn staff_standards_are_the_only_ones_the_staffing_filter_returns() {
        let standard_set_id = StandardSetId::new();
        let job_shift_id = JobShiftId::new();

        let standard_of = |work_type: WorkType| {
            ShiftStandard::new(
                JobId::new(),
                standard_set_id,
                job_shift_id,
                BusinessDriverId::new(),
                work_type,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 100, 1.0)],
            )
        };

        let job = Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            vec![
                standard_of(WorkType::Variable),
                standard_of(WorkType::Staff),
                standard_of(WorkType::Daily),
            ],
        );

        let standards =
            job.staff_shift_standards_for_standard_set_and_shift(standard_set_id, job_shift_id);

        assert_eq!(standards.len(), 1);
        assert_eq!(standards[0].work_type(), WorkType::Staff);
    }

    #[test]
    fn standards_for_another_shift_are_excluded() {
        let standard_set_id = StandardSetId::new();
        let job = Job::new(
            LocationId::new(),
            PlannerSettings::default(),
            Vec::new(),
            Vec::new(),
            vec![ShiftStandard::new(
                JobId::new(),
                standard_set_id,
                JobShiftId::new(),
                BusinessDriverId::new(),
                WorkType::Variable,
                Units::MinutesPerUnit,
                0,
                vec![ShiftStandardRange::new(0, 100, 1.0)],
            )],
        );

        assert!(
            job.non_staff_shift_standards_for_standard_set_and_shift(
                standard_set_id,
                JobShiftId::new()
            )
            .is_empty()
        );
    }
}
