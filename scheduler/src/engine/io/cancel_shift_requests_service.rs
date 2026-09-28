//! Port of `com.unifocus.watson.server.scheduler.engine.io.CancelShiftRequestsService`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! CancelShiftRequestsService.java`. `cancelShiftRequests`'s entire body is commented out in the
//! real Java source (a `//TODO: need to implement this correctly` block) — the method is a
//! genuine no-op in production today, not a gap this port introduced. Ported faithfully as a
//! real no-op rather than a stub awaiting implementation.

use crate::engine::model::schedule_model::ScheduleModel;

/// `CancelShiftRequestsService`.
pub struct CancelShiftRequestsService;

impl CancelShiftRequestsService {
    /// `cancelShiftRequests(ScheduleModel)` — no-op; see module doc.
    pub fn cancel_shift_requests(&self, _schedule_model: &ScheduleModel) {}
}
