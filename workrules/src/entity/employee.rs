//! Port of `com.unifocus.watson.server.hibernate.entity.Employee`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Employee.java`.
//!
//! The Java entity is 1,486 lines and around 70 fields; rules read about
//! thirteen. The rest — address, phone, emergency contact, marital status,
//! locale, portal credentials — is HR profile data no rule consults, and none
//! of it comes across.
//!
//! What rules do need is the job statuses (to find the rate in force on a date)
//! and the employee-level rule sets, which are the first step of resolution.

use crate::entity::employee_job_status::EmployeeJobStatus;
use crate::entity::rule_set::RuleSet;
use crate::rules::rule_type::RuleType;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// A person. `Employee`.
#[derive(Debug, Clone, PartialEq)]
pub struct Employee {
    id: i32,
    property_id: i32,
    name: String,
    emp_id: String,
    hire_date: Option<LocalDate>,
    seniority_date: Option<LocalDate>,
    birth_date: Option<LocalDate>,
    pay_group_id: Option<i32>,
    employee_job_statuses: Vec<EmployeeJobStatus>,
    rule_sets: Vec<RuleSet>,
}

impl Employee {
    /// Build an employee with their job statuses.
    pub fn new(
        id: i32,
        property_id: i32,
        name: impl Into<String>,
        employee_job_statuses: Vec<EmployeeJobStatus>,
    ) -> Self {
        Self {
            id,
            property_id,
            name: name.into(),
            emp_id: String::new(),
            hire_date: None,
            seniority_date: None,
            birth_date: None,
            pay_group_id: None,
            employee_job_statuses,
            rule_sets: Vec::new(),
        }
    }

    /// Attach employee-level rule sets — the ones that win resolution outright.
    #[must_use]
    pub fn with_rule_sets(mut self, rule_sets: Vec<RuleSet>) -> Self {
        self.rule_sets = rule_sets;
        self
    }

    /// Attach the dates accrual and eligibility rules measure from.
    #[must_use]
    pub fn with_dates(
        mut self,
        hire_date: Option<LocalDate>,
        seniority_date: Option<LocalDate>,
        birth_date: Option<LocalDate>,
    ) -> Self {
        self.hire_date = hire_date;
        self.seniority_date = seniority_date;
        self.birth_date = birth_date;
        self
    }

    /// Attach the pay group. `setPayGroup()`.
    #[must_use]
    pub fn with_pay_group_id(mut self, pay_group_id: Option<i32>) -> Self {
        self.pay_group_id = pay_group_id;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getProperty().getID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The customer-facing employee number. `getEmpID()`.
    pub fn emp_id(&self) -> &str {
        &self.emp_id
    }

    /// `getHireDate()`.
    pub fn hire_date(&self) -> Option<LocalDate> {
        self.hire_date
    }

    /// `getSeniorityDate()`.
    pub fn seniority_date(&self) -> Option<LocalDate> {
        self.seniority_date
    }

    /// `getBirthDate()`.
    pub fn birth_date(&self) -> Option<LocalDate> {
        self.birth_date
    }

    /// `getPayGroup().getID()`.
    pub fn pay_group_id(&self) -> Option<i32> {
        self.pay_group_id
    }

    /// Every job status, current or not. `getEmployeeJobStatuses()`.
    pub fn employee_job_statuses(&self) -> &[EmployeeJobStatus] {
        &self.employee_job_statuses
    }

    /// The job statuses in force on a date. `getEmployeeJobStatuses(LocalDate)`.
    pub fn employee_job_statuses_on(&self, date: LocalDate) -> Vec<&EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .filter(|status| status.contains_date(date))
            .collect()
    }

