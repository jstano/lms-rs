//! Port of `com.unifocus.watson.server.hibernate.entity.Assignment`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Assignment.java`
//! (1,133 lines). A job, or any node of the labor-structure tree — Java uses the same class for
//! both, and `Assignment.getJob()` on a leaf assignment returns itself. Only the fields
//! `process/comparators/`, `process/plannedshiftsorters/`, `process/checkers/`, and
//! `engine/model/` read are here; see `entity` module docs and `DATA_MODEL.md` §3.
//!
//! `new` takes the fields every caller needs; the rest are optional builder methods
//! (`with_*`, following `workrules::entity::assignment::Assignment::with_pay_rates`) so adding
//! another narrow slice later doesn't grow an already-long positional constructor.

use crate::entity::assignment_sort_order::AssignmentSortOrder;
use crate::entity::planned_shift_sorting_method::PlannedShiftSortingMethod;
use crate::entity::projected_hours_reduction_method::ProjectedHoursReductionMethod;
use crate::entity::rotation_plan::RotationPlan;
use joda_rs::DayOfWeek;

/// A job / labor-structure node. `Assignment`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    id: i32,
    full_name: String,
    is_balance_schedules: bool,
    min_hours_off: Option<f64>,
    min_days_off: Option<i32>,
    parent_assignment_id: Option<i32>,
    is_departmental_seniority: bool,
    sort_order: Vec<AssignmentSortOrder>,
    day_of_week_order: Vec<DayOfWeek>,
    planned_shift_sorting_method: Option<PlannedShiftSortingMethod>,
    rotation_plan: Option<RotationPlan>,
    job_rotation_plan: Option<RotationPlan>,
    projected_hours_reduction_method: Option<ProjectedHoursReductionMethod>,
}

impl Assignment {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        full_name: impl Into<String>,
        is_balance_schedules: bool,
        min_hours_off: Option<f64>,
        min_days_off: Option<i32>,
        parent_assignment_id: Option<i32>,
        is_departmental_seniority: bool,
        sort_order: Vec<AssignmentSortOrder>,
        day_of_week_order: Vec<DayOfWeek>,
        planned_shift_sorting_method: Option<PlannedShiftSortingMethod>,
    ) -> Self {
        Self {
            id,
            full_name: full_name.into(),
            is_balance_schedules,
            min_hours_off,
            min_days_off,
            parent_assignment_id,
            is_departmental_seniority,
            sort_order,
            day_of_week_order,
            planned_shift_sorting_method,
            rotation_plan: None,
            job_rotation_plan: None,
            projected_hours_reduction_method: None,
        }
    }

    /// `setRotationPlan(RotationPlan)` — the assignment's own rotation plan, walked up the
    /// parent chain by `EmployeeAssignmentRotationChecker`.
    #[must_use]
    pub fn with_rotation_plan(mut self, rotation_plan: RotationPlan) -> Self {
        self.rotation_plan = Some(rotation_plan);
        self
    }

    /// `setJobRotationPlan(RotationPlan)` — read directly by `EmployeeJobRotationChecker`, no
    /// chain walk.
    #[must_use]
    pub fn with_job_rotation_plan(mut self, job_rotation_plan: RotationPlan) -> Self {
        self.job_rotation_plan = Some(job_rotation_plan);
        self
    }

    /// `setProjectedHoursReductionMethod(ProjectedHoursReductionMethod)`.
    #[must_use]
    pub fn with_projected_hours_reduction_method(
        mut self,
        projected_hours_reduction_method: ProjectedHoursReductionMethod,
    ) -> Self {
        self.projected_hours_reduction_method = Some(projected_hours_reduction_method);
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getFullName()`. Used for the case-insensitive job-name sort in
    /// `JobList.getAllJobs()`/`ScheduleModel.getJobScheduleLogs()`.
    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    /// `isBalanceSchedules()`.
    pub fn is_balance_schedules(&self) -> bool {
        self.is_balance_schedules
    }

    /// `getMinHoursOff()` — `null` in Java means "no override at this level", walked up the
    /// parent-assignment chain by the caller (`EmployeeData.getMinHoursOff`).
    pub fn min_hours_off(&self) -> Option<f64> {
        self.min_hours_off
    }

    /// `getMinDaysOff()` — same nullable-override shape as `min_hours_off`.
    pub fn min_days_off(&self) -> Option<i32> {
        self.min_days_off
    }

    /// `getParentAssignment().getID()`.
    pub fn parent_assignment_id(&self) -> Option<i32> {
        self.parent_assignment_id
    }

    /// `isDepartmentalSeniority()`.
    pub fn is_departmental_seniority(&self) -> bool {
        self.is_departmental_seniority
    }

    /// `getSortOrder()` — the ordered tie-break chain `EmployeeSeniorityComparator` walks.
    pub fn sort_order(&self) -> &[AssignmentSortOrder] {
        &self.sort_order
    }

    /// `getDayOfWeekOrder()` — the day-of-week priority order `DayOfWeekPlannedShiftSorter`
    /// sorts a job's planned-shift dates by.
    pub fn day_of_week_order(&self) -> &[DayOfWeek] {
        &self.day_of_week_order
    }

    /// `getPlannedShiftSortingMethod()` — `null` in Java falls back to the default sorter.
    pub fn planned_shift_sorting_method(&self) -> Option<PlannedShiftSortingMethod> {
        self.planned_shift_sorting_method
    }

    /// `getRotationPlan()`.
    pub fn rotation_plan(&self) -> Option<RotationPlan> {
        self.rotation_plan
    }

    /// `getJobRotationPlan()`.
    pub fn job_rotation_plan(&self) -> Option<RotationPlan> {
        self.job_rotation_plan
    }

    /// `getProjectedHoursReductionMethod()`.
    pub fn projected_hours_reduction_method(&self) -> Option<ProjectedHoursReductionMethod> {
        self.projected_hours_reduction_method
    }
}
