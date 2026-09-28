//! `FNYPeriod.java` (`watson.labor.data`) — a property's financial year, broken into periods each
//! with a start/end date. Only the slice `KBIReaderWriter`/`PercentOfBaseKBIAnalyzer` touch
//! (`getStartDate`/`getEndDate`/period lookup by date) is ported, per `PLAN_FORECASTER.md`'s
//! "narrow local stubs, not full ports" guidance for cross-cutting types.

use joda_rs::LocalDate;

#[derive(Debug, Clone, PartialEq)]
pub struct FnyPeriod {
    pub year: i32,
    /// One `(start, end)` pair per period, 0-indexed; `None` for a period the DB hasn't
    /// configured dates for yet (`FNYPeriod.getStartDate`/`getEndDate` return `null` in that case).
    pub periods: Vec<Option<(LocalDate, LocalDate)>>,
}

impl FnyPeriod {
    /// `KBIReaderWriter.computePercentOfBaseKBI`'s inline period-lookup loop:
    /// `date.isGreaterOrEqual(periods.getStartDate(p)) && date.isLessOrEqual(periods.getEndDate(p))`.
    /// Returns the 0-based period index, matching that loop's `periodNo`.
    pub fn period_containing(&self, date: LocalDate) -> Option<usize> {
        self.periods.iter().position(|period| match period {
            Some((start, end)) => date >= *start && date <= *end,
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_containing_finds_the_matching_period() {
        let period = FnyPeriod {
            year: 2026,
            periods: vec![
                Some((LocalDate::of(2026, 1, 1), LocalDate::of(2026, 1, 31))),
                Some((LocalDate::of(2026, 2, 1), LocalDate::of(2026, 2, 28))),
            ],
        };
        assert_eq!(period.period_containing(LocalDate::of(2026, 2, 15)), Some(1));
        assert_eq!(period.period_containing(LocalDate::of(2026, 3, 1)), None);
    }

    #[test]
    fn period_containing_skips_unconfigured_periods() {
        let period = FnyPeriod {
            year: 2026,
            periods: vec![None, Some((LocalDate::of(2026, 2, 1), LocalDate::of(2026, 2, 28)))],
        };
        assert_eq!(period.period_containing(LocalDate::of(2026, 2, 15)), Some(1));
    }
}
