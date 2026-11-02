//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeJobStatus`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeJobStatus.java`.
//!
//! An employee's engagement in one job over a date range: how they are paid and
//! at what rate. 17 of the 32 rule families read it — usually to find the rate
//! to price an earning at, via the pay type.

use crate::common::enums::employee_pay_type::EmployeePayType;
use crate::entity::salary_dist::SalaryDist;
use joda_rs::LocalDate;

/// An employee's standing in one job over a period. `EmployeeJobStatus`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeJobStatus {
    id: i32,
    employee_id: i32,
    job_id: i32,
    start_date: LocalDate,
    end_date: LocalDate,
    pay_type: EmployeePayType,
    hourly_rate: f64,
    annual_rate: f64,
    piece_rate: f64,
    contract_hours: Option<f64>,
    contract_days: f64,
    salary_dist_id: Option<i32>,
    salary_dist: Option<SalaryDist>,
    annual_based: bool,
    seniority_date: Option<LocalDate>,
    home: bool,
}

impl EmployeeJobStatus {
    /// Build a job status.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        employee_id: i32,
        job_id: i32,
        start_date: LocalDate,
        end_date: LocalDate,
        pay_type: EmployeePayType,
        hourly_rate: f64,
        home: bool,
    ) -> Self {
        Self {
            id,
            employee_id,
            job_id,
            start_date,
            end_date,
            pay_type,
            hourly_rate,
            annual_rate: 0.0,
            piece_rate: 0.0,
            contract_hours: None,
            contract_days: 0.0,
            salary_dist_id: None,
            salary_dist: None,
            annual_based: false,
            seniority_date: None,
            home,
        }
    }

    /// Attach the fields only some pay types use.
    ///
    /// Kept apart from [`new`](Self::new) so an hourly status — the common case
    /// — need not supply four values that do not apply to it.
    #[must_use]
    pub fn with_salaried_fields(
        mut self,
        annual_rate: f64,
        piece_rate: f64,
        contract_hours: Option<f64>,
        salary_dist_id: Option<i32>,
    ) -> Self {
        self.annual_rate = annual_rate;
        self.piece_rate = piece_rate;
        self.contract_hours = contract_hours;
        self.salary_dist_id = salary_dist_id;
        self
    }

    /// Attach a seniority date that differs from the employee's own.
    #[must_use]
    pub fn with_seniority_date(mut self, seniority_date: Option<LocalDate>) -> Self {
        self.seniority_date = seniority_date;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getEmployee().getID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getStartDate()`.
    pub fn start_date(&self) -> LocalDate {
        self.start_date
    }

    /// `getEndDate()`.
    pub fn end_date(&self) -> LocalDate {
        self.end_date
    }

    /// How this engagement is paid. `getPayType()`.
    pub fn pay_type(&self) -> EmployeePayType {
        self.pay_type
    }

    /// `getHourlyRate()`.
    pub fn hourly_rate(&self) -> f64 {
        self.hourly_rate
    }

    /// `getAnnualRate()`.
    pub fn annual_rate(&self) -> f64 {
        self.annual_rate
    }

    /// `getPieceRate()`.
    pub fn piece_rate(&self) -> f64 {
        self.piece_rate
    }

    /// Guaranteed hours, for contract employees. `getContractHours()`.
    pub fn contract_hours(&self) -> Option<f64> {
        self.contract_hours
    }

    /// Guaranteed days, for contract employees. `getContractDays()`.
    pub fn contract_days(&self) -> f64 {
        self.contract_days
    }

    /// Attach a contract-days figure that differs from the default `0.0`.
    #[must_use]
    pub fn with_contract_days(mut self, contract_days: f64) -> Self {
        self.contract_days = contract_days;
        self
    }

    /// `getSalaryDist().getID()`: the attached distribution's id, else the stored one.
    pub fn salary_dist_id(&self) -> Option<i32> {
        self.salary_dist
            .as_ref()
            .map(SalaryDist::id)
            .or(self.salary_dist_id)
    }

    /// The attached weekly hours pattern. `getSalaryDist()`.
    pub fn salary_dist(&self) -> Option<&SalaryDist> {
        self.salary_dist.as_ref()
    }

    /// Attach (or clear) the weekly hours pattern. `setSalaryDist()`.
    pub fn set_salary_dist(&mut self, salary_dist: Option<SalaryDist>) {
        self.salary_dist_id = salary_dist.as_ref().map(SalaryDist::id);
        self.salary_dist = salary_dist;
    }

    /// Is the pay stored as an annual figure (hourly derived), rather than hourly?
    /// `isAnnualBased()`.
    pub fn annual_based(&self) -> bool {
        self.annual_based
    }

    /// `setAnnualBased()`.
    pub fn set_annual_based(&mut self, annual_based: bool) {
        self.annual_based = annual_based;
    }

    /// `setAnnualRate()`.
    pub fn set_annual_rate(&mut self, annual_rate: f64) {
        self.annual_rate = annual_rate;
    }

    /// `setHourlyRate()`.
    pub fn set_hourly_rate(&mut self, hourly_rate: f64) {
        self.hourly_rate = hourly_rate;
    }

    /// Recompute the rates from the pay type. `calculateAndSetRates()`.
    ///
    /// `HOURLY` clears the annual figures and the distribution; `PIECE` zeroes
    /// the hourly and annual rates; `SALARIED_EXEMPT` converts through the
    /// distribution's weekly hours (0 with none) and the contract types through
    /// the contract hours. The conversion goes annual-to-hourly when the status
    /// is annual-based and hourly-to-annual otherwise, over 52 weeks, rounded to
    /// currency. Java skips the conversion for a status with no employee
    /// (`employee == null`); a status here always has an employee id.
    pub fn calculate_and_set_rates(&mut self) {
        use crate::common::enums::employee_pay_type::EmployeePayType as T;
        match self.pay_type {
            T::Hourly => {
                self.annual_based = false;
                self.set_salary_dist(None);
                self.piece_rate = 0.0;
                self.annual_rate = 0.0;
            }
            T::Piece => {
                self.hourly_rate = 0.0;
                self.annual_rate = 0.0;
            }
            T::SalariedExempt => {
                let hours = self.salary_dist.as_ref().map_or(0.0, |d| {
                    crate::common::numbers::round_hours(d.hours_per_week())
                });
                self.calculate_and_set_rates_by_hours(hours);
            }
            T::SalariedNonExempt | T::Contract => {
                let hours = self.contract_hours.unwrap_or(0.0);
                self.calculate_and_set_rates_by_hours(hours);
            }
        }
    }

    /// `calculateAndSetRatesByHours`, with `PayRateConverter`.
    fn calculate_and_set_rates_by_hours(&mut self, hours: f64) {
        use crate::common::numbers::round_currency;
        const WEEKS_PER_YEAR: f64 = 52.0;
        self.piece_rate = 0.0;
        if self.annual_based {
            self.hourly_rate = if hours == 0.0 {
                0.0
            } else {
                round_currency(self.annual_rate / WEEKS_PER_YEAR / hours)
            };
        } else {
            self.annual_rate = round_currency(hours * WEEKS_PER_YEAR * self.hourly_rate);
        }
    }

    /// `getSeniorityDate()`.
    pub fn seniority_date(&self) -> Option<LocalDate> {
        self.seniority_date
    }

    /// Is this the employee's home job? `isHome()`.
    pub fn home(&self) -> bool {
        self.home
    }

    /// Is `date` within this engagement? `containsDate(LocalDate)` — inclusive
    /// at both ends.
    pub fn contains_date(&self, date: LocalDate) -> bool {
        self.start_date.is_on_or_before(date) && self.end_date.is_on_or_after(date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn status() -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 1),
            LocalDate::of(2010, 12, 31),
            EmployeePayType::Hourly,
            12.50,
            true,
        )
    }

    #[test]
    fn a_status_carries_its_rate_and_pay_type() {
        let s = status();
        assert_eq!(s.employee_id(), 100);
        assert_eq!(s.job_id(), 200);
        assert_eq!(s.pay_type(), EmployeePayType::Hourly);
        assert_eq!(s.hourly_rate(), 12.50);
        assert!(s.home());
    }

    #[rstest]
    #[case(LocalDate::of(2010, 1, 1), true)]
    #[case(LocalDate::of(2010, 6, 15), true)]
    #[case(LocalDate::of(2010, 12, 31), true)]
    #[case(LocalDate::of(2009, 12, 31), false)]
    #[case(LocalDate::of(2011, 1, 1), false)]
    fn contains_date_includes_both_ends(#[case] date: LocalDate, #[case] expected: bool) {
        assert_eq!(status().contains_date(date), expected);
    }

    #[test]
    fn the_salaried_fields_default_to_absent() {
        let s = status();
        assert_eq!(s.annual_rate(), 0.0);
        assert_eq!(s.contract_hours(), None);
        assert_eq!(s.salary_dist_id(), None);
    }

    #[test]
    fn the_salaried_fields_can_be_attached() {
        let s = status().with_salaried_fields(52_000.0, 0.0, Some(37.5), Some(3));
        assert_eq!(s.annual_rate(), 52_000.0);
        assert_eq!(s.contract_hours(), Some(37.5));
        assert_eq!(s.salary_dist_id(), Some(3));
    }

    #[test]
    fn the_pay_type_decides_which_rate_field_applies() {
        let hourly = status();
        assert!(hourly.pay_type().is_hourly_rate_based());

        let salaried = EmployeeJobStatus::new(
            2,
            100,
            200,
            LocalDate::of(2010, 1, 1),
            LocalDate::of(2010, 12, 31),
            EmployeePayType::SalariedExempt,
            0.0,
            false,
        )
        .with_salaried_fields(52_000.0, 0.0, None, None);

        assert!(salaried.pay_type().is_annual_rate_based());
    }
}
