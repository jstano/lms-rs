//! `PercentOfBaseKBIAnalyzer.java` — computes a `PercentOfBaseKBI`'s value as
//! `baseKBI.value(date) * dayOfWeekPercent`. Note this class does **not** touch `FNYPeriod` (that
//! reference in `PLAN_FORECASTER.md`'s Phase 2 section pointed at the wrong class — confirmed by
//! reading the file directly; `FNYPeriod` only shows up in `KBICode.java`/`KBIReaderWriter.java`,
//! which are Phase 3 territory). The real I/O here is `LaborKBIReaderWriter.readKBIOverrideValue`/
//! `readKBIValue` (or, for the budget-mode branch, a recursive `KBI.computeValue` call) — modeled
//! behind `PercentOfBaseLookupPort` per the usual narrow-port treatment.
//!
//! Java's `computing` recursion guard only ever matters for the budget-mode branch's recursive
//! `baseKBI.computeValue(...)` call; since that branch collapses into `read_base_kbi_value` here
//! (per the plan's "`LaborKBIReaderWriter` is the only concrete reader/writer" simplification),
//! there is no recursion left at this layer to guard, so the flag is dropped rather than ported.

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::KbiId;

/// `KBIPercent.java` — one KBI's per-day-of-week percentages for a given period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KbiPercent {
    pub kbi_id: KbiId,
    pub period_no: i32,
    /// Index 0-6, matching `date.dayOfWeek() - 1` (Java `TDate.dayOfWeek()` returns 1-7 Sun-Sat).
    pub percents: [f64; 7],
}

impl KbiPercent {
    pub fn new(kbi_id: KbiId, period_no: i32) -> Self {
        KbiPercent {
            kbi_id,
            period_no,
            percents: [0.0; 7],
        }
    }

    pub fn percent(&self, day_no: usize) -> f64 {
        self.percents[day_no]
    }
}

/// See the module doc comment. `read_base_kbi_value` collapses Java's mode-conditional read: for
/// `LaborKBIReaderWriter`, override-value-else-plain-value; the non-Labor branch (recursive
/// `baseKBI.computeValue(date, FORECASTED)`) isn't modeled separately since `LaborKBIReaderWriter`
/// is the only concrete reader/writer this engine ever runs with, per `PLAN_FORECASTER.md`.
pub trait PercentOfBaseLookupPort {
    fn read_base_kbi_value(&self, base_kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError>;
}

pub struct PercentOfBaseKbiAnalyzer;

impl PercentOfBaseKbiAnalyzer {
    /// `computeValue(TDate date, int periodNo)`. `day_of_week` is 1-7 (Java `TDate.dayOfWeek()`
    /// convention); `kbi_code`/`kbi_id`/`base_kbi_id` mirror the `KBIComputeException` context Java
    /// attaches at each throw site.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_value(
        port: &dyn PercentOfBaseLookupPort,
        kbi_code: &str,
        kbi_id: KbiId,
        base_kbi_id: Option<KbiId>,
        percents: &KbiPercent,
        day_of_week: i32,
        date: LocalDate,
    ) -> Result<f64, ForecasterError> {
        let Some(base_kbi_id) = base_kbi_id else {
            return Ok(0.0);
        };

        if base_kbi_id == kbi_id {
            return Err(ForecasterError::KbiCompute {
                kbi_code: kbi_code.to_string(),
                message: "The Base KBI cannot be the same as the Percent Of Base KBI itself.".to_string(),
            });
        }

        // `dayOfWeek() - 1` indexing into the 0-6 percents array.
        let day_of_week_percent = percents.percent((day_of_week - 1) as usize) / 100.0;

        let day_value = port.read_base_kbi_value(base_kbi_id, date)?;

        Ok(match day_value {
            Some(v) => v * day_of_week_percent,
            None => 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePort {
        value: Option<f64>,
    }

    impl PercentOfBaseLookupPort for FakePort {
        fn read_base_kbi_value(&self, _base_kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(self.value)
        }
    }

    fn percents_with(day_index: usize, value: f64) -> KbiPercent {
        let mut p = KbiPercent::new(KbiId(2), 1);
        p.percents[day_index] = value;
        p
    }

    #[test]
    fn no_base_kbi_returns_zero() {
        let port = FakePort { value: Some(100.0) };
        let percents = percents_with(0, 50.0);
        let result = PercentOfBaseKbiAnalyzer::compute_value(
            &port,
            "PCT",
            KbiId(1),
            None,
            &percents,
            1,
            LocalDate::of(2026, 1, 4),
        )
        .unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn base_kbi_same_as_self_is_an_error() {
        let port = FakePort { value: Some(100.0) };
        let percents = percents_with(0, 50.0);
        let result = PercentOfBaseKbiAnalyzer::compute_value(
            &port,
            "PCT",
            KbiId(1),
            Some(KbiId(1)),
            &percents,
            1,
            LocalDate::of(2026, 1, 4),
        );
        assert!(matches!(result, Err(ForecasterError::KbiCompute { .. })));
    }

    #[test]
    fn applies_day_of_week_percent_to_base_value() {
        let port = FakePort { value: Some(200.0) };
        // Sunday (dayOfWeek() == 1) -> index 0.
        let percents = percents_with(0, 50.0);
        let result = PercentOfBaseKbiAnalyzer::compute_value(
            &port,
            "PCT",
            KbiId(1),
            Some(KbiId(2)),
            &percents,
            1,
            LocalDate::of(2026, 1, 4),
        )
        .unwrap();
        assert_eq!(result, 100.0);
    }

    #[test]
    fn missing_base_value_yields_zero() {
        let port = FakePort { value: None };
        let percents = percents_with(0, 50.0);
        let result = PercentOfBaseKbiAnalyzer::compute_value(
            &port,
            "PCT",
            KbiId(1),
            Some(KbiId(2)),
            &percents,
            1,
            LocalDate::of(2026, 1, 4),
        )
        .unwrap();
        assert_eq!(result, 0.0);
    }
}
