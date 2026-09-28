//! Partial port of `com.unifocus.watson.server.hibernate.entity.HoursDistribution`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/
//! HoursDistribution.java`. Only `date`/`hours` are modeled, plus `is_premium` — a pre-resolved
//! flag standing in for `TimeCard.distributionIsPremium(HoursDistribution)`
//! (`!getRegularHoursDistributionTypeIds().contains(distribution.getHoursDistributionTypeID())`),
//! whose `getRegularHoursDistributionTypeIds()` half is a property-level configuration lookup this
//! crate doesn't model (same "narrow entity slice, defer the classification lookup" treatment as
//! `entity::employee_pay_type`/`entity::scheduling_method`). `propertyID`/`hoursDistributionTypeID`/
//! `hoursRuleItemID`/`rateRuleItemID`/`baseRate`/`premiumRate`/`totalCosts`/`originalHours` aren't
//! read by any ported call site (`ScheduleHoursDistributionValidator`/`ScheduleCalcDataSet`'s
//! `TimeCard` default methods, both `EmployeeShift::hours_distributions`' first real readers).

use joda_rs::LocalDate;

/// `HoursDistribution`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoursDistribution {
    date: LocalDate,
    hours: f64,
    is_premium: bool,
}

impl HoursDistribution {
    pub fn new(date: LocalDate, hours: f64, is_premium: bool) -> Self {
        Self {
            date,
            hours,
            is_premium,
        }
    }

    /// `getDate()`.
    pub fn date(&self) -> LocalDate {
        self.date
    }

    /// `getHours()`.
    pub fn hours(&self) -> f64 {
        self.hours
    }

    /// `TimeCard.distributionIsPremium(this)`.
    pub fn is_premium(&self) -> bool {
        self.is_premium
    }
}
