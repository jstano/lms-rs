//! Port of `com.unifocus.watson.common.enums.StartCalculationFromType`.

use crate::coded_enum;

coded_enum! {
    /// Which boundary a rule counts back from when it needs prior history.
    /// `StartCalculationFromType`.
    StartCalculationFromType {
        PriorPayPeriod => "PPP",
        PriorWeek => "PW",
        PriorDay => "PD",
        PriorWorkWeek => "PWW",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in StartCalculationFromType::VALUES {
            assert_eq!(
                StartCalculationFromType::from_code(value.code()),
                Some(*value)
            );
        }
    }
}
