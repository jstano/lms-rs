//! Port of `com.unifocus.watson.common.security.Module`.
//!
//! A licensable product module. It lives in `common.security` rather than
//! `common.enums`, and nothing in the rules tree reads it directly — it arrives
//! only as the licensing metadata each `RuleType` carries, saying which modules
//! a site must have licensed for rules of that type to be configurable.
//!
//! Ported here rather than dropped because [`crate::rules::rule_type::RuleType`]
//! cannot be represented faithfully without it.

/// A licensable product module. `Module`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Module {
    Labor,
    TaskStandards,
    Schedules,
    Budgeter,
    Actuals,
    Epep,
    Budgeter306090,
    /// Time and attendance.
    Ta,
    TipPools,
    Benefits,
    Attendance,
    Revenue,
    Events,
    Reports,
    Mobile,
    SurveySolutions,
    GuestScope,
    MeetingScope,
    StaffScope,
    UserEngagement,
    Employees,
    DigitalTipping,
    XStromberg,
    XClarionPayroll,
    XClarionRevenue,
    XCubes,
    /// What an unrecognised code resolves to.
    Invalid,
}

impl Module {
    /// Every variant, in the order Java declares them.
    pub const VALUES: &'static [Self] = &[
        Self::Labor,
        Self::TaskStandards,
        Self::Schedules,
        Self::Budgeter,
        Self::Actuals,
        Self::Epep,
        Self::Budgeter306090,
        Self::Ta,
        Self::TipPools,
        Self::Benefits,
        Self::Attendance,
        Self::Revenue,
        Self::Events,
        Self::Reports,
        Self::Mobile,
        Self::SurveySolutions,
        Self::GuestScope,
        Self::MeetingScope,
        Self::StaffScope,
        Self::UserEngagement,
        Self::Employees,
        Self::DigitalTipping,
        Self::XStromberg,
        Self::XClarionPayroll,
        Self::XClarionRevenue,
        Self::XCubes,
        Self::Invalid,
    ];

    /// The persisted code, as it appears in a licence file. `getCode()`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Labor => "labor",
            Self::TaskStandards => "task_standards",
            Self::Schedules => "schedules",
            Self::Budgeter => "budget",
            Self::Actuals => "actuals",
            Self::Epep => "epep",
            Self::Budgeter306090 => "306090",
            Self::Ta => "ta",
            Self::TipPools => "tip_pools",
            Self::Benefits => "benefits",
            Self::Attendance => "attendance",
            Self::Revenue => "revenue",
            Self::Events => "events",
            Self::Reports => "reports",
            Self::Mobile => "mobile",
            Self::SurveySolutions => "survey_solutions",
            Self::GuestScope => "guest_scope",
            Self::MeetingScope => "meeting_scope",
            Self::StaffScope => "staff_scope",
            Self::UserEngagement => "user_engagement",
            Self::Employees => "employees",
            Self::DigitalTipping => "digital_tipping",
            Self::XStromberg => "stromberg",
            Self::XClarionPayroll => "clarion_payroll",
            Self::XClarionRevenue => "clarion_revenue",
            Self::XCubes => "cubes",
            Self::Invalid => "___invalid___",
        }
    }

    /// Look up by code, case-insensitively. `Module.fromCode(String)`.
    ///
    /// Unlike every other coded enum in the port, this one does **not** signal
    /// failure: Java returns [`Module::Invalid`] for an unrecognised code rather
    /// than throwing, because old licence files carry modules that no longer
    /// exist. Kept as-is, so the return type is `Self` rather than `Option`.
    pub fn from_code(code: &str) -> Self {
        Self::VALUES
            .iter()
            .copied()
            .find(|module| module.code().eq_ignore_ascii_case(code))
            .unwrap_or(Self::Invalid)
    }

    /// The modules retained only so old licence files still parse.
    /// `deprecatedModules()`. Note Java counts `INVALID` among them.
    pub fn deprecated() -> &'static [Self] {
        &[
            Self::XStromberg,
            Self::XClarionPayroll,
            Self::XClarionRevenue,
            Self::XCubes,
            Self::Invalid,
        ]
    }

    /// Every module still in use. `nonDeprecatedModules()`.
    pub fn non_deprecated() -> Vec<Self> {
        Self::VALUES
            .iter()
            .copied()
            .filter(|module| !Self::deprecated().contains(module))
            .collect()
    }

    /// Is this module retained only for backwards compatibility?
    pub fn is_deprecated(&self) -> bool {
        Self::deprecated().contains(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in Module::VALUES {
            assert_eq!(Module::from_code(value.code()), *value);
        }
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(Module::from_code("TA"), Module::Ta);
        assert_eq!(Module::from_code("Schedules"), Module::Schedules);
    }

    #[test]
    fn an_unknown_code_is_invalid_rather_than_an_error() {
        // The one coded enum that absorbs bad input instead of rejecting it.
        assert_eq!(Module::from_code("no_such_module"), Module::Invalid);
        assert_eq!(Module::from_code(""), Module::Invalid);
    }

    #[test]
    fn the_codes_are_not_derivable_from_the_names() {
        assert_eq!(Module::Budgeter.code(), "budget");
        assert_eq!(Module::Budgeter306090.code(), "306090");
        assert_eq!(Module::XStromberg.code(), "stromberg");
    }

    #[test]
    fn deprecated_and_non_deprecated_partition_the_enum() {
        let non_deprecated = Module::non_deprecated();
        assert_eq!(
            non_deprecated.len() + Module::deprecated().len(),
            Module::VALUES.len()
        );
        assert!(!non_deprecated.contains(&Module::XCubes));
        assert!(non_deprecated.contains(&Module::Ta));
    }

    #[test]
    fn invalid_counts_as_deprecated() {
        // Java puts it in deprecatedModules(), so it is excluded from the
        // non-deprecated list along with the four retired products.
        assert!(Module::Invalid.is_deprecated());
        assert!(!Module::non_deprecated().contains(&Module::Invalid));
    }
}
