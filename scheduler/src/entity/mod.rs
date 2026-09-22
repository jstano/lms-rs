//! The Hibernate entity model the engine reads, ported to plain owned structs.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/`. Those are large
//! JPA entities — `Assignment` is 1,133 lines, `Employee` far more — but the scheduling engine
//! reads a narrow slice of each. Only that slice comes across here, following `workrules`'s
//! precedent (`workrules/src/entity/`): back-references become plain `i32` ids (see
//! `DATA_MODEL.md` §2 — no `id_type!` uuid wrapper, these are legacy DB keys) instead of live
//! object references, so the entity graph here is one-way. Every field/method is added only when
//! a ported `scheduler` file actually reads it — see `PARITY_AUDIT.md` for what's driven each
//! addition.
//!
//! Two exceptions to "ids only" going in at once, both deliberate:
//! - [`EmployeeJobStatus`] stores `job_parent_assignment_id` directly rather than a nested
//!   `Assignment`, because the only thing ever read off `EmployeeJobStatus.getJob()` in the
//!   engine is the job's own id and *its* parent's id (`JobRankComparator`/
//!   `JobSeniorityDateComparator`'s department-seniority path) — carrying a full `Assignment`
//!   there would be an unused, ever-stale copy.
//! - [`RegularSchedule`] (in `engine::model`, not here) stores its job as `Option<i32>`, not an
//!   `Assignment`, for the same reason.

pub mod assignment;
pub mod assignment_sort_order;
pub mod avail_period;
pub mod availability;
pub mod day_off_pattern;
pub mod day_off_plan;
pub mod employee;
pub mod employee_assignment;
pub mod employee_calculation_mode;
pub mod employee_job_status;
pub mod employee_regular_period;
pub mod employee_shift;
pub mod employee_shift_request_detail;
pub mod employee_time_off;
pub mod employee_type;
pub mod jc_sort_order_type;
pub mod planned_shift;
pub mod planned_shift_sorting_method;
pub mod pre_schedule;
pub mod pre_schedule_jobclass;
pub mod projected_hours_reduction_method;
pub mod property;
pub mod rotation_plan;
pub mod schedule_calc_data_set;
pub mod schedule_mode;
pub mod work_class;
