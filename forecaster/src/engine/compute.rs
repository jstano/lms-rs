//! `Kbi::compute_value` — `KBI.computeValue()`'s four overrides
//! (`InputKBI`/`CalculatedKBI`/`StatisticalKBI`/`PercentOfBaseKBI`), each of which does nothing but
//! delegate to a `KBIReaderWriter.computeXxxKBI` method (`KBIReaderWriter.java` lines 93-172) that
//! in turn dispatches to the Phase 1 formula evaluator or a Phase 2 analyzer. This is the piece
//! every earlier phase deferred to "Phase 3/4 once real ports exist" — it now exists, still against
//! trait ports rather than a concrete DB-backed implementation (per `PLAN_FORECASTER.md`'s
//! "no persistence layer of its own" stance, same as `scheduler`).
//!
//! Returns `Option<f64>`, not `f64`: `KBI.computeValue()` is a nullable `Double` in Java, and
//! `StatisticalKBI`'s branch (`RegressionAnalyzer`/`TAESAnalyzer`) can genuinely produce `null` —
//! see `io::reader_writer`'s doc comment on `read_kbi_value` for why that nullability threads all
//! the way through.

use joda_rs::LocalDate;

use crate::analyzers::{
    KbiPercent, PercentOfBaseKbiAnalyzer, PercentOfBaseLookupPort, RegressionAnalyzer, RegressionKbi, RegressionLookupPort, StatisticalDispatch,
    StatisticalDispatchResult, StatisticalKbiAnalyzer, TaesAnalyzer, TaesLookupPort,
};
use crate::engine::ForecasterError;
use crate::formula::{self, FormulaContext};
use crate::io::{find_period_for_date, FinancialYearPeriodPort, KbiLoaderPort, KbiStatReaderPort};
use crate::kbi::{Kbi, KbiMode, KbiStatType};
use crate::PropertyId;

/// `TDate.dayOfWeek()` — Sunday = 1 .. Saturday = 7 — from `joda_rs::LocalDate::day_of_week()`,
/// which is ISO-based (Monday = 1 .. Sunday = 7). `RevenueCenterPeriodDay`/`CalendarPlan`'s
/// `SunOpen`-first column ordering and `KBI.daysOpen`'s `dayOfWeek()`-keyed map both assume Java's
/// Sunday-first convention, so every call site in this crate that needs a day-of-week from a
/// `LocalDate` goes through this conversion rather than `DayOfWeek::value()` directly.
pub fn java_day_of_week(date: LocalDate) -> i32 {
    (date.day_of_week().value() % 7) + 1
}

/// Everything `compute_kbi_value` needs from the outside world, bundling the Phase 1-3 port traits
/// each `Kbi` variant's Java `computeXxxKBI` delegates to.
pub struct ComputeContext<'a> {
    pub property_id: PropertyId,
    pub kbi_mode: KbiMode,
    pub stat_reader: &'a dyn KbiStatReaderPort,
    pub formula_ctx: &'a dyn FormulaContext,
    pub stat_dispatch: &'a dyn StatisticalDispatch,
    pub regression: &'a dyn RegressionLookupPort,
    pub taes: &'a dyn TaesLookupPort,
    pub percent_of_base: &'a dyn PercentOfBaseLookupPort,
    pub kbi_loader: &'a dyn KbiLoaderPort,
    pub fny: &'a dyn FinancialYearPeriodPort,
}

