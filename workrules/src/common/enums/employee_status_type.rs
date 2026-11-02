//! Port of `com.unifocus.watson.common.enums.EmployeeStatusType`.

use crate::coded_enum;

coded_enum! {
    /// An employee's employment state over an interval. `EmployeeStatusType`.
    EmployeeStatusType {
        Active => "A",
        LeaveOfAbsence => "L",
        Rehire => "R",
        Terminated => "T",
        Transferred => "F",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in EmployeeStatusType::VALUES {
            assert_eq!(EmployeeStatusType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn all_five_are_present() {
        assert_eq!(EmployeeStatusType::VALUES.len(), 5);
    }
}
