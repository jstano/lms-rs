//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.comparators`.
//!
//! Ground truth: `taps/.../process/variable/comparators/`.

pub mod non_pre_scheduled_job_comparator;
pub mod pre_scheduled_job_comparator;

pub use non_pre_scheduled_job_comparator::NonPreScheduledJobComparator;
pub use pre_scheduled_job_comparator::PreScheduledJobComparator;
