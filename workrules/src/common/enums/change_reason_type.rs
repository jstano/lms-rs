//! Port of `com.unifocus.watson.common.enums.ChangeReasonType`.

use crate::coded_enum;

coded_enum! {
    /// Why a scheduled shift changed hands or went unworked. `ChangeReasonType`.
    ChangeReasonType {
        Absence => "A",
        Tardy => "T",
        Drop => "D",
        Swap => "S",
        Giveaway => "G",
        Open => "O",
        Take => "K",
        BlastManual => "M",
        BlastAuto => "U",
        BlastFirstCome => "F",
        EmployeeSelection => "E",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in ChangeReasonType::VALUES {
            assert_eq!(ChangeReasonType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn take_is_coded_k_because_tardy_took_t() {
        assert_eq!(ChangeReasonType::Tardy.code(), "T");
        assert_eq!(ChangeReasonType::Take.code(), "K");
    }
}
