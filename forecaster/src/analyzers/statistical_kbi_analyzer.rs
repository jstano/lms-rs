//! `StatisticalKBIAnalyzer.java` — picks which of TAES or regression analysis a `StatisticalKBI`
//! uses for a given date, based on that date's day-of-week slot in `StatisticalKBI.statDays`.
//!
//! `getAbsoluteDayIndex` comes from `KBIReaderWriter.getDates()` — the forecast run's overall date
//! period (`TDatePeriod`), which only exists once the Phase 3/4 orchestrator sets one up. That
//! single lookup is the only I/O this class does, so it's the only thing behind a port; the
//! dispatch itself (which `StatKbi` variant maps to which analyzer) is a plain match, ported here
//! in full and unit-tested directly against `kbi::StatKbi` values, with no fake needed beyond the
//! index lookup.

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::kbi::{StatKbi, StatKbiType, StatRelatedKbi};

/// `KBIReaderWriter.getDates()` + `TDatePeriod.getAbsoluteDayIndex(dowIndex)`, collapsed to "which
/// slot of `statDays` applies to this date". Java throws `IllegalStateException` when the computed
/// index is negative; that maps to `ForecasterError::InvalidDate` here.
pub trait StatisticalDispatch {
    fn absolute_day_index(&self, date: LocalDate) -> Result<usize, ForecasterError>;
}

/// What `StatisticalKBIAnalyzer.calculate` would hand off to next: either a TAES analysis against
/// one related KBI, or a regression analysis against a list of related KBIs. Phase 4's orchestrator
/// is what actually owns both `TaesAnalyzer`/`RegressionAnalyzer` port implementations and calls
/// them from here.
#[derive(Debug, Clone, PartialEq)]
pub enum StatisticalDispatchResult {
    /// `StatKBIType.TAES` — `statKBI.getRelatedKBIs().get(0)`.
    Taes { related_kbi: StatRelatedKbi },
    /// `StatKBIType.REGRESSION` — the full related-KBI list feeds `RegressionAnalyzer`.
    Regression { related_kbis: Vec<StatRelatedKbi> },
}

pub struct StatisticalKbiAnalyzer;

impl StatisticalKbiAnalyzer {
    /// `calculate(TDate date)`'s dispatch decision (the part before constructing and invoking a
    /// `TAESAnalyzer`/`RegressionAnalyzer`).
    pub fn dispatch(
        port: &dyn StatisticalDispatch,
        stat_days: &[StatKbi; 7],
        date: LocalDate,
    ) -> Result<StatisticalDispatchResult, ForecasterError> {
        let index = port.absolute_day_index(date)?;
        let stat_kbi = &stat_days[index];

        Ok(match stat_kbi.kbi_type {
            StatKbiType::Taes => StatisticalDispatchResult::Taes {
                related_kbi: stat_kbi.related_kbis[0],
            },
            StatKbiType::Regression => StatisticalDispatchResult::Regression {
                related_kbis: stat_kbi.related_kbis.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kbi::StatOpType;
    use crate::KbiId;

    struct FixedIndex(usize);

    impl StatisticalDispatch for FixedIndex {
        fn absolute_day_index(&self, _date: LocalDate) -> Result<usize, ForecasterError> {
            Ok(self.0)
        }
    }

    #[test]
    fn dispatches_to_taes_for_a_taes_configured_day() {
        let mut stat_days: [StatKbi; 7] = std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression));
        stat_days[2] = StatKbi::taes(KbiId(11));

        let result = StatisticalKbiAnalyzer::dispatch(&FixedIndex(2), &stat_days, LocalDate::of(2026, 1, 1)).unwrap();
        assert_eq!(
            result,
            StatisticalDispatchResult::Taes {
                related_kbi: StatRelatedKbi::new(KbiId(11), StatOpType::Independent)
            }
        );
    }

    #[test]
    fn dispatches_to_regression_with_full_related_list() {
        let mut stat_days: [StatKbi; 7] = std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression));
        stat_days[0].add_related_kbi(StatRelatedKbi::new(KbiId(5), StatOpType::Independent));
        stat_days[0].add_related_kbi(StatRelatedKbi::new(KbiId(6), StatOpType::Add));

        let result = StatisticalKbiAnalyzer::dispatch(&FixedIndex(0), &stat_days, LocalDate::of(2026, 1, 1)).unwrap();
        assert_eq!(
            result,
            StatisticalDispatchResult::Regression {
                related_kbis: vec![
                    StatRelatedKbi::new(KbiId(5), StatOpType::Independent),
                    StatRelatedKbi::new(KbiId(6), StatOpType::Add),
                ]
            }
        );
    }
}
