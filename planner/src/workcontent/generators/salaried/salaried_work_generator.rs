use std::any::Any;
use crate::workcontent::domain::job::Job;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::work_generators::{WorkGenerator, WorkResults};
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::generators::salaried::salaried_standards_processor::{SalariedStandardsProcessor, SalariedStandardsProcessorImpl};

pub struct SalariedWorkGenerator {
    salaried_standard_processor: Box<dyn SalariedStandardsProcessor>,
}

impl SalariedWorkGenerator {
    pub fn new() -> Self {
        Self {
            salaried_standard_processor: Box::new(SalariedStandardsProcessorImpl::new())
        }
    }
}

impl WorkGenerator for SalariedWorkGenerator{
    fn generate_work(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<WorkResults, GenerationError> {
        // Salaried hours come straight from the contract; nothing on this path
        // can fail to plan.
        Ok(self.salaried_standard_processor.process(planner_model, job))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::type_name;

    // Type-level helper to require 'static lifetime
    fn require_static<T: 'static>() {}

    #[test]
    fn trait_object_is_object_safe() {
        // If the trait is not object-safe, this signature won't compile.
        fn takes_box(_: Box<dyn SalariedStandardsProcessor>) {}
        let _f: fn(Box<dyn SalariedStandardsProcessor>) = takes_box;
    }

    #[test]
    fn can_refer_to_generator_type() {
        // Just referencing the type ensures it is visible and concrete.
        let _name = type_name::<SalariedWorkGenerator>();
        assert!(!_name.is_empty());
    }

    #[test]
    fn generator_type_is_static() {
        // Ensures the type has 'static lifetime (common for service objects).
        require_static::<SalariedWorkGenerator>();
    }

    #[test]
    fn generator_type_can_be_boxed_and_optioned() {
        // We avoid constructing an instance since constructor/signature is not assumed.
        // These lines ensure the type is usable with common containers if a value exists.
        let _: Option<Box<SalariedWorkGenerator>> = None;
        let _: Option<SalariedWorkGenerator> = None;
    }
}
