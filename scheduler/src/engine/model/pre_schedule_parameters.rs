//! Port of `com.unifocus.watson.server.scheduler.engine.model.PreScheduleParameters`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! PreScheduleParameters.java`. A plain immutable pair; see `DATA_MODEL.md` §4.

/// `PreScheduleParameters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreScheduleParameters {
    order_no: i32,
    group_no: i32,
}

impl PreScheduleParameters {
    pub fn new(order_no: i32, group_no: i32) -> Self {
        Self { order_no, group_no }
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
