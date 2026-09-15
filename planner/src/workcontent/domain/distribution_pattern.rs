use crate::id_type;
use crate::workcontent::domain::location::LocationId;

id_type!(DistributionPatternId, uuid_v4);

/// The intraday curve a day's volume is spread along.
///
/// The pattern's own granularity is implied by how many periods it has — 24
/// hourly periods, 96 quarter-hourly, and so on — which need not match the
/// planner's period length. Converting between the two is the job of the
/// pattern conversion stage.
pub struct DistributionPattern {
    id: DistributionPatternId,
    property_id: LocationId,
    name: String,
    total_percent: f64,
    total_units: i32,
    periods: Vec<DistributionPatternPeriod>,
}

impl DistributionPattern {
    pub fn new(
        property_id: LocationId,
        name: String,
        total_percent: f64,
        total_units: i32,
        periods: Vec<DistributionPatternPeriod>,
    ) -> Self {
        Self {
            id: DistributionPatternId::new(),
            property_id,
            name,
            total_percent,
            total_units,
            periods,
        }
    }

    pub fn id(&self) -> DistributionPatternId {
        self.id
    }

    pub fn property_id(&self) -> LocationId {
        self.property_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn total_percent(&self) -> f64 {
        self.total_percent
    }

    pub fn total_units(&self) -> i32 {
        self.total_units
    }

    pub fn periods(&self) -> &[DistributionPatternPeriod] {
        &self.periods
    }

    /// How long each of this pattern's periods is, in minutes.
    ///
    /// Derived from the period count rather than stored, matching the Java
    /// engine's lookup: 24/48/96/144/288 periods a day map to 60/30/15/10/5
    /// minute periods. `None` for any other count.
    pub fn period_length_in_minutes(&self) -> Option<i32> {
        match self.periods.len() {
            24 => Some(60),
            48 => Some(30),
            96 => Some(15),
            144 => Some(10),
            288 => Some(5),
            _ => None,
        }
    }
}

/// One period of a [`DistributionPattern`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistributionPatternPeriod {
    /// The share of the day's volume falling in this period, as a percentage.
    pattern_value: f64,
    /// An observed count for this period, when one has been recorded.
    actual_value: Option<i32>,
    period_no: i32,
}

impl DistributionPatternPeriod {
    pub fn new(period_no: i32, pattern_value: f64, actual_value: Option<i32>) -> Self {
        Self {
            pattern_value,
            actual_value,
            period_no,
        }
    }

    pub fn pattern_value(&self) -> f64 {
        self.pattern_value
    }

    pub fn actual_value(&self) -> Option<i32> {
        self.actual_value
    }

    pub fn period_no(&self) -> i32 {
        self.period_no
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn pattern_with(period_count: usize) -> DistributionPattern {
        DistributionPattern::new(
            LocationId::new(),
            "Weekday".to_string(),
            100.0,
            1000,
            (0..period_count)
                .map(|period_no| DistributionPatternPeriod::new(period_no as i32, 1.0, None))
                .collect(),
        )
    }

    #[rstest]
    #[case(24, Some(60))]
    #[case(48, Some(30))]
    #[case(96, Some(15))]
    #[case(144, Some(10))]
    #[case(288, Some(5))]
    fn the_period_length_follows_from_the_period_count(
        #[case] period_count: usize,
        #[case] expected: Option<i32>,
    ) {
        assert_eq!(pattern_with(period_count).period_length_in_minutes(), expected);
    }

    #[rstest]
    #[case(0)]
    #[case(12)]
    #[case(100)]
    fn an_unrecognised_period_count_has_no_period_length(#[case] period_count: usize) {
        assert_eq!(pattern_with(period_count).period_length_in_minutes(), None);
    }
}
