// Ported from taps/src/java/com/unifocus/watson/server/scheduler/autosched/.
// Phase 3 (final wave, layered on top of the engine).
//
// `AvailPeriod` was already ported at `entity::avail_period` (a direct dependency of Phase 1's
// `EmployeeAvailabilityChecker`, long before this wave). `ScheduleEmployee` is not ported — dead
// code in `taps` (nothing in the entire checkout, not even the other five `autosched` files,
// constructs or references it).

pub mod exceeds_available_hours_conflict_validator;
pub mod ports;
pub mod schedule_checker;
pub mod schedule_hours_distribution_validator;
pub mod schedule_matcher;
