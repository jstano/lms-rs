//! Port of `com.unifocus.watson.server.scheduler.engine.model.Schedules`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/model/Schedules.java`.
//! Read-only query surface over one employee's shifts + time off, built on demand by checkers/
//! comparators callers; see `DATA_MODEL.md` §4.
//!
//! `DateUtil.daysBetween(a, b)` is `(b - a).to_days()` in `joda_rs` terms — see
//! [`days_between`].

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_time_off::EmployeeTimeOff;
use joda_rs::LocalDate;

fn days_between(from: LocalDate, to: LocalDate) -> i32 {
    (to - from).to_days() as i32
}

/// `Schedules`.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedules {
    employee_shifts: Vec<EmployeeShift>,
    employee_time_off_list: Vec<EmployeeTimeOff>,
}

impl Schedules {
    pub fn new(
        employee_shifts: Vec<EmployeeShift>,
        employee_time_off_list: Vec<EmployeeTimeOff>,
    ) -> Self {
        Self {
            employee_shifts,
            employee_time_off_list,
        }
    }

    /// `determineConsecutiveDaysPriorDates(LocalDate, int)`.
    pub fn determine_consecutive_days_prior_dates(
        &self,
        date: LocalDate,
        min_days_off: i32,
    ) -> i32 {
        let mut consecutive_days = 0;
        let mut prior_days = 0;

        while consecutive_days == 0 && prior_days < min_days_off {
            consecutive_days =
                self.consecutive_days_scheduled_before_date(date.minus_days(prior_days as i64));
            prior_days += 1;
        }

        consecutive_days
    }

    /// `determineConsecutiveDaysFutureDates(LocalDate, int)`.
    pub fn determine_consecutive_days_future_dates(
        &self,
        date: LocalDate,
        min_days_off: i32,
    ) -> i32 {
        let mut consecutive_days = 0;
        let mut future_days = 0;

        while consecutive_days == 0 && future_days < min_days_off {
            consecutive_days =
                self.consecutive_days_scheduled_after_date(date.plus_days(future_days as i64));
            future_days += 1;
        }

        consecutive_days
    }

    fn consecutive_days_scheduled_before_date(&self, date: LocalDate) -> i32 {
        let mut prior_date = date.minus_days(1);

        while self.has_shift_on_date(prior_date) {
            prior_date = prior_date.minus_days(1);
        }

        days_between(prior_date, date.minus_days(1))
    }

    fn consecutive_days_scheduled_after_date(&self, date: LocalDate) -> i32 {
        let mut next_date = date.plus_days(1);

        while self.has_shift_on_date(next_date) {
            next_date = next_date.plus_days(1);
        }

        days_between(date.plus_days(1), next_date)
    }

    /// `hasShiftOnDate(LocalDate)`.
    pub fn has_shift_on_date(&self, shift_date: LocalDate) -> bool {
        self.employee_shifts
            .iter()
            .any(|s| s.shift_date() == shift_date)
    }

    /// `findFirstShiftOnDate(LocalDate)`.
    pub fn find_first_shift_on_date(&self, shift_date: LocalDate) -> Option<EmployeeShift> {
        self.employee_shifts
            .iter()
            .filter(|s| s.shift_date() == shift_date)
            .min_by_key(|s| s.start_date_time())
            .cloned()
    }

    /// `findLastShiftOnDate(LocalDate)`.
    pub fn find_last_shift_on_date(&self, shift_date: LocalDate) -> Option<EmployeeShift> {
        self.employee_shifts
            .iter()
            .filter(|s| s.shift_date() == shift_date)
            .max_by_key(|s| s.start_date_time())
            .cloned()
    }

    /// `hasTimeOffOnDate(LocalDate)`.
    pub fn has_time_off_on_date(&self, shift_date: LocalDate) -> bool {
        self.employee_time_off_list
            .iter()
            .any(|t| t.to_date_range().contains_date(shift_date))
    }

    /// `getNumberOfPriorDaysOff(LocalDate)`.
    pub fn number_of_prior_days_off(&self, shift_date: LocalDate) -> i32 {
        let last_scheduled_date = self.last_scheduled_date(shift_date, None);
        let last_scheduled_date = self.last_time_off_date(shift_date, last_scheduled_date);

        match last_scheduled_date {
            Some(date) => days_between(date, shift_date) - 1,
            None => 0,
        }
    }

    /// `hasShiftWithJobOnDate(Assignment, LocalDate)`.
    pub fn has_shift_with_job_on_date(&self, job_id: i32, date: LocalDate) -> bool {
        self.employee_shifts
            .iter()
            .any(|s| s.shift_date() == date && s.job_id() == job_id)
    }

    /// `hasShiftWithAssignmentOnDate(Assignment, LocalDate)`.
    pub fn has_shift_with_assignment_on_date(&self, assignment_id: i32, date: LocalDate) -> bool {
        self.employee_shifts
            .iter()
            .any(|s| s.shift_date() == date && s.assignment_id() == Some(assignment_id))
    }

    fn last_scheduled_date(
        &self,
        shift_date: LocalDate,
        mut last_scheduled_date: Option<LocalDate>,
    ) -> Option<LocalDate> {
        for shift in &self.employee_shifts {
            if shift.shift_date().is_before(shift_date) {
                last_scheduled_date = Some(Self::max_date(last_scheduled_date, shift.shift_date()));
            }
        }
        last_scheduled_date
    }

