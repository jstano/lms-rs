//! Failures the forecaster engine can report.
//!
//! Replaces `KBIComputeException` (carries a KBI code + message), `InvalidDateException`, and
//! `DataAccessException` with a single crate-wide enum, following `planner`'s `GenerationError`
//! precedent (`planner/src/workcontent/generators/error.rs`).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecasterError {
    /// `KBIComputeException` — carries the KBI code of the formula being evaluated when the
    /// failure occurred, plus a message.
    KbiCompute { kbi_code: String, message: String },

    /// `InvalidDateException`.
    InvalidDate { message: String },

    /// `DataAccessException`, surfaced through the I/O port traits (Phase 3).
    DataAccess { message: String },
}

impl fmt::Display for ForecasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KbiCompute { kbi_code, message } => {
                write!(f, "KBI compute error for {kbi_code}: {message}")
            }
            Self::InvalidDate { message } => write!(f, "invalid date: {message}"),
            Self::DataAccess { message } => write!(f, "data access error: {message}"),
        }
    }
}

impl std::error::Error for ForecasterError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_describe_themselves() {
        assert_eq!(
            ForecasterError::KbiCompute {
                kbi_code: "ROOMS".to_string(),
                message: "unable to find the KBI in the database".to_string(),
            }
            .to_string(),
            "KBI compute error for ROOMS: unable to find the KBI in the database"
        );
    }
}
