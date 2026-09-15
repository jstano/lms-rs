//! Failures the calculation core can report.
//!
//! The generators are a pure functional core: they never panic and never do
//! I/O, so anything they cannot compute comes back as one of these instead.
//! Several variants mark parts of the Java engine that are deliberately not
//! ported yet — flagged rather than silently producing zero work.

use crate::workcontent::domain::units::Units;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationError {
    /// Task standards are real production behavior but are not modeled yet:
    /// they carry their own frequency/expectancy formulas and environment
    /// scoped lookups. Reaching this means a job configured work the engine
    /// cannot yet account for.
    TaskStandardsNotSupported,

    /// The `FillGaps` distribution method has no distributor yet.
    FillGapsNotImplemented,

    /// A dynamic spread standard was configured without a service to resolve
    /// its values against.
    NoDynamicSpreadService,

    /// No common period length exists for the two granularities being
    /// converted between. Unreachable for the supported 10/15/30/60 minute
    /// set, but kept total rather than panicking.
    LowestCommonDenominatorUndefined { a: i32, b: i32 },

    /// A standard is expressed in a unit that has no work-minute formula.
    /// Java throws `IllegalArgumentException` in the same place.
    InvalidUnits(Units),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskStandardsNotSupported => {
                write!(f, "task standards are not supported yet")
            }
            Self::FillGapsNotImplemented => {
                write!(f, "the fill-gaps distribution method is not implemented yet")
            }
            Self::NoDynamicSpreadService => {
                write!(f, "no dynamic spread service is available")
            }
            Self::LowestCommonDenominatorUndefined { a, b } => {
                write!(
                    f,
                    "no lowest common denominator exists for period lengths {a} and {b}"
                )
            }
            Self::InvalidUnits(units) => {
                write!(f, "{units:?} has no work-minute formula")
            }
        }
    }
}

impl std::error::Error for GenerationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_describe_themselves() {
        assert_eq!(
            GenerationError::TaskStandardsNotSupported.to_string(),
            "task standards are not supported yet"
        );
        assert_eq!(
            GenerationError::LowestCommonDenominatorUndefined { a: 7, b: 10 }.to_string(),
            "no lowest common denominator exists for period lengths 7 and 10"
        );
    }
}
