//! `StatOpType.java` — regression/past-average/TAES dispatch key: whether a related KBI feeds a
//! regression as an independent variable, or is simply added/subtracted into the result.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatOpType {
    Independent,
    Add,
    Subtract,
}

impl StatOpType {
    pub fn code(self) -> &'static str {
        match self {
            StatOpType::Independent => "I",
            StatOpType::Add => "A",
            StatOpType::Subtract => "S",
        }
    }

    /// `StatOpType.getStatOpType(code)`. Returns `None` instead of throwing
    /// `IllegalArgumentException` on an unrecognized code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "I" => Some(StatOpType::Independent),
            "A" => Some(StatOpType::Add),
            "S" => Some(StatOpType::Subtract),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_code() {
        for t in [StatOpType::Independent, StatOpType::Add, StatOpType::Subtract] {
            assert_eq!(StatOpType::from_code(t.code()), Some(t));
        }
    }

    #[test]
    fn unknown_code_is_none() {
        assert_eq!(StatOpType::from_code("X"), None);
    }
}
