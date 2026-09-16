//! Port of `com.unifocus.watson.common.enums.TimeOffStatus`.

use crate::coded_enum;

coded_enum! {
    /// Where a time-off request stands. `TimeOffStatus`.
    TimeOffStatus {
        /// A filter value meaning "any status", not a state a request holds.
        All => "L",
        Pending => "P",
        Approved => "A",
        Denied => "D",
        Cancelled => "C",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in TimeOffStatus::VALUES {
            assert_eq!(TimeOffStatus::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn all_is_coded_l_rather_than_a() {
        // A is taken by Approved, so the catch-all had to go somewhere else.
        assert_eq!(TimeOffStatus::All.code(), "L");
        assert_eq!(TimeOffStatus::Approved.code(), "A");
    }
}
