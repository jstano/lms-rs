use crate::id_type;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::location::LocationId;
use crate::workcontent::domain::plan_type::PlanType;
use date_range_rs::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime};

id_type!(WorkContentId, uuid_v4);

/// A block of work to be covered on a date.
///
/// The three "wanted" times bound when the work may happen, while the two
/// "calculated" times are where the generator actually put it. For fixed work
/// — everything the non-flowed generators produce — they coincide.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkContent {
    id: WorkContentId,
    job_id: JobId,
    property_id: LocationId,
    shift_type: PlanType,
    shift_date: LocalDate,
    earliest_start_date_time: LocalDateTime,
    preferred_start_date_time: LocalDateTime,
    latest_end_date_time: LocalDateTime,
    calculated_start_date_time: LocalDateTime,
    calculated_end_date_time: LocalDateTime,
    calculated_hours: f64,
    adjusted_hours: f64,
    locked: bool,
    description: String,
    min_number_employees: u32,
    min_skill_level: u32,
    distributed_to_date_time: Option<LocalDateTime>,
}

impl WorkContent {
    pub fn new(
        job_id: JobId,
        property_id: LocationId,
        shift_type: PlanType,
        shift_date: LocalDate,
        earliest_start_date_time: LocalDateTime,
        preferred_start_date_time: LocalDateTime,
        latest_end_date_time: LocalDateTime,
        calculated_start_date_time: LocalDateTime,
        calculated_end_date_time: LocalDateTime,
        calculated_hours: f64,
        adjusted_hours: f64,
        locked: bool,
        description: String,
        min_number_employees: u32,
        min_skill_level: u32,
        distributed_to_date_time: Option<LocalDateTime>,
    ) -> Self {
        Self {
            id: WorkContentId::new(),
            job_id,
            property_id,
            shift_type,
            shift_date,
            earliest_start_date_time,
            preferred_start_date_time,
            latest_end_date_time,
            calculated_start_date_time,
            calculated_end_date_time,
            calculated_hours,
            adjusted_hours,
            locked,
            description,
            min_number_employees,
            min_skill_level,
            distributed_to_date_time,
        }
    }

    /// Work pinned to an exact time range: every start time is the same, every
    /// end time is the same, and the hours match the range exactly.
    pub fn fixed(
        job_id: JobId,
        property_id: LocationId,
        shift_type: PlanType,
        shift_date: LocalDate,
        range: &DateTimeRange,
    ) -> Self {
        let hours = range.duration().fractional_hours();

        Self::new(
            job_id,
            property_id,
            shift_type,
            shift_date,
            range.start(),
            range.start(),
            range.end(),
            range.start(),
            range.end(),
            hours,
            hours,
            false,
            String::new(),
            0,
            0,
            None,
        )
    }

    pub fn id(&self) -> WorkContentId {
        self.id
    }
    pub fn job_id(&self) -> JobId {
        self.job_id
    }
    pub fn property_id(&self) -> LocationId {
        self.property_id
    }
    pub fn shift_type(&self) -> PlanType {
        self.shift_type
    }
    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }
    pub fn earliest_start_date_time(&self) -> LocalDateTime {
        self.earliest_start_date_time
    }
    pub fn preferred_start_date_time(&self) -> LocalDateTime {
        self.preferred_start_date_time
    }
    pub fn latest_end_date_time(&self) -> LocalDateTime {
        self.latest_end_date_time
    }
    pub fn calculated_start_date_time(&self) -> LocalDateTime {
        self.calculated_start_date_time
    }
    pub fn calculated_end_date_time(&self) -> LocalDateTime {
        self.calculated_end_date_time
    }
    pub fn calculated_hours(&self) -> f64 {
        self.calculated_hours
    }
    pub fn adjusted_hours(&self) -> f64 {
        self.adjusted_hours
    }
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    pub fn description(&self) -> &str {
        self.description.as_str()
    }
    pub fn min_number_employees(&self) -> u32 {
        self.min_number_employees
    }
    pub fn min_skill_level(&self) -> u32 {
        self.min_skill_level
    }
    pub fn distributed_to_date_time(&self) -> Option<LocalDateTime> {
        self.distributed_to_date_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalTime;

    fn date() -> LocalDate {
        LocalDate::new(2025, 10, 6)
    }

    fn range() -> DateTimeRange {
        DateTimeRange::of(
            date().at_time(LocalTime::of_hour_minute(9, 0)),
            date().at_time(LocalTime::of_hour_minute(17, 30)),
        )
    }

    #[test]
    fn fixed_work_pins_every_time_to_the_range() {
        let range = range();
        let work_content = WorkContent::fixed(
            JobId::new(),
            LocationId::new(),
            PlanType::Forecast,
            date(),
            &range,
        );

        assert_eq!(work_content.earliest_start_date_time(), range.start());
        assert_eq!(work_content.preferred_start_date_time(), range.start());
        assert_eq!(work_content.calculated_start_date_time(), range.start());
        assert_eq!(work_content.latest_end_date_time(), range.end());
        assert_eq!(work_content.calculated_end_date_time(), range.end());
    }

    #[test]
    fn fixed_work_takes_its_hours_from_the_range() {
        let work_content = WorkContent::fixed(
            JobId::new(),
            LocationId::new(),
            PlanType::Forecast,
            date(),
            &range(),
        );

        assert_eq!(work_content.calculated_hours(), 8.5);
        assert_eq!(work_content.adjusted_hours(), 8.5);
    }

    #[test]
    fn fixed_work_starts_undistributed_and_unlocked() {
        let work_content = WorkContent::fixed(
            JobId::new(),
            LocationId::new(),
            PlanType::Forecast,
            date(),
            &range(),
        );

        assert!(!work_content.is_locked());
        assert_eq!(work_content.distributed_to_date_time(), None);
    }

    #[test]
    fn each_work_content_gets_its_own_id() {
        let work_content = || {
            WorkContent::fixed(
                JobId::new(),
                LocationId::new(),
                PlanType::Forecast,
                date(),
                &range(),
            )
        };

        assert_ne!(work_content().id(), work_content().id());
    }
}
