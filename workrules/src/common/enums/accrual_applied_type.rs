//! Port of `com.unifocus.watson.common.enums.AccrualAppliedType`.

use crate::coded_enum;

coded_enum! {
    /// How an accrual was settled. `AccrualAppliedType`.
    AccrualAppliedType {
        /// Expired hours written off.
        ExpWaived => "W",
        /// Expired hours paid out as an earning.
        ExpEarning => "E",
        /// Carried forward against the balance.
        Balanced => "B",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in AccrualAppliedType::VALUES {
            assert_eq!(AccrualAppliedType::from_code(value.code()), Some(*value));
        }
    }
}
