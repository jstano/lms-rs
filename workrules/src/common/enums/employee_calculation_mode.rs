//! Port of `com.unifocus.watson.common.enums.EmployeeCalculationMode`.
//!
//! The one enum in the set with no code and no resource key — seven lines in
//! Java, constants only. It says which pipeline is running, which some rules
//! use to skip work that only matters for time and attendance.

/// Which calculation is in progress. `EmployeeCalculationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmployeeCalculationMode {
    /// Time and attendance.
    Ta,
    AutoSchedule,
    EditSchedule,
}

impl EmployeeCalculationMode {
    /// Every variant, in the order Java declares them.
    pub const VALUES: &'static [Self] = &[Self::Ta, Self::AutoSchedule, Self::EditSchedule];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_has_the_three_modes_in_declaration_order() {
        assert_eq!(
            EmployeeCalculationMode::VALUES,
            &[
                EmployeeCalculationMode::Ta,
                EmployeeCalculationMode::AutoSchedule,
                EmployeeCalculationMode::EditSchedule
            ]
        );
    }
}
