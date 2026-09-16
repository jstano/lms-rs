//! Port of `com.unifocus.watson.common.enums.EarningSource`.
//!
//! The most-referenced enum in the rules tree (50 imports). Rules both read it,
//! to decide whether an earning is theirs to touch, and write it — an earning a
//! rule creates is stamped [`EarningSource::Rule`].

use crate::coded_enum;

coded_enum! {
    /// Where an earning came from. `EarningSource`.
    EarningSource {
        /// Produced by the calculation pipeline.
        Auto => "A",
        /// Entered by hand.
        Manual => "M",
        /// Produced by a work rule.
        Rule => "R",
        /// Distributed from a tip pool.
        TipPool => "T",
        /// Brought in from an external feed.
        Import => "I",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in EarningSource::VALUES {
            assert_eq!(EarningSource::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn rule_written_earnings_are_stamped_r() {
        assert_eq!(EarningSource::Rule.code(), "R");
    }
}
