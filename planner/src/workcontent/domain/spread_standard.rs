use crate::id_type;
use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::job_shift::JobShiftId;
use crate::workcontent::domain::standard_set::StandardSetId;
use crate::workcontent::domain::units::Units;

id_type!(SpreadStandardId, uuid_v4);
id_type!(SpreadStandardValueId, uuid_v4);
id_type!(DynamicSpreadStandardId, uuid_v4);

/// Whether a spread standard's shape is configured up front or computed per
/// plan from observed volumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadStandardType {
    Fixed,
    Dynamic,
}

/// A standard that spreads work along a pre-set intraday shape rather than
/// deriving one from a volume curve.
///
/// Which values apply depends on the driver volume for the date (picking a
/// range) and then on the environment (picking a value within it).
pub struct SpreadStandard {
    id: SpreadStandardId,
    job_id: JobId,
    standard_set_id: StandardSetId,
    job_shift_id: JobShiftId,
    business_driver_id: BusinessDriverId,
    dynamic_spread_unit_type: Units,
    spread_standard_type: SpreadStandardType,
    ranges: Vec<SpreadStandardRange>,
}

impl SpreadStandard {
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: BusinessDriverId,
        dynamic_spread_unit_type: Units,
        spread_standard_type: SpreadStandardType,
        ranges: Vec<SpreadStandardRange>,
    ) -> Self {
        Self {
            id: SpreadStandardId::new(),
            job_id,
            standard_set_id,
            job_shift_id,
            business_driver_id,
            dynamic_spread_unit_type,
            spread_standard_type,
            ranges,
        }
    }

    pub fn id(&self) -> SpreadStandardId {
        self.id
    }

    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn standard_set_id(&self) -> StandardSetId {
        self.standard_set_id
    }

    pub fn job_shift_id(&self) -> JobShiftId {
        self.job_shift_id
    }

    pub fn business_driver_id(&self) -> BusinessDriverId {
        self.business_driver_id
    }

    pub fn dynamic_spread_unit_type(&self) -> Units {
        self.dynamic_spread_unit_type
    }

    pub fn spread_standard_type(&self) -> SpreadStandardType {
        self.spread_standard_type
    }

    pub fn ranges(&self) -> &[SpreadStandardRange] {
        &self.ranges
    }

    /// The range covering `volume`, if any.
    pub fn range_for_volume(&self, volume: i32) -> Option<&SpreadStandardRange> {
        self.ranges.iter().find(|range| range.contains_value(volume))
    }

    /// The fixed values for `volume` in `environment_id`, if configured.
    pub fn fixed_value_for(
        &self,
        volume: i32,
        environment_id: EnvironmentId,
    ) -> Option<&SpreadStandardValue> {
        self.range_for_volume(volume)?
            .fixed_value_for_environment(environment_id)
    }

    /// The dynamic standard for `volume` in `environment_id`, if configured.
    pub fn dynamic_value_for(
        &self,
        volume: i32,
        environment_id: EnvironmentId,
    ) -> Option<&DynamicSpreadStandard> {
        self.range_for_volume(volume)?
            .dynamic_value_for_environment(environment_id)
    }
}

/// One volume band of a spread standard, inclusive at both ends.
///
/// A range carries both flavors of value; which one is read depends on the
/// owning standard's [`SpreadStandardType`], so the unused list is simply empty.
pub struct SpreadStandardRange {
    from_volume: i32,
    to_volume: i32,
    fixed_values: Vec<SpreadStandardValue>,
    dynamic_values: Vec<DynamicSpreadStandard>,
}

impl SpreadStandardRange {
    pub fn new(
        from_volume: i32,
        to_volume: i32,
        fixed_values: Vec<SpreadStandardValue>,
        dynamic_values: Vec<DynamicSpreadStandard>,
    ) -> Self {
        Self {
            from_volume,
            to_volume,
            fixed_values,
            dynamic_values,
        }
    }

    pub fn from_volume(&self) -> i32 {
        self.from_volume
    }

    pub fn to_volume(&self) -> i32 {
        self.to_volume
    }

    pub fn fixed_values(&self) -> &[SpreadStandardValue] {
        &self.fixed_values
    }

