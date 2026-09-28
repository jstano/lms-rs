//! Port of `com.unifocus.watson.server.scheduler.engine.model.logging.JobScheduleLog`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/logging/
//! JobScheduleLog.java`. Blocked since Phase 1 (`PARITY_AUDIT.md` finding 3), unblocked the same
//! way as `ScheduleLog` (see its doc).
//!
//! `job_data` is stored as a plain `i32` job id, not an owned `JobData`, matching the
//! `entity`/`engine::model` convention of flattening back-references to ids rather than carrying
//! a copy that could go stale (`JobData` is mutated in place by the scheduling pipeline this log
//! belongs to). `ScheduleModel` (the one real caller) already has the `JobData` on hand when it
//! creates one, so this loses nothing — see `job_schedule_log`'s constructor.

use crate::engine::model::logging::schedule_log::ScheduleLog;
use crate::engine::process::variable::filters::{EmployeeFilter, EmployeeFilterKey};
use std::collections::HashMap;

/// `JobScheduleLog`.
pub struct JobScheduleLog {
    job_id: i32,
    employee_filter_keys: Vec<EmployeeFilterKey>,
    employee_filter_schedule_log_map: HashMap<EmployeeFilterKey, ScheduleLog>,
}

impl std::fmt::Debug for JobScheduleLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobScheduleLog")
            .field("job_id", &self.job_id)
            .field("employee_filter_keys", &self.employee_filter_keys)
            .finish_non_exhaustive()
    }
}

impl JobScheduleLog {
    pub fn new(job_id: i32) -> Self {
        Self {
            job_id,
            employee_filter_keys: Vec::new(),
            employee_filter_schedule_log_map: HashMap::new(),
        }
    }

    /// `getJobData()` — flattened to the id; see module doc.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getScheduleLog(EmployeeFilter)`.
    pub fn schedule_log(&mut self, employee_filter: Box<dyn EmployeeFilter>) -> &mut ScheduleLog {
        let key = employee_filter.key();

        if !self.employee_filter_schedule_log_map.contains_key(&key) {
            self.employee_filter_keys.push(key);
            self.employee_filter_schedule_log_map
                .insert(key, ScheduleLog::new(employee_filter));
        }

        self.employee_filter_schedule_log_map
            .get_mut(&key)
            .expect("just inserted or already present")
    }

    /// `getEmployeeFilters()` — returns the keys, not `Box<dyn EmployeeFilter>` trait objects
    /// (nothing ported reads the filters back through this list; `ScheduleLog::employee_filter`
    /// covers the one real reader).
    pub fn employee_filter_keys(&self) -> &[EmployeeFilterKey] {
        &self.employee_filter_keys
    }

    /// `getScheduleLog(EmployeeFilter)` — read-only lookup by key, unlike
    /// [`schedule_log`](Self::schedule_log) doesn't create one if missing.
    /// `SaveScheduleLogService` (step 10), the first reader that never needs to create: it always
    /// iterates `employee_filter_keys()` first, so every key it looks up here is guaranteed
    /// present.
    pub fn schedule_log_for_key(&self, key: EmployeeFilterKey) -> Option<&ScheduleLog> {
        self.employee_filter_schedule_log_map.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::process::variable::filters::SalariedHomeJobOnlyEmployeeFilter;

    #[test]
    fn schedule_log_is_created_once_per_filter_key() {
        let mut job_schedule_log = JobScheduleLog::new(500);

        job_schedule_log.schedule_log(Box::new(SalariedHomeJobOnlyEmployeeFilter));
        job_schedule_log.schedule_log(Box::new(SalariedHomeJobOnlyEmployeeFilter));

        assert_eq!(job_schedule_log.employee_filter_keys().len(), 1);
    }
}
