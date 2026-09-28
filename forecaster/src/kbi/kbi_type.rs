//! `KBIType.java` — selects which `Kbi` variant a row deserializes into.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KbiType {
    Input,
    Calculated,
    Statistical,
    PercentOfBase,
}

impl KbiType {
    pub fn code(self) -> &'static str {
        match self {
            KbiType::Input => "I",
            KbiType::Calculated => "C",
            KbiType::Statistical => "S",
            KbiType::PercentOfBase => "P",
        }
    }

    /// `KBIType.getKBIType(code)`. Returns `None` instead of throwing
    /// `IllegalArgumentException` on an unrecognized code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "I" => Some(KbiType::Input),
            "C" => Some(KbiType::Calculated),
            "S" => Some(KbiType::Statistical),
            "P" => Some(KbiType::PercentOfBase),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_code() {
        for t in [
            KbiType::Input,
            KbiType::Calculated,
            KbiType::Statistical,
            KbiType::PercentOfBase,
        ] {
            assert_eq!(KbiType::from_code(t.code()), Some(t));
        }
    }

    #[test]
    fn unknown_code_is_none() {
        assert_eq!(KbiType::from_code("X"), None);
    }
}
