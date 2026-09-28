//! Orchestrator-level port traits — `ForecastThread`'s own DB access surfaces, distinct from
//! `io::ports` (`KBIReaderWriter`/`LaborKBIReaderWriter`'s surfaces). Same "one narrow trait per
//! DB access surface" treatment as `io::ports`, following `PLAN_FORECASTER.md` §3's precedent.

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::kbi::Kbi;
use crate::{KbiId, PropertyId, StandardSetId};

/// `ForecastThread.getKbiSetIdForStandardSetId` + `KBIList.loadKBIs`/`KBI.createKBI`. There is no
/// `TRecordset` equivalent in this crate (per `DATA_MODEL.md`'s "no persistence layer of its own"
/// stance, same as `scheduler`), so `load_kbi_list` hands back fully-built `Kbi` domain objects
/// directly — the multi-table join (`KBI`/`KBIConfig`/`KBIBudgetConfig`/`KBILaborConfig`) plus each
/// subtype's own load (`CalculatedKBI`'s `KBIConfigFormula` read, `StatisticalKBI`'s
/// `KBIConfigStatistical`/`KBIConfigTAES`/`KBIConfigRegression` reads, `PercentOfBaseKBI`'s
/// `KBIConfigPercent` read) and `KBI.load()`'s flag loading (`io::load_kbi_flags`) are all a real
/// implementation's concern, not this crate's. `KBIList.loadBasisForPercentOfBaseKBIs` needs no
/// separate modeling: it only resolves each `PercentOfBaseKBI`'s `baseKBIID` into a `KBI` object
/// reference for `PercentOfBaseKBIAnalyzer` to call back into, but the Rust port's
/// `PercentOfBaseLookupPort::read_base_kbi_value` (Phase 2) already reads the base KBI's *value*
/// directly rather than holding a reference to its domain object, so `PercentOfBaseKbiData
/// ::base_kbi_id` is all `load_kbi_list` needs to populate.
pub trait KbiSetPort {
    /// `ForecastThread.getKbiSetIdForStandardSetId` — `SELECT KBISetID FROM StandardSet WHERE ID
    /// = ?`. Java throws `IllegalStateException` when no row matches; that maps to
    /// `ForecasterError::DataAccess` here.
    fn kbi_set_id_for_standard_set(&self, standard_set_id: StandardSetId) -> Result<i32, ForecasterError>;

    /// `KBIList.loadKBIs`, ordered by `KBI.Name` (`ORDER BY KBI.Name` in the Java query) so
    /// iteration order matches Java's.
    fn load_kbi_list(&self, property_id: PropertyId, kbi_set_id: i32) -> Result<Vec<Kbi>, ForecasterError>;
}

/// `ForecastThread.checkMarketSegments`'s `MarketSegment` enumeration. The `KBIStat`/
/// `KBIStatOverride` reads that method also does are already covered by `io::KbiStatReaderPort`
/// (`read_kbi_stat_data`'s `fst_value()` / `read_kbi_override_value`) — no separate port needed for
/// those, they're the exact same row shapes Phase 3 already modeled.
pub trait MarketSegmentCheckPort {
    fn market_segments(&self, property_id: PropertyId) -> Result<Vec<MarketSegmentRow>, ForecasterError>;
}

/// One `MarketSegment` row (excluding the `MSTypeCode = 'CS'` casino rows, which
/// `checkMarketSegments` skips before ever looking at these ids — `market_segments` should not
/// return them in the first place, matching the Java loop's `if (!getMSTypeCode().equals("CS"))`
/// guard being the *first* thing it does with each row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketSegmentRow {
    pub rooms_kbi_id: KbiId,
    /// `arrivalsKBIID > 0 ? kbiList.getKBI(arrivalsKBIID) : null`.
    pub arrivals_kbi_id: Option<KbiId>,
    pub guests_kbi_id: Option<KbiId>,
    pub departures_kbi_id: Option<KbiId>,
}

/// `ForecastThread.checkRevenueCenters`'s three query surfaces: the `RevenueCenter`/
/// `RevenueCenterPeriod`/`RevenueCenterPeriodStandardSet` join, the `CalendarPlan`/
/// `CalendarPlanDate` lookup, and `RevenueCenterPeriodDay`.
pub trait RevenueCenterCheckPort {
    /// The `revCenterRS` query: one row per revenue-center KBI in scope for this property/standard
    /// set.
    fn revenue_center_kbis(&self, property_id: PropertyId, standard_set_id: StandardSetId) -> Result<Vec<RevenueCenterKbiRow>, ForecasterError>;

    /// `CalendarPlan` lookup by id, plus its `Sun..SatOpen` flags (index 0 = Sunday, matching
    /// `daysOpen[date.dayOfWeek() - 1]`'s Sun-first convention). `None` when `calendarPlanID == 0`
    /// or no `CalendarPlan` row exists — the caller skips the whole calendar-date check in either
    /// case, matching Java's `if (calendarPlanID != 0) { ... if (calendarRS.next()) { ... } }`.
    fn calendar_plan(&self, calendar_plan_id: i32) -> Result<Option<CalendarPlan>, ForecasterError>;

    /// `CalendarPlanDate` rows for a plan — `(start_month, start_day, end_month, end_day)`, the
    /// month/day components Java re-derives into `TDate`s per rebase year in the `years[]` loop.
    fn calendar_plan_dates(&self, calendar_plan_id: i32) -> Result<Vec<CalendarPlanDateRange>, ForecasterError>;

    /// `RevenueCenterPeriodDay` rows for a `RevenueCenterPeriodStandardSetID` — `(DayNo, IsOpen)`.
    fn revenue_center_period_days(&self, revenue_center_period_standard_set_id: i32) -> Result<Vec<(i32, bool)>, ForecasterError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevenueCenterKbiRow {
    pub kbi_id: KbiId,
    pub revenue_center_period_standard_set_id: i32,
    pub calendar_plan_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarPlan {
    /// Index 0 = Sunday .. index 6 = Saturday (`SunOpen`..`SatOpen`).
    pub days_open: [bool; 7],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarPlanDateRange {
    pub start_month: u32,
    pub start_day: u32,
    pub end_month: u32,
    pub end_day: u32,
}

/// `year`/`month`/`day` validity check — `LocalDate::of` panics on an invalid combination (e.g.
/// Feb 29 rebased onto a non-leap year), which the 3-year calendar-plan-date rebase
/// (`years[0] = periodYear - 1`, `years[2] = periodYear + 1`) can genuinely hit, so
/// `CalendarPlanDateRange::contains_in_year` checks first rather than relying on `LocalDate::of`
/// to reject it.
fn is_valid_ymd(year: i32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    day <= days_in_month
}

impl CalendarPlanDateRange {
    /// Does `date` fall within this range when rebased onto `year`, inclusive both ends
    /// (`TDate.isBetween`)? Mirrors the Java loop's `new TDate(calStartMonth, calStartDay, years[i])`
    /// reconstruction for one candidate year.
    pub fn contains_in_year(&self, date: LocalDate, year: i32) -> bool {
        if !is_valid_ymd(year, self.start_month, self.start_day) || !is_valid_ymd(year, self.end_month, self.end_day) {
            return false;
        }
        let start = LocalDate::of(year, self.start_month as i32, self.start_day as i32);
        let end = LocalDate::of(year, self.end_month as i32, self.end_day as i32);
        date >= start && date <= end
    }
}
