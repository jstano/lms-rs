//! Port of `DailyData`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/hoursdistribution/DailyData.java`.
//!
//! One day of a work week, split the way the rules consume it: the earnings
//! that stand on their own, and the shifts worked that day each paired with the
//! earnings attached to it. `CaliforniaOTHrsRuleImpl` builds a map of these
//! keyed by date and walks the week.
//!
//! # Watch the name
//!
//! `CaliforniaExtSpecialJobOTHrsRuleImpl` declares a **private inner class also
//! called `DailyData`**, with a different three-argument constructor
//! (`date`, shifts, a day-of-week flag) and no earnings. It shadows this one
//! inside that file. Do not port that rule against this struct.
//!
//! # Indices, not references
//!
//! Java holds `EmployeeShift` and `EmployeeEarning` references, and the rule
//! then writes distributions onto those shifts while still reading the time
//! card — divergence 22's aliasing problem. So this holds positions into
//! [`TimeCard::shifts`](crate::entity::time_card::TimeCard::shifts) and
//! [`TimeCard::earnings`](crate::entity::time_card::TimeCard::earnings), and a
//! rule resolves them against
//! [`shifts_mut`](crate::entity::time_card::TimeCard::shifts_mut) when it is
//! ready to write.

use joda_rs::LocalDate;

/// One shift and the earnings attached to it.
/// `TTuples.TTuple2<EmployeeShift, List<EmployeeEarning>>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShiftWithEarnings {
    shift: usize,
    earnings: Vec<usize>,
}

impl ShiftWithEarnings {
    /// Pair a shift position with the positions of its earnings.
    pub fn new(shift: usize, earnings: Vec<usize>) -> Self {
        Self { shift, earnings }
    }

    /// The shift's position in `TimeCard::shifts`. `getValue1()`.
    pub fn shift(&self) -> usize {
        self.shift
    }

    /// The positions in `TimeCard::earnings` of the earnings on this shift,
    /// which is empty for most shifts. `getValue2()`.
    pub fn earnings(&self) -> &[usize] {
        &self.earnings
    }
}

/// One day of a work week. `DailyData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyData {
    date: LocalDate,
    earnings: Vec<usize>,
    shifts: Vec<ShiftWithEarnings>,
}

impl DailyData {
    /// A day, its standalone earnings, and its shifts with theirs.
    pub fn new(date: LocalDate, earnings: Vec<usize>, shifts: Vec<ShiftWithEarnings>) -> Self {
        Self {
            date,
            earnings,
            shifts,
        }
    }

    /// `getDate()`.
    pub fn date(&self) -> LocalDate {
        self.date
    }

    /// The earnings dated this day that are **not** attached to a shift —
    /// Java partitions on `earning.getShift() != null` and these are the
    /// `false` side. `getEarnings()`.
    pub fn earnings(&self) -> &[usize] {
        &self.earnings
    }

    /// The shifts worked this day, in start-time order, each with its own
    /// earnings. `getShifts()`.
    pub fn shifts(&self) -> &[ShiftWithEarnings] {
        &self.shifts
    }

    /// Whether this day was worked — the only question
    /// `WeeklyAccumulator.updateConsecutiveDays(DailyData)` asks of it.
    ///
    /// Note it tests the **shifts**, not the earnings: a day with a holiday
    /// earning and no shift breaks a consecutive-day run.
    pub fn was_worked(&self) -> bool {
        !self.shifts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> LocalDate {
        LocalDate::of(2010, 1, 4)
    }

    #[test]
    fn a_day_carries_its_shifts_and_their_earnings() {
        let data = DailyData::new(
            date(),
            vec![7],
            vec![
                ShiftWithEarnings::new(0, vec![1, 2]),
                ShiftWithEarnings::new(3, Vec::new()),
            ],
        );

        assert_eq!(data.date(), date());
        assert_eq!(data.earnings(), &[7]);
        assert_eq!(data.shifts().len(), 2);
        assert_eq!(data.shifts()[0].shift(), 0);
        assert_eq!(data.shifts()[0].earnings(), &[1, 2]);
        assert!(data.shifts()[1].earnings().is_empty());
    }

    #[test]
    fn a_day_with_a_shift_was_worked() {
        let data = DailyData::new(date(), Vec::new(), vec![ShiftWithEarnings::new(0, vec![])]);

        assert!(data.was_worked());
    }

    #[test]
    fn a_day_with_only_earnings_was_not_worked() {
        // A holiday earning and no shift: the consecutive-day run breaks.
        let data = DailyData::new(date(), vec![7], Vec::new());

        assert!(!data.was_worked());
    }

    #[test]
    fn an_empty_day_was_not_worked() {
        assert!(!DailyData::new(date(), Vec::new(), Vec::new()).was_worked());
    }
}
