//! Port of `com.unifocus.watson.common.enums.ScheduleLockoutLevel`.

use crate::coded_enum;

coded_enum! {
    /// How hard a schedule lockout rule pushes back. `ScheduleLockoutLevel`.
    ScheduleLockoutLevel {
        /// No restriction.
        None => "NONE",
        /// Warn, but allow.
        Warn => "WARN",
        /// Refuse.
        Lock => "LOCK",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ScheduleLockoutLevel::VALUES {
            assert_eq!(ScheduleLockoutLevel::from_code(value.code()), Some(*value));
        }
    }
}
