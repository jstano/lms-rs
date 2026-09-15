use crate::id_type;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::standard_set::StandardSetId;
use date_range_rs::DateTimeRange;
use joda_rs::{DayOfWeek, LocalDate, LocalTime};

id_type!(JobShiftId, uuid_v4);

/// A shift template on a job: a named pattern of start and end times, one
/// definition per day of week it runs on.
pub struct JobShift {
    id: JobShiftId,
    job_id: JobId,
    standard_set_id: StandardSetId,
    name: String,
    sequence: u32,
    shift_definitions: Vec<JobShiftDefinition>,
}

impl JobShift {
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        name: String,
        sequence: u32,
        shift_definitions: Vec<JobShiftDefinition>,
    ) -> Self {
        Self {
            id: JobShiftId::new(),
            job_id,
            standard_set_id,
            name,
            sequence,
            shift_definitions,
        }
    }

    pub fn id(&self) -> JobShiftId {
        self.id
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sequence(&self) -> u32 {
        self.sequence
    }

    /// The definition that applies on `date`, if the shift runs that day.
    ///
    /// This stands in for Java's `AssignmentShiftDetailService`, which resolves
    /// a detail through the operational environment for the date; this port
    /// keys the definitions on day of week directly.
    pub fn shift_detail_for_date(&self, date: LocalDate) -> Option<&JobShiftDefinition> {
        self.shift_definitions
            .iter()
            .find(|shift_definition| shift_definition.day_of_week() == date.day_of_week())
    }

    #[cfg(test)]
    pub fn test() -> Self {
        Self::new(
            JobId::new(),
            StandardSetId::new(),
            String::new(),
            0,
            Vec::new(),
        )
    }
}

/// The times a shift template runs on one day of the week.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JobShiftDefinition {
    day_of_week: DayOfWeek,
    start_time: Option<LocalTime>,
    end_time: Option<LocalTime>,
    hours_before: f64,
    hours_after: f64,
    min_number_shifts: u32,
}

impl JobShiftDefinition {
    pub fn new(
        day_of_week: DayOfWeek,
        start_time: LocalTime,
        end_time: LocalTime,
        hours_before: f64,
        hours_after: f64,
        min_number_shifts: u32,
    ) -> Self {
        Self {
            day_of_week,
            start_time: Some(start_time),
            end_time: Some(end_time),
            hours_before,
            hours_after,
            min_number_shifts,
        }
    }

    /// A definition for a day the shift is configured on but has no times set.
    pub fn without_times(day_of_week: DayOfWeek) -> Self {
        Self {
            day_of_week,
            start_time: None,
            end_time: None,
            hours_before: 0.0,
            hours_after: 0.0,
            min_number_shifts: 0,
        }
    }

    pub fn day_of_week(&self) -> DayOfWeek {
        self.day_of_week
    }

    pub fn start_time(&self) -> Option<LocalTime> {
        self.start_time
    }

    pub fn end_time(&self) -> Option<LocalTime> {
        self.end_time
    }

    pub fn hours_before(&self) -> f64 {
        self.hours_before
    }

    pub fn hours_after(&self) -> f64 {
        self.hours_after
    }

    pub fn min_number_shifts(&self) -> u32 {
        self.min_number_shifts
    }

    pub fn has_times(&self) -> bool {
        self.start_time.is_some() && self.end_time.is_some()
    }

    /// The nominal length of the shift in fractional hours.
    ///
    /// Equal start and end times mean a round-the-clock shift rather than a
    /// zero-length one; otherwise the shift may wrap past midnight.
    pub fn shift_length(&self) -> f64 {
        match (self.start_time, self.end_time) {
            (Some(start_time), Some(end_time)) => {
                if start_time == end_time {
                    joda_rs::constants::HOURS_PER_DAY as f64
                } else {
                    duration_in_fractional_hours(start_time, end_time)
                }
            }
            _ => 0.0,
        }
    }

    /// The shift placed on a concrete date, rolling the end into the next day
    /// when the shift wraps past midnight.
    pub fn to_date_time_range(&self, date: LocalDate) -> Option<DateTimeRange> {
        let start_time = self.start_time?;
        let end_time = self.end_time?;

        let end_date = if end_time.is_on_or_before(start_time) {
            date.plus_days(1)
        } else {
            date
        };

        Some(DateTimeRange::of(
            date.at_time(start_time),
            end_date.at_time(end_time),
        ))
    }
}

