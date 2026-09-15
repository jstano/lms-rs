use crate::id_type;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::standard_set::StandardSetId;

id_type!(JobMinMaxCoverageId, uuid_v4);

/// Staffing floors and ceilings for a job, at five-minute granularity.
///
/// A maximum of zero means "no ceiling configured" rather than "staff nobody" —
/// the same convention the unconfigured-day fallback produces — so it must not
/// be applied as a real cap.
pub struct JobMinMaxCoverage {
    id: JobMinMaxCoverageId,
    job_id: JobId,
    standard_set_id: StandardSetId,
    environment_id: EnvironmentId,
    staff_values: Vec<MinMaxStaffing>,
}

impl JobMinMaxCoverage {
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        environment_id: EnvironmentId,
        staff_values: Vec<MinMaxStaffing>,
    ) -> Self {
        Self {
            id: JobMinMaxCoverageId::new(),
            job_id,
            standard_set_id,
            environment_id,
            staff_values,
        }
    }

    pub fn id(&self) -> JobMinMaxCoverageId {
        self.id
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn environment_id(&self) -> EnvironmentId {
        self.environment_id
    }

    pub fn staff_values(&self) -> &[MinMaxStaffing] {
        &self.staff_values
    }
}

/// The staffing bounds for one five-minute slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinMaxStaffing {
    minimum: i32,
    maximum: i32,
}

impl MinMaxStaffing {
    pub fn new(minimum: i32, maximum: i32) -> Self {
        Self { minimum, maximum }
    }

    /// The bounds used for a slot with nothing configured: no floor, no ceiling.
    pub fn unconfigured() -> Self {
        Self::new(0, 0)
    }

    pub fn minimum(&self) -> i32 {
        self.minimum
    }

    pub fn maximum(&self) -> i32 {
        self.maximum
    }

    /// Whether a real ceiling was configured. A zero maximum is a sentinel for
    /// "uncapped", not a cap of nobody.
    pub fn has_maximum(&self) -> bool {
        self.maximum > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unconfigured_slot_has_no_floor_and_no_ceiling() {
        let staffing = MinMaxStaffing::unconfigured();

        assert_eq!(staffing.minimum(), 0);
        assert_eq!(staffing.maximum(), 0);
        assert!(!staffing.has_maximum());
    }

    #[test]
    fn a_zero_maximum_is_uncapped_rather_than_a_cap_of_none() {
        assert!(!MinMaxStaffing::new(2, 0).has_maximum());
        assert!(MinMaxStaffing::new(2, 1).has_maximum());
    }
}
