//! Port of `HolidayMatcher`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/HolidayMatcher.java`.
//!
//! Finds which of a property's holidays a shift overlaps, and builds a
//! holiday's time-of-day period. A holiday is a *date*; the period is that date
//! from the configured start time to the end time, rolling to the next day when
//! the end is not after the start — so the default midnight–midnight is a full
//! 24 hours, and `02:00`–`02:00` is 24 hours starting at 02:00.
//!
//! A holiday overlaps the shift when its period touches the shift's first-IN to
//! last-OUT span (**inclusive** at both ends) *and* its type is among the
//! requested ones. The calendar is the shift's **job's** property's, read once
//! per property and kept for the life of the matcher (`holidayMap`).
//!
//! Java returns a `HashSet<Holiday>`; the port returns a `Vec` ordered by date
//! then id, with a holiday listed once.

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::holiday::Holiday;
use crate::rules::ports::{AssignmentPort, HolidayPort};
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDate, LocalTime};
use std::cell::RefCell;
use std::collections::HashMap;

/// `HolidayMatcher`.
pub struct HolidayMatcher<H: HolidayPort, A: AssignmentPort> {
    holidays: H,
    assignments: A,
    /// `holidayMap`: a property's holidays, filled on first use.
    cache: RefCell<HashMap<i32, Vec<Holiday>>>,
}

