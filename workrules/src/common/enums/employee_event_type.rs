//! Port of `com.unifocus.watson.common.enums.EmployeeEventType`.

use crate::coded_enum;

coded_enum! {
    /// Whether an employee event is positive or negative. `EmployeeEventType`.
    EmployeeEventType {
        Award => "A",
        Violation => "V",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in EmployeeEventType::VALUES {
            assert_eq!(EmployeeEventType::from_code(value.code()), Some(*value));
        }
    }
}
