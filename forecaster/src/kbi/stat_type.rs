//! `KBIStatType.java`. Introduced ahead of the rest of Phase 2's enum set because the Phase 1
//! formula evaluator's `calculate(date, valueType)` signature already needs it.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KbiStatType {
    Estimated,
    Forecasted,
    Adjusted,
    Actual,
}

impl KbiStatType {
    pub fn code(self) -> &'static str {
        match self {
            KbiStatType::Estimated => "EST",
            KbiStatType::Forecasted => "FST",
            KbiStatType::Adjusted => "ADJ",
            KbiStatType::Actual => "ACT",
        }
    }

    /// `KBIStatType.getStatType(code)`. Returns `None` instead of throwing
    /// `IllegalArgumentException` on an unrecognized code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "EST" => Some(KbiStatType::Estimated),
            "FST" => Some(KbiStatType::Forecasted),
            "ADJ" => Some(KbiStatType::Adjusted),
            "ACT" => Some(KbiStatType::Actual),
            _ => None,
        }
    }
}
