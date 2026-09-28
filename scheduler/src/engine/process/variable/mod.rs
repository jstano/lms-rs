//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable`.
//!
//! Ground truth: `taps/.../process/variable/`. Phase 2, step 9.

pub mod comparators;
pub mod filters;
pub mod generators;
pub mod planned_shift_scheduler;
pub mod variable_can_work_checker;
pub mod variable_can_work_checker_factory;
pub mod variable_checker;
pub mod variable_job_scheduler;

pub use planned_shift_scheduler::PlannedShiftScheduler;
pub use variable_can_work_checker::VariableCanWorkChecker;
pub use variable_can_work_checker_factory::VariableCanWorkCheckerFactory;
pub use variable_checker::VariableChecker;
pub use variable_job_scheduler::VariableJobScheduler;

pub mod non_pre_scheduled_job_process;
pub mod pre_scheduled_job_process;
pub mod variable_job_scheduling_process;

pub use non_pre_scheduled_job_process::NonPreScheduledJobProcess;
pub use pre_scheduled_job_process::PreScheduledJobProcess;
pub use variable_job_scheduling_process::VariableJobSchedulingProcess;
