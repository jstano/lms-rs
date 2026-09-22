//! Port of `com.unifocus.watson.common.labor.planner.{GenerateSchedulesParameters,
//! GeneratorParameters}`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/planner/
//! {GenerateSchedulesParameters,GeneratorParameters}.java`. The engine's top-level input; gets
//! its own flat Rust struct rather than mirroring the Java base/subclass split or sharing a type
//! with `planner` — decided in `PLAN_SCHEDULER.md`, cataloged in `DATA_MODEL.md` §6. Lives at
//! `engine::generate_schedules_parameters` rather than under `engine::model` (which mirrors Java's
//! `engine.model` package 1:1) since this type's ground truth is a different Java package
//! entirely.
//!
//! All seven `bool`/id fields default `false`/`0` in Java (primitive defaults, no explicit
//! initialization); `job_ids` defaults to an empty list (`setJobIDs(null)` resets to empty, never
//! null). `date_range`/`property_id` are the only fields every caller must supply, so they're the
//! only `new` parameters — the rest are `with_*` builder methods, following
//! `entity::assignment::Assignment`'s precedent for optional-ish fields.

use date_range_rs::DateRange;

/// `GenerateSchedulesParameters`, flattened with its `GeneratorParameters` base.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateSchedulesParameters {
    date_range: DateRange,
    property_id: i32,
    division_id: i32,
    department_id: i32,
    job_id: i32,
    job_ids: Vec<i32>,
    clear_schedules: bool,
    generate_pre_schedules: bool,
    generate_permanent_schedules: bool,
    generate_regular_schedules: bool,
    generate_variable_schedules: bool,
    rotate_days_off: bool,
}

impl GenerateSchedulesParameters {
    pub fn new(date_range: DateRange, property_id: i32) -> Self {
        Self {
            date_range,
            property_id,
            division_id: 0,
            department_id: 0,
            job_id: 0,
            job_ids: Vec::new(),
            clear_schedules: false,
            generate_pre_schedules: false,
            generate_permanent_schedules: false,
            generate_regular_schedules: false,
            generate_variable_schedules: false,
            rotate_days_off: false,
        }
    }

    /// `setDivisionID(int)`.
    #[must_use]
    pub fn with_division_id(mut self, division_id: i32) -> Self {
        self.division_id = division_id;
        self
    }

    /// `setDepartmentID(int)`.
    #[must_use]
    pub fn with_department_id(mut self, department_id: i32) -> Self {
        self.department_id = department_id;
        self
    }

    /// `setJobID(int)`.
    #[must_use]
    pub fn with_job_id(mut self, job_id: i32) -> Self {
        self.job_id = job_id;
        self
    }

    /// `setJobIDs(List<Integer>)`.
    #[must_use]
    pub fn with_job_ids(mut self, job_ids: Vec<i32>) -> Self {
        self.job_ids = job_ids;
        self
    }

    /// `setClearSchedules(boolean)`.
    #[must_use]
    pub fn with_clear_schedules(mut self, clear_schedules: bool) -> Self {
        self.clear_schedules = clear_schedules;
        self
    }

    /// `setGeneratePreSchedules(boolean)` — gates pipeline step 6.
    #[must_use]
    pub fn with_generate_pre_schedules(mut self, generate_pre_schedules: bool) -> Self {
        self.generate_pre_schedules = generate_pre_schedules;
        self
    }

    /// `setGeneratePermanentSchedules(boolean)` — gates step 7.
    #[must_use]
    pub fn with_generate_permanent_schedules(mut self, generate_permanent_schedules: bool) -> Self {
        self.generate_permanent_schedules = generate_permanent_schedules;
        self
    }

    /// `setGenerateRegularSchedules(boolean)` — gates step 8.
    #[must_use]
    pub fn with_generate_regular_schedules(mut self, generate_regular_schedules: bool) -> Self {
        self.generate_regular_schedules = generate_regular_schedules;
        self
    }

    /// `setGenerateVariableSchedules(boolean)` — gates step 9.
    #[must_use]
    pub fn with_generate_variable_schedules(mut self, generate_variable_schedules: bool) -> Self {
        self.generate_variable_schedules = generate_variable_schedules;
        self
    }

    /// `setRotateDaysOff(boolean)` — gates step 2 (`isRotateDaysOff()`).
    #[must_use]
    pub fn with_rotate_days_off(mut self, rotate_days_off: bool) -> Self {
        self.rotate_days_off = rotate_days_off;
        self
    }

    /// `getDateRange()`.
    pub fn date_range(&self) -> &DateRange {
        &self.date_range
    }

    /// `getPropertyID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getDivisionID()`.
    pub fn division_id(&self) -> i32 {
        self.division_id
    }

    /// `getDepartmentID()`.
    pub fn department_id(&self) -> i32 {
        self.department_id
    }

    /// `getJobID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getJobIDs()`.
    pub fn job_ids(&self) -> &[i32] {
        &self.job_ids
    }

    /// `isClearSchedules()`.
    pub fn clear_schedules(&self) -> bool {
        self.clear_schedules
    }

    /// `isGeneratePreSchedules()`.
    pub fn generate_pre_schedules(&self) -> bool {
        self.generate_pre_schedules
    }

    /// `isGeneratePermanentSchedules()`.
    pub fn generate_permanent_schedules(&self) -> bool {
        self.generate_permanent_schedules
    }

    /// `isGenerateRegularSchedules()`.
    pub fn generate_regular_schedules(&self) -> bool {
        self.generate_regular_schedules
    }

    /// `isGenerateVariableSchedules()`.
    pub fn generate_variable_schedules(&self) -> bool {
        self.generate_variable_schedules
    }

    /// `isRotateDaysOff()`.
    pub fn rotate_days_off(&self) -> bool {
        self.rotate_days_off
    }
}
