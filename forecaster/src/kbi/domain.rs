//! `KBI.java` + `InputKBI`/`CalculatedKBI`/`StatisticalKBI`/`PercentOfBaseKBI` — the domain
//! hierarchy collapsed into one `Kbi` enum per `PLAN_FORECASTER.md` §2, dispatched via
//! `KbiType`/`Kbi::kbi_type` rather than `instanceof`.
//!
//! `KBI.computeValue()` (and the SQL-driven `load()`/`setRoomsFlags`/`setRevenueFlags`/
//! `loadDaysOpen`) are **not** ported here: every `computeValue` override does nothing but call a
//! method on `KBIReaderWriter` (`computeInputKBI`/`computeCalculatedKBI`/`computeStatisticalKBI`/
//! `computePercentOfBaseKBI`), which is the Phase 3 I/O boundary this crate hasn't built yet. What
//! *is* pure — `isEditableKBI`/`isDanglingKBI`/`isOpen` — is ported below and will back
//! `Kbi::compute_value` once Phase 3/4 exist to drive it.

use std::collections::HashMap;

use crate::kbi::{KbiType, StatKbi};
use crate::{KbiConfigId, KbiId, PropertyId, UnitId};

/// Fields every `KBI` subtype carries — `KBI.java`'s protected fields (minus the two transient
/// `kbiReaderWriter`/`kbiList` collaborators, which belong to the I/O boundary, not the domain
/// record).
#[derive(Debug, Clone, PartialEq)]
pub struct KbiRecord {
    pub id: KbiId,
    pub property_id: PropertyId,
    pub name: String,
    pub code: String,
    pub unit_id: UnitId,
    pub kbi_config_id: KbiConfigId,
    pub kbi_type: KbiType,
    pub primary: bool,
    pub is_rooms_kbi: bool,
    pub is_revenue_center_kbi: bool,
    pub is_departures_kbi: bool,
    /// `Map<Integer, Boolean> daysOpen` — keyed by `TDate.dayOfWeek()` (1-7), `true` unless a
    /// `RevenueCenterPeriodDay.IsOpen = false` row says otherwise for that day. Missing entries
    /// default to open, matching `daysOpen.getOrDefault(date.dayOfWeek(), true)`.
    pub days_open: HashMap<i32, bool>,
}

impl KbiRecord {
    /// `KBI.isOpen(TDate date)`.
    pub fn is_open(&self, day_of_week: i32) -> bool {
        *self.days_open.get(&day_of_week).unwrap_or(&true)
    }

