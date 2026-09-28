//! Port of `com.unifocus.watson.common.enums.PropertyDataKey`, narrowed to the two tags
//! `SaveSchedulesService` reads (`systemScheduleAudit`/`systemPlannedShiftAudit`) — the real enum
//! has dozens of unrelated feature-flag entries nothing in `scheduler` touches.

/// `PropertyDataKey`, narrowed to `SaveSchedulesService`'s two tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyDataKeyTag {
    SystemScheduleAudit,
    SystemPlannedShiftAudit,
}
