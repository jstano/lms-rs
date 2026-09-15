//! What a run of the advanced generator produced.
//!
//! Java also carries a `WorkContentLogger` here, recording how each figure was
//! arrived at; the audit trail is out of scope for this port.

use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::work_content::WorkContent;

#[derive(Debug, Default)]
pub struct AdvancedResults {
    planned_shifts: Vec<PlannedShift>,
    work_contents: Vec<WorkContent>,
}

impl AdvancedResults {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn planned_shifts(&self) -> &[PlannedShift] {
        &self.planned_shifts
    }

    pub fn work_contents(&self) -> &[WorkContent] {
        &self.work_contents
    }

    pub fn add_planned_shifts(&mut self, planned_shifts: impl IntoIterator<Item = PlannedShift>) {
        self.planned_shifts.extend(planned_shifts);
    }

    pub fn add_work_contents(&mut self, work_contents: impl IntoIterator<Item = WorkContent>) {
        self.work_contents.extend(work_contents);
    }

    pub fn clear(&mut self) {
        self.planned_shifts.clear();
        self.work_contents.clear();
    }

    /// Hand the results on, consuming the container.
    pub fn into_parts(self) -> (Vec<PlannedShift>, Vec<WorkContent>) {
        (self.planned_shifts, self.work_contents)
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Ported from `KbiRelatedResultsTest.groovy`, minus its logger
    //! assertions.

    use super::*;
    use crate::workcontent::domain::plan_type::PlanType;
    use crate::workcontent::domain::work_content::WorkContent;
    use date_range_rs::DateTimeRange;
    use joda_rs::{LocalDate, LocalTime};

    fn planned_shift() -> PlannedShift {
        PlannedShift::new()
    }

    fn work_content() -> WorkContent {
        let date = LocalDate::new(2013, 8, 26);

        WorkContent::fixed(
            crate::workcontent::domain::job::JobId::new(),
            crate::workcontent::domain::location::LocationId::new(),
            PlanType::Forecast,
            date,
            &DateTimeRange::of(
                date.at_time(LocalTime::new(8, 0, 0)),
                date.at_time(LocalTime::new(16, 0, 0)),
            ),
        )
    }

    #[test]
    fn results_hold_what_was_added_to_them() {
        let mut results = AdvancedResults::new();
        let shifts: Vec<_> = (0..3).map(|_| planned_shift()).collect();
        let shift_ids: Vec<_> = shifts.iter().map(|shift| shift.id()).collect();
        let contents: Vec<_> = (0..2).map(|_| work_content()).collect();
        let content_ids: Vec<_> = contents.iter().map(|content| content.id()).collect();

        results.add_planned_shifts(shifts);
        results.add_work_contents(contents);

        assert_eq!(results.planned_shifts().len(), 3);
        assert_eq!(results.work_contents().len(), 2);
        assert!(
            shift_ids
                .iter()
                .all(|id| results.planned_shifts().iter().any(|shift| shift.id() == *id))
        );
        assert!(
            content_ids
                .iter()
                .all(|id| results
                    .work_contents()
                    .iter()
                    .any(|content| content.id() == *id))
        );
    }

    #[test]
    fn clearing_the_results_empties_them() {
        let mut results = AdvancedResults::new();
        results.add_planned_shifts((0..3).map(|_| planned_shift()));
        results.add_work_contents((0..2).map(|_| work_content()));

        results.clear();

        assert!(results.planned_shifts().is_empty());
        assert!(results.work_contents().is_empty());
    }

    #[test]
    fn fresh_results_are_empty() {
        let results = AdvancedResults::new();

        assert!(results.planned_shifts().is_empty());
        assert!(results.work_contents().is_empty());
    }
}
