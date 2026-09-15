use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::job_shift::JobShiftId;
use crate::workcontent::domain::salary_mode::SalaryMode;
use crate::workcontent::domain::standard_set::StandardSetId;

/// The salaried hours entitlement for one shift of a job.
///
/// The three hour figures are optional because the Java entity holds them as
/// nullable columns and treats a null as zero at every use.
pub struct SalariedStandard {
    job_id: JobId,
    standard_set_id: StandardSetId,
    shift_id: JobShiftId,
    salary_mode: SalaryMode,
    hours_per_week: Option<f64>,
    vacation_hours_per_year: Option<f64>,
    hours_per_year: Option<f64>,
}

impl SalariedStandard {
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        shift_id: JobShiftId,
        salary_mode: SalaryMode,
        hours_per_week: Option<f64>,
        vacation_hours_per_year: Option<f64>,
        hours_per_year: Option<f64>,
    ) -> Self {
        Self {
            job_id,
            standard_set_id,
            shift_id,
            salary_mode,
            hours_per_week,
            vacation_hours_per_year,
            hours_per_year,
        }
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn shift_id(&self) -> JobShiftId {
        self.shift_id
    }

    pub fn salary_mode(&self) -> SalaryMode {
        self.salary_mode
    }

    pub fn hours_per_week(&self) -> f64 {
        self.hours_per_week.unwrap_or(0.0)
    }

    pub fn vacation_hours_per_year(&self) -> f64 {
        self.vacation_hours_per_year.unwrap_or(0.0)
    }

    pub fn hours_per_year(&self) -> f64 {
        self.hours_per_year.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard(
        hours_per_week: Option<f64>,
        vacation_hours_per_year: Option<f64>,
        hours_per_year: Option<f64>,
    ) -> SalariedStandard {
        SalariedStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            SalaryMode::WEEKLY,
            hours_per_week,
            vacation_hours_per_year,
            hours_per_year,
        )
    }

    #[test]
    fn unset_hours_read_as_zero() {
        let standard = standard(None, None, None);

        assert_eq!(standard.hours_per_week(), 0.0);
        assert_eq!(standard.vacation_hours_per_year(), 0.0);
        assert_eq!(standard.hours_per_year(), 0.0);
    }

    #[test]
    fn set_hours_read_back_unchanged() {
        let standard = standard(Some(40.0), Some(80.0), Some(2000.0));

        assert_eq!(standard.hours_per_week(), 40.0);
        assert_eq!(standard.vacation_hours_per_year(), 80.0);
        assert_eq!(standard.hours_per_year(), 2000.0);
    }
}
