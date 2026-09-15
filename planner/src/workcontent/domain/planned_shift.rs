use crate::id_type;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::location::LocationId;
use crate::workcontent::domain::plan_type::PlanType;
use crate::workcontent::domain::shift_source::ShiftSource;
use joda_rs::{LocalDate, LocalDateTime};

id_type!(PlannedShiftId, uuid_v4);

/// A shift placed on the calendar, mirroring the work content it was created
/// from.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedShift {
    id: PlannedShiftId,
    job_id: Option<JobId>,
    property_id: Option<LocationId>,
    shift_type: Option<PlanType>,
    shift_date: Option<LocalDate>,
    date_shift_generated_from: Option<LocalDate>,
    start_date_time: Option<LocalDateTime>,
    end_date_time: Option<LocalDateTime>,
    duration: f64,
    source: Option<ShiftSource>,
    assignment_id: Option<JobId>,
}

impl Default for PlannedShift {
    fn default() -> Self {
        Self {
            id: PlannedShiftId::new(),
            job_id: None,
            property_id: None,
            shift_type: None,
            shift_date: None,
            date_shift_generated_from: None,
            start_date_time: None,
            end_date_time: None,
            duration: 0.0,
            source: None,
            assignment_id: None,
        }
    }
}

impl PlannedShift {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(&self) -> PlannedShiftId {
        self.id
    }

    pub fn job_id(&self) -> Option<JobId> {
        self.job_id
    }
    pub fn set_job_id(&mut self, job_id: Option<JobId>) {
        self.job_id = job_id;
    }

    pub fn property_id(&self) -> Option<LocationId> {
        self.property_id
    }
    pub fn set_property_id(&mut self, property_id: Option<LocationId>) {
        self.property_id = property_id;
    }

    pub fn shift_type(&self) -> Option<PlanType> {
        self.shift_type
    }
    pub fn set_shift_type(&mut self, shift_type: Option<PlanType>) {
        self.shift_type = shift_type;
    }

    pub fn shift_date(&self) -> Option<LocalDate> {
        self.shift_date
    }
    pub fn set_shift_date(&mut self, shift_date: Option<LocalDate>) {
        self.shift_date = shift_date;
    }

    pub fn date_shift_generated_from(&self) -> Option<LocalDate> {
        self.date_shift_generated_from
    }
    pub fn set_date_shift_generated_from(&mut self, date: Option<LocalDate>) {
        self.date_shift_generated_from = date;
    }

    pub fn start_date_time(&self) -> Option<LocalDateTime> {
        self.start_date_time
    }
    pub fn set_start_date_time(&mut self, value: Option<LocalDateTime>) {
        self.start_date_time = value;
    }

    pub fn end_date_time(&self) -> Option<LocalDateTime> {
        self.end_date_time
    }
    pub fn set_end_date_time(&mut self, value: Option<LocalDateTime>) {
        self.end_date_time = value;
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }
    pub fn set_duration(&mut self, duration: f64) {
        self.duration = duration;
    }

    pub fn source(&self) -> Option<ShiftSource> {
        self.source
    }
    pub fn set_source(&mut self, source: Option<ShiftSource>) {
        self.source = source;
    }

    pub fn assignment_id(&self) -> Option<JobId> {
        self.assignment_id
    }
    pub fn set_assignment_id(&mut self, assignment_id: Option<JobId>) {
        self.assignment_id = assignment_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalTime;

    #[test]
    fn planned_shift_setters_and_getters_work() {
        let mut ps = PlannedShift::new();
        let job_id = JobId::new();
        let property_id = LocationId::new();
        ps.set_job_id(Some(job_id));
        ps.set_property_id(Some(property_id));
        ps.set_shift_type(Some(PlanType::Forecast));
        let date = LocalDate::new(2025, 1, 2);
        ps.set_shift_date(Some(date));
        ps.set_date_shift_generated_from(Some(date));
        let start = LocalDateTime::of_date_time(date, LocalTime::new(9, 0, 0));
        let end = LocalDateTime::of_date_time(date, LocalTime::new(17, 0, 0));
        ps.set_start_date_time(Some(start));
        ps.set_end_date_time(Some(end));
        ps.set_duration(8.0);
        ps.set_source(Some(ShiftSource::Auto));

        assert_eq!(ps.job_id(), Some(job_id));
        assert_eq!(ps.property_id(), Some(property_id));
        assert_eq!(ps.shift_type(), Some(PlanType::Forecast));
        assert_eq!(ps.shift_date(), Some(date));
        assert_eq!(ps.date_shift_generated_from(), Some(date));
        assert_eq!(ps.start_date_time(), Some(start));
        assert_eq!(ps.end_date_time(), Some(end));
        assert_eq!(ps.duration(), 8.0);
        assert_eq!(ps.source(), Some(ShiftSource::Auto));
    }

    #[test]
    fn each_planned_shift_gets_its_own_id() {
        assert_ne!(PlannedShift::new().id(), PlannedShift::new().id());
    }
}
