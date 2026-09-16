//! Port of `com.unifocus.watson.common.enums.ShiftAdjustSource`.

use crate::coded_enum;

coded_enum! {
    /// What created a shift adjustment. `ShiftAdjustSource`.
    ShiftAdjustSource {
        Auto => "A",
        Manual => "M",
        Rule => "R",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ShiftAdjustSource::VALUES {
            assert_eq!(ShiftAdjustSource::from_code(value.code()), Some(*value));
        }
    }
}
