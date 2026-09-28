//! External subsystem boundaries `autosched` reaches into — same "narrow port, real logic around
//! it" treatment as `PARITY_AUDIT.md` findings 10/16.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/security/CurrentUser.java`,
//! `watson/server/labor/employee/EmployeeCertificationValidator.java`. `ScheduleChecker`'s calls to
//! `ScheduleRestrictionRuleChecker.runStrictRestrictions`/`runNonStrictRestrictions` go through
//! `engine::process::checkers::schedule_restriction_rule_checker` directly (the real Java class,
//! already ported for its `CanWorkChecker` method) rather than a port defined here — see that
//! module's doc for why those two methods stayed unported until now.

use crate::entity::employee::Employee;
use date_range_rs::DateRange;

/// `CurrentUser.canAccessItem(SecurityItem.EXCEED_AVAILABLE_HOURS)`.
/// `ExceedsAvailableHoursConflictValidator::exceeding_hours_should_create_fatal_conflict`'s
/// dependency.
pub trait CurrentUserPort {
    fn can_exceed_available_hours(&self) -> bool;
}

/// `new EmployeeCertificationValidator(employee).verifyAndGetCertificationErrorKey(job, property,
/// shiftRange)`, narrowed to the one overload `ScheduleChecker` calls (the shift's own property is
/// always the current property here, so this port doesn't take one separately).
pub trait CertificationPort {
    fn certification_error_key(
        &self,
        employee: &Employee,
        job_id: i32,
        shift_range: &DateRange,
    ) -> Option<&'static str>;
}
