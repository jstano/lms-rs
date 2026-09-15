//! Date arithmetic the engine needs that `joda_rs` does not provide directly.

use joda_rs::{DayOfWeek, LocalDate};

const SECONDS_PER_DAY: i64 = joda_rs::constants::SECONDS_PER_DAY;

/// Whole days from `from` to `to`, negative when `to` is the earlier date.
pub fn days_between(from: LocalDate, to: LocalDate) -> i64 {
    let epoch_day =
        |date: LocalDate| date.at_start_of_day().epoch_seconds().div_euclid(SECONDS_PER_DAY);

    epoch_day(to) - epoch_day(from)
}

/// The seven dates of the week `date` falls in, given the day the week ends on.
///
/// Weeks are property-configured rather than fixed to a calendar convention, so
/// the ending day has to be supplied. The returned dates run in order, ending
/// with `week_ending_day`.
pub fn week_containing(date: LocalDate, week_ending_day: DayOfWeek) -> Vec<LocalDate> {
    let days_until_end =
        (week_ending_day.value() - date.day_of_week().value()).rem_euclid(7) as i64;
    let week_end = date.plus_days(days_until_end);

    (0..7).map(|offset| week_end.plus_days(offset - 6)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(LocalDate::new(2013, 7, 24), LocalDate::new(2013, 7, 24), 0)]
    #[case(LocalDate::new(2013, 7, 24), LocalDate::new(2013, 7, 25), 1)]
    #[case(LocalDate::new(2013, 7, 25), LocalDate::new(2013, 7, 24), -1)]
    #[case(LocalDate::new(2013, 7, 24), LocalDate::new(2013, 8, 7), 14)]
    // Across a year boundary, and a leap day.
    #[case(LocalDate::new(2013, 12, 31), LocalDate::new(2014, 1, 1), 1)]
    #[case(LocalDate::new(2016, 2, 28), LocalDate::new(2016, 3, 1), 2)]
    fn counts_whole_days_between_dates(
        #[case] from: LocalDate,
        #[case] to: LocalDate,
        #[case] expected: i64,
    ) {
        assert_eq!(days_between(from, to), expected);
    }

    #[test]
    fn a_week_ends_on_the_configured_day() {
        // 2013-07-24 is a Wednesday; with weeks ending Saturday the week runs
        // Sunday the 21st through Saturday the 27th.
        let week = week_containing(LocalDate::new(2013, 7, 24), DayOfWeek::Saturday);

        assert_eq!(week.len(), 7);
        assert_eq!(week[0], LocalDate::new(2013, 7, 21));
        assert_eq!(week[6], LocalDate::new(2013, 7, 27));
    }

    #[test]
    fn a_date_already_on_the_ending_day_ends_its_own_week() {
        // 2013-07-27 is a Saturday.
        let week = week_containing(LocalDate::new(2013, 7, 27), DayOfWeek::Saturday);

        assert_eq!(week[0], LocalDate::new(2013, 7, 21));
        assert_eq!(week[6], LocalDate::new(2013, 7, 27));
    }

    #[test]
    fn the_week_always_contains_the_date_it_was_built_from() {
        let date = LocalDate::new(2013, 7, 24);

        for ending_day in [
            DayOfWeek::Monday,
            DayOfWeek::Tuesday,
            DayOfWeek::Wednesday,
            DayOfWeek::Thursday,
            DayOfWeek::Friday,
            DayOfWeek::Saturday,
            DayOfWeek::Sunday,
        ] {
            let week = week_containing(date, ending_day);

            assert_eq!(week.len(), 7);
            assert!(week.contains(&date), "week ending {ending_day:?} lost its own date");
            assert_eq!(week[6].day_of_week(), ending_day);
        }
    }
}
