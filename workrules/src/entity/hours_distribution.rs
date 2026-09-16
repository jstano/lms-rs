//! Port of `com.unifocus.watson.server.hibernate.entity.HoursDistribution`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/HoursDistribution.java`.
//!
//! How one day's hours break down: how many, at what base and premium rate, and
//! which rule items decided each. Nine of the 32 rule families read or write it.
//!
//! Unlike the other entities this is `@Embeddable`, not `@Entity` — it has no
//! id of its own and lives inside its owner.
//!
//! # Derived state in the setters
//!
//! `setHours`, `setBaseRate` and `setPremiumRate` each recompute `totalCosts`:
//!
//! ```java
//! public void setHours(double hours) { this.hours = hours; calculateTotalCosts(); }
//! private void calculateTotalCosts() {
//!    totalCosts = TDouble.roundCurrency(hours * (baseRate + premiumRate));
//! }
//! ```
//!
//! Same class of coupling as the punch/shift callback, but self-contained: the
//! recomputation stays inside this struct, so the setters simply do both and no
//! cursor is needed.

use crate::common::numbers::round_currency;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// One day's distributed hours. `HoursDistribution`.
#[derive(Debug, Clone, PartialEq)]
pub struct HoursDistribution {
    property_id: i32,
    hours_distribution_type_id: Option<i32>,
    hours_rule_item_id: Option<i32>,
    rate_rule_item_id: Option<i32>,
    date: LocalDate,
    original_hours: f64,
    hours: f64,
    base_rate: f64,
    premium_rate: f64,
    total_costs: f64,
}

impl HoursDistribution {
    /// Build a distribution, computing its cost.
    pub fn new(
        property_id: i32,
        date: LocalDate,
        hours_distribution_type_id: Option<i32>,
        hours: f64,
        base_rate: f64,
    ) -> Self {
        let mut distribution = Self {
            property_id,
            hours_distribution_type_id,
            hours_rule_item_id: None,
            rate_rule_item_id: None,
            date,
            original_hours: hours,
            hours,
            base_rate,
            premium_rate: 0.0,
            total_costs: 0.0,
        };
        distribution.calculate_total_costs();
        distribution
    }

    /// `getPropertyID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// Which distribution bucket these hours fall in — regular, overtime and so
    /// on. `getHoursDistributionTypeID()`.
    pub fn hours_distribution_type_id(&self) -> Option<i32> {
        self.hours_distribution_type_id
    }

    /// The rule item that decided the hours. `getHoursRuleItemID()`.
    pub fn hours_rule_item_id(&self) -> Option<i32> {
        self.hours_rule_item_id
    }

    /// The rule item that decided the rate. `getRateRuleItemID()`.
    pub fn rate_rule_item_id(&self) -> Option<i32> {
        self.rate_rule_item_id
    }

    /// `getDate()`.
    pub fn date(&self) -> LocalDate {
        self.date
    }

    /// The hours as first distributed, before any rule adjusted them.
    /// `getOriginalHours()`.
    pub fn original_hours(&self) -> f64 {
        self.original_hours
    }

    /// `getHours()`.
    pub fn hours(&self) -> f64 {
        self.hours
    }

    /// `getBaseRate()`.
    pub fn base_rate(&self) -> f64 {
        self.base_rate
    }

    /// `getPremiumRate()`.
    pub fn premium_rate(&self) -> f64 {
        self.premium_rate
    }

    /// Hours times the combined rate, to currency precision.
    /// `getTotalCosts()`.
    pub fn total_costs(&self) -> f64 {
        self.total_costs
    }

    /// `setHours()` — recomputes the cost.
    pub fn set_hours(&mut self, hours: f64) {
        self.hours = hours;
        self.calculate_total_costs();
    }

    /// `setBaseRate()` — recomputes the cost.
    pub fn set_base_rate(&mut self, base_rate: f64) {
        self.base_rate = base_rate;
        self.calculate_total_costs();
    }

    /// `setPremiumRate()` — recomputes the cost.
    pub fn set_premium_rate(&mut self, premium_rate: f64) {
        self.premium_rate = premium_rate;
        self.calculate_total_costs();
    }

    /// `setOriginalHours()` — no cost recomputation in Java either.
    pub fn set_original_hours(&mut self, original_hours: f64) {
        self.original_hours = original_hours;
    }

    /// `setHoursDistributionTypeID()`.
    pub fn set_hours_distribution_type_id(&mut self, id: Option<i32>) {
        self.hours_distribution_type_id = id;
    }

    /// `setHoursRuleItemID()`.
    pub fn set_hours_rule_item_id(&mut self, id: Option<i32>) {
        self.hours_rule_item_id = id;
    }

