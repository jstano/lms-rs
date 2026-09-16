//! Port of `com.unifocus.watson.common.enums.ViolationEventUnitType`.

use crate::coded_enum;

coded_enum! {
    /// Whether an attendance violation is counted in points or occurrences.
    /// `ViolationEventUnitType`.
    ViolationEventUnitType {
        Point => "P",
        Event => "E",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ViolationEventUnitType::VALUES {
            assert_eq!(
                ViolationEventUnitType::from_code(value.code()),
                Some(*value)
            );
        }
    }
}
