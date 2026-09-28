// Ported from taps/src/java/com/unifocus/watson/server/scheduler/engine/model/. See
// DATA_MODEL.md §4. RegularSchedules (Phase 2 steps 7-8) is done now that
// process/regularschedules::EmployeeRegularPeriodComparator exists.

pub mod employee_data;
pub mod employee_list;
pub mod hours_by_date;
pub mod job_data;
pub mod job_list;
pub mod logging;
pub mod pre_schedule_parameters;
pub mod regular_schedule;
pub mod regular_schedules;
pub mod schedule_model;
pub mod schedules;
pub mod shift_list;
pub mod weekly_available_hours;
