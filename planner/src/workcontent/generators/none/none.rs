use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::work_generators::{WorkGenerator, WorkResults};
use std::any::Any;

/// The generator for jobs with no standards: it plans nothing.
pub struct NoneWorkGenerator;

impl NoneWorkGenerator {
    pub fn new() -> Self {
        Self {}
    }
}

impl WorkGenerator for NoneWorkGenerator {
    fn generate_work(
        &self,
        _planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<WorkResults, GenerationError> {
        Ok(WorkResults::with_work_content(
            job.id(),
            Vec::new(),
            Vec::new(),
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::location::LocationId;
    use crate::workcontent::domain::planner_mode::PlannerMode;
    use crate::workcontent::domain::standard_set::StandardSetId;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    #[test]
    fn generates_no_work_at_all() {
        let planner_model = PlannerModel::new(
            DateRange::new(LocalDate::new(2025, 10, 1), LocalDate::new(2025, 10, 31)),
            PlannerMode::Standard,
            LocationId::new(),
            StandardSetId::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
        );
        let job = Job::test();

        let results = NoneWorkGenerator::new()
            .generate_work(&planner_model, &job)
            .expect("the fixture is configured to plan");

        assert_eq!(results.job_id(), job.id());
        assert!(results.work_content().unwrap().is_empty());
        assert!(results.planned_shifts().unwrap().is_empty());
    }
}
