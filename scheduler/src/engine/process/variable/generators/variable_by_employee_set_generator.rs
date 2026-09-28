//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators.
//! VariableByEmployeeSetGenerator`.
//!
//! Ground truth: `taps/.../process/variable/generators/VariableByEmployeeSetGenerator.java`.
//! `runPhases`'s inner loop is the same "sort shifts, skip if it would exceed projected hours,
//! log, schedule" shape as `VariableJobScheduler::schedule_job` — inlined here rather than
//! factored out into a third shared helper, matching how Java itself keeps this as its own
//! private method rather than reusing `VariableJobScheduler`.

use crate::engine::model::logging::planned_shift_log::PlannedShiftLog;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::plannedshiftsorters::planned_shift_sorter_factory::PlannedShiftSorterFactory;
use crate::engine::process::variable::filters::{
    ContractFtHomeJobOnlyEmployeeFilter, ContractFtSecondaryJobEmployeeFilter,
    ContractPtHomeJobOnlyEmployeeFilter, ContractPtSecondaryJobEmployeeFilter, EmployeeFilter,
    SalariedHomeJobOnlyEmployeeFilter, SalariedSecondaryJobEmployeeFilter,
};
use crate::engine::process::variable::generators::variable_scheduling_generator::VariableSchedulingGenerator;
use crate::engine::process::variable::planned_shift_scheduler::PlannedShiftScheduler;

/// `VariableByEmployeeSetGenerator`.
pub struct VariableByEmployeeSetGenerator<'a> {
    planned_shift_scheduler: &'a PlannedShiftScheduler<'a>,
}

impl<'a> VariableByEmployeeSetGenerator<'a> {
    pub fn new(planned_shift_scheduler: &'a PlannedShiftScheduler<'a>) -> Self {
        Self {
            planned_shift_scheduler,
        }
    }

    /// `getEmployeeFilters()` — fixed order: home-job filters, then secondary-job filters.
    fn employee_filters(&self) -> Vec<Box<dyn EmployeeFilter>> {
        vec![
            Box::new(SalariedHomeJobOnlyEmployeeFilter),
            Box::new(ContractFtHomeJobOnlyEmployeeFilter),
            Box::new(ContractPtHomeJobOnlyEmployeeFilter),
            Box::new(SalariedSecondaryJobEmployeeFilter),
            Box::new(ContractFtSecondaryJobEmployeeFilter),
            Box::new(ContractPtSecondaryJobEmployeeFilter),
        ]
    }

    /// `runPhases(ScheduleModel, JobData)`.
    fn run_phases(&self, schedule_model: &mut ScheduleModel, job_id: i32) {
        for employee_filter in self.employee_filters() {
            let Some(job_data) = schedule_model
                .job_list()
                .and_then(|job_list| job_list.job_data(job_id))
            else {
                return;
            };

            let planned_shifts = PlannedShiftSorterFactory::planned_shift_sorter(job_data)
                .sort_planned_shifts(schedule_model, job_data);

            for planned_shift in planned_shifts {
                let exceeds = schedule_model
                    .job_list()
                    .and_then(|job_list| job_list.job_data(job_id))
                    .is_none_or(|job_data| {
                        job_data.will_scheduled_hours_exceed_projected_hours(
                            planned_shift.shift_date(),
                            planned_shift.duration(),
                        )
                    });

                if exceeds {
                    continue;
                }

                let planned_shift_log = PlannedShiftLog::new(planned_shift);

                schedule_model
                    .job_schedule_log(job_id)
                    .schedule_log(clone_filter(employee_filter.as_ref()))
                    .add_planned_shift_log(planned_shift_log.clone());

                schedule_model.set_current_planned_shift_log(Some(planned_shift_log));

                self.planned_shift_scheduler.schedule_planned_shift(
                    schedule_model,
                    planned_shift,
                    employee_filter.as_ref(),
                );
            }
        }
    }
}

impl VariableSchedulingGenerator for VariableByEmployeeSetGenerator<'_> {
    /// `generateSchedules(ScheduleModel, JobData)`.
    fn generate_schedules(&self, schedule_model: &mut ScheduleModel, job_id: i32) {
        if let Some(job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.job_data_mut(job_id))
        {
            job_data.reset_balance_level();
        }
        self.run_phases(schedule_model, job_id);

        if let Some(job_data) = schedule_model
            .job_list_mut()
            .and_then(|job_list| job_list.job_data_mut(job_id))
        {
            job_data.increment_balance_level();
        }
        self.run_phases(schedule_model, job_id);
    }
}

/// All six filters this generator uses are zero-sized unit structs, so re-`Box`ing a fresh one
/// from its [`EmployeeFilterKey`](crate::engine::process::variable::filters::EmployeeFilterKey)
/// is exactly as cheap as cloning, and keeps `JobScheduleLog::schedule_log`'s
/// `Box<dyn EmployeeFilter>` parameter uniform across every caller in this wave (no `Clone` bound
/// added to the trait just for this one call site).
fn clone_filter(employee_filter: &dyn EmployeeFilter) -> Box<dyn EmployeeFilter> {
    use crate::engine::process::variable::filters::EmployeeFilterKey;

    match employee_filter.key() {
        EmployeeFilterKey::SalariedHomeJobOnly => Box::new(SalariedHomeJobOnlyEmployeeFilter),
        EmployeeFilterKey::ContractFtHomeJobOnly => Box::new(ContractFtHomeJobOnlyEmployeeFilter),
        EmployeeFilterKey::ContractPtHomeJobOnly => Box::new(ContractPtHomeJobOnlyEmployeeFilter),
        EmployeeFilterKey::SalariedSecondaryJob => Box::new(SalariedSecondaryJobEmployeeFilter),
        EmployeeFilterKey::ContractFtSecondaryJob => Box::new(ContractFtSecondaryJobEmployeeFilter),
        EmployeeFilterKey::ContractPtSecondaryJob => Box::new(ContractPtSecondaryJobEmployeeFilter),
        EmployeeFilterKey::JobLevel(job_level) => Box::new(
            crate::engine::process::variable::filters::JobLevelEmployeeFilter::new(job_level),
        ),
    }
}
