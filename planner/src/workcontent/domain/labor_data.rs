use joda_rs::LocalDate;
use crate::workcontent::domain::job::JobId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaborData {
    job_id: JobId,
    date: LocalDate,
    hours: f64,
}

impl LaborData {
    pub fn new(job_id: JobId, date: LocalDate, hours: f64) -> Self {
        Self {
            job_id,
            date,
            hours
        }
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn date(&self) -> LocalDate {
        self.date
    }

    pub fn hours(&self) -> f64 {
        self.hours
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn d(y: i32, m: i32, d: i32) -> LocalDate {
        LocalDate::new(y, m, d)
    }

    #[test]
    fn new_sets_fields_and_accessors_return_values() {
        let job_id = JobId::new();
        let date = d(2025, 1, 15);
        let hours = 8.75;

        let ld = LaborData::new(job_id, date, hours);

        assert_eq!(ld.job_id(), job_id);
        assert_eq!(ld.date(), date);
        assert_eq!(ld.hours(), hours);
    }

    #[rstest]
    #[case(0.0)]
    #[case(8.5)]
    #[case(1234.25)]
    #[case(-3.75)]
    fn hours_are_stored_precisely_for_common_values(#[case] hours: f64) {
        let job_id = JobId::new();
        let date = d(2024, 12, 31);

        let ld = LaborData::new(job_id, date, hours);

        assert_eq!(ld.hours(), hours);
    }

    #[test]
    fn clone_and_copy_behave_as_expected() {
        let job_id = JobId::new();
        let date = d(2023, 6, 1);

        let original = LaborData::new(job_id, date, 10.0);

        // Copy (because Copy is derived)
        let copied: LaborData = original;
        assert_eq!(copied, original);

        // Clone (because Clone is derived)
        let cloned = original.clone();
        assert_eq!(cloned, original);
    }

    #[test]
    fn equality_same_values() {
        let job_id = JobId::new();
        let date = d(2022, 2, 2);
        let hours = 5.0;

        let a = LaborData::new(job_id, date, hours);
        let b = LaborData::new(job_id, date, hours);

        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_job_id() {
        let date = d(2022, 2, 2);
        let hours = 5.0;

        let a = LaborData::new(JobId::new(), date, hours);
        let b = LaborData::new(JobId::new(), date, hours);

        assert_ne!(a, b);
    }

    #[test]
    fn inequality_different_date() {
        let job_id = JobId::new();

        let a = LaborData::new(job_id, d(2022, 1, 1), 5.0);
        let b = LaborData::new(job_id, d(2022, 1, 2), 5.0);

        assert_ne!(a, b);
    }

    #[test]
    fn inequality_different_hours() {
        let job_id = JobId::new();
        let date = d(2022, 1, 1);

        let a = LaborData::new(job_id, date, 5.0);
        let b = LaborData::new(job_id, date, 6.0);

        assert_ne!(a, b);
    }
}
