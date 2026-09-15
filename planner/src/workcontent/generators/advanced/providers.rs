//! The things the flowed engine consults but does not own.
//!
//! Java reaches several structures hanging off `PlannerModel` that this port
//! does not model — the seasonal calendar, the forecast structure, the
//! share-with schedule, the revenue-centre capacity tree. Each becomes a trait
//! here, so the distribution maths can be exercised against known inputs while
//! the real data sources stay a separate concern (the same "read" deferral the
//! non-flowed path already makes).
//!
//! Every trait ships a do-nothing implementation for tests and for callers that
//! have no such data configured.

use crate::workcontent::domain::business_driver::BusinessDriverId;
use crate::workcontent::domain::environment::EnvironmentId;
use crate::workcontent::domain::job::JobId;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::common::date_time_range_with_period_length::DateTimeRangeWithPeriodLength;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use date_range_rs::DateTimeRange;
use joda_rs::{LocalDate, LocalDateTime};

/// Which operating environment applies, in the two tiers Java resolves.
///
/// A standard value is looked up against the environment for *this driver and
/// date* first; only if that yields nothing does the engine fall back to the
/// environment for the date alone. Both tiers live here because they are one
/// decision from the caller's point of view.
pub trait EnvironmentResolver {
    /// The environment for the date. Every resolver can answer this.
    fn environment_for_date(&self, date: LocalDate) -> Option<EnvironmentId>;

    /// The environment configured for this driver on this date, if any.
    ///
    /// Defaults to none: most sources hold only the date-level calendar, and
    /// Java's driver-specific lookup is itself optional — it returns null
    /// whenever the driver has no environment of its own, which is the common
    /// case.
    fn environment_for_driver_and_date(
        &self,
        _business_driver_id: BusinessDriverId,
        _date: LocalDate,
    ) -> Option<EnvironmentId> {
        None
    }
}

/// Whether a business driver is trading at all.
///
/// Java asks `ForecastStructure.kbiIsOpen(kbi, standardSet, date, value)`.
/// A closed driver generates no work from the standards that consult it —
/// though notably not from every standard, which is a real asymmetry the
/// generators reproduce.
pub trait BusinessDriverOpenness {
    fn is_open(
        &self,
        business_driver_id: BusinessDriverId,
        date: LocalDate,
        driver_value: i32,
    ) -> bool;
}

/// Volumes observed per period, for standards that spread dynamically.
///
/// Java resolves a `DynamicSpreadService` per driver and range; no service, or
/// an empty one, means the standard contributes nothing rather than failing.
/// The values run from the start of `range`, one per planner period.
pub trait DynamicSpreadProvider {
    fn spread_values(
        &self,
        business_driver_id: BusinessDriverId,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Option<Vec<i32>>;
}

/// Work already covered jointly with another schedule, and so deducted here.
pub trait ShareWithScheduleProvider {
    /// The window of the first schedule covering `date_time` for this job.
    ///
    /// Matching here is **inclusive of the window's end**, matching Java's
    /// `findFirstMatching`. The distributor then re-checks the point against
    /// the returned window *exclusively*, so a period landing exactly on the
    /// end matches but is not deducted from — a two-step dance that matters,
    /// because only a complete miss short-circuits the distributor's scan.
    fn first_matching_schedule(&self, job_id: JobId, date_time: LocalDateTime)
    -> Option<DateTimeRange>;
}

/// How long a guest occupies capacity, and how much capacity exists.
///
/// Feeds the retention smear and the capacity clamp. Java sources this from the
/// revenue-centre tree, which is not modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetentionCapacityUtilization {
    /// How long one guest holds capacity, in hours.
    pub retention_hours: f64,
    pub capacity: f64,
    pub utilization: f64,
}

pub trait RetentionCapacityProvider {
    fn utilization_for(
        &self,
        business_driver_id: BusinessDriverId,
        date_time: LocalDateTime,
    ) -> Option<RetentionCapacityUtilization>;
}

/// The intraday curve a standard follows, ready to use.
///
/// Java's `FlowPatternDataProvider` resolves the schedule for the date, picks
/// the pattern for that day, and converts it to the planner's granularity. All
/// of that is behind this one call; `None` means no curve is configured, and
/// the flowed distributor then plans nothing.
pub trait DistributionPatternProvider {
    fn pattern_data(
        &self,
        standard: &ShiftStandard,
        date: LocalDate,
        range: &DateTimeRangeWithPeriodLength,
    ) -> Option<Vec<DistributionItem>>;
}

/// The providers a generation run consults, gathered so the deep call chain
/// threads one borrow rather than five.
pub struct Providers<'a> {
    pub environments: &'a dyn EnvironmentResolver,
    pub openness: &'a dyn BusinessDriverOpenness,
    pub dynamic_spread: &'a dyn DynamicSpreadProvider,
    pub share_with: &'a dyn ShareWithScheduleProvider,
    pub retention_capacity: &'a dyn RetentionCapacityProvider,
    pub patterns: &'a dyn DistributionPatternProvider,
}