impl<H: HolidayPort, A: AssignmentPort> HolidayMatcher<H, A> {
    /// Build the matcher over the holiday calendar and the job lookup that
    /// resolves a shift's job to its property.
    pub fn new(holidays: H, assignments: A) -> Self {
        Self {
            holidays,
            assignments,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// The job lookup, for rules that need a job's parent or pay rate as well.
    pub fn assignments(&self) -> &A {
        &self.assignments
    }

    /// Every holiday configured for the property of `job_id`, cached.
    /// `findOrCreateHolidayList`; empty if the job cannot be resolved.
    pub fn holidays_for_job(&self, job_id: i32) -> Vec<Holiday> {
        let Some(job) = self.assignments.find_by_id(job_id) else {
            return Vec::new();
        };
        let property_id = job.property_id();
        self.cache
            .borrow_mut()
            .entry(property_id)
            .or_insert_with(|| self.holidays.find_all_for_property(property_id))
            .clone()
    }

    /// `findOverlappingHolidays(shift, typeIds, useActual)` — the default
    /// midnight–midnight period.
    pub fn find_overlapping_holidays(
        &self,
        shift: &EmployeeShift,
        holiday_type_ids: &[i32],
        use_actual: bool,
    ) -> Vec<Holiday> {
        let midnight = LocalTime::of(0, 0, 0);
        self.find_overlapping_holidays_between(
            shift,
            holiday_type_ids,
            use_actual,
            midnight,
            midnight,
        )
    }

    /// `findOverlappingHolidays(shift, typeIds, useActual, start, end)`.
    pub fn find_overlapping_holidays_between(
        &self,
        shift: &EmployeeShift,
        holiday_type_ids: &[i32],
        use_actual: bool,
        holiday_start: LocalTime,
        holiday_end: LocalTime,
    ) -> Vec<Holiday> {
        if shift.has_errors() || !shift.has_both_times() {
            return Vec::new();
        }
        let (Some(start), Some(end)) = (
            shift.punch_time_of_type(PunchType::In, use_actual),
            shift.punch_time_of_type(PunchType::Out, use_actual),
        ) else {
            return Vec::new();
        };
        let shift_period = DateTimeRange::of(start, end);

        let mut found: Vec<Holiday> = self
            .holidays_for_job(shift.job_id())
            .into_iter()
            .filter(|holiday| {
                shift_period.overlaps(&holiday_period(
                    holiday.holiday_date(),
                    holiday_start,
                    holiday_end,
                )) && holiday_type_ids.contains(&holiday.holiday_type_id())
            })
            .collect();
        found.sort_by_key(|h| (h.holiday_date(), h.id()));
        found.dedup_by_key(|h| (h.holiday_date(), h.id()));
        found
    }
}

/// `getHolidayPeriod(holiday, startTime, endTime)` for a holiday on `date`.
pub fn holiday_period(
    date: LocalDate,
    start_time: LocalTime,
    end_time: LocalTime,
) -> DateTimeRange {
    let start = date.at_time(start_time);
    let mut end = date.at_time(end_time);
    if end <= start {
        end = end.plus_days(1);
    }
    DateTimeRange::of(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use joda_rs::LocalDateTime;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Jobs;
    impl AssignmentPort for Jobs {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            (id != 99).then(|| Assignment::new(id, 7, "job", "J", None))
        }
    }
    struct Calendar(Vec<Holiday>, Rc<Cell<u32>>);
    impl HolidayPort for Calendar {
        fn find_all_for_property(&self, _p: i32) -> Vec<Holiday> {
            self.1.set(self.1.get() + 1);
            self.0.clone()
        }
    }

    fn at(day: i32, h: i32) -> LocalDateTime {
        LocalDateTime::of(2012, 11, day, h, 0, 0)
    }
    fn shift(job: i32, from: LocalDateTime, to: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            1,
            job,
            from.to_local_date(),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Clock, from),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Clock, to),
            ],
        )
        .with_times(Some(from), Some(to))
    }
    fn matcher(holidays: Vec<Holiday>) -> (HolidayMatcher<Calendar, Jobs>, Rc<Cell<u32>>) {
        let reads = Rc::new(Cell::new(0));
        (
            HolidayMatcher::new(Calendar(holidays, reads.clone()), Jobs),
            reads,
        )
    }
    fn thanksgiving(type_id: i32) -> Holiday {
        Holiday::new(1, 7, LocalDate::of(2012, 11, 22), "Thanksgiving", type_id)
    }

    #[test]
    fn a_shift_overlapping_the_holiday_day_with_a_requested_type_matches() {
        let (m, _) = matcher(vec![thanksgiving(5)]);
        let found = m.find_overlapping_holidays(&shift(1, at(21, 23), at(22, 7)), &[5, 6], true);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn another_type_or_a_shift_on_another_day_does_not_match() {
        let (m, _) = matcher(vec![thanksgiving(5)]);
        assert!(
            m.find_overlapping_holidays(&shift(1, at(21, 23), at(22, 7)), &[6], true)
                .is_empty()
        );
        assert!(
            m.find_overlapping_holidays(&shift(1, at(24, 8), at(24, 16)), &[5], true)
                .is_empty()
        );
    }

    #[test]
    fn touching_the_period_counts_as_overlapping() {
        let (m, _) = matcher(vec![thanksgiving(5)]);
        // Ends exactly at midnight when the holiday begins.
        let found = m.find_overlapping_holidays(&shift(1, at(21, 16), at(22, 0)), &[5], true);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn the_period_rolls_to_the_next_day_when_the_end_is_not_after_the_start() {
        let two = LocalTime::of(2, 0, 0);
        let p = holiday_period(LocalDate::of(2012, 11, 22), two, two);
        assert_eq!(p, DateTimeRange::of(at(22, 2), at(23, 2)));
        let midnight = LocalTime::of(0, 0, 0);
        assert_eq!(
            holiday_period(LocalDate::of(2012, 11, 22), midnight, midnight),
            DateTimeRange::of(at(22, 0), at(23, 0))
        );
        assert_eq!(
            holiday_period(
                LocalDate::of(2012, 11, 22),
                LocalTime::of(9, 0, 0),
                LocalTime::of(17, 0, 0)
            ),
            DateTimeRange::of(at(22, 9), at(22, 17))
        );
    }

    #[test]
    fn the_calendar_is_read_once_per_property() {
        let (m, reads) = matcher(vec![thanksgiving(5)]);
        for _ in 0..3 {
            m.find_overlapping_holidays(&shift(1, at(22, 8), at(22, 16)), &[5], true);
        }
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn errored_unfinished_or_unresolvable_shifts_match_nothing() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let (m, _) = matcher(vec![thanksgiving(5)]);
        let s = shift(1, at(22, 8), at(22, 16));
        assert!(
            m.find_overlapping_holidays(
                &s.clone().with_errors(vec![ShiftErrorType::MissingIn]),
                &[5],
                true
            )
            .is_empty()
        );
        assert!(
            m.find_overlapping_holidays(&s.with_times(Some(at(22, 8)), None), &[5], true)
                .is_empty()
        );
        assert!(
            m.find_overlapping_holidays(&shift(99, at(22, 8), at(22, 16)), &[5], true)
                .is_empty()
        );
    }
}
