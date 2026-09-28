//! Port of `com.unifocus.watson.server.scheduler.engine.process`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/`.
//! `PreScheduleProcess` (step 6), `PermanentScheduleProcess`/`RegularScheduleProcess` (steps
//! 7-8), and `VariableScheduleProcess` (step 9) are all done. `SaveSchedulesService` (step 10)
//! and `ScheduleEngine` itself (Phase 3) remain.

pub mod checkers;
pub mod comparators;
pub mod permanent_schedule_process;
pub mod plannedshiftsorters;
pub mod ports;
pub mod pre_schedule_process;
pub mod projectedhoursreducers;
pub mod regular_schedule_process;
pub mod regularschedules;
pub mod schedulebalancers;
pub mod variable;
pub mod variable_schedule_process;
