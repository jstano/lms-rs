//! Port of `com.unifocus.watson.common.enums.ShiftAdjustType`.

use crate::coded_enum;

coded_enum! {
    /// Which bucket of a shift an adjustment moves. `ShiftAdjustType`.
    ShiftAdjustType {
        Worked => "W",
        Ot => "O",
        Dt => "D",
        Break => "B",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ShiftAdjustType::VALUES {
            assert_eq!(ShiftAdjustType::from_code(value.code()), Some(*value));
        }
    }
}
