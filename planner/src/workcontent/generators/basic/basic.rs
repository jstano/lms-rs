use crate::workcontent::domain::job::Job;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::basic::basic_planned_shift_creator::{
    BasicPlannedShiftCreator, BasicPlannedShiftCreatorImpl,
};
use crate::workcontent::generators::basic::basic_standards_processor::{
    BasicStandardsProcessor, BasicStandardsProcessorImpl,
};
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::work_generators::{WorkGenerator, WorkResults};
use std::any::Any;

/// The generator for simple non-flowed standards: work is scheduled in whole
/// blocks at each end of the shift rather than spread across it.
pub struct BasicWorkGenerator {
    processor: Box<dyn BasicStandardsProcessor>,
    planned_shift_creator: Box<dyn BasicPlannedShiftCreator>,
}

impl BasicWorkGenerator {
    pub fn new() -> Self {
        Self {
            processor: Box::new(BasicStandardsProcessorImpl::new()),
            planned_shift_creator: Box::new(BasicPlannedShiftCreatorImpl::new()),
        }
    }
}

impl WorkGenerator for BasicWorkGenerator {
    fn generate_work(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<WorkResults, GenerationError> {
        let work_contents = self.processor.process(planner_model, job)?;
        let planned_shifts = self
            .planned_shift_creator
            .create_planned_shifts_from_work_contents(&work_contents);

        Ok(WorkResults::with_work_content(
            job.id(),
            work_contents,
            planned_shifts,
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
