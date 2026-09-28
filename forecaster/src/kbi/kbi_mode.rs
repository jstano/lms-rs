//! `KBIMode.java` — selects which SQL column (`BudgetKBITypeCode` vs `LaborKBITypeCode`) a `KBI`
//! loads from. Stays relevant at the I/O boundary (Phase 3), not just a display enum.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KbiMode {
    Budget,
    Labor,
}

impl KbiMode {
    pub fn code(self) -> &'static str {
        match self {
            KbiMode::Budget => "B",
            KbiMode::Labor => "L",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            KbiMode::Budget => "Budget",
            KbiMode::Labor => "Labor",
        }
    }

    /// `KBIMode.getValue(code)`. Returns `None` instead of throwing
    /// `IllegalArgumentException` on an unrecognized code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "B" => Some(KbiMode::Budget),
            "L" => Some(KbiMode::Labor),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_code() {
        for m in [KbiMode::Budget, KbiMode::Labor] {
            assert_eq!(KbiMode::from_code(m.code()), Some(m));
        }
    }

    #[test]
    fn unknown_code_is_none() {
        assert_eq!(KbiMode::from_code("X"), None);
    }
}
