use crate::workcontent::domain::job::{Job, JobId};
use crate::workcontent::domain::labor_data::LaborData;
use crate::workcontent::domain::planned_shift::PlannedShift;
use crate::workcontent::domain::planner_model::PlannerModel;
use crate::workcontent::domain::standard_type::StandardType;
use crate::workcontent::domain::work_content::WorkContent;
use crate::workcontent::generators::advanced::advanced::AdvancedWorkGenerator;
use crate::workcontent::generators::basic::basic::BasicWorkGenerator;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::none::none::NoneWorkGenerator;
use crate::workcontent::generators::salaried::salaried_work_generator::SalariedWorkGenerator;
use std::any::Any;

/// What one generator produced for one job.
///
/// The two shapes are exclusive: the hourly generators plan blocks of work and
/// the shifts covering them, while the salaried generator only records the
/// hours owed per day.
pub struct WorkResults {
    job_id: JobId,
    work_content: Option<Vec<WorkContent>>,
    planned_shifts: Option<Vec<PlannedShift>>,
    labor_data: Option<Vec<LaborData>>,
}

impl WorkResults {
    pub fn with_work_content(
        job_id: JobId,
        work_content: Vec<WorkContent>,
        planned_shifts: Vec<PlannedShift>,
    ) -> Self {
        Self {
            job_id,
            work_content: Some(work_content),
            planned_shifts: Some(planned_shifts),
            labor_data: None,
        }
    }

    pub fn with_labor_data(job_id: JobId, labor_data: Vec<LaborData>) -> Self {
        Self {
            job_id,
            work_content: None,
            planned_shifts: None,
            labor_data: Some(labor_data),
        }
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn work_content(&self) -> Option<&Vec<WorkContent>> {
        self.work_content.as_ref()
    }

    pub fn planned_shifts(&self) -> Option<&Vec<PlannedShift>> {
        self.planned_shifts.as_ref()
    }

    pub fn labor_data(&self) -> Option<&Vec<LaborData>> {
        self.labor_data.as_ref()
    }
}

pub trait WorkGenerator: Any {
    /// Plan one job's work.
    ///
    /// Fallible because the flowed generator can meet configuration it cannot
    /// plan — task standards, the unimplemented fill-gaps distribution — and
    /// those must surface rather than silently yield no work.
    fn generate_work(
        &self,
        planner_model: &PlannerModel,
        job: &Job,
    ) -> Result<WorkResults, GenerationError>;

    fn as_any(&self) -> &dyn Any;
}

pub fn create(standard_type: StandardType) -> Box<dyn WorkGenerator> {
    match standard_type {
        StandardType::NONE => Box::new(NoneWorkGenerator::new()),
        StandardType::BASIC => Box::new(BasicWorkGenerator::new()),
        StandardType::ADVANCED => Box::new(AdvancedWorkGenerator::new()),
        StandardType::SALARIED => Box::new(SalariedWorkGenerator::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::job::JobId;
    use crate::workcontent::domain::labor_data::LaborData;
    use crate::workcontent::domain::planned_shift::PlannedShift;
    use joda_rs::LocalDate;
    use rstest::rstest;
    use std::any::Any;

    #[test]
    fn should_be_able_to_create_work_results_with_work_content() {
        let job_id = JobId::new();
        let planned_shift = PlannedShift::new();
        let work_results =
            WorkResults::with_work_content(job_id, Vec::new(), vec![planned_shift.clone()]);

        assert_eq!(work_results.job_id(), job_id);
        assert!(work_results.work_content().unwrap().is_empty());
        assert_eq!(work_results.planned_shifts().unwrap().len(), 1);
        assert_eq!(work_results.planned_shifts().unwrap()[0], planned_shift);
        assert!(work_results.labor_data().is_none());
    }

    #[test]
    fn should_be_able_to_create_work_results_with_labor_data() {
        let job_id = JobId::new();
        let labor_data = LaborData::new(JobId::new(), LocalDate::new(2025, 10, 6), 123.45);
        let work_results = WorkResults::with_labor_data(job_id, vec![labor_data]);

        assert_eq!(work_results.job_id(), job_id);
        assert_eq!(work_results.labor_data().unwrap().len(), 1);
        assert_eq!(work_results.labor_data().unwrap()[0], labor_data);
        assert!(work_results.work_content().is_none());
        assert!(work_results.planned_shifts().is_none());
    }

    #[rstest]
    #[case(StandardType::NONE, |g: &dyn Any| g.is::<NoneWorkGenerator>())]
    #[case(StandardType::BASIC, |g: &dyn Any| g.is::<BasicWorkGenerator>())]
    #[case(StandardType::ADVANCED, |g: &dyn Any| g.is::<AdvancedWorkGenerator>())]
    #[case(StandardType::SALARIED, |g: &dyn Any| g.is::<SalariedWorkGenerator>())]
    fn create_should_return_the_correct_generator(
        #[case] standard_type: StandardType,
        #[case] type_check: fn(&dyn Any) -> bool,
    ) {
        let generator = create(standard_type);

        assert!(type_check(generator.as_any()));
    }
}
