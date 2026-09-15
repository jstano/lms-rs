use crate::id_type;
use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::domain::distribution_method::DistributionMethod;
use crate::workcontent::domain::distribution_schedule::DistributionScheduleId;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::job_shift::JobShiftId;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::standard_set::StandardSetId;
use crate::workcontent::domain::units::Units;
use crate::workcontent::domain::work_type::WorkType;
use joda_rs::LocalTime;

id_type!(ShiftStandardId, uuid_v4);

/// A standard attached to one shift of one job, saying how much work a given
/// volume of a business driver generates.
///
/// A volume band holds one value per operating environment, mirroring Java's
/// `ShiftRelatedStandardValue` level. Callers with environment data resolve
/// through [`value_for_volume_in_environment`](Self::value_for_volume_in_environment);
/// the non-flowed path, which has none, uses
/// [`value_for_volume`](Self::value_for_volume) and takes whatever the band
/// holds.
pub struct ShiftStandard {
    id: ShiftStandardId,
    job_id: JobId,
    standard_set_id: StandardSetId,
    job_shift_id: JobShiftId,
    business_driver_id: BusinessDriverId,
    work_type: WorkType,
    units: Units,
    /// Volume subtracted from the driver before the standard is applied; the
    /// work below this level is assumed to be absorbed elsewhere.
    suppress_value: i32,
    ranges: Vec<ShiftStandardRange>,
    /// How the work is placed across the shift. Defaults to
    /// [`DistributionMethod::NonFlowed`] — whole blocks, the behavior the
    /// non-flowed generators assume — and is set explicitly for flowed work.
    distribution_method: DistributionMethod,
    /// The curve shape to use when `distribution_method` is `NonFlowed`;
    /// falling back to the planner settings' own default when unset.
    non_flowed_distribution_method: Option<NonFlowedDistributionMethod>,
    /// The intraday curve to follow when `distribution_method` is `Flowed`.
    distribution_schedule_id: Option<DistributionScheduleId>,
    /// Whether to skip the retention smear when distributing this standard.
    ignore_retention: bool,
    /// Bounds on when opening/closing work may be placed; unset means the
    /// corresponding shift boundary.
    earliest_work_start_time: Option<LocalTime>,
    latest_work_end_time: Option<LocalTime>,
}

impl ShiftStandard {
    pub fn new(
        job_id: JobId,
        standard_set_id: StandardSetId,
        job_shift_id: JobShiftId,
        business_driver_id: BusinessDriverId,
        work_type: WorkType,
        units: Units,
        suppress_value: i32,
        ranges: Vec<ShiftStandardRange>,
    ) -> Self {
        Self {
            id: ShiftStandardId::new(),
            job_id,
            standard_set_id,
            job_shift_id,
            business_driver_id,
            work_type,
            units,
            suppress_value,
            ranges,
            distribution_method: DistributionMethod::NonFlowed,
            non_flowed_distribution_method: None,
            distribution_schedule_id: None,
            ignore_retention: false,
            earliest_work_start_time: None,
            latest_work_end_time: None,
        }
    }

    /// Place this standard's work by `distribution_method` instead of in whole
    /// blocks.
    #[must_use]
    pub fn distributed_by(mut self, distribution_method: DistributionMethod) -> Self {
        self.distribution_method = distribution_method;
        self
    }

    /// Override the curve shape used for non-flowed work.
    #[must_use]
    pub fn with_non_flowed_distribution_method(
        mut self,
        method: NonFlowedDistributionMethod,
    ) -> Self {
        self.non_flowed_distribution_method = Some(method);
        self
    }

    /// Follow `distribution_schedule_id`'s curve when flowed.
    #[must_use]
    pub fn with_distribution_schedule(
        mut self,
        distribution_schedule_id: DistributionScheduleId,
    ) -> Self {
        self.distribution_schedule_id = Some(distribution_schedule_id);
        self
    }

    #[must_use]
    pub fn ignoring_retention(mut self) -> Self {
        self.ignore_retention = true;
        self
    }

    /// Constrain when this standard's work may start and end. Either bound left
    /// as `None` falls back to the shift's own boundary.
    #[must_use]
    pub fn within_work_window(
        mut self,
        earliest_work_start_time: Option<LocalTime>,
        latest_work_end_time: Option<LocalTime>,
    ) -> Self {
        self.earliest_work_start_time = earliest_work_start_time;
        self.latest_work_end_time = latest_work_end_time;
        self
    }

