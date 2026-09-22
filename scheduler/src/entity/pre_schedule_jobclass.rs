//! Port of `com.unifocus.watson.server.hibernate.entity.PreScheduleJobclass`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/
//! PreScheduleJobclass.java`. Only the three fields `PreScheduleJobLoader` reads — `job` is
//! flattened to the job's id rather than a nested `Assignment` (see `entity` module docs).
//! `property` isn't carried: `findAllForProperty` uses it only as a query filter, never read off
//! the returned rows.

/// `PreScheduleJobclass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreScheduleJobclass {
    job_id: i32,
    order_no: i32,
    group_no: i32,
}

impl PreScheduleJobclass {
    pub fn new(job_id: i32, order_no: i32, group_no: i32) -> Self {
        Self {
            job_id,
            order_no,
            group_no,
        }
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getOrderNo()`.
    pub fn order_no(&self) -> i32 {
        self.order_no
    }

    /// `getGroupNo()`.
    pub fn group_no(&self) -> i32 {
        self.group_no
    }
}