    /// `KBI.isDanglingKBI()`.
    pub fn is_dangling_kbi(&self) -> bool {
        !self.is_rooms_kbi && !self.is_revenue_center_kbi
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalculatedKbiData {
    pub base: KbiRecord,
    /// `CalculatedKBI.formulaText` — the raw formula source; parsed lazily into a `KBIFormula` in
    /// Java (`getFormula()`), left as the AST-building responsibility of Phase 3/4 call sites here.
    pub formula_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticalKbiData {
    pub base: KbiRecord,
    /// `StatisticalKBI.statDays` — always exactly 7 entries (one per day-of-week 1-7), each
    /// defaulted to `StatKbi::of_type(Regression)` when the DB didn't configure that day
    /// (`StatisticalKBI.load()`'s "make sure that all 7 days contain a valid object" pass).
    pub stat_days: [StatKbi; 7],
}

impl StatisticalKbiData {
    /// `StatisticalKBI.isTAESKBI(TDate date)`. `day_of_week` is 1-7 (Java `TDate.dayOfWeek()`
    /// convention), matching `statDays.get(date.dayOfWeek() - 1)`.
    pub fn is_taes_kbi(&self, day_of_week: i32) -> bool {
        self.stat_days[(day_of_week - 1) as usize].kbi_type == crate::kbi::StatKbiType::Taes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PercentOfBaseKbiData {
    pub base: KbiRecord,
    pub base_kbi_id: Option<KbiId>,
}

/// `KBI` + its four subtypes, dispatched by `KBIType` instead of `instanceof`.
#[derive(Debug, Clone, PartialEq)]
pub enum Kbi {
    Input(KbiRecord),
    Calculated(CalculatedKbiData),
    Statistical(StatisticalKbiData),
    PercentOfBase(PercentOfBaseKbiData),
}

impl Kbi {
    pub fn record(&self) -> &KbiRecord {
        match self {
            Kbi::Input(r) => r,
            Kbi::Calculated(d) => &d.base,
            Kbi::Statistical(d) => &d.base,
            Kbi::PercentOfBase(d) => &d.base,
        }
    }

    /// `KBI.isEditableKBI()`:
    /// `kbiType == INPUT || ((isRoomsKBI || isRevenueCenterKBI) && !(this instanceof CalculatedKBI))`.
    pub fn is_editable_kbi(&self) -> bool {
        let record = self.record();
        record.kbi_type == KbiType::Input
            || ((record.is_rooms_kbi || record.is_revenue_center_kbi)
                && !matches!(self, Kbi::Calculated(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kbi::{StatKbiType, StatOpType, StatRelatedKbi};

    fn record(kbi_type: KbiType, is_rooms: bool, is_revenue: bool) -> KbiRecord {
        KbiRecord {
            id: KbiId(1),
            property_id: PropertyId(1),
            name: "Rooms".to_string(),
            code: "RMS".to_string(),
            unit_id: UnitId(1),
            kbi_config_id: KbiConfigId(1),
            kbi_type,
            primary: false,
            is_rooms_kbi: is_rooms,
            is_revenue_center_kbi: is_revenue,
            is_departures_kbi: false,
            days_open: HashMap::new(),
        }
    }

    #[test]
    fn is_open_defaults_true_when_day_not_configured() {
        let r = record(KbiType::Input, false, false);
        assert!(r.is_open(3));
    }

    #[test]
    fn is_open_honors_explicit_closed_day() {
        let mut r = record(KbiType::Input, false, true);
        r.days_open.insert(3, false);
        assert!(!r.is_open(3));
        assert!(r.is_open(4));
    }

    #[test]
    fn is_dangling_kbi_when_neither_rooms_nor_revenue() {
        assert!(record(KbiType::Input, false, false).is_dangling_kbi());
        assert!(!record(KbiType::Input, true, false).is_dangling_kbi());
        assert!(!record(KbiType::Input, false, true).is_dangling_kbi());
    }

    #[test]
    fn input_kbi_is_always_editable() {
        let kbi = Kbi::Input(record(KbiType::Input, false, false));
        assert!(kbi.is_editable_kbi());
    }

    #[test]
    fn calculated_kbi_is_never_editable_even_if_rooms_or_revenue() {
        let kbi = Kbi::Calculated(CalculatedKbiData {
            base: record(KbiType::Calculated, true, false),
            formula_text: None,
        });
        assert!(!kbi.is_editable_kbi());
    }

    #[test]
    fn statistical_kbi_is_editable_only_when_rooms_or_revenue() {
        let dangling = Kbi::Statistical(StatisticalKbiData {
            base: record(KbiType::Statistical, false, false),
            stat_days: std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression)),
        });
        assert!(!dangling.is_editable_kbi());

        let rooms = Kbi::Statistical(StatisticalKbiData {
            base: record(KbiType::Statistical, true, false),
            stat_days: std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression)),
        });
        assert!(rooms.is_editable_kbi());
    }

    #[test]
    fn is_taes_kbi_checks_the_configured_day_of_week() {
        let mut stat_days: [StatKbi; 7] =
            std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression));
        stat_days[2] = StatKbi::taes(KbiId(7));
        let data = StatisticalKbiData {
            base: record(KbiType::Statistical, true, false),
            stat_days,
        };
        assert!(data.is_taes_kbi(3));
        assert!(!data.is_taes_kbi(1));
    }

    #[test]
    fn stat_related_kbi_add_still_constructs() {
        // sanity check the imports above are actually exercised end-to-end
        let related = StatRelatedKbi::new(KbiId(9), StatOpType::Add);
        assert_eq!(related.kbi_id, KbiId(9));
    }
}