    fn last_time_off_date(
        &self,
        shift_date: LocalDate,
        mut last_scheduled_date: Option<LocalDate>,
    ) -> Option<LocalDate> {
        for time_off in &self.employee_time_off_list {
            let time_off_start_date = time_off.start_date_time().to_local_date();
            let time_off_end_date = time_off.end_date_time().to_local_date();

            if time_off_end_date.is_before(shift_date) {
                last_scheduled_date = Some(Self::max_date(last_scheduled_date, time_off_end_date));
            } else if time_off_start_date.is_before(shift_date) {
                last_scheduled_date = Some(shift_date.minus_days(1));
            }
        }
        last_scheduled_date
    }

    fn max_date(last_scheduled_date: Option<LocalDate>, date: LocalDate) -> LocalDate {
        match last_scheduled_date {
            None => date,
            Some(last) if date.is_after(last) => date,
            Some(last) => last,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalDateTime;

    fn shift(id: i32, date: LocalDate) -> EmployeeShift {
        EmployeeShift::new(
            id,
            date,
            date.at_time(joda_rs::LocalTime::of(9, 0, 0)),
            1,
            None,
        )
    }

    fn time_off(start: LocalDate, end: LocalDate) -> EmployeeTimeOff {
        EmployeeTimeOff::new(
            start.at_time(joda_rs::LocalTime::of(0, 0, 0)),
            end.at_time(joda_rs::LocalTime::of(23, 59, 0)),
        )
    }

    #[test]
    fn has_shift_on_date_checks_shift_date_only() {
        let schedules = Schedules::new(vec![shift(1, LocalDate::of(2024, 1, 10))], vec![]);

        assert!(schedules.has_shift_on_date(LocalDate::of(2024, 1, 10)));
        assert!(!schedules.has_shift_on_date(LocalDate::of(2024, 1, 11)));
    }

    #[test]
    fn no_prior_shifts_or_time_off_means_zero_prior_days_off() {
        let schedules = Schedules::new(vec![], vec![]);
        assert_eq!(
            schedules.number_of_prior_days_off(LocalDate::of(2024, 1, 10)),
            0
        );
    }

    #[test]
    fn prior_days_off_counts_the_gap_since_the_last_shift() {
        // Worked 1/5, nothing since; asking about 1/10 should count 4 days off (1/6-1/9).
        let schedules = Schedules::new(vec![shift(1, LocalDate::of(2024, 1, 5))], vec![]);
        assert_eq!(
            schedules.number_of_prior_days_off(LocalDate::of(2024, 1, 10)),
            4
        );
    }

    #[test]
    fn an_in_progress_time_off_period_collapses_to_the_day_before() {
        // Time off starts 1/8 and runs past the date being asked about (1/10) — Java's
        // getLastTimeOffDate collapses "last scheduled date" to shiftDate.minusDays(1) (1/9),
        // not the period's own end date, so daysBetween(1/9, 1/10) - 1 = 0.
        let schedules = Schedules::new(
            vec![],
            vec![time_off(
                LocalDate::of(2024, 1, 8),
                LocalDate::of(2024, 1, 20),
            )],
        );
        assert_eq!(
            schedules.number_of_prior_days_off(LocalDate::of(2024, 1, 10)),
            0
        );
    }

    #[test]
    fn determine_consecutive_days_prior_dates_stops_at_the_first_gap() {
        // Worked 1/8, 1/9, 1/10 with a shift, not 1/7. Asking from 1/11 with min_days_off=3
        // should find the run ending 1/10 and count back to it.
        let schedules = Schedules::new(
            vec![
                shift(1, LocalDate::of(2024, 1, 8)),
                shift(2, LocalDate::of(2024, 1, 9)),
                shift(3, LocalDate::of(2024, 1, 10)),
            ],
            vec![],
        );

        assert_eq!(
            schedules.determine_consecutive_days_prior_dates(LocalDate::of(2024, 1, 11), 3),
            3
        );
    }

    #[test]
    fn determine_consecutive_days_future_dates_stops_at_the_first_gap() {
        let schedules = Schedules::new(
            vec![
                shift(1, LocalDate::of(2024, 1, 12)),
                shift(2, LocalDate::of(2024, 1, 13)),
            ],
            vec![],
        );

        assert_eq!(
            schedules.determine_consecutive_days_future_dates(LocalDate::of(2024, 1, 11), 3),
            2
        );
    }

    #[test]
    fn find_first_and_last_shift_on_date_pick_by_start_time() {
        let earlier = EmployeeShift::new(
            1,
            LocalDate::of(2024, 1, 10),
            LocalDateTime::of(2024, 1, 10, 7, 0, 0),
            1,
            None,
        );
        let later = EmployeeShift::new(
            2,
            LocalDate::of(2024, 1, 10),
            LocalDateTime::of(2024, 1, 10, 15, 0, 0),
            1,
            None,
        );
        let schedules = Schedules::new(vec![later, earlier], vec![]);

        assert_eq!(
            schedules
                .find_first_shift_on_date(LocalDate::of(2024, 1, 10))
                .map(|s| s.id()),
            Some(1)
        );
        assert_eq!(
            schedules
                .find_last_shift_on_date(LocalDate::of(2024, 1, 10))
                .map(|s| s.id()),
            Some(2)
        );
    }
}