    pub fn id(&self) -> ShiftStandardId {
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

    pub fn work_type(&self) -> WorkType {
        self.work_type
    }

    pub fn units(&self) -> Units {
        self.units
    }

    pub fn suppress_value(&self) -> i32 {
        self.suppress_value
    }

    pub fn ranges(&self) -> &[ShiftStandardRange] {
        &self.ranges
    }

    pub fn distribution_method(&self) -> DistributionMethod {
        self.distribution_method
    }

    pub fn non_flowed_distribution_method(&self) -> Option<NonFlowedDistributionMethod> {
        self.non_flowed_distribution_method
    }

    pub fn distribution_schedule_id(&self) -> Option<DistributionScheduleId> {
        self.distribution_schedule_id
    }

    pub fn ignore_retention(&self) -> bool {
        self.ignore_retention
    }

    pub fn earliest_work_start_time(&self) -> Option<LocalTime> {
        self.earliest_work_start_time
    }

    pub fn latest_work_end_time(&self) -> Option<LocalTime> {
        self.latest_work_end_time
    }

    /// The volume the work is actually calculated from.
    ///
    /// The suppressed portion is assumed to be absorbed elsewhere, so it comes
    /// off before the standard is applied — though the *raw* volume is what
    /// chooses the band. Never negative: suppressing more than the volume
    /// leaves nothing rather than owing work.
    pub fn suppressed_driver_value(&self, driver_value: i32) -> i32 {
        (driver_value - self.suppress_value).max(0)
    }

    /// The standard value covering `volume`, ignoring environments.
    ///
    /// For callers with no environment data — the non-flowed path — which take
    /// whichever value the containing range holds.
    pub fn value_for_volume(&self, volume: i32) -> Option<f64> {
        self.ranges
            .iter()
            .find(|range| range.contains_value(volume))
            .and_then(|range| range.any_value())
    }

    /// The standard value covering `volume` in `environment_id`.
    ///
    /// Ranges are scanned in order and the *first* whose values include a match
    /// wins; a containing range with no value for this environment does not
    /// stop the search, it just contributes nothing. Faithful to Java's nested
    /// loop in `ShiftRelatedStandardValueFilter`.
    pub fn value_for_volume_in_environment(
        &self,
        volume: i32,
        environment_id: EnvironmentId,
    ) -> Option<f64> {
        self.ranges
            .iter()
            .filter(|range| range.contains_value(volume))
            .find_map(|range| range.value_for_environment(environment_id))
    }
}

/// One volume band of a standard, inclusive at both ends.
///
/// A band holds one value per operating environment, so the same volume can
/// require different work depending on which environment the date falls in.
#[derive(Debug, Clone, PartialEq)]
pub struct ShiftStandardRange {
    from_volume: i32,
    to_volume: i32,
    values: Vec<ShiftStandardValue>,
}

impl ShiftStandardRange {
    /// A band with a single value that applies whatever the environment.
    ///
    /// Java has no such thing — every value there is environment-keyed — but
    /// the non-flowed path has no environment data to key on, so this is how
    /// its standards are shaped.
    pub fn new(from_volume: i32, to_volume: i32, value: f64) -> Self {
        Self {
            from_volume,
            to_volume,
            values: vec![ShiftStandardValue::for_any_environment(value)],
        }
    }

    pub fn with_values(from_volume: i32, to_volume: i32, values: Vec<ShiftStandardValue>) -> Self {
        Self {
            from_volume,
            to_volume,
            values,
        }
    }

    pub fn from_volume(&self) -> i32 {
        self.from_volume
    }

    pub fn to_volume(&self) -> i32 {
        self.to_volume
    }

    pub fn values(&self) -> &[ShiftStandardValue] {
        &self.values
    }

    /// The band's value when the caller has no environment to key on.
    pub fn any_value(&self) -> Option<f64> {
        self.values.first().map(|value| value.value())
    }

    /// The value configured for `environment_id`, falling back to one that
    /// applies to any environment.
    pub fn value_for_environment(&self, environment_id: EnvironmentId) -> Option<f64> {
        self.values
            .iter()
            .find(|value| value.environment_id() == Some(environment_id))
            .or_else(|| {
                self.values
                    .iter()
                    .find(|value| value.environment_id().is_none())
            })
            .map(|value| value.value())
    }

    pub fn contains_value(&self, value: i32) -> bool {
        value >= self.from_volume && value <= self.to_volume
    }
}

/// One environment's value within a volume band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShiftStandardValue {
    /// `None` means the value is not environment-scoped and applies to any —
    /// see [`ShiftStandardRange::new`].
    environment_id: Option<EnvironmentId>,
    value: f64,
}

impl ShiftStandardValue {
    pub fn new(environment_id: EnvironmentId, value: f64) -> Self {
        Self {
            environment_id: Some(environment_id),
            value,
        }
    }

    pub fn for_any_environment(value: f64) -> Self {
        Self {
            environment_id: None,
            value,
        }
    }

