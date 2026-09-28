//! The capability surface the formula evaluator needs from the outside world.
//!
//! Every `Function`/`KBICode` DB call site in the Java source (`KBIReaderWriter.readKBIValue`/
//! `readKBIOverrideValue`, the market-segment/revenue-center SQL in `MarketSegmentTotalFunction`/
//! `RevenueTotalFunction`, `Property.getNumberRooms()`, `PastAverageAnalyzer`) is collapsed into
//! one of these methods, following the plan's "narrow port trait, typed parameters" treatment of
//! the I/O boundary (`DATA_MODEL.md` §3). Real implementations land in Phase 3 (DB-backed) and
//! Phase 2 (`past_average`, backed by `PastAverageAnalyzer`); Phase 1 only defines the seam and
//! exercises it with in-memory stubs in tests.
//!
//! `read_kbi_stat_value` deliberately backs three different Java call sites that all reduce to
//! "read this KBI's override value, else its per-stat-type value, defaulting to 0.0 if both are
//! absent": `KBICode.calculate`, `ReadLaborKBIFunction.calculate` (whose `dataSlot` argument uses
//! the same four codes as `KBIStatType`), and `SameDayLastMonthFunction.calculate`. Java re-derives
//! this pattern three times (with `ReadLaborKBIFunction`/`SameDayLastMonthFunction` additionally
//! branching on the concrete `KBIReaderWriter` subclass, a branch that is always taken for
//! `LaborKBIReaderWriter` — the only concrete reader/writer this engine ever runs with, per the
//! plan) — one trait method is enough to cover all three here.

use crate::kbi::KbiStatType;
use crate::engine::ForecasterError;
use crate::KbiId;
use joda_rs::LocalDate;

/// `segment_types: ALL,TRANSIENT,GROUP,CONTRACT,CASINO` — the first argument to `ARRIVALSTTL`/
/// `DEPARTSTTL`/`GUESTSTTL`/`ROOMSTTL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSegmentType {
    All,
    Transient,
    Group,
    Contract,
    Casino,
}

impl MarketSegmentType {
    /// Case-insensitive, matching `MarketSegmentTotalFunction`'s `equalsIgnoreCase` checks.
    pub fn from_str_ci(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "ALL" => Some(Self::All),
            "TRANSIENT" => Some(Self::Transient),
            "GROUP" => Some(Self::Group),
            "CONTRACT" => Some(Self::Contract),
            "CASINO" => Some(Self::Casino),
            _ => None,
        }
    }
}

pub trait FormulaContext {
    /// `KBIList.getKBI(code)` with the SQL fallback in `KBICode.getKBIID`/`ReadLaborKBIFunction
    /// .getKBIID`/etc. collapsed in: every call site in Java either finds it in the in-memory list
    /// or falls back to `select ID from KBI where PropertyID = ? and Code = ?`.
    fn kbi_id_for_code(&self, code: &str) -> Result<Option<KbiId>, ForecasterError>;

    /// See the module doc comment above.
    fn read_kbi_stat_value(
        &self,
        kbi_id: KbiId,
        date: LocalDate,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `ArrivalsTotalFunction` / `MarketSegmentTotalFunction.getMarketSegmentTotal("A", ...)`.
    fn arrivals_total(
        &self,
        segment: MarketSegmentType,
        date: LocalDate,
        day_offset: i32,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `DeparturesTotalFunction` / `getMarketSegmentTotal("D", ...)`.
    fn departures_total(
        &self,
        segment: MarketSegmentType,
        date: LocalDate,
        day_offset: i32,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `GuestsTotalFunction` / `getMarketSegmentTotal("G", ...)`.
    fn guests_total(
        &self,
        segment: MarketSegmentType,
        date: LocalDate,
        day_offset: i32,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `RoomsTotalFunction` / `getMarketSegmentTotal("R", ...)`.
    fn rooms_total(
        &self,
        segment: MarketSegmentType,
        date: LocalDate,
        day_offset: i32,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `RevenueTotalFunction`.
    fn revenue_total(
        &self,
        revenue_center_name: &str,
        units: &str,
        date: LocalDate,
        day_offset: i32,
        stat_type: KbiStatType,
    ) -> Result<f64, ForecasterError>;

    /// `ConfigRoomsFunction` — `Property.getNumberRooms()`.
    fn config_rooms(&self) -> Result<f64, ForecasterError>;

    /// `PastAverageFunction` — backed by `PastAverageAnalyzer` (Phase 2).
    fn past_average(
        &self,
        kbi_id: KbiId,
        number_data_points: i32,
        date: LocalDate,
    ) -> Result<f64, ForecasterError>;
}
