//! One period's worth of value in a distribution array.
//!
//! The flowed engine represents a day's work as an array of these — one per
//! planner period — and every stage of the pipeline (spreading, min/max
//! adjustment, break redistribution) is a transformation over such an array.

use crate::workcontent::common::numbers;
use joda_rs::LocalDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct DistributionItem {
    period: i32,
    date_time: LocalDateTime,
    value_per_period: f64,
}

impl DistributionItem {
    pub fn new(period: i32, date_time: LocalDateTime, value_per_period: f64) -> Self {
        Self {
            period,
            date_time,
            value_per_period,
        }
    }

    pub fn period(&self) -> i32 {
        self.period
    }

    pub fn date_time(&self) -> LocalDateTime {
        self.date_time
    }

    pub fn value_per_period(&self) -> f64 {
        self.value_per_period
    }

    pub fn set_value_per_period(&mut self, value: f64) {
        self.value_per_period = value;
    }

    /// Add to this period, rounding the result.
    ///
    /// Rounding on every add is what keeps a long accumulation across an array
    /// from drifting. Note the asymmetry with
    /// [`subtract_from_period_value`](Self::subtract_from_period_value), which
    /// does not round: it is the confirmed Java behavior and several callers
    /// depend on the exact numbers each produces, so it is replicated rather
    /// than smoothed over.
    pub fn add_to_period_value(&mut self, amount: f64) {
        self.value_per_period = numbers::round_percent(self.value_per_period + amount);
    }

    /// Subtract from this period, without rounding. See the note on
    /// [`add_to_period_value`](Self::add_to_period_value).
    pub fn subtract_from_period_value(&mut self, amount: f64) {
        self.value_per_period -= amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> DistributionItem {
        DistributionItem::new(12, LocalDateTime::new(2013, 7, 23, 12, 30, 0), 30.0)
    }

    #[test]
    fn carries_its_period_time_and_value() {
        let item = item();

        assert_eq!(item.period(), 12);
        assert_eq!(item.date_time(), LocalDateTime::new(2013, 7, 23, 12, 30, 0));
        assert_eq!(item.value_per_period(), 30.0);
    }

    #[test]
    fn the_value_can_be_replaced_outright() {
        let mut item = item();

        item.set_value_per_period(100.0);

        assert_eq!(item.value_per_period(), 100.0);
    }

    #[test]
    fn adding_accumulates_onto_the_current_value() {
        let mut item = item();

        item.add_to_period_value(30.0);

        assert_eq!(item.value_per_period(), 60.0);
    }

    #[test]
    fn subtracting_takes_away_from_the_current_value() {
        let mut item = item();

        item.subtract_from_period_value(30.0);

        assert_eq!(item.value_per_period(), 0.0);
    }

    #[test]
    fn adding_rounds_the_result_but_subtracting_does_not() {
        let mut added = DistributionItem::new(0, LocalDateTime::new(2013, 7, 23, 0, 0, 0), 0.0);
        added.add_to_period_value(1.0 / 3.0);

        assert_eq!(added.value_per_period(), 0.3333);

        let mut subtracted = DistributionItem::new(0, LocalDateTime::new(2013, 7, 23, 0, 0, 0), 0.0);
        subtracted.subtract_from_period_value(1.0 / 3.0);

        assert_eq!(subtracted.value_per_period(), -(1.0 / 3.0));
    }
}
