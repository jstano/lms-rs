//! Port of `com.unifocus.watson.server.scheduler.engine.model.WeeklyAvailableHours`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/
//! WeeklyAvailableHours.java`. Per-(employee, job) cache of hours available in a given week; see
//! `DATA_MODEL.md` §4.
//!
//! # Divergence: no live `EmployeeData`/`JobData` reference
//!
//! Java's constructor captures `employeeData`/`jobData` once and reads them live on every
//! `getHoursAvailableForWeek` call — including `jobData.getBalanceFactor()`/`getBalanceLevel()`,
//! which step 4 (`EmployeeAvailableHoursBalancer`, not yet ported) mutates *after* this object is
//! created. An owned Rust field would go stale the moment that step runs. Every value Java reads
//! off `employeeData`/`jobData` at call time is instead passed as a parameter to
//! [`hours_available_for_week`](WeeklyAvailableHours::hours_available_for_week), keeping the
//! same "always current" behavior without a live-reference type.
//!
//! `time_off_requests` stands in for `employeeData.getDataSet().getTimeOffRequests()` —
//! `ScheduleCalcDataSet` isn't ported yet (see `DATA_MODEL.md` §7); fold this parameter back into
//! a real dataset lookup once it is.

use crate::entity::employee::Employee;
use crate::entity::employee_time_off::EmployeeTimeOff;
use crate::entity::employee_type::EmployeeType;
use date_range_rs::WeeklyDateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

const HOURS_PER_TIME_OFF_REQUEST_DAY: f64 = 8.0;

/// `WeeklyAvailableHours`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeeklyAvailableHours {
    weekly_hours_available: HashMap<LocalDate, f64>,
}

impl WeeklyAvailableHours {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getHoursAvailableForWeek(LocalDate)`.
    #[allow(clippy::too_many_arguments)]
    pub fn hours_available_for_week(
        &mut self,
        employee: &Employee,
        job_balance_factor: f64,
        job_balance_level: i32,
        job_average_shift_length: f64,
        job_is_balance_schedules: bool,
        time_off_requests: &[EmployeeTimeOff],
        week_end_date: LocalDate,
    ) -> f64 {
        if let Some(&hours) = self.weekly_hours_available.get(&week_end_date) {
            return hours;
        }

        let weekly_date_range = WeeklyDateRange::with_end_date(week_end_date);
        let hours = Self::calculate_weekly_hours_available(
            employee,
            job_balance_factor,
            job_balance_level,
            job_average_shift_length,
            job_is_balance_schedules,
            time_off_requests,
            &weekly_date_range,
        );
        self.weekly_hours_available.insert(week_end_date, hours);
        hours
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_weekly_hours_available(
        employee: &Employee,
        job_balance_factor: f64,
        job_balance_level: i32,
        job_average_shift_length: f64,
        job_is_balance_schedules: bool,
        time_off_requests: &[EmployeeTimeOff],
        weekly_date_range: &date_range_rs::DateRange,
    ) -> f64 {
        let hours_available = Self::base_available_hours(employee);

        match employee.employee_type() {
            EmployeeType::Regular | EmployeeType::Permanent => {
                Self::available_hours_for_regular_employees(
                    time_off_requests,
                    weekly_date_range,
                    hours_available,
                )
            }
            EmployeeType::Other => Self::available_hours_for_variable_employees(
                hours_available,
                job_balance_factor,
                job_balance_level,
                job_average_shift_length,
                job_is_balance_schedules,
            ),
        }
    }

    fn base_available_hours(employee: &Employee) -> f64 {
        employee
            .hours_available()
            .unwrap_or_else(|| employee.work_class().hours_available())
    }

    fn available_hours_for_variable_employees(
        hours_available: f64,
        job_balance_factor: f64,
        job_balance_level: i32,
        job_average_shift_length: f64,
        job_is_balance_schedules: bool,
    ) -> f64 {
        if !job_is_balance_schedules {
            return hours_available;
        }

        let mut temp_hours_available = hours_available * job_balance_factor;

        if job_balance_level > 0 {
            temp_hours_available += job_average_shift_length * job_balance_level as f64;
        }

        if temp_hours_available > hours_available {
            temp_hours_available = hours_available;
        }

        temp_hours_available
    }

    fn available_hours_for_regular_employees(
        time_off_requests: &[EmployeeTimeOff],
        weekly_date_range: &date_range_rs::DateRange,
        mut hours_available: f64,
    ) -> f64 {
        for time_off in time_off_requests {
            for date in time_off.to_date_range().dates() {
                if weekly_date_range.contains_date(date) {
                    hours_available -= HOURS_PER_TIME_OFF_REQUEST_DAY;
                }
            }
        }

        hours_available
    }
}
