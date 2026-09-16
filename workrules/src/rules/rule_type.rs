//! Port of `com.unifocus.watson.common.enums.RuleType`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/enums/RuleType.java`.
//!
//! The 32 buckets the 225 rule classes fall into. A `RuleType` is what a rule
//! set is configured *for*: resolution asks "which rule set applies to this
//! employee, job and date, for this rule type", and each runner names the one
//! type it drives.
//!
//! It lives here rather than in `common::enums` because it and
//! [`RuleClass`](crate::rules::rule_class::RuleClass) are defined against each
//! other.

use crate::coded_enum;
use crate::common::enums::module::Module;

coded_enum! {
    /// A category of work rule. `RuleType`.
    RuleType {
        EmployeeAlert => "EA",
        ShiftDifferential => "DF",
        DailyDifferential => "DD",
        WeeklyDifferential => "WD",
        ShiftAdjust => "SA",
        HoursDistribution => "HD",
        RegularRate => "RR",
        OvertimeRate => "OR",
        DoubleTimeRate => "DR",
        HolidayEligibility => "HE",
        PunchValidation => "PV",
        ReconcileEmployee => "RE",
        EarningRate => "ER",
        ShiftCorrection => "SC",
        ScheduleLunch => "SL",
        PunchRounding => "PR",
        PostCalc => "PC",
        EmployeeEvent => "EE",
        EmployeePoints => "EP",
        BenefitAccrual => "BA",
        BenefitExpiration => "BE",
        TimeOffEarning => "TE",
        ClosePayPeriod => "CP",
        MealPunch => "MP",
        PostPunch => "PP",
        ScheduleRestriction => "SR",
        TimeOffDistribution => "TD",
        RegularHoursDistribution => "RH",
        ScheduleLabel => "LB",
        ScheduleLabelAddendum => "LA",
        ScheduleChangeValidation => "SV",
        ShiftDiffOt => "SD",
    }
}

impl RuleType {
    /// May a rule set of this type hold more than one rule item?
    /// `isAllowCategories()`.
    ///
    /// Where this is `false` a rule set carries exactly one item, so the order
    /// rule items run in cannot matter — which is why most runners get away
    /// with not sorting. The seven single-item types are the distribution and
    /// rate types plus meal punch.
    pub fn allow_categories(&self) -> bool {
        !matches!(
            self,
            Self::HoursDistribution
                | Self::OvertimeRate
                | Self::DoubleTimeRate
                | Self::MealPunch
                | Self::TimeOffDistribution
                | Self::RegularHoursDistribution
                | Self::ShiftDiffOt
        )
    }

    /// Which product modules must be licensed for this rule type.
    /// `getModules()`.
    pub fn modules(&self) -> &'static [Module] {
        const TA: &[Module] = &[Module::Ta];
        const SCHEDULES: &[Module] = &[Module::Schedules];
        const BOTH: &[Module] = &[Module::Ta, Module::Schedules];
        const ALERT: &[Module] = &[Module::Ta, Module::Schedules];

        match self {
            Self::EmployeeAlert => ALERT,
            Self::HoursDistribution
            | Self::RegularRate
            | Self::OvertimeRate
            | Self::DoubleTimeRate
            | Self::ShiftDiffOt => BOTH,
            Self::ScheduleLunch
            | Self::ScheduleRestriction
            | Self::ScheduleLabel
            | Self::ScheduleLabelAddendum
            | Self::ScheduleChangeValidation => SCHEDULES,
            _ => TA,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn every_code_round_trips() {
        for value in RuleType::VALUES {
            assert_eq!(RuleType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn all_thirty_two_types_are_present() {
        assert_eq!(RuleType::VALUES.len(), 32);
    }

    #[test]
    fn the_codes_are_unique() {
        let mut codes: Vec<_> = RuleType::VALUES.iter().map(|t| t.code()).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count);
    }

    #[rstest]
    #[case(RuleType::HoursDistribution)]
    #[case(RuleType::OvertimeRate)]
    #[case(RuleType::DoubleTimeRate)]
    #[case(RuleType::MealPunch)]
    #[case(RuleType::TimeOffDistribution)]
    #[case(RuleType::RegularHoursDistribution)]
    #[case(RuleType::ShiftDiffOt)]
    fn the_seven_single_item_rule_types(#[case] rule_type: RuleType) {
        assert!(!rule_type.allow_categories());
    }

    #[test]
    fn exactly_seven_rule_types_are_single_item() {
        let single = RuleType::VALUES
            .iter()
            .filter(|t| !t.allow_categories())
            .count();
        assert_eq!(single, 7);
    }

    #[rstest]
    #[case(RuleType::PunchRounding)]
    #[case(RuleType::RegularRate)]
    #[case(RuleType::BenefitAccrual)]
    #[case(RuleType::PostCalc)]
    fn everything_else_may_hold_several_rule_items(#[case] rule_type: RuleType) {
        assert!(rule_type.allow_categories());
    }

    #[rstest]
    #[case(RuleType::ScheduleLunch)]
    #[case(RuleType::ScheduleRestriction)]
    #[case(RuleType::ScheduleLabel)]
    #[case(RuleType::ScheduleLabelAddendum)]
    #[case(RuleType::ScheduleChangeValidation)]
    fn the_schedules_only_rule_types(#[case] rule_type: RuleType) {
        assert_eq!(rule_type.modules(), &[Module::Schedules]);
    }

    #[rstest]
    #[case(RuleType::EmployeeAlert)]
    #[case(RuleType::HoursDistribution)]
    #[case(RuleType::RegularRate)]
    #[case(RuleType::OvertimeRate)]
    #[case(RuleType::DoubleTimeRate)]
    #[case(RuleType::ShiftDiffOt)]
    fn the_rule_types_needing_both_modules(#[case] rule_type: RuleType) {
        assert_eq!(rule_type.modules(), &[Module::Ta, Module::Schedules]);
    }

    #[test]
    fn the_remainder_are_time_and_attendance_only() {
        assert_eq!(RuleType::PunchRounding.modules(), &[Module::Ta]);
        assert_eq!(RuleType::BenefitAccrual.modules(), &[Module::Ta]);
        assert_eq!(RuleType::PostPunch.modules(), &[Module::Ta]);
    }

    #[test]
    fn every_rule_type_names_at_least_one_module() {
        for rule_type in RuleType::VALUES {
            assert!(!rule_type.modules().is_empty(), "{rule_type:?}");
        }
    }
}
