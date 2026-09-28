//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.generators`.
//!
//! Ground truth: `taps/.../process/variable/generators/`.

pub mod abstract_variable_scheduling_generator;
pub mod variable_by_employee_set_generator;
pub mod variable_by_job_schedule_order_generator;
pub mod variable_by_seniority_generator;
pub mod variable_scheduling_generator;

pub use variable_by_employee_set_generator::VariableByEmployeeSetGenerator;
pub use variable_by_job_schedule_order_generator::VariableByJobScheduleOrderGenerator;
pub use variable_by_seniority_generator::VariableBySeniorityGenerator;
pub use variable_scheduling_generator::VariableSchedulingGenerator;
