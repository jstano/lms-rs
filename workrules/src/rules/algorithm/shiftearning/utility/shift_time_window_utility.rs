//! Port of `ShiftTimeWindowUtility`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/utility/ShiftTimeWindowUtility.java`.
//!
//! A configured "starts between these two times" window can itself cross
//! midnight, and the shift being checked against it can fall on the day
//! before or after the reference date and still belong to that window — so
//! this builds the window three times over, anchored the day before, on, and
//! after `reference_date`, and a caller checks all three for a match.
//!
//! # `null` inputs are not reachable here
//!
//! `cannotCheckTimeRange` returns an empty list when any of the three
//! arguments is null. Every call site in this crate reaches this after
//! `RuleParams::fixed`, which guarantees a value for every configured key, so
//! the null case is not reproduced — the signature takes owned
//! `LocalDate`/`LocalTime` rather than `Option`.

use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime, LocalTime};

/// The three candidate day-anchored windows a shift starting near
/// `reference_date` might fall into. `getTimeRangesIncludingOverlaps`.
pub fn time_ranges_including_overlaps(
    reference_date: LocalDate,
    earliest_time: LocalTime,
    latest_time: LocalTime,
) -> Vec<DateTimeRange> {
    (-1i64..=1)
        .map(|offset| {
            let date = reference_date.plus_days(offset);
            let start = convert_start_time(date, earliest_time);
            let end = convert_end_time(date, earliest_time, latest_time);
            DateTimeRange::of(start, end)
        })
        .collect()
}

/// `DTUtil.convertStartTime`.
fn convert_start_time(date: LocalDate, time: LocalTime) -> LocalDateTime {
    date.at_time(time)
}

/// `DTUtil.convertEndTime` — rolls to the next day when the end time is
/// before the start time, so a window spanning midnight still ends after it
/// starts.
fn convert_end_time(date: LocalDate, start_time: LocalTime, end_time: LocalTime) -> LocalDateTime {
    let end = date.at_time(end_time);
    if end_time.is_before(start_time) {
        end.plus_days(1)
    } else {
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_windows_are_built_around_the_reference_date() {
        let windows = time_ranges_including_overlaps(
            LocalDate::of(2014, 5, 6),
            LocalTime::of(9, 0, 0),
            LocalTime::of(17, 0, 0),
        );

        assert_eq!(windows.len(), 3);
        assert_eq!(
            windows[0],
            DateTimeRange::of(
                LocalDate::of(2014, 5, 5).at_time(LocalTime::of(9, 0, 0)),
                LocalDate::of(2014, 5, 5).at_time(LocalTime::of(17, 0, 0)),
            )
        );
        assert_eq!(
            windows[1],
            DateTimeRange::of(
                LocalDate::of(2014, 5, 6).at_time(LocalTime::of(9, 0, 0)),
                LocalDate::of(2014, 5, 6).at_time(LocalTime::of(17, 0, 0)),
            )
        );
        assert_eq!(
            windows[2],
            DateTimeRange::of(
                LocalDate::of(2014, 5, 7).at_time(LocalTime::of(9, 0, 0)),
                LocalDate::of(2014, 5, 7).at_time(LocalTime::of(17, 0, 0)),
            )
        );
    }

    #[test]
    fn a_window_crossing_midnight_ends_the_day_after_it_starts() {
        let windows = time_ranges_including_overlaps(
            LocalDate::of(2014, 5, 6),
            LocalTime::of(17, 0, 0),
            LocalTime::of(1, 0, 0),
        );

        assert_eq!(
            windows[1],
            DateTimeRange::of(
                LocalDate::of(2014, 5, 6).at_time(LocalTime::of(17, 0, 0)),
                LocalDate::of(2014, 5, 7).at_time(LocalTime::of(1, 0, 0)),
            )
        );
    }
}