    /// `setRateRuleItemID()`.
    pub fn set_rate_rule_item_id(&mut self, id: Option<i32>) {
        self.rate_rule_item_id = id;
    }

    /// Whether this distribution's date falls inside `period`.
    /// `distributionFallsWithinPeriod(DateRange)`.
    ///
    /// Java has two spellings of this — a `Predicate` factory and a curried
    /// `Function<DateRange, Predicate<…>>` — with identical bodies. One method
    /// here; the currying was only there to compose into a stream.
    pub fn falls_within_period(&self, period: &DateRange) -> bool {
        period.contains_date(self.date)
    }

    /// Whether this distribution sits in a given bucket. `isOfType(int)`.
    ///
    /// Java compares `Integer == int`, which unboxes: a distribution with no
    /// type id throws rather than answering `false`. An untyped distribution is
    /// in no bucket under any reading, so it answers `false` here.
    pub fn is_of_type(&self, hours_distribution_type_id: i32) -> bool {
        self.hours_distribution_type_id == Some(hours_distribution_type_id)
    }

    /// `calculateTotalCosts()` — note it rounds to **currency** precision, four
    /// places, not the two of display currency.
    fn calculate_total_costs(&mut self) {
        self.total_costs = round_currency(self.hours * (self.base_rate + self.premium_rate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distribution() -> HoursDistribution {
        HoursDistribution::new(11, LocalDate::of(2010, 1, 2), Some(1), 8.0, 10.0)
    }

    #[test]
    fn a_new_distribution_computes_its_cost() {
        let d = distribution();
        assert_eq!(d.hours(), 8.0);
        assert_eq!(d.base_rate(), 10.0);
        assert_eq!(d.total_costs(), 80.0);
    }

    #[test]
    fn original_hours_start_equal_to_hours() {
        assert_eq!(distribution().original_hours(), 8.0);
    }

    #[test]
    fn setting_hours_recomputes_the_cost() {
        let mut d = distribution();
        d.set_hours(4.0);
        assert_eq!(d.total_costs(), 40.0);
        assert_eq!(d.original_hours(), 8.0, "the original is not disturbed");
    }

    #[test]
    fn setting_either_rate_recomputes_the_cost() {
        let mut d = distribution();
        d.set_base_rate(20.0);
        assert_eq!(d.total_costs(), 160.0);
        d.set_premium_rate(5.0);
        assert_eq!(d.total_costs(), 200.0, "both rates are combined");
    }

    #[test]
    fn the_cost_is_rounded_to_currency_precision() {
        // Four places, not two: 3 * 3.33333 = 9.99999 -> 10.0 at 4dp.
        let mut d = HoursDistribution::new(11, LocalDate::of(2010, 1, 2), None, 3.0, 3.33333);
        assert_eq!(d.total_costs(), 10.0);

        d.set_base_rate(1.000_05);
        assert_eq!(d.total_costs(), round_currency(3.0 * 1.000_05));
    }

    #[test]
    fn rule_item_ids_record_which_rules_decided_what() {
        let mut d = distribution();
        assert_eq!(d.hours_rule_item_id(), None);

        d.set_hours_rule_item_id(Some(10));
        d.set_rate_rule_item_id(Some(20));

        assert_eq!(d.hours_rule_item_id(), Some(10));
        assert_eq!(d.rate_rule_item_id(), Some(20));
    }

    #[test]
    fn a_distribution_falls_within_a_period_inclusively() {
        let d = distribution(); // 2010-01-02
        let january = DateRange::new(LocalDate::of(2010, 1, 2), LocalDate::of(2010, 1, 8));

        assert!(d.falls_within_period(&january), "the start date counts");
        assert!(d.falls_within_period(&DateRange::new(
            LocalDate::of(2009, 12, 27),
            LocalDate::of(2010, 1, 2)
        )));
        assert!(!d.falls_within_period(&DateRange::new(
            LocalDate::of(2010, 1, 3),
            LocalDate::of(2010, 1, 9)
        )));
    }

    #[test]
    fn a_distribution_is_of_exactly_one_type() {
        assert!(distribution().is_of_type(1));
        assert!(!distribution().is_of_type(2));
    }

    #[test]
    fn an_untyped_distribution_is_of_no_type() {
        // Java unboxes and throws here; no bucket is the only sound reading.
        let untyped = HoursDistribution::new(11, LocalDate::of(2010, 1, 2), None, 8.0, 10.0);
        assert!(!untyped.is_of_type(1));
    }

    #[test]
    fn setting_original_hours_does_not_touch_the_cost() {
        let mut d = distribution();
        d.set_original_hours(99.0);
        assert_eq!(d.total_costs(), 80.0);
    }
}