    /// The status for one job on a date. `getEmployeeJobStatus(Assignment, LocalDate)`.
    pub fn employee_job_status(&self, job_id: i32, date: LocalDate) -> Option<&EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .find(|status| status.job_id() == job_id && status.contains_date(date))
    }

    /// The home job status on a date. `getHomeEmployeeJobStatus(LocalDate)`.
    ///
    /// Runners fall back to this when a punch or request does not name a job.
    pub fn home_employee_job_status(&self, date: LocalDate) -> Option<&EmployeeJobStatus> {
        self.employee_job_statuses
            .iter()
            .find(|status| status.home() && status.contains_date(date))
    }

    /// The first home job status held on any date of `period`, scanning
    /// forward. `BasicEmployee.getFirstHomeEmployeeJobStatusForPeriod(DateRange)`.
    ///
    /// Java iterates the period's dates in order and returns the first date's
    /// home status, so a period that opens before the employee was hired
    /// answers with the status covering the first date they held one.
    pub fn first_home_employee_job_status_for_period(
        &self,
        period: &DateRange,
    ) -> Option<&EmployeeJobStatus> {
        period
            .dates()
            .into_iter()
            .find_map(|date| self.home_employee_job_status(date))
    }

    /// The last one, scanning backward.
    /// `getLastHomeEmployeeJobStatusForPeriod(DateRange)`.
    pub fn last_home_employee_job_status_for_period(
        &self,
        period: &DateRange,
    ) -> Option<&EmployeeJobStatus> {
        period
            .dates()
            .into_iter()
            .rev()
            .find_map(|date| self.home_employee_job_status(date))
    }

    /// Is the employee active in a job on a date? `isActiveJobOnDate`.
    pub fn is_active_job_on_date(&self, job_id: i32, date: LocalDate) -> bool {
        self.employee_job_status(job_id, date).is_some()
    }

    /// The employee-level rule set for a type, if one is configured.
    /// `getRuleSetForType(RuleType)` — the first step of resolution, and the
    /// only one that wins outright.
    pub fn rule_set_for_type(&self, rule_type: RuleType) -> Option<&RuleSet> {
        self.rule_sets
            .iter()
            .find(|rule_set| rule_set.rule_type() == rule_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::employee_pay_type::EmployeePayType;

    fn status(id: i32, job_id: i32, home: bool) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            id,
            100,
            job_id,
            LocalDate::of(2010, 1, 1),
            LocalDate::of(2010, 12, 31),
            EmployeePayType::Hourly,
            12.50,
            home,
        )
    }

    fn employee() -> Employee {
        Employee::new(
            100,
            11,
            "Alex Kim",
            vec![status(1, 200, true), status(2, 201, false)],
        )
    }

    #[test]
    fn an_employee_carries_their_job_statuses() {
        let e = employee();
        assert_eq!(e.id(), 100);
        assert_eq!(e.property_id(), 11);
        assert_eq!(e.name(), "Alex Kim");
        assert_eq!(e.employee_job_statuses().len(), 2);
    }

    #[test]
    fn the_home_job_status_is_the_one_flagged_home() {
        let e = employee();
        let home = e
            .home_employee_job_status(LocalDate::of(2010, 6, 1))
            .unwrap();
        assert_eq!(home.job_id(), 200);
    }

    #[test]
    fn there_is_no_home_job_status_outside_its_dates() {
        assert!(
            employee()
                .home_employee_job_status(LocalDate::of(2011, 1, 1))
                .is_none()
        );
    }

    #[test]
    fn a_job_status_is_found_by_job_and_date() {
        let e = employee();
        assert!(
            e.employee_job_status(201, LocalDate::of(2010, 6, 1))
                .is_some()
        );
        assert!(
            e.employee_job_status(999, LocalDate::of(2010, 6, 1))
                .is_none()
        );
        assert!(
            e.employee_job_status(201, LocalDate::of(2009, 1, 1))
                .is_none()
        );
    }

    #[test]
    fn active_on_a_date_follows_the_job_status() {
        let e = employee();
        assert!(e.is_active_job_on_date(200, LocalDate::of(2010, 6, 1)));
        assert!(!e.is_active_job_on_date(200, LocalDate::of(2011, 6, 1)));
    }

    #[test]
    fn statuses_on_a_date_returns_every_match() {
        assert_eq!(
            employee()
                .employee_job_statuses_on(LocalDate::of(2010, 6, 1))
                .len(),
            2
        );
        assert!(
            employee()
                .employee_job_statuses_on(LocalDate::of(2009, 1, 1))
                .is_empty()
        );
    }

    #[test]
    fn an_employee_has_no_rule_sets_unless_given_some() {
        assert!(
            employee()
                .rule_set_for_type(RuleType::PunchRounding)
                .is_none()
        );
    }

    #[test]
    fn an_employee_level_rule_set_is_found_by_type() {
        let e = employee().with_rule_sets(vec![RuleSet::new(
            1,
            11,
            "Override",
            RuleType::PunchRounding,
            0,
            Vec::new(),
        )]);

        assert_eq!(
            e.rule_set_for_type(RuleType::PunchRounding).unwrap().name(),
            "Override"
        );
        assert!(e.rule_set_for_type(RuleType::BenefitAccrual).is_none());
    }

    #[test]
    fn the_accrual_dates_default_to_absent() {
        let e = employee();
        assert_eq!(e.hire_date(), None);
        assert_eq!(e.seniority_date(), None);

        let dated = employee().with_dates(
            Some(LocalDate::of(2008, 3, 1)),
            Some(LocalDate::of(2008, 3, 1)),
            None,
        );
        assert_eq!(dated.hire_date(), Some(LocalDate::of(2008, 3, 1)));
    }
}
