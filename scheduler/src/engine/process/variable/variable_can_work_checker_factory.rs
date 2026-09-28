//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.VariableCanWorkCheckerFactory`.
//!
//! Ground truth: `taps/.../process/variable/VariableCanWorkCheckerFactory.java`. Same
//! Spring-DI-to-plain-`Vec`-of-checkers idiom as `RegularScheduleCanWorkCheckerFactory`
//! (`PARITY_AUDIT.md` findings 34-35) — `EmployeeAvailabilityChecker`/`EmployeeWeeklyAvailableHoursChecker`
//! both borrow a `&JobData` for the duration of one check, so the list is built fresh per call,
//! not cached on `self`.

use crate::engine::model::job_data::JobData;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::engine::process::checkers::employee_assignment_checker::EmployeeAssignmentChecker;
use crate::engine::process::checkers::employee_assignment_rotation_checker::EmployeeAssignmentRotationChecker;
use crate::engine::process::checkers::employee_availability_checker::EmployeeAvailabilityChecker;
use crate::engine::process::checkers::employee_available_hours_checker::EmployeeAvailableHoursChecker;
use crate::engine::process::checkers::employee_certifications_checker::{
    EmployeeCertificationPort, EmployeeCertificationsChecker,
};
use crate::engine::process::checkers::employee_day_off_rotation_plan_checker::EmployeeDayOffRotationPlanChecker;
use crate::engine::process::checkers::employee_job_rotation_checker::EmployeeJobRotationChecker;
use crate::engine::process::checkers::employee_min_days_off_checker::EmployeeMinDaysOffChecker;
use crate::engine::process::checkers::employee_min_hours_off_checker::EmployeeMinHoursOffChecker;
use crate::engine::process::checkers::employee_monthly_available_hours_checker::{
    EmployeeMonthlyAvailableHoursChecker, MonthlyContractHoursAvailablePort,
};
use crate::engine::process::checkers::employee_overtime_checker::EmployeeOvertimeChecker;
use crate::engine::process::checkers::employee_schedule_checker::EmployeeScheduleChecker;
use crate::engine::process::checkers::employee_time_off_checker::EmployeeTimeOffChecker;
use crate::engine::process::checkers::employee_weekly_available_hours_checker::EmployeeWeeklyAvailableHoursChecker;
use crate::engine::process::checkers::schedule_restriction_rule_checker::{
    ScheduleRestrictionRuleChecker, ScheduleRestrictionRulePort,
};
use crate::engine::process::ports::AssignmentPort;
use crate::engine::process::ports::OvertimeForDateRangePort;

/// `VariableCanWorkCheckerFactory`.
pub struct VariableCanWorkCheckerFactory<'a> {
    assignments: &'a dyn AssignmentPort,
    monthly_contract_hours: &'a dyn MonthlyContractHoursAvailablePort,
    overtime: &'a dyn OvertimeForDateRangePort,
    schedule_restriction_rules: &'a dyn ScheduleRestrictionRulePort,
    certifications: &'a dyn EmployeeCertificationPort,
}

impl<'a> VariableCanWorkCheckerFactory<'a> {
    pub fn new(
        assignments: &'a dyn AssignmentPort,
        monthly_contract_hours: &'a dyn MonthlyContractHoursAvailablePort,
        overtime: &'a dyn OvertimeForDateRangePort,
        schedule_restriction_rules: &'a dyn ScheduleRestrictionRulePort,
        certifications: &'a dyn EmployeeCertificationPort,
    ) -> Self {
        Self {
            assignments,
            monthly_contract_hours,
            overtime,
            schedule_restriction_rules,
            certifications,
        }
    }

    /// `getCanWorkCheckerClasses()` — fixed order; `VariableCanWorkChecker` short-circuits on the
    /// first `false`.
    pub fn can_work_checkers<'b>(
        &'b self,
        job_data: &'b JobData,
    ) -> Vec<Box<dyn CanWorkChecker + 'b>>
    where
        'a: 'b,
    {
        vec![
            Box::new(EmployeeScheduleChecker),
            Box::new(EmployeeAvailableHoursChecker::new(
                EmployeeMonthlyAvailableHoursChecker::new(self.monthly_contract_hours),
                EmployeeWeeklyAvailableHoursChecker::new(job_data),
            )),
            Box::new(EmployeeTimeOffChecker),
            Box::new(EmployeeAvailabilityChecker::new(job_data)),
            Box::new(EmployeeMinDaysOffChecker::new(self.assignments)),
            Box::new(EmployeeMinHoursOffChecker::new(self.assignments)),
            Box::new(EmployeeAssignmentChecker),
            Box::new(EmployeeJobRotationChecker::new(self.assignments)),
            Box::new(EmployeeAssignmentRotationChecker::new(self.assignments)),
            Box::new(EmployeeDayOffRotationPlanChecker),
            Box::new(EmployeeOvertimeChecker::new(self.overtime)),
            Box::new(ScheduleRestrictionRuleChecker::new(
                self.schedule_restriction_rules,
            )),
            Box::new(EmployeeCertificationsChecker::new(self.certifications)),
        ]
    }
}