/// `KBI.computeValue(TDate date, KBIStatType statType)`, dispatched over the `Kbi` enum instead of
/// `instanceof`.
pub fn compute_kbi_value(kbi: &Kbi, date: LocalDate, stat_type: KbiStatType, ctx: &ComputeContext) -> Result<Option<f64>, ForecasterError> {
    match kbi {
        // `KBIReaderWriter.computeInputKBI`: `readKBIStatData(kbi, date).getValue(valueType)`,
        // defaulting to `0.0`. Reads straight through the port rather than through
        // `KbiReaderWriter`'s cache — the `compute` closure passed into `read_kbi_value` doesn't
        // have mutable access to that cache, and re-reading the same idempotent row is harmless.
        Kbi::Input(record) => {
            let data = ctx.stat_reader.read_kbi_stat_data(record.id, date, ctx.property_id)?;
            Ok(Some(data.value(stat_type).unwrap_or(0.0)))
        }

        // `KBIReaderWriter.computeCalculatedKBI`: `kbi.getFormula().calculate(date, statType)`. A
        // missing `formulaText` (no `KBIConfigFormula` row) is treated as an empty formula
        // (`parse_formula` already returns no tokens for an empty string, and `evaluate` returns
        // `0.0` for an empty token list) rather than reproducing whatever NPE Java would hit
        // constructing a `KBIFormula` around a `null` string — not a real formula-less KBI
        // configuration Java ever expects to run in production.
        Kbi::Calculated(data) => {
            let text = data.formula_text.as_deref().unwrap_or("");
            let tokens = formula::parse_formula(&data.base.code, text)?;
            let value = formula::evaluate(&tokens, &data.base.code, date, stat_type, ctx.formula_ctx)?;
            Ok(Some(value))
        }

        // `KBIReaderWriter.computeStatisticalKBI`: `kbi.getAnalyzer().calculate(date)`.
        Kbi::Statistical(data) => match StatisticalKbiAnalyzer::dispatch(ctx.stat_dispatch, &data.stat_days, date)? {
            StatisticalDispatchResult::Taes { related_kbi } => TaesAnalyzer::calculate(ctx.taes, related_kbi.kbi_id, ctx.property_id, date),
            StatisticalDispatchResult::Regression { related_kbis } => RegressionAnalyzer::calculate(
                ctx.regression,
                RegressionKbi {
                    id: data.base.id,
                    is_rooms_kbi: data.base.is_rooms_kbi,
                },
                &related_kbis,
                date,
            ),
        },

        // `KBIReaderWriter.computePercentOfBaseKBI`.
        Kbi::PercentOfBase(data) => compute_percent_of_base(data, date, ctx).map(Some),
    }
}

/// `KBIReaderWriter.computePercentOfBaseKBI` in full: the `FNYPeriod` null-check (returns `0.0`
/// immediately, distinct from `checkPeriodForDate`'s `InvalidDateException`), the bounded
/// period-index search (`io::find_period_for_date`), `PercentOfBaseKBI.getAnalyzer`'s
/// percents-load-with-fallback, and finally `PercentOfBaseKBIAnalyzer.computeValue`.
fn compute_percent_of_base(data: &crate::kbi::PercentOfBaseKbiData, date: LocalDate, ctx: &ComputeContext) -> Result<f64, ForecasterError> {
    let record = &data.base;

    if ctx.fny.fny_period(date)?.is_none() {
        return Ok(0.0);
    }

    // `find_period_for_date` re-derives the same `FNYPeriod` lookup; safe to call now that we know
    // one exists. `unwrap_or(0)` matches Java's `periodNo` staying at its initial `0` when the date
    // falls in none of the first `numPeriods` periods.
    let period_no = find_period_for_date(ctx.fny, date)?.unwrap_or(0);

    // `PercentOfBaseKBI.getAnalyzer(year)`'s percents load, plus `LaborKBIReaderWriter
    // .loadKBIPercents`'s "no rows for the working budget" fallback (one all-zero `KBIPercent` per
    // financial-year period, 1-indexed `PeriodNo` matching Java's `for (p = 1; p <=
    // periodCount; p++)`).
    let mut percents = ctx.kbi_loader.load_kbi_percents(record.id, record.kbi_config_id, ctx.kbi_mode.code())?;
    if percents.is_empty() {
        let period_count = ctx.fny.financial_year_period_count()?;
        percents = (1..=period_count).map(|p| KbiPercent::new(record.id, p)).collect();
    }

    // `PercentOfBaseKBIAnalyzer.getPercents(periodNo)` indexes the loaded list *positionally* by
    // `periodNo`, not by matching a `KBIPercent.periodNo` field value — ported verbatim (Java would
    // throw an unchecked `IndexOutOfBoundsException` for an out-of-range index; this surfaces a
    // typed error instead).
    let percents = percents.get(period_no).ok_or_else(|| ForecasterError::DataAccess {
        message: format!("no KBIPercent entry for period index {period_no} (KBI {})", record.code),
    })?;

    let day_of_week = java_day_of_week(date);

    PercentOfBaseKbiAnalyzer::compute_value(ctx.percent_of_base, &record.code, record.id, data.base_kbi_id, percents, day_of_week, date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_day_of_week_matches_sunday_first_convention() {
        // 2026-01-04 is a Sunday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 4)), 1);
        // 2026-01-05 is a Monday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 5)), 2);
        // 2026-01-10 is a Saturday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 10)), 7);
    }
}
