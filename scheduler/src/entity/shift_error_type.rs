//! Port of `com.unifocus.watson.common.enums.ShiftErrorType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/ShiftErrorType.java`. Every
//! variant is modeled for fidelity (`EmployeeShift.getErrors()` can carry any of them), but only
//! `resource_key()` is ported off each — `toString()` calls `ResourceMgr.lookup(resourceKey)` to
//! translate it, and this crate doesn't model localization anywhere (see `ScheduleChecker`'s own
//! doc): callers that in Java would get the translated message get the bare resource key instead,
//! same as every `ResourceMgr.lookup(...)` call site `autosched` ports. `getCode()`/`fromCode`/
//! `isFixableFromTimeclock`/`isPunchError` aren't read by anything ported.

/// `ShiftErrorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShiftErrorType {
    MinHoursOff,
    MinDaysOff,
    MinShiftLength,
    MaxShiftLength,
    TimeOff,
    Availability,
    AvailableHours,
    EmptyShift,
    MissingIn,
    MissingOut,
    MissingBreakIn,
    MissingBreakOut,
    UnbalancedBreak,
    InOutSame,
    InvalidTimes,
    RoundNextDayInactiveEmp,
    RoundNextDayInactiveJob,
    RequiredDaysOff,
    ShiftDefinition,
    NoHomeJob,
    JobNotActive,
    NotActive,
    DifferentPaygroups,
    PaygroupNotActive,
    MaxHoursOnDay,
    MaxHoursPerWeek,
    EarliestStartLatestEnd,
    MaxDaysWorkedPerWeek,
}

impl ShiftErrorType {
    /// `getResourceKey()`.
    pub fn resource_key(&self) -> &'static str {
        match self {
            Self::MinHoursOff => "res_minHourOffViolation",
            Self::MinDaysOff => "res_minDaysOffViolation",
            Self::MinShiftLength => "res_minShiftLengthViolation",
            Self::MaxShiftLength => "res_maxShiftLengthViolation",
            Self::TimeOff => "res_timeOffRequestConflict",
            Self::Availability => "res_availabilityConflict",
            Self::AvailableHours => "res_exceedsAvailableHours",
            Self::EmptyShift => "res_emptyShift",
            Self::MissingIn => "res_missingIn",
            Self::MissingOut => "res_missingOut",
            Self::MissingBreakIn => "res_missingBreakIn",
            Self::MissingBreakOut => "res_missingBreakOut",
            Self::UnbalancedBreak => "res_unbalancedBreak",
            Self::InOutSame => "res_inOutSame",
            Self::InvalidTimes => "res_invalidPunchTimes",
            Self::RoundNextDayInactiveEmp => "res_employeeInactiveOnRoundedDate",
            Self::RoundNextDayInactiveJob => "res_jobInactiveOnRoundedDate",
            Self::RequiredDaysOff => "res_minRequiredDaysOff",
            Self::ShiftDefinition => "res_shiftDefinitionConflict",
            Self::NoHomeJob => "res_noHomeJob",
            Self::JobNotActive => "res_jobViolation",
            Self::NotActive => "res_employeeIsNotActive",
            Self::DifferentPaygroups => "res_differentPayGroups",
            Self::PaygroupNotActive => "res_payGroupNotActive",
            Self::MaxHoursOnDay => "res_maxHoursOnDayExceeded",
            Self::MaxHoursPerWeek => "res_maxHoursPerWeekExceeded",
            Self::EarliestStartLatestEnd => "res_earliestStartLatestEndTimeShiftError",
            Self::MaxDaysWorkedPerWeek => "res_maxDaysWorkedPerWeekExceeded",
        }
    }
}
