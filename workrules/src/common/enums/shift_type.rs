//! Port of `com.unifocus.watson.common.enums.ShiftType`.

use crate::coded_enum;

coded_enum! {
    /// Whether a shift is worked, planned, or machine-generated. `ShiftType`.
    ShiftType {
        /// A shift that was actually worked.
        Actual => "A",
        /// A shift produced by the scheduling engine.
        Generated => "G",
        /// A scheduled shift.
        Schedule => "S",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ShiftType::VALUES {
            assert_eq!(ShiftType::from_code(value.code()), Some(*value));
        }
    }
}
