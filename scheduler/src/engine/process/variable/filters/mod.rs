//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.filters`.
//!
//! Ground truth: `taps/.../process/variable/filters/`.

pub mod contract_ft_home_job_only_employee_filter;
pub mod contract_ft_secondary_job_employee_filter;
pub mod contract_pt_home_job_only_employee_filter;
pub mod contract_pt_secondary_job_employee_filter;
pub mod employee_filter;
pub mod job_level_employee_filter;
pub mod salaried_home_job_only_employee_filter;
pub mod salaried_secondary_job_employee_filter;

pub use contract_ft_home_job_only_employee_filter::ContractFtHomeJobOnlyEmployeeFilter;
pub use contract_ft_secondary_job_employee_filter::ContractFtSecondaryJobEmployeeFilter;
pub use contract_pt_home_job_only_employee_filter::ContractPtHomeJobOnlyEmployeeFilter;
pub use contract_pt_secondary_job_employee_filter::ContractPtSecondaryJobEmployeeFilter;
pub use employee_filter::{EmployeeFilter, EmployeeFilterKey};
pub use job_level_employee_filter::JobLevelEmployeeFilter;
pub use salaried_home_job_only_employee_filter::SalariedHomeJobOnlyEmployeeFilter;
pub use salaried_secondary_job_employee_filter::SalariedSecondaryJobEmployeeFilter;