    pub fn environment_id(&self) -> Option<EnvironmentId> {
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

    fn standard(ranges: Vec<ShiftStandardRange>) -> ShiftStandard {
        ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::MinutesPerUnit,
            0,
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
        assert_eq!(
            ShiftStandardRange::new(0, 100, 1.5).contains_value(volume),
            expected
        );
    }

    #[test]
    fn the_value_for_a_volume_comes_from_the_containing_range() {
        let standard = standard(vec![
            ShiftStandardRange::new(0, 99, 1.0),
            ShiftStandardRange::new(100, 199, 2.0),
        ]);

        assert_eq!(standard.value_for_volume(0), Some(1.0));
        assert_eq!(standard.value_for_volume(99), Some(1.0));
        assert_eq!(standard.value_for_volume(100), Some(2.0));
    }

    #[test]
    fn a_volume_outside_every_range_has_no_value() {
        let standard = standard(vec![ShiftStandardRange::new(0, 99, 1.0)]);

        assert_eq!(standard.value_for_volume(100), None);
    }

    #[test]
    fn a_standard_with_no_ranges_has_no_value() {
        assert_eq!(standard(Vec::new()).value_for_volume(10), None);
    }

    #[test]
    fn a_value_can_be_scoped_to_an_environment() {
        let summer = EnvironmentId::new();
        let winter = EnvironmentId::new();
        let standard = standard(vec![ShiftStandardRange::with_values(
            0,
            100,
            vec![
                ShiftStandardValue::new(summer, 1.0),
                ShiftStandardValue::new(winter, 2.0),
            ],
        )]);

        assert_eq!(standard.value_for_volume_in_environment(50, summer), Some(1.0));
        assert_eq!(standard.value_for_volume_in_environment(50, winter), Some(2.0));
    }

    #[test]
    fn an_environment_with_no_value_of_its_own_falls_back_to_the_unscoped_one() {
        let summer = EnvironmentId::new();
        let unconfigured = EnvironmentId::new();
        let standard = standard(vec![ShiftStandardRange::with_values(
            0,
            100,
            vec![
                ShiftStandardValue::new(summer, 1.0),
                ShiftStandardValue::for_any_environment(9.0),
            ],
        )]);

        assert_eq!(standard.value_for_volume_in_environment(50, summer), Some(1.0));
        assert_eq!(
            standard.value_for_volume_in_environment(50, unconfigured),
            Some(9.0)
        );
    }

    #[test]
    fn an_environment_matched_by_no_value_at_all_has_none() {
        let summer = EnvironmentId::new();
        let standard = standard(vec![ShiftStandardRange::with_values(
            0,
            100,
            vec![ShiftStandardValue::new(summer, 1.0)],
        )]);

        assert_eq!(
            standard.value_for_volume_in_environment(50, EnvironmentId::new()),
            None
        );
    }

    #[test]
    fn a_containing_band_without_a_match_does_not_stop_the_search() {
        // Two bands both contain the volume; only the second has a value for
        // this environment. Java keeps scanning, so the second one wins.
        let summer = EnvironmentId::new();
        let winter = EnvironmentId::new();
        let standard = standard(vec![
            ShiftStandardRange::with_values(0, 100, vec![ShiftStandardValue::new(winter, 2.0)]),
            ShiftStandardRange::with_values(0, 100, vec![ShiftStandardValue::new(summer, 7.0)]),
        ]);

        assert_eq!(standard.value_for_volume_in_environment(50, summer), Some(7.0));
    }

    #[test]
    fn the_environment_free_lookup_takes_whatever_the_band_holds() {
        let standard = standard(vec![ShiftStandardRange::new(0, 100, 1.5)]);

        assert_eq!(standard.value_for_volume(50), Some(1.5));
    }

    #[test]
    fn a_standard_is_placed_in_whole_blocks_unless_told_otherwise() {
        let standard = standard(Vec::new());

        assert_eq!(standard.distribution_method(), DistributionMethod::NonFlowed);
        assert_eq!(standard.non_flowed_distribution_method(), None);
        assert_eq!(standard.distribution_schedule_id(), None);
        assert!(!standard.ignore_retention());
        assert_eq!(standard.earliest_work_start_time(), None);
        assert_eq!(standard.latest_work_end_time(), None);
    }

    #[test]
    fn the_distribution_settings_can_be_built_up() {
        let schedule_id = DistributionScheduleId::new();
        let standard = standard(Vec::new())
            .distributed_by(DistributionMethod::Flowed)
            .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING)
            .with_distribution_schedule(schedule_id)
            .ignoring_retention()
            .within_work_window(Some(LocalTime::new(7, 0, 0)), Some(LocalTime::new(17, 0, 0)));

        assert_eq!(standard.distribution_method(), DistributionMethod::Flowed);
        assert_eq!(
            standard.non_flowed_distribution_method(),
            Some(NonFlowedDistributionMethod::BEGINNING)
        );
        assert_eq!(standard.distribution_schedule_id(), Some(schedule_id));
        assert!(standard.ignore_retention());
        assert_eq!(
            standard.earliest_work_start_time(),
            Some(LocalTime::new(7, 0, 0))
        );
        assert_eq!(
            standard.latest_work_end_time(),
            Some(LocalTime::new(17, 0, 0))
        );
    }
}
