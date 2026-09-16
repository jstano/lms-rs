//! Port of `com.unifocus.watson.common.enums.HolidayCalculationPeriodType`.

use crate::coded_enum;

coded_enum! {
    /// The window a holiday-eligibility rule measures over.
    /// `HolidayCalculationPeriodType`.
    HolidayCalculationPeriodType {
        AnniversaryYear => "ANIV",
        Prior2Quarters => "P2QRT",
        PriorPayPeriod => "PPP",
        LastXDays => "LASTX",
        LastXWeeksWorked => "LXWW",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in HolidayCalculationPeriodType::VALUES {
            assert_eq!(
                HolidayCalculationPeriodType::from_code(value.code()),
                Some(*value)
            );
        }
    }
}
