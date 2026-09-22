//! Port of the availability accessor off `ScheduleCalcDataSet`
//! (`com.unifocus.watson.server.labor.calcshift.ScheduleCalcDataSet.getAvailability()`), which
//! returns some `Availability`-shaped object — ground truth not read directly, only its two
//! `getAvailPeriods*` methods that `EmployeeData`/`process/checkers/` call.

use crate::entity::avail_period::AvailPeriod;

/// An employee's availability, split into the two views the engine reads.
/// `Availability`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Availability {
    avail_periods_including_preferred: Vec<AvailPeriod>,
    avail_periods_required_off_only: Vec<AvailPeriod>,
}

impl Availability {
    pub fn new(
        avail_periods_including_preferred: Vec<AvailPeriod>,
        avail_periods_required_off_only: Vec<AvailPeriod>,
    ) -> Self {
        Self {
            avail_periods_including_preferred,
            avail_periods_required_off_only,
        }
    }

    /// `getAvailPeriodsIncludingPreferred()`.
    pub fn avail_periods_including_preferred(&self) -> &[AvailPeriod] {
        &self.avail_periods_including_preferred
    }

    /// `getAvailPeriodsRequiredOffOnly()`.
    pub fn avail_periods_required_off_only(&self) -> &[AvailPeriod] {
        &self.avail_periods_required_off_only
    }
}