/// Hours from `start_time` to `end_time`, wrapping past midnight when the end
/// is earlier in the day than the start.
fn duration_in_fractional_hours(start_time: LocalTime, end_time: LocalTime) -> f64 {
    let start_seconds = second_of_day(start_time);
    let end_seconds = second_of_day(end_time);
    let seconds_per_hour = joda_rs::constants::SECONDS_PER_HOUR as f64;

    if start_seconds > end_seconds {
        joda_rs::constants::HOURS_PER_DAY as f64
            - ((start_seconds - end_seconds) as f64 / seconds_per_hour)
    } else {
        (end_seconds - start_seconds) as f64 / seconds_per_hour
    }
}

fn second_of_day(time: LocalTime) -> i32 {
    time.hour() * joda_rs::constants::SECONDS_PER_HOUR as i32
        + time.minute() * joda_rs::constants::SECONDS_PER_MINUTE as i32
        + time.second()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn definition(start: LocalTime, end: LocalTime) -> JobShiftDefinition {
        JobShiftDefinition::new(DayOfWeek::Monday, start, end, 0.0, 0.0, 1)
    }

    fn time(hour: i32, minute: i32) -> LocalTime {
        LocalTime::of_hour_minute(hour, minute)
    }

    #[test]
    fn a_definition_with_times_has_times() {
        assert!(definition(time(9, 0), time(17, 0)).has_times());
        assert!(!JobShiftDefinition::without_times(DayOfWeek::Monday).has_times());
    }

    #[rstest]
    #[case(9, 0, 17, 0, 8.0)]
    #[case(9, 30, 17, 0, 7.5)]
    #[case(0, 0, 0, 0, 24.0)]
    #[case(22, 0, 6, 0, 8.0)]
    #[case(23, 45, 0, 15, 0.5)]
    fn shift_length_handles_wrapping_past_midnight(
        #[case] start_hour: i32,
        #[case] start_minute: i32,
        #[case] end_hour: i32,
        #[case] end_minute: i32,
        #[case] expected: f64,
    ) {
        let definition = definition(
            time(start_hour, start_minute),
            time(end_hour, end_minute),
        );

        assert!((definition.shift_length() - expected).abs() < 1e-9);
    }

    #[test]
    fn a_definition_without_times_has_no_length() {
        assert_eq!(
            JobShiftDefinition::without_times(DayOfWeek::Monday).shift_length(),
            0.0
        );
    }

    #[test]
    fn a_same_day_shift_stays_on_the_date() {
        let date = LocalDate::new(2025, 10, 6);
        let range = definition(time(9, 0), time(17, 0))
            .to_date_time_range(date)
            .unwrap();

        assert_eq!(range.start(), date.at_time(time(9, 0)));
        assert_eq!(range.end(), date.at_time(time(17, 0)));
    }

    #[test]
    fn an_overnight_shift_ends_on_the_next_date() {
        let date = LocalDate::new(2025, 10, 6);
        let range = definition(time(22, 0), time(6, 0))
            .to_date_time_range(date)
            .unwrap();

        assert_eq!(range.start(), date.at_time(time(22, 0)));
        assert_eq!(range.end(), date.plus_days(1).at_time(time(6, 0)));
    }

    #[test]
    fn a_round_the_clock_shift_ends_on_the_next_date() {
        let date = LocalDate::new(2025, 10, 6);
        let range = definition(time(0, 0), time(0, 0))
            .to_date_time_range(date)
            .unwrap();

        assert_eq!(range.end(), date.plus_days(1).at_time(time(0, 0)));
    }

    #[test]
    fn a_definition_without_times_has_no_range() {
        assert!(
            JobShiftDefinition::without_times(DayOfWeek::Monday)
                .to_date_time_range(LocalDate::new(2025, 10, 6))
                .is_none()
        );
    }

    #[test]
    fn the_detail_for_a_date_is_the_one_for_its_day_of_week() {
        let monday = JobShiftDefinition::new(DayOfWeek::Monday, time(9, 0), time(17, 0), 0.0, 0.0, 1);
        let tuesday =
            JobShiftDefinition::new(DayOfWeek::Tuesday, time(10, 0), time(18, 0), 0.0, 0.0, 1);
        let shift = JobShift::new(
            JobId::new(),
            StandardSetId::new(),
            "Day".to_string(),
            1,
            vec![monday, tuesday],
        );

        // 2025-10-06 is a Monday, 2025-10-07 a Tuesday, 2025-10-08 a Wednesday.
        assert_eq!(
            shift.shift_detail_for_date(LocalDate::new(2025, 10, 6)),
            Some(&monday)
        );
        assert_eq!(
            shift.shift_detail_for_date(LocalDate::new(2025, 10, 7)),
            Some(&tuesday)
        );
        assert_eq!(shift.shift_detail_for_date(LocalDate::new(2025, 10, 8)), None);
    }
}
