//! `StatKBIType.java`, `StatRelatedKBI.java`, `StatKBI.java` — per-day-of-week configuration of how
//! a `StatisticalKBI` computes its value: either TAES against one related KBI, or a regression
//! against a list of related KBIs (each `Independent`/`Add`/`Subtract`).

use crate::KbiId;
use crate::kbi::StatOpType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatKbiType {
    Taes,
    Regression,
}

impl StatKbiType {
    pub fn code(self) -> &'static str {
        match self {
            StatKbiType::Taes => "T",
            StatKbiType::Regression => "R",
        }
    }

    /// `StatKBIType.getValue(code)`. Returns `None` instead of throwing
    /// `IllegalArgumentException` on an unrecognized code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "T" => Some(StatKbiType::Taes),
            "R" => Some(StatKbiType::Regression),
            _ => None,
        }
    }
}

/// `StatRelatedKBI.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatRelatedKbi {
    pub kbi_id: KbiId,
    pub op_type: StatOpType,
}

impl StatRelatedKbi {
    pub fn new(kbi_id: KbiId, op_type: StatOpType) -> Self {
        StatRelatedKbi { kbi_id, op_type }
    }
}

/// `StatKBI.java` — one day-of-week's statistical configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatKbi {
    pub kbi_type: StatKbiType,
    pub related_kbis: Vec<StatRelatedKbi>,
    /// Only meaningful when `kbi_type == PastAverage` in the Java doc comment, but Java never
    /// actually sets a `PAST_AVERAGE` variant on `StatKBIType` (only `TAES`/`REGRESSION` exist) —
    /// `numberDataPoints` is dead state on this class. Kept for parity/documentation, unused.
    pub number_data_points: i32,
}

impl StatKbi {
    /// `StatKBI(StatKBIType type)`.
    pub fn of_type(kbi_type: StatKbiType) -> Self {
        StatKbi {
            kbi_type,
            related_kbis: Vec::new(),
            number_data_points: 0,
        }
    }

    /// `StatKBI(int kbiId)` — the TAES constructor: wraps a single related KBI as `Independent`.
    pub fn taes(kbi_id: KbiId) -> Self {
        StatKbi {
            kbi_type: StatKbiType::Taes,
            related_kbis: vec![StatRelatedKbi::new(kbi_id, StatOpType::Independent)],
            number_data_points: 0,
        }
    }

    pub fn add_related_kbi(&mut self, kbi: StatRelatedKbi) {
        self.related_kbis.push(kbi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taes_constructor_wraps_single_independent_related_kbi() {
        let stat = StatKbi::taes(KbiId(42));
        assert_eq!(stat.kbi_type, StatKbiType::Taes);
        assert_eq!(stat.related_kbis, vec![StatRelatedKbi::new(KbiId(42), StatOpType::Independent)]);
    }

    #[test]
    fn stat_kbi_type_round_trips() {
        for t in [StatKbiType::Taes, StatKbiType::Regression] {
            assert_eq!(StatKbiType::from_code(t.code()), Some(t));
        }
    }
}
