//! Narrow port traits for `KBIReaderWriter`/`LaborKBIReaderWriter`'s DB access surface, following
//! `scheduler::engine::io::ports`'s "one trait per DAO/query surface" treatment.
//!
//! Ground truth: `KBI.load()` (`setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen`) and
//! `KBIReaderWriter`/`LaborKBIReaderWriter`'s `readKBIStatData`/`writeKBIStatData`/
//! `readKBIOverrideValue`/`recordExists`/`loadKBIPercents`/`checkPeriodForDate` /
//! `computePercentOfBaseKBI`'s `FNYPeriodsSqlImpl` lookup.
//!
//! `PLAN_FORECASTER.md`'s Phase 3 checklist named `RevenueCenterConfigPort` and
//! `MarketSegmentConfigPort` as separate traits; reading `KBI.java` directly shows both only ever
//! get called together, from `KBI.load()`, to populate one KBI's rooms/departures/revenue-center/
//! days-open flags — so they're modeled as one `KbiLoaderPort` method surface here instead
//! (see `PARITY_AUDIT.md` Phase 3). `CalendarPlanPort` is `ForecastThread.checkRevenueCenters`'s
//! dependency (Phase 4 territory — nothing in this phase's scope calls it) and stays deferred.

use std::collections::HashMap;

use joda_rs::LocalDate;

use crate::analyzers::KbiPercent;
use crate::engine::ForecasterError;
use crate::io::{FnyPeriod, KbiStatData};
use crate::{KbiConfigId, KbiId, PropertyId, StandardSetId};

/// `KBI.load()`'s `setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen`, plus
/// `KBIReaderWriter.loadKBIPercents` (also a KBI-config load, keyed off `kbiConfigId` rather than
/// `kbiID`).
pub trait KbiLoaderPort {
    /// `KBI.setRoomsFlags` — `(is_rooms_kbi, is_departures_kbi)`, from the
    /// `MarketSegment` row (if any) naming this KBI as its `RoomsKBIID`/`ArrivalsKBIID`/
    /// `GuestsKBIID`/`DeparturesKBIID`.
    fn rooms_and_departures_flags(&self, kbi_id: KbiId) -> Result<(bool, bool), ForecasterError>;

    /// `KBI.setRevenueFlags`'s `RevenueCenterPeriod` lookup — the period id, if this KBI has one
    /// (there is only ever at most one `RevenueCenterPeriod` per KBI).
    fn revenue_center_period_id(&self, kbi_id: KbiId) -> Result<Option<i32>, ForecasterError>;

    /// `KBI.loadDaysOpen` — keyed by `RevenueCenterPeriodDay.DayNo`.
    fn days_open(
        &self,
        property_id: PropertyId,
        period_id: i32,
        standard_set_id: StandardSetId,
    ) -> Result<HashMap<i32, bool>, ForecasterError>;

    /// `KBIReaderWriter.loadKBIPercents` — one `KbiPercent` per `KBIConfigPercentOfBase` row,
    /// ordered by `PeriodNo`; the caller (Phase 4) is responsible for the "no rows" fallback
    /// (`KBIPercent` per financial-year period, all-zero percents) since that needs
    /// `Property.getFinancialYearType().getPeriodCount()`, not a KBI-scoped query.
    fn load_kbi_percents(&self, kbi_id: KbiId, kbi_config_id: KbiConfigId, kbi_mode_code: &str) -> Result<Vec<KbiPercent>, ForecasterError>;
}

/// `KBIReaderWriter.readKBIStatData`/`readKBIOverrideValue`.
pub trait KbiStatReaderPort {
    /// `LaborKBIReaderWriter.readKBIStatData(int kbiID, TDate date)` — the raw `KBIStat` row read,
    /// rounded per-column with `TDouble.round` exactly as Java does. Returns an empty
    /// (all-`None`) `KbiStatData` when no row exists, matching Java's "row not found" behavior.
    fn read_kbi_stat_data(&self, kbi_id: KbiId, date: LocalDate, property_id: PropertyId) -> Result<KbiStatData, ForecasterError>;

    /// `KBIReaderWriter.readKBIOverrideValue` — `None` when there is no `KBIStatOverride` row.
    fn read_kbi_override_value(&self, kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError>;
}

/// `LaborKBIReaderWriter.writeKBIStatData` — the update-else-insert upsert of all four value
/// columns (`recordExists` + `TUpdateCommand`/`TInsertCommand`, each column rounded with
/// `TDouble.round(value, 0)` before it's written) is one DB access surface, so the
/// exists-check/insert-vs-update branch is the concrete port implementation's concern, not
/// modeled here — same treatment `scheduler`'s save-style ports give upserts.
pub trait KbiStatWriterPort {
    fn write_kbi_stat_data(&self, data: &KbiStatData, property_id: PropertyId) -> Result<(), ForecasterError>;
}

/// `FNYPeriodsSqlImpl.getFNYPeriod` (via `KBIReaderWriter.fnyPeriodsSql`), used by
/// `checkPeriodForDate` and `computePercentOfBaseKBI`.
pub trait FinancialYearPeriodPort {
    fn fny_period(&self, date: LocalDate) -> Result<Option<FnyPeriod>, ForecasterError>;

    /// `Property.getFinancialYearType().getPeriodCount()` — the loop bound
    /// `computePercentOfBaseKBI` iterates over when locating a date's period.
    fn financial_year_period_count(&self) -> Result<i32, ForecasterError>;
}