impl<'a> Providers<'a> {
    /// The environment to price a standard in: the driver's own if it has one,
    /// otherwise the date's. Mirrors Java's
    /// `ShiftRelatedStandardValueFilter` two-tier resolution.
    pub fn environment_for(
        &self,
        business_driver_id: BusinessDriverId,
        date: LocalDate,
    ) -> Option<EnvironmentId> {
        self.environments
            .environment_for_driver_and_date(business_driver_id, date)
            .or_else(|| self.environments.environment_for_date(date))
    }
}

/// Providers that know nothing: no environments, every driver open, no spread
/// service, no shared schedules, no capacity data, no curves.
///
/// The defaults are chosen so that a run configured with none of this data
/// behaves the way Java does when the corresponding structure is empty.
pub struct NoProviders;

impl EnvironmentResolver for NoProviders {
    fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
        None
    }
}

impl BusinessDriverOpenness for NoProviders {
    fn is_open(
        &self,
        _business_driver_id: BusinessDriverId,
        _date: LocalDate,
        _driver_value: i32,
    ) -> bool {
        true
    }
}

impl DynamicSpreadProvider for NoProviders {
    fn spread_values(
        &self,
        _business_driver_id: BusinessDriverId,
        _range: &DateTimeRangeWithPeriodLength,
    ) -> Option<Vec<i32>> {
        None
    }
}

impl ShareWithScheduleProvider for NoProviders {
    fn first_matching_schedule(
        &self,
        _job_id: JobId,
        _date_time: LocalDateTime,
    ) -> Option<DateTimeRange> {
        None
    }
}

impl RetentionCapacityProvider for NoProviders {
    fn utilization_for(
        &self,
        _business_driver_id: BusinessDriverId,
        _date_time: LocalDateTime,
    ) -> Option<RetentionCapacityUtilization> {
        None
    }
}

impl DistributionPatternProvider for NoProviders {
    fn pattern_data(
        &self,
        _standard: &ShiftStandard,
        _date: LocalDate,
        _range: &DateTimeRangeWithPeriodLength,
    ) -> Option<Vec<DistributionItem>> {
        None
    }
}

impl Providers<'static> {
    /// A bundle backed entirely by [`NoProviders`].
    pub fn none() -> Self {
        const NONE: &NoProviders = &NoProviders;

        Self {
            environments: NONE,
            openness: NONE,
            dynamic_spread: NONE,
            share_with: NONE,
            retention_capacity: NONE,
            patterns: NONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolves a driver-specific environment, a date-level one, or neither.
    struct Configured {
        for_driver: Option<EnvironmentId>,
        for_date: Option<EnvironmentId>,
    }

    impl EnvironmentResolver for Configured {
        fn environment_for_date(&self, _date: LocalDate) -> Option<EnvironmentId> {
            self.for_date
        }

        fn environment_for_driver_and_date(
            &self,
            _business_driver_id: BusinessDriverId,
            _date: LocalDate,
        ) -> Option<EnvironmentId> {
            self.for_driver
        }
    }

    fn providers_with(environments: &Configured) -> Providers<'_> {
        Providers {
            environments,
            ..Providers::none()
        }
    }

    #[test]
    fn the_drivers_own_environment_wins_when_it_has_one() {
        let driver_environment = EnvironmentId::new();
        let environments = Configured {
            for_driver: Some(driver_environment),
            for_date: Some(EnvironmentId::new()),
        };

        assert_eq!(
            providers_with(&environments)
                .environment_for(BusinessDriverId::new(), LocalDate::new(2013, 11, 4)),
            Some(driver_environment)
        );
    }

    #[test]
    fn the_date_environment_is_the_fallback() {
        let date_environment = EnvironmentId::new();
        let environments = Configured {
            for_driver: None,
            for_date: Some(date_environment),
        };

        assert_eq!(
            providers_with(&environments)
                .environment_for(BusinessDriverId::new(), LocalDate::new(2013, 11, 4)),
            Some(date_environment)
        );
    }

    #[test]
    fn neither_tier_configured_resolves_to_no_environment() {
        let environments = Configured {
            for_driver: None,
            for_date: None,
        };

        assert_eq!(
            providers_with(&environments)
                .environment_for(BusinessDriverId::new(), LocalDate::new(2013, 11, 4)),
            None
        );
    }

    #[test]
    fn the_empty_bundle_matches_java_with_nothing_configured() {
        let providers = Providers::none();
        let date = LocalDate::new(2013, 11, 4);
        let driver = BusinessDriverId::new();

        // A driver with no forecast structure is open, so work is still planned.
        assert!(providers.openness.is_open(driver, date, 100));
        assert_eq!(providers.environment_for(driver, date), None);
    }
}
