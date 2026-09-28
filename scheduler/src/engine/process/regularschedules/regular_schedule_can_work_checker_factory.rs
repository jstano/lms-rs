//! Port of `com.unifocus.watson.server.scheduler.engine.process.regularschedules.
//! RegularScheduleCanWorkCheckerFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! regularschedules/RegularScheduleCanWorkCheckerFactory.java`. Java resolves its fixed checker
//! list through Spring's `AbstractCanWorkCheckerFactory`/bean lookup; ported as a plain ordered
//! list instead, same idiom as `RotationPlanCheckerFactory`/`SeniorityComparatorFactory` (no
//! DI-lookup shim, since this crate has no bean container).
//!
//! `can_work_checkers` is built fresh per call, not stored on `self`, because
//! `EmployeeAvailabilityChecker` borrows a `&JobData` for the duration of one check — see
//! `RegularScheduleCanWorkChecker`'s doc for why that reference has to be a short-lived clone, not
//! a field held for this factory's own lifetime.

use crate::engine::model::job_data::JobData;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::employee_assignment_checker::EmployeeAssignmentChecker;
use crate::engine::process::checkers::employee_assignment_rotation_checker::EmployeeAssignmentRotationChecker;
use crate::engine::process::checkers::employee_availability_checker::EmployeeAvailabilityChecker;
use crate::engine::process::checkers::employee_certifications_checker::{
    EmployeeCertificationPort, EmployeeCertificationsChecker,
};
use crate::engine::process::checkers::employee_job_rotation_checker::EmployeeJobRotationChecker;
use crate::engine::process::checkers::employee_min_days_off_checker::EmployeeMinDaysOffChecker;
use crate::engine::process::checkers::employee_min_hours_off_checker::EmployeeMinHoursOffChecker;
use crate::engine::process::checkers::employee_schedule_checker::EmployeeScheduleChecker;
use crate::engine::process::checkers::employee_time_off_checker::EmployeeTimeOffChecker;
use crate::engine::process::checkers::schedule_restriction_rule_checker::{
    ScheduleRestrictionRuleChecker, ScheduleRestrictionRulePort,
};
use crate::engine::process::ports::AssignmentPort;

/// `RegularScheduleCanWorkCheckerFactory`.
pub struct RegularScheduleCanWorkCheckerFactory<'a> {
    assignments: &'a dyn AssignmentPort,
    schedule_restriction_rules: &'a dyn ScheduleRestrictionRulePort,
    certifications: &'a dyn EmployeeCertificationPort,
}

impl<'a> RegularScheduleCanWorkCheckerFactory<'a> {
    pub fn new(
        assignments: &'a dyn AssignmentPort,
        schedule_restriction_rules: &'a dyn ScheduleRestrictionRulePort,
        certifications: &'a dyn EmployeeCertificationPort,
    ) -> Self {
        Self {
            assignments,
            schedule_restriction_rules,
            certifications,
        }
    }

    /// `getCanWorkCheckers()` — fixed order; `RegularScheduleCanWorkChecker` short-circuits on
    /// the first `false`.
    pub fn can_work_checkers<'b>(
        &'b self,
        job_data: &'b JobData,
    ) -> Vec<Box<dyn CanWorkChecker + 'b>>
    where
        'a: 'b,
    {
        vec![
            Box::new(EmployeeScheduleChecker),
            Box::new(EmployeeTimeOffChecker),
            Box::new(EmployeeAvailabilityChecker::new(job_data)),
            Box::new(EmployeeMinDaysOffChecker::new(self.assignments)),
            Box::new(EmployeeMinHoursOffChecker::new(self.assignments)),
            Box::new(EmployeeAssignmentChecker),
            Box::new(EmployeeJobRotationChecker::new(self.assignments)),
            Box::new(EmployeeAssignmentRotationChecker::new(self.assignments)),
            Box::new(ScheduleRestrictionRuleChecker::new(
                self.schedule_restriction_rules,
            )),
            Box::new(EmployeeCertificationsChecker::new(self.certifications)),
        ]
    }
}
