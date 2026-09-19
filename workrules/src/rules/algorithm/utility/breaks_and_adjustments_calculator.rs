//! Port of
//! `com.unifocus.watson.server.labor.rules.algorithm.utility.BreaksAndAdjustmentsCalculator`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/utility/BreaksAndAdjustmentsCalculator.java`.
//!
//! How a shift's breaks and adjustment hours get split across a midnight
//! boundary, for the `regularhoursdistribution` rules that split one shift
//! into a distribution per calendar day. What's returned is a pair of
//! **corrections** — how many hours to add to or take off the naive duration
//! of each half — not the break or adjustment totals themselves.
//!
//! # The two halves are corrected independently
//!
//! `calculateBreaksAndAdjustments` always measures against midnight on the
//! shift's own date, not against whatever split point the caller is actually
//! using — `RegularHoursByDayRuleImpl`'s split can be any configured time, but
//! this always assumes midnight. That is faithful to the Java, which has the
//! same mismatch.
//!
//! Breaks overlapping each half are subtracted (a break inside the naive
//! duration was never worked); the adjustment is apportioned by what
//! proportion of the shift's *worked* hours (breaks already excluded) falls in
//! the first half, rounded with [`round_raw_hours`], and the remainder goes to
//! the second half. Zero adjustment hours skips the apportionment entirely,
//! rather than computing a zero-times-something.

use crate::entity::employee_shift::EmployeeShift;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDateTime;

/// The correction to apply to each half of a split shift.
/// `TTuple2<Double, Double>` in Java — first day, next day.
///
/// `BreaksAndAdjustmentsCalculator.calculateBreaksAndAdjustments`.
pub fn calculate_breaks_and_adjustments(shift: &EmployeeShift) -> (f64, f64) {
    let day_split = midnight_after(shift);
    let first_day_range = DateTimeRange::of(shift.start_date_time().unwrap(), day_split);
    let next_day_range = DateTimeRange::of(day_split, shift.end_date_time().unwrap());

    let (first_day_breaks, next_day_breaks) =
        calculate_break_adjustments(shift, &first_day_range, &next_day_range);
    let (first_day_adjustments, next_day_adjustments) =
        calculate_adjustments(shift, &first_day_range, first_day_breaks);

    (
        first_day_adjustments - first_day_breaks,
        next_day_adjustments - next_day_breaks,
    )
}

/// Midnight starting the day after the shift's date. `startDate.plusDays(1)`.
fn midnight_after(shift: &EmployeeShift) -> LocalDateTime {
    shift.shift_date().plus_days(1).at_start_of_day()
}

/// How many hours of break time fall in each half. `calculateBreakAdjustments`.
fn calculate_break_adjustments(
    shift: &EmployeeShift,
    first_day_range: &DateTimeRange,
    next_day_range: &DateTimeRange,
) -> (f64, f64) {
    shift
        .breaks()
        .iter()
        .fold((0.0, 0.0), |(first, next), shift_break| {
            (
                first
                    + shift_break
                        .overlap_duration(first_day_range)
                        .fractional_hours(),
                next + shift_break
                    .overlap_duration(next_day_range)
                    .fractional_hours(),
            )
        })
}

/// How the shift's adjustment hours split across the two halves.
/// `calculateAdjustments`.
fn calculate_adjustments(
    shift: &EmployeeShift,
    first_day_range: &DateTimeRange,
    first_day_breaks: f64,
) -> (f64, f64) {
    let adjustment_hours = shift.adj_hours();
    if adjustment_hours == 0.0 {
        return (0.0, 0.0);
    }

    let first_day_proportion =
        (first_day_range.duration().fractional_hours() - first_day_breaks) / shift.worked_hours();

    let first_day_adjustments =
        crate::common::numbers::round_raw_hours(adjustment_hours * first_day_proportion);
    let next_day_adjustments = adjustment_hours - first_day_adjustments;

    (first_day_adjustments, next_day_adjustments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::punch_type::PunchType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use joda_rs::LocalDate;

    fn at(day: i32, hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, day, hour, minute, 0)
    }

    fn punch(
        id: i32,
        punch_type: PunchType,
        day: i32,
        hour: i32,
        minute: i32,
    ) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Clock, at(day, hour, minute))
    }

    /// A punch's rounded time is set at construction, but the shift's own
    /// start/end and worked hours are only recomputed when
    /// [`EmployeeShift::punch_cursor`] fires — and it skips a write that
    /// doesn't change the value. So force it through `None` first.
    fn settle_shift_start_and_end(
        mut shift: EmployeeShift,
        in_index: usize,
        out_index: usize,
    ) -> EmployeeShift {
        for index in [in_index, out_index] {
            let time = shift.punch(index).rounded_time();
            shift.punch_cursor(index).set_rounded_time(None);
            shift.punch_cursor(index).set_rounded_time(time);
        }
        shift
    }

    /// An overnight shift, 22:00 to 02:00, with a 15-minute break at 23:00.
    fn overnight_shift_with_break() -> EmployeeShift {
        let shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, 2, 22, 0),
                punch(2, PunchType::Break, 2, 23, 0),
                punch(3, PunchType::Back, 2, 23, 15),
                punch(4, PunchType::Out, 3, 2, 0),
            ],
        );
        settle_shift_start_and_end(shift, 0, 3)
    }

    #[test]
    fn a_break_inside_the_first_half_is_subtracted_from_it_alone() {
        let shift = overnight_shift_with_break();

        let (first_day, next_day) = calculate_breaks_and_adjustments(&shift);

        assert_eq!(first_day, -0.25);
        assert_eq!(next_day, 0.0);
    }

    #[test]
    fn zero_adjustment_hours_skips_apportionment() {
        let shift = overnight_shift_with_break().with_adj_hours(0.0);

        let (first_day, _) = calculate_breaks_and_adjustments(&shift);

        // Same as the no-adjustment case: nothing but the break correction.
        assert_eq!(first_day, -0.25);
    }

    #[test]
    fn an_adjustment_is_apportioned_by_the_first_halfs_share_of_worked_hours() {
        // No break, so worked hours are the naive 4-hour span, split evenly
        // by the midnight boundary: 2 hours before, 2 after.
        let shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            vec![
                punch(1, PunchType::In, 2, 22, 0),
                punch(2, PunchType::Out, 3, 2, 0),
            ],
        );
        let shift = settle_shift_start_and_end(shift, 0, 1).with_adj_hours(2.0);

        let (first_day, next_day) = calculate_breaks_and_adjustments(&shift);

        assert_eq!(first_day, 1.0);
        assert_eq!(next_day, 1.0);
    }
}
