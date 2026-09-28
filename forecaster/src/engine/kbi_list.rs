//! `KBIList.java` — the in-memory id-indexed collection `ForecastThread`/`checkMarketSegments`/
//! `checkRevenueCenters` all use (`kbiList.getKBI(kbiID)`). The DB load
//! (`KBIList.loadKBIs`/`KBI.createKBI`) is `KbiSetPort::load_kbi_list` (`engine::ports`); this type
//! is purely the resulting collection, built once and indexed by id thereafter.
//!
//! `getKBI(String code)` (the code-keyed lookup) is not ported: nothing in `ForecastThread`'s own
//! code calls it — only `formula::context::FormulaContext::kbi_id_for_code` needs a code-to-id
//! lookup, and that's a real implementation's concern (Phase 1's `PARITY_AUDIT.md` note on
//! `KBIList.getKBI(code)`'s SQL-fallback collapse already covers it).

use std::collections::HashMap;

use crate::kbi::Kbi;
use crate::KbiId;

pub struct KbiList {
    kbis: Vec<Kbi>,
    by_id: HashMap<i32, usize>,
}

impl KbiList {
    /// `KBIList.addKBI`, applied to every row `KbiSetPort::load_kbi_list` hands back.
    pub fn new(kbis: Vec<Kbi>) -> Self {
        let by_id = kbis.iter().enumerate().map(|(i, kbi)| (kbi.record().id.0, i)).collect();
        KbiList { kbis, by_id }
    }

    /// `KBIList.getKBIIterator()`.
    pub fn iter(&self) -> impl Iterator<Item = &Kbi> {
        self.kbis.iter()
    }

    /// `KBIList.getKBI(int kbiID)`.
    pub fn get(&self, kbi_id: KbiId) -> Option<&Kbi> {
        self.by_id.get(&kbi_id.0).map(|&i| &self.kbis[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kbi::KbiType;
    use crate::{KbiConfigId, PropertyId, UnitId};
    use std::collections::HashMap as Map;

    fn record(id: i32) -> crate::kbi::KbiRecord {
        crate::kbi::KbiRecord {
            id: KbiId(id),
            property_id: PropertyId(1),
            name: format!("KBI {id}"),
            code: format!("K{id}"),
            unit_id: UnitId(1),
            kbi_config_id: KbiConfigId(1),
            kbi_type: KbiType::Input,
            primary: false,
            is_rooms_kbi: false,
            is_revenue_center_kbi: false,
            is_departures_kbi: false,
            days_open: Map::new(),
        }
    }

    #[test]
    fn looks_up_by_id_after_construction() {
        let list = KbiList::new(vec![Kbi::Input(record(1)), Kbi::Input(record(2))]);
        assert_eq!(list.get(KbiId(2)).unwrap().record().code, "K2");
        assert!(list.get(KbiId(99)).is_none());
    }

    #[test]
    fn iterates_in_insertion_order() {
        let list = KbiList::new(vec![Kbi::Input(record(3)), Kbi::Input(record(1))]);
        let codes: Vec<_> = list.iter().map(|k| k.record().code.clone()).collect();
        assert_eq!(codes, vec!["K3".to_string(), "K1".to_string()]);
    }
}
