//! Port of `com.unifocus.rms.timekeeping.domain.hoursdistribution.HoursDistributionType`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/rms/timekeeping/domain/hoursdistribution/HoursDistributionType.java`.
//!
//! The bucket a [`HoursDistribution`](super::hours_distribution::HoursDistribution)
//! falls in. Three are system-generated — regular, overtime, double time — and a
//! site may configure more.
//!
//! The hours-distribution rules find the overtime and double-time buckets **by
//! name**, not by id, even though Java also declares well-known ids for them.
//! That is worth preserving: a site whose buckets carry different ids still
//! works, and one that renames them silently stops.

/// A bucket hours fall into. `HoursDistributionType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoursDistributionType {
    id: i32,
    name: String,
    processor_code: String,
    premium: bool,
}

impl HoursDistributionType {
    /// The name of the regular bucket. `REGULAR_NAME`.
    pub const REGULAR_NAME: &'static str = "Regular";
    /// The name of the overtime bucket. `OVERTIME_NAME` — what
    /// [`TimeCard::ot_hours_distribution_type_id`] matches on.
    ///
    /// [`TimeCard::ot_hours_distribution_type_id`]: super::time_card::TimeCard::ot_hours_distribution_type_id
    pub const OVERTIME_NAME: &'static str = "Overtime";
    /// The name of the double-time bucket. `DOUBLE_TIME_NAME`.
    pub const DOUBLE_TIME_NAME: &'static str = "Double Time";

    /// The well-known ids Java declares alongside the names. The rules do not
    /// use them — they match on name — but a fixture may.
    pub const REGULAR_ID: i32 = 1;
    /// See [`REGULAR_ID`](Self::REGULAR_ID).
    pub const OVERTIME_ID: i32 = 2;
    /// See [`REGULAR_ID`](Self::REGULAR_ID).
    pub const DT_ID: i32 = 3;

    /// Build a distribution type.
    pub fn new(id: i32, name: impl Into<String>, premium: bool) -> Self {
        Self {
            id,
            name: name.into(),
            processor_code: String::new(),
            premium,
        }
    }

    /// The three buckets every site has. `SYSTEM_GENERATED_NAMES`.
    pub fn system_generated() -> Vec<Self> {
        vec![
            Self::new(Self::REGULAR_ID, Self::REGULAR_NAME, false),
            Self::new(Self::OVERTIME_ID, Self::OVERTIME_NAME, true),
            Self::new(Self::DT_ID, Self::DOUBLE_TIME_NAME, true),
        ]
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getProcessorCode()`.
    pub fn processor_code(&self) -> &str {
        &self.processor_code
    }

    /// Is this a premium bucket — anything other than regular time?
    /// `isPremium()`.
    pub fn premium(&self) -> bool {
        self.premium
    }

    /// Is this one of the three the system generates? `SYSTEM_GENERATED_NAMES`.
    pub fn is_system_generated(&self) -> bool {
        matches!(
            self.name.as_str(),
            Self::REGULAR_NAME | Self::OVERTIME_NAME | Self::DOUBLE_TIME_NAME
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_system_buckets_are_regular_overtime_and_double_time() {
        let system = HoursDistributionType::system_generated();

        assert_eq!(system.len(), 3);
        assert_eq!(system[0].name(), "Regular");
        assert_eq!(system[1].name(), "Overtime");
        assert_eq!(system[2].name(), "Double Time");
        assert!(
            system
                .iter()
                .all(HoursDistributionType::is_system_generated)
        );
    }

    #[test]
    fn only_regular_is_not_premium() {
        let system = HoursDistributionType::system_generated();

        assert!(!system[0].premium());
        assert!(system[1].premium());
        assert!(system[2].premium());
    }

    #[test]
    fn a_site_configured_bucket_is_not_system_generated() {
        let shift_differential = HoursDistributionType::new(9, "Shift Differential", true);

        assert!(!shift_differential.is_system_generated());
        assert!(shift_differential.premium());
    }

    #[test]
    fn the_well_known_ids_match_the_system_buckets() {
        let system = HoursDistributionType::system_generated();

        assert_eq!(system[0].id(), HoursDistributionType::REGULAR_ID);
        assert_eq!(system[1].id(), HoursDistributionType::OVERTIME_ID);
        assert_eq!(system[2].id(), HoursDistributionType::DT_ID);
    }
}
