//! Port of `ShiftTimeInPeriodCalculator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/ShiftTimeInPeriodCalculator.java`.
//!
//! How many hours of a shift fall inside a period, optionally less the break
//! time that falls inside it. Used by the holiday rules to measure the part of
//! a shift worked on a holiday.
//!
//! The shift's span is first IN to last-listed OUT (`ShiftUtil.getPunchTime`,
//! adjusted or rounded). Overlap uses **inclusive** touching, as
//! `DateTimePeriod.overlapsPeriod` does, so a shift that merely ends where the
//! period begins overlaps it — by zero hours, which is harmless — and a break
//! entirely outside the period overlaps it by zero too.
//!
//! A shift with errors or without both times is worth zero; one without an IN
//! or OUT punch is too (Java would throw).

use crate::common::enums::punch_type::PunchType;
use crate::entity::employee_shift::EmployeeShift;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;

/// Hours of `shift` inside `period`. `ShiftTimeInPeriodCalculator.calculateTimeInPeriod`.
pub fn calculate_time_in_period(
    shift: &EmployeeShift,
    period: &DateTimeRange,
    use_actual: bool,
    deduct_breaks: bool,
) -> f64 {
    if shift.has_errors() || !shift.has_both_times() {
        return 0.0;
    }
    let (Some(start), Some(end)) = (
        shift.punch_time_of_type(PunchType::In, use_actual),
        shift.punch_time_of_type(PunchType::Out, use_actual),
    ) else {
        return 0.0;
    };

    let mut time = overlap_hours(&DateTimeRange::of(start, end), period);
    if deduct_breaks {
        for brk in shift.breaks_using(use_actual) {
            time -= overlap_hours(&brk, period);
        }
    }
    time
}

/// `calculateOverlapTime`.
fn overlap_hours(a: &DateTimeRange, b: &DateTimeRange) -> f64 {
    if !a.overlaps(b) {
        return 0.0;
    }
    let start = if b.start() > a.start() {
        b.start()
    } else {
        a.start()
    };
    let end = if b.end() < a.end() { b.end() } else { a.end() };
    DateTimeRange::of(start, end).duration().fractional_hours()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use joda_rs::{LocalDate, LocalDateTime};

    fn at(day: i32, h: i32, m: i32) -> LocalDateTime {
        LocalDateTime::of(2012, 11, day, h, m, 0)
    }
    fn p(id: i32, t: PunchType, when: LocalDateTime) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, t, PunchSource::Clock, when)
    }
    fn shift(
        punches: Vec<EmployeeShiftPunch>,
        start: LocalDateTime,
        end: LocalDateTime,
    ) -> EmployeeShift {
        EmployeeShift::new(
            1,
            1,
            1,
            LocalDate::of(2012, 11, 22),
            ShiftType::Actual,
            punches,
        )
        .with_times(Some(start), Some(end))
    }
    fn day22() -> DateTimeRange {
        DateTimeRange::of(at(22, 0, 0), at(23, 0, 0))
    }

    #[test]
    fn only_the_part_of_the_shift_inside_the_period_counts() {
        let s = shift(
            vec![
                p(1, PunchType::In, at(21, 23, 0)),
                p(2, PunchType::Out, at(22, 7, 0)),
            ],
            at(21, 23, 0),
            at(22, 7, 0),
        );
        assert_eq!(calculate_time_in_period(&s, &day22(), true, true), 7.0);
        let s = shift(
            vec![
                p(1, PunchType::In, at(22, 20, 0)),
                p(2, PunchType::Out, at(23, 5, 0)),
            ],
            at(22, 20, 0),
            at(23, 5, 0),
        );
        assert_eq!(calculate_time_in_period(&s, &day22(), true, true), 4.0);
    }

    #[test]
    fn breaks_inside_the_period_are_deducted_only_when_asked() {
        let s = shift(
            vec![
                p(1, PunchType::In, at(22, 8, 0)),
                p(2, PunchType::Break, at(22, 12, 0)),
                p(3, PunchType::Back, at(22, 12, 30)),
                p(4, PunchType::Out, at(22, 16, 0)),
            ],
            at(22, 8, 0),
            at(22, 16, 0),
        );
        assert_eq!(calculate_time_in_period(&s, &day22(), true, true), 7.5);
        assert_eq!(calculate_time_in_period(&s, &day22(), true, false), 8.0);
    }

    #[test]
    fn a_break_straddling_the_period_edge_is_deducted_only_for_its_overlap() {
        // Break 21:30 on the 21st to 00:45 on the 22nd: 45 minutes are in the 22nd.
        let s = shift(
            vec![
                p(1, PunchType::In, at(21, 20, 0)),
                p(2, PunchType::Break, at(21, 23, 30)),
                p(3, PunchType::Back, at(22, 0, 45)),
                p(4, PunchType::Out, at(22, 4, 0)),
            ],
            at(21, 20, 0),
            at(22, 4, 0),
        );
        assert_eq!(calculate_time_in_period(&s, &day22(), true, true), 3.25);
    }

    #[test]
    fn an_errored_unfinished_or_punchless_shift_is_worth_zero() {
        use crate::common::enums::shift_error_type::ShiftErrorType;
        let ok = shift(
            vec![
                p(1, PunchType::In, at(22, 8, 0)),
                p(2, PunchType::Out, at(22, 16, 0)),
            ],
            at(22, 8, 0),
            at(22, 16, 0),
        );
        assert_eq!(
            calculate_time_in_period(
                &ok.clone().with_errors(vec![ShiftErrorType::MissingIn]),
                &day22(),
                true,
                true
            ),
            0.0
        );
        assert_eq!(
            calculate_time_in_period(
                &ok.clone().with_times(Some(at(22, 8, 0)), None),
                &day22(),
                true,
                true
            ),
            0.0
        );
        let no_punches = shift(vec![], at(22, 8, 0), at(22, 16, 0));
        assert_eq!(
            calculate_time_in_period(&no_punches, &day22(), true, true),
            0.0
        );
    }

    #[test]
    fn a_shift_outside_the_period_is_worth_zero() {
        let s = shift(
            vec![
                p(1, PunchType::In, at(24, 8, 0)),
                p(2, PunchType::Out, at(24, 16, 0)),
            ],
            at(24, 8, 0),
            at(24, 16, 0),
        );
        assert_eq!(calculate_time_in_period(&s, &day22(), true, true), 0.0);
    }
}
