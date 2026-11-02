//! Port of `com.unifocus.watson.common.enums.EmployeeAvailType`.

use crate::coded_enum;

coded_enum! {
    /// What an availability row asks for. `EmployeeAvailType`.
    EmployeeAvailType {
        AvailableToWork => "AV",
        RequiredOff => "RQ",
        PreferredOff => "PF",
        RotatedOff => "RO",
    }
}
