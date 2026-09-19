//! Port of `com.unifocus.watson.common.enums.ShiftErrorType`.
//!
//! What can be wrong with a shift. Punch-validation and post-punch rules raise
//! these; shift-correction rules clear them.

use crate::coded_enum;

coded_enum! {
    /// A problem recorded against a shift. `ShiftErrorType`.
    ShiftErrorType {
        MinHoursOff => "MH",
        MinDaysOff => "MD",
        MinShiftLength => "MS",
        MaxShiftLength => "XS",
        TimeOff => "TO",
        Availability => "AV",
        AvailableHours => "AH",
        EmptyShift => "ES",
        MissingIn => "MI",
        MissingOut => "MO",
        MissingBreakIn => "BI",
        MissingBreakOut => "BO",
        UnbalancedBreak => "UB",
        InOutSame => "IO",
        InvalidTimes => "IT",
        RoundNextDayInactiveEmp => "RIE",
        RoundNextDayInactiveJob => "RIJ",
        RequiredDaysOff => "RDO",
        ShiftDefinition => "SD",
        NoHomeJob => "NHJ",
        JobNotActive => "JNA",
        NotActive => "NA",
        DifferentPaygroups => "DPG",
        PaygroupNotActive => "PNA",
        MaxHoursOnDay => "MHD",
        MaxHoursPerWeek => "MPW",
        EarliestStartLatestEnd => "ESLE",
        MaxDaysWorkedPerWeek => "MDWPW",
    }
}

impl ShiftErrorType {
    /// Can an employee resolve this at the time clock?
    /// `isFixableFromTimeclock()`.
    ///
    /// In Java this is a constructor flag rather than a derived predicate. The
    /// five that are fixable are exactly the punch errors — see
    /// [`is_punch_error`](Self::is_punch_error), which lists the same set
    /// independently. Both are kept, because Java keeps both and a future
    /// error could set one without the other.
    pub fn is_fixable_from_timeclock(&self) -> bool {
        matches!(
            self,
            Self::MissingIn
                | Self::MissingOut
                | Self::MissingBreakIn
                | Self::MissingBreakOut
                | Self::UnbalancedBreak
        )
    }

    /// Is this error about the punches themselves? `isPunchError()`.
    pub fn is_punch_error(&self) -> bool {
        matches!(
            self,
            Self::MissingBreakIn
                | Self::MissingOut
                | Self::MissingIn
                | Self::MissingBreakOut
                | Self::UnbalancedBreak
        )
    }

    /// The Java enum constant's own spelling — `Enum.toString()`'s default,
    /// which Java never overrides for this type. First needed by
    /// `ScheduleRestrictionResult.getMessage()`, which returns exactly this.
    pub fn name(&self) -> &'static str {
        match self {
            Self::MinHoursOff => "MIN_HOURS_OFF",
            Self::MinDaysOff => "MIN_DAYS_OFF",
            Self::MinShiftLength => "MIN_SHIFT_LENGTH",
            Self::MaxShiftLength => "MAX_SHIFT_LENGTH",
            Self::TimeOff => "TIME_OFF",
            Self::Availability => "AVAILABILITY",
            Self::AvailableHours => "AVAILABLE_HOURS",
            Self::EmptyShift => "EMPTY_SHIFT",
            Self::MissingIn => "MISSING_IN",
            Self::MissingOut => "MISSING_OUT",
            Self::MissingBreakIn => "MISSING_BREAK_IN",
            Self::MissingBreakOut => "MISSING_BREAK_OUT",
            Self::UnbalancedBreak => "UNBALANCED_BREAK",
            Self::InOutSame => "IN_OUT_SAME",
            Self::InvalidTimes => "INVALID_TIMES",
            Self::RoundNextDayInactiveEmp => "ROUND_NEXT_DAY_INACTIVE_EMP",
            Self::RoundNextDayInactiveJob => "ROUND_NEXT_DAY_INACTIVE_JOB",
            Self::RequiredDaysOff => "REQUIRED_DAYS_OFF",
            Self::ShiftDefinition => "SHIFT_DEFINITION",
            Self::NoHomeJob => "NO_HOME_JOB",
            Self::JobNotActive => "JOB_NOT_ACTIVE",
            Self::NotActive => "NOT_ACTIVE",
            Self::DifferentPaygroups => "DIFFERENT_PAYGROUPS",
            Self::PaygroupNotActive => "PAYGROUP_NOT_ACTIVE",
            Self::MaxHoursOnDay => "MAX_HOURS_ON_DAY",
            Self::MaxHoursPerWeek => "MAX_HOURS_PER_WEEK",
            Self::EarliestStartLatestEnd => "EARLIEST_START_LATEST_END",
            Self::MaxDaysWorkedPerWeek => "MAX_DAYS_WORKED_PER_WEEK",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn every_code_round_trips() {
        for value in ShiftErrorType::VALUES {
            assert_eq!(ShiftErrorType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn all_twenty_eight_are_present() {
        assert_eq!(ShiftErrorType::VALUES.len(), 28);
    }

    #[test]
    fn the_codes_are_unique() {
        let mut codes: Vec<_> = ShiftErrorType::VALUES.iter().map(|e| e.code()).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count);
    }

    #[test]
    fn the_names_are_unique_and_screaming_snake() {
        let mut names: Vec<_> = ShiftErrorType::VALUES.iter().map(|e| e.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
        for name in names {
            assert!(name.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
        }
    }

    #[test]
    fn required_days_off_names_itself_the_way_java_to_string_would() {
        assert_eq!(ShiftErrorType::RequiredDaysOff.name(), "REQUIRED_DAYS_OFF");
    }

    #[rstest]
    #[case(ShiftErrorType::MissingIn)]
    #[case(ShiftErrorType::MissingOut)]
    #[case(ShiftErrorType::MissingBreakIn)]
    #[case(ShiftErrorType::MissingBreakOut)]
    #[case(ShiftErrorType::UnbalancedBreak)]
    fn the_five_punch_errors(#[case] error: ShiftErrorType) {
        assert!(error.is_punch_error());
        assert!(error.is_fixable_from_timeclock());
    }

    #[rstest]
    #[case(ShiftErrorType::InOutSame)]
    #[case(ShiftErrorType::InvalidTimes)]
    #[case(ShiftErrorType::EmptyShift)]
    #[case(ShiftErrorType::MaxShiftLength)]
    fn other_errors_are_neither_punch_errors_nor_clock_fixable(#[case] error: ShiftErrorType) {
        assert!(!error.is_punch_error());
        assert!(!error.is_fixable_from_timeclock());
    }

    #[test]
    fn the_two_predicates_currently_agree_across_the_whole_enum() {
        // They are separate flags in Java and could diverge; this pins the
        // present state so a future divergence is a deliberate edit.
        for error in ShiftErrorType::VALUES {
            assert_eq!(
                error.is_punch_error(),
                error.is_fixable_from_timeclock(),
                "{error:?}"
            );
        }
    }
}
