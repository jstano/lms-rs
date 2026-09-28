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
    use crate::analyzers::{StatisticalDispatch, TaesLookupPort};
    use crate::formula::MarketSegmentType;
    use crate::io::{FnyPeriod, KbiLoaderPort};
    use crate::kbi::{CalculatedKbiData, KbiRecord, KbiType, PercentOfBaseKbiData, StatKbi, StatKbiType, StatisticalKbiData};
    use crate::{KbiConfigId, KbiId, UnitId};
    use std::collections::HashMap;

    #[test]
    fn java_day_of_week_matches_sunday_first_convention() {
        // 2026-01-04 is a Sunday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 4)), 1);
        // 2026-01-05 is a Monday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 5)), 2);
        // 2026-01-10 is a Saturday.
        assert_eq!(java_day_of_week(LocalDate::of(2026, 1, 10)), 7);
    }

    fn record(id: i32, kbi_type: KbiType) -> KbiRecord {
        KbiRecord {
            id: KbiId(id),
            property_id: PropertyId(1),
            name: format!("K{id}"),
            code: format!("K{id}"),
            unit_id: UnitId(1),
            kbi_config_id: KbiConfigId(1),
            kbi_type,
            primary: false,
            is_rooms_kbi: false,
            is_revenue_center_kbi: false,
            is_departures_kbi: false,
            days_open: HashMap::new(),
        }
    }

    #[derive(Default)]
    struct Stub {
        stat_data: HashMap<(i32, LocalDate), crate::io::KbiStatData>,
        fny: Option<FnyPeriod>,
        fny_period_count: i32,
        percents: Vec<crate::analyzers::KbiPercent>,
        base_kbi_value: Option<f64>,
        taes_actuals: Option<(Vec<i32>, LocalDate)>,
        day_index: usize,
    }

    impl KbiStatReaderPort for Stub {
        fn read_kbi_stat_data(&self, kbi_id: KbiId, date: LocalDate, _property_id: PropertyId) -> Result<crate::io::KbiStatData, ForecasterError> {
            Ok(self
                .stat_data
                .get(&(kbi_id.0, date))
                .copied()
                .unwrap_or_else(|| crate::io::KbiStatData::new(kbi_id, date)))
        }

        fn read_kbi_override_value(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }
    }

    impl FinancialYearPeriodPort for Stub {
        fn fny_period(&self, _date: LocalDate) -> Result<Option<FnyPeriod>, ForecasterError> {
            Ok(self.fny.clone())
        }

        fn financial_year_period_count(&self) -> Result<i32, ForecasterError> {
            Ok(self.fny_period_count)
        }
    }

    impl KbiLoaderPort for Stub {
        fn rooms_and_departures_flags(&self, _kbi_id: KbiId) -> Result<(bool, bool), ForecasterError> {
            Ok((false, false))
        }

        fn revenue_center_period_id(&self, _kbi_id: KbiId) -> Result<Option<i32>, ForecasterError> {
            Ok(None)
        }

        fn days_open(&self, _property_id: PropertyId, _period_id: i32, _standard_set_id: crate::StandardSetId) -> Result<HashMap<i32, bool>, ForecasterError> {
            Ok(HashMap::new())
        }

        fn load_kbi_percents(&self, _kbi_id: KbiId, _kbi_config_id: KbiConfigId, _kbi_mode_code: &str) -> Result<Vec<crate::analyzers::KbiPercent>, ForecasterError> {
            Ok(self.percents.clone())
        }
    }

    impl crate::analyzers::PercentOfBaseLookupPort for Stub {
        fn read_base_kbi_value(&self, _base_kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(self.base_kbi_value)
        }
    }

    impl TaesLookupPort for Stub {
        fn recent_actuals(&self, _kbi_id: KbiId, _property_id: PropertyId, _requested_date: LocalDate) -> Result<Option<(Vec<i32>, LocalDate)>, ForecasterError> {
            Ok(self.taes_actuals.clone())
        }

        fn should_log_taes_calculation(&self) -> bool {
            false
        }

        fn insert_taes_calculation_log(&self, _kbi_id: KbiId, _date: LocalDate, _message: &str) {}
    }

    impl crate::analyzers::RegressionLookupPort for Stub {
        fn days_out(&self) -> Result<i32, ForecasterError> {
            Ok(0)
        }
        fn find_season_id(&self, _date: LocalDate) -> Result<i32, ForecasterError> {
            Ok(0)
        }
        fn find_environment_id(&self, _kbi_id: KbiId, _date: LocalDate, _actual_env: bool) -> Result<i32, ForecasterError> {
            Ok(0)
        }
        fn is_global_environment(&self, _date: LocalDate) -> Result<bool, ForecasterError> {
            Ok(false)
        }
        fn ignore_environment_dow(&self, _environment_id: i32) -> Result<bool, ForecasterError> {
            Ok(false)
        }
        fn min_stat_date(&self, _kbi_id: KbiId) -> Result<LocalDate, ForecasterError> {
            Ok(LocalDate::of(2020, 1, 1))
        }
        fn read_kbi_stat_value(&self, _kbi_id: KbiId, _date: LocalDate, _stat_type: Option<KbiStatType>) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }
        fn read_kbi_value(&self, _kbi_id: KbiId, _date: LocalDate, _stat_type: KbiStatType) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }
        fn read_kbi_override_value(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }
        fn read_kbi_stat_data(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<crate::analyzers::KbiStatSnapshot, ForecasterError> {
            Ok(crate::analyzers::KbiStatSnapshot {
                est: None,
                adj: None,
                act: None,
                fst: None,
            })
        }
        fn is_taes_kbi(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<bool, ForecasterError> {
            Ok(false)
        }
        fn is_actual_mode(&self) -> bool {
            false
        }
    }

    impl StatisticalDispatch for Stub {
        fn absolute_day_index(&self, _date: LocalDate) -> Result<usize, ForecasterError> {
            Ok(self.day_index)
        }
    }

    impl FormulaContext for Stub {
        fn kbi_id_for_code(&self, _code: &str) -> Result<Option<KbiId>, ForecasterError> {
            Ok(None)
        }
        fn read_kbi_stat_value(&self, _kbi_id: KbiId, _date: LocalDate, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn arrivals_total(&self, _segment: MarketSegmentType, _date: LocalDate, _day_offset: i32, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn departures_total(&self, _segment: MarketSegmentType, _date: LocalDate, _day_offset: i32, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn guests_total(&self, _segment: MarketSegmentType, _date: LocalDate, _day_offset: i32, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn rooms_total(&self, _segment: MarketSegmentType, _date: LocalDate, _day_offset: i32, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn revenue_total(&self, _revenue_center_name: &str, _units: &str, _date: LocalDate, _day_offset: i32, _stat_type: KbiStatType) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn config_rooms(&self) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
        fn past_average(&self, _kbi_id: KbiId, _number_data_points: i32, _date: LocalDate) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
    }

    fn ctx(stub: &Stub) -> ComputeContext<'_> {
        ComputeContext {
            property_id: PropertyId(1),
            kbi_mode: KbiMode::Labor,
            stat_reader: stub,
            formula_ctx: stub,
            stat_dispatch: stub,
            regression: stub,
            taes: stub,
            percent_of_base: stub,
            kbi_loader: stub,
            fny: stub,
        }
    }

    #[test]
    fn input_kbi_reads_the_requested_stat_column_defaulting_to_zero() {
        let date = LocalDate::of(2026, 1, 5);
        let mut data = crate::io::KbiStatData::new(KbiId(1), date);
        data.set_fst_value(Some(42.0));
        let mut stub = Stub::default();
        stub.stat_data.insert((1, date), data);

        let kbi = Kbi::Input(record(1, KbiType::Input));
        let value = compute_kbi_value(&kbi, date, KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(42.0));

        // No row for this date/column -> defaults to 0.0, not an error.
        let value = compute_kbi_value(&kbi, date, KbiStatType::Estimated, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(0.0));
    }

    #[test]
    fn calculated_kbi_with_no_formula_text_evaluates_to_zero() {
        let stub = Stub::default();
        let kbi = Kbi::Calculated(CalculatedKbiData {
            base: record(1, KbiType::Calculated),
            formula_text: None,
        });
        let value = compute_kbi_value(&kbi, LocalDate::of(2026, 1, 5), KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(0.0));
    }

    #[test]
    fn calculated_kbi_evaluates_its_formula_text() {
        let stub = Stub::default();
        let kbi = Kbi::Calculated(CalculatedKbiData {
            base: record(1, KbiType::Calculated),
            formula_text: Some("5+2".to_string()),
        });
        let value = compute_kbi_value(&kbi, LocalDate::of(2026, 1, 5), KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(7.0));
    }

    #[test]
    fn statistical_kbi_dispatches_to_taes_and_returns_none_without_actuals() {
        let stub = Stub { taes_actuals: None, ..Default::default() };
        let mut stat_days: [StatKbi; 7] = std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression));
        stat_days[0] = StatKbi::taes(KbiId(9));
        let kbi = Kbi::Statistical(StatisticalKbiData {
            base: record(1, KbiType::Statistical),
            stat_days,
        });
        let value = compute_kbi_value(&kbi, LocalDate::of(2026, 1, 5), KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn statistical_kbi_dispatches_to_regression_and_returns_none_for_a_configuration_error() {
        // No related KBIs and not a rooms KBI -> `ind_count == 0`, Java's "configuration error"
        // early return.
        let stub = Stub::default();
        let stat_days: [StatKbi; 7] = std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression));
        let kbi = Kbi::Statistical(StatisticalKbiData {
            base: record(1, KbiType::Statistical),
            stat_days,
        });
        let value = compute_kbi_value(&kbi, LocalDate::of(2026, 1, 5), KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn percent_of_base_kbi_is_zero_when_no_fny_period_exists_for_the_date() {
        let stub = Stub { fny: None, ..Default::default() };
        let kbi = Kbi::PercentOfBase(PercentOfBaseKbiData {
            base: record(1, KbiType::PercentOfBase),
            base_kbi_id: Some(KbiId(2)),
        });
        let value = compute_kbi_value(&kbi, LocalDate::of(2026, 1, 5), KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(0.0));
    }

    #[test]
    fn percent_of_base_kbi_falls_back_to_all_zero_percents_when_none_are_loaded() {
        let date = LocalDate::of(2026, 1, 4); // Sunday -> day_of_week 1
        let stub = Stub {
            fny: Some(FnyPeriod {
                year: 2026,
                periods: vec![Some((LocalDate::of(2026, 1, 1), LocalDate::of(2026, 1, 31)))],
            }),
            fny_period_count: 1,
            percents: vec![], // triggers the "no rows -> all-zero per period" fallback
            base_kbi_value: Some(100.0),
            ..Default::default()
        };

        let kbi = Kbi::PercentOfBase(PercentOfBaseKbiData {
            base: record(1, KbiType::PercentOfBase),
            base_kbi_id: Some(KbiId(2)),
        });
        let value = compute_kbi_value(&kbi, date, KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        // The fallback builds one all-zero `KbiPercent` per financial-year period; its day-of-week
        // percent is therefore 0.0 regardless of the base KBI's value.
        assert_eq!(value, Some(0.0));
    }

    #[test]
    fn percent_of_base_kbi_uses_the_loaded_percent_for_the_matching_period() {
        let date = LocalDate::of(2026, 1, 4); // Sunday -> day_of_week 1 -> percents index 0
        let mut percent = crate::analyzers::KbiPercent::new(KbiId(1), 0);
        percent.percents[0] = 50.0;
        let stub = Stub {
            fny: Some(FnyPeriod {
                year: 2026,
                periods: vec![
                    Some((LocalDate::of(2026, 1, 1), LocalDate::of(2026, 1, 31))),
                    Some((LocalDate::of(2026, 2, 1), LocalDate::of(2026, 2, 28))),
                ],
            }),
            fny_period_count: 2,
            percents: vec![percent],
            base_kbi_value: Some(200.0),
            ..Default::default()
        };

        let kbi = Kbi::PercentOfBase(PercentOfBaseKbiData {
            base: record(1, KbiType::PercentOfBase),
            base_kbi_id: Some(KbiId(2)),
        });
        let value = compute_kbi_value(&kbi, date, KbiStatType::Forecasted, &ctx(&stub)).unwrap();
        assert_eq!(value, Some(100.0));
    }
}
