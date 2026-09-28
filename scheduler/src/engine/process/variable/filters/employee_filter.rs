//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.filters.EmployeeFilter`.
//!
//! Ground truth: `taps/.../process/variable/filters/EmployeeFilter.java`.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::planned_shift::PlannedShift;

/// A plain-value stand-in for Java's identity-keyed `EmployeeFilter` instances (see
/// `engine::model::logging::job_schedule_log`'s `employeeFilterScheduleLogMap`, `PARITY_AUDIT.md`
/// finding 5). Each concrete `EmployeeFilter` maps to one variant; `JobLevel` carries the level
/// since Java constructs one `JobLevelEmployeeFilter` bean per job level (all sharing the name
/// `"Default"`, so `name()` alone can't distinguish them the way this key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmployeeFilterKey {
    ContractFtHomeJobOnly,
    ContractFtSecondaryJob,
    ContractPtHomeJobOnly,
    ContractPtSecondaryJob,
    SalariedHomeJobOnly,
    SalariedSecondaryJob,
    JobLevel(i32),
}

/// `EmployeeFilter`. None of the implementations read or mutate `ScheduleModel`/`EmployeeData`
/// through anything other than plain getters, so unlike `CanWorkChecker` neither reference needs
/// to be `&mut` here.
pub trait EmployeeFilter {
    /// `getName()`.
    fn name(&self) -> &str;

    /// Identity stand-in for Java's reference-equality/hashCode `HashMap` key use (no Java
    /// equivalent method — see [`EmployeeFilterKey`]'s doc).
    fn key(&self) -> EmployeeFilterKey;

    /// `includeEmployee(ScheduleModel, EmployeeData, PlannedShift)`.
    fn include_employee(
        &self,
        schedule_model: &ScheduleModel,
        employee_data: &EmployeeData,
        planned_shift: &PlannedShift,
    ) -> bool;
}