    pub fn dynamic_values(&self) -> &[DynamicSpreadStandard] {
        &self.dynamic_values
    }

    pub fn contains_value(&self, value: i32) -> bool {
        value >= self.from_volume && value <= self.to_volume
    }

    pub fn fixed_value_for_environment(
        &self,
        environment_id: EnvironmentId,
    ) -> Option<&SpreadStandardValue> {
        self.fixed_values
            .iter()
            .find(|value| value.environment_id() == environment_id)
    }

    pub fn dynamic_value_for_environment(
        &self,
        environment_id: EnvironmentId,
    ) -> Option<&DynamicSpreadStandard> {
        self.dynamic_values
            .iter()
            .find(|value| value.environment_id() == environment_id)
    }
}

/// A configured intraday shape, at five-minute granularity across a day.
///
/// A `None` slot means nothing was configured for that five minutes — it
/// contributes no work, which is not the same as an explicit zero only in that
/// it never overwrites anything.
pub struct SpreadStandardValue {
    id: SpreadStandardValueId,
    environment_id: EnvironmentId,
    spread_values: Vec<Option<i32>>,
}

impl SpreadStandardValue {
    pub fn new(environment_id: EnvironmentId, spread_values: Vec<Option<i32>>) -> Self {
        Self {
            id: SpreadStandardValueId::new(),
            environment_id,
            spread_values,
        }
    }

    pub fn id(&self) -> SpreadStandardValueId {
        self.id
    }

    pub fn environment_id(&self) -> EnvironmentId {
        self.environment_id
    }

    pub fn spread_values(&self) -> &[Option<i32>] {
        &self.spread_values
    }
}

/// A spread value computed from the plan's own volumes rather than configured.
pub struct DynamicSpreadStandard {
    id: DynamicSpreadStandardId,
    environment_id: EnvironmentId,
    value: f64,
}

impl DynamicSpreadStandard {
    pub fn new(environment_id: EnvironmentId, value: f64) -> Self {
        Self {
            id: DynamicSpreadStandardId::new(),
            environment_id,
            value,
        }
    }

    pub fn id(&self) -> DynamicSpreadStandardId {
        self.id
    }

    pub fn environment_id(&self) -> EnvironmentId {
        self.environment_id
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn standard_with(ranges: Vec<SpreadStandardRange>) -> SpreadStandard {
        SpreadStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            Units::MinutesPerUnit,
            SpreadStandardType::Fixed,
            ranges,
        )
    }

    #[rstest]
    #[case(-1, false)]
    #[case(0, true)]
    #[case(50, true)]
    #[case(100, true)]
    #[case(101, false)]
    fn a_range_is_inclusive_at_both_ends(#[case] volume: i32, #[case] expected: bool) {
        let range = SpreadStandardRange::new(0, 100, Vec::new(), Vec::new());

        assert_eq!(range.contains_value(volume), expected);
    }

    #[test]
    fn a_fixed_value_is_found_by_volume_then_environment() {
        let environment_id = EnvironmentId::new();
        let other_environment_id = EnvironmentId::new();
        let standard = standard_with(vec![SpreadStandardRange::new(
            0,
            99,
            vec![SpreadStandardValue::new(environment_id, vec![Some(3)])],
            Vec::new(),
        )]);

        assert!(standard.fixed_value_for(50, environment_id).is_some());
        assert!(standard.fixed_value_for(50, other_environment_id).is_none());
        // A volume outside every range has no value at all.
        assert!(standard.fixed_value_for(100, environment_id).is_none());
    }

    #[test]
    fn a_dynamic_value_is_found_by_volume_then_environment() {
        let environment_id = EnvironmentId::new();
        let standard = standard_with(vec![SpreadStandardRange::new(
            0,
            99,
            Vec::new(),
            vec![DynamicSpreadStandard::new(environment_id, 12.5)],
        )]);

        assert_eq!(
            standard
                .dynamic_value_for(50, environment_id)
                .map(|value| value.value()),
            Some(12.5)
        );
        assert!(standard.dynamic_value_for(100, environment_id).is_none());
    }
}
