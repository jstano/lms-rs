//! Port of `com.unifocus.watson.server.hibernate.entity.RotationPlan`.
//!
//! Ground truth not read directly — only the fields `process/checkers/{
//! EmployeeAssignmentRotationChecker,EmployeeJobRotationChecker},
//! process/checkers/rotationplans::{RotationPlanCheckerFactory,RotationPlanUtils,
//! DailyRotationPlanCheckerProcess,WeeklyRotationPlanCheckerProcess}` read.

/// A job/assignment rotation rule. `RotationPlan`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationPlan {
    rotate_interval: i32,
    is_rotate_every_day: bool,
    apply_to_regular_employees: bool,
    apply_to_permanent_employees: bool,
}

impl RotationPlan {
    pub fn new(
        rotate_interval: i32,
        is_rotate_every_day: bool,
        apply_to_regular_employees: bool,
        apply_to_permanent_employees: bool,
    ) -> Self {
        Self {
            rotate_interval,
            is_rotate_every_day,
            apply_to_regular_employees,
            apply_to_permanent_employees,
        }
    }

    /// `getRotateInterval()`.
    pub fn rotate_interval(&self) -> i32 {
        self.rotate_interval
    }

    /// `isRotateEveryDay()`.
    pub fn is_rotate_every_day(&self) -> bool {
        self.is_rotate_every_day
    }

    /// `isApplyToRegularEmployees()`.
    pub fn apply_to_regular_employees(&self) -> bool {
        self.apply_to_regular_employees
    }

    /// `isApplyToPermanentEmployees()`.
    pub fn apply_to_permanent_employees(&self) -> bool {
        self.apply_to_permanent_employees
    }
}
